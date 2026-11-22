//! The Gateway's client of the Notification Service inbox socket (#348, T-07 CHOICE-21 A):
//! HTTP/1.1 on `VINK_NOTIFICATIONS_INBOX_SOCKET`, served by `VINK_NOTIFICATIONS_SERVER_UID`.
//! Before sending anything the client checks the OS user serving the socket, so a socket planted
//! by another user is never trusted; the service in turn answers only the App's user. The
//! recipient in every path is the principal the Gateway resolved from the session (K5): no client
//! input ever names it.
//!
//! The App keeps its own representation of the wire types (`contracts/rpc/v1` of the service) and
//! never imports the service crate. Every failure that is not a contract answer (refused
//! connection, wrong server user, timeout, non-contract status or body) is
//! [`InboxFailure::Unavailable`], which the Gateway reports as `503 dependency_unavailable`, never
//! as an empty inbox or a zero count (P-04B).

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use axum::body::Bytes;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::{BodyExt, Full, Limited};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use tokio::net::UnixStream;
use uuid::Uuid;

use crate::domain::ids::PrincipalId;
use crate::notification_transport::MAX_SOCKET_PATH;

/// A page of 50 comment groups with their contributions is ~150 KiB.
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
/// One call; the service's own deadline is 5 s.
pub const CALL_TIMEOUT: Duration = Duration::from_millis(2500);

pub type InboxFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, InboxFailure>> + Send + 'a>>;

/// Where the Gateway reads inboxes, from the server's environment. Both variables or neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxTransport {
    pub socket: PathBuf,
    pub server_uid: u32,
}

impl InboxTransport {
    /// `Ok(None)` when neither variable is set: inbox routes answer `503 dependency_unavailable`.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Option<Self>, String> {
        let value = |key: &str| get(key).filter(|v| !v.trim().is_empty());
        match (
            value("VINK_NOTIFICATIONS_INBOX_SOCKET"),
            value("VINK_NOTIFICATIONS_SERVER_UID"),
        ) {
            (None, _) => Ok(None),
            (Some(_), None) => Err(
                "VINK_NOTIFICATIONS_SERVER_UID: required with VINK_NOTIFICATIONS_INBOX_SOCKET (user id that must serve the socket)"
                    .into(),
            ),
            (Some(socket), Some(_)) if socket.len() > MAX_SOCKET_PATH => Err(format!(
                "VINK_NOTIFICATIONS_INBOX_SOCKET: longer than {MAX_SOCKET_PATH} bytes, the Unix socket path limit"
            )),
            (Some(socket), Some(uid)) => match uid.trim().parse::<u32>() {
                Ok(server_uid) => Ok(Some(Self {
                    socket: PathBuf::from(socket),
                    server_uid,
                })),
                Err(_) => Err("VINK_NOTIFICATIONS_SERVER_UID: expected a numeric user id".into()),
            },
        }
    }
}

/// Why an inbox call did not succeed. Only the first is an outage; the rest are contract answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxFailure {
    Unavailable,
    /// `422 invalid_snapshot`: unknown or future snapshot for this inbox.
    InvalidSnapshot,
    /// `422 invalid_cursor`.
    InvalidCursor,
    /// `422 invalid_group_version`: the group never had that version.
    InvalidVersion,
    /// `422 idempotency_key_reused`.
    KeyReused,
    /// `404 not_found`: no such live group in this inbox.
    NotFound,
}

/// The service's badge count (unfiltered: an upper bound of what the Gateway may show).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WireCount {
    pub status: String,
    pub unseen_group_count: i64,
    pub as_of_snapshot: String,
    pub max_staleness_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WireContribution {
    pub comment_id: Uuid,
    pub actor_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WireContributions {
    pub newest: Vec<WireContribution>,
    pub total: i64,
}

/// One inbox group as the service stores it: identifiers and versions only (Thin Pointer).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WireGroup {
    pub group_id: Uuid,
    pub event_type: String,
    pub target: Value,
    pub group_version: i64,
    pub signal_version: i64,
    pub unread: bool,
    pub unseen: bool,
    pub actor_count: i32,
    pub item_count: i32,
    pub actors_preview: Vec<Uuid>,
    #[serde(default)]
    pub contributions: Option<WireContributions>,
    pub action_required: bool,
    pub publication_epoch: Option<i64>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub last_signal_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WirePage {
    pub inbox_snapshot: String,
    pub seen_snapshot: String,
    pub groups: Vec<WireGroup>,
    pub next_cursor: Option<String>,
}

/// The three idempotent ACK commands, with snapshot numbers already authenticated by the Gateway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxCommand {
    Seen {
        up_to: i64,
    },
    Read {
        group_id: Uuid,
        observed_version: i64,
    },
    MarkAllRead {
        up_to: i64,
    },
}

/// The port the Gateway reads inboxes through; tests replace the socket.
pub trait NotificationInbox: Send + Sync {
    fn count(&self, recipient: PrincipalId) -> InboxFuture<'_, WireCount>;
    fn page<'a>(
        &'a self,
        recipient: PrincipalId,
        limit: u8,
        cursor: Option<&'a str>,
    ) -> InboxFuture<'a, WirePage>;
    fn command<'a>(
        &'a self,
        recipient: PrincipalId,
        command_key: &'a str,
        command: &'a InboxCommand,
    ) -> InboxFuture<'a, Value>;
}

/// `s<n>` as the service writes it; `None` for anything else.
pub fn parse_snapshot(token: &str) -> Option<i64> {
    let digits = token.strip_prefix('s')?;
    if digits.is_empty() || digits.len() > 18 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    digits.parse().ok()
}

/// The production inbox: one connection per call, bounded by [`CALL_TIMEOUT`].
#[derive(Debug, Clone)]
pub struct SocketInbox {
    socket: PathBuf,
    server_uid: u32,
    timeout: Duration,
}

impl SocketInbox {
    pub fn new(transport: InboxTransport) -> Self {
        Self {
            socket: transport.socket,
            server_uid: transport.server_uid,
            timeout: CALL_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    async fn exchange(
        &self,
        method: Method,
        path: String,
        body: Option<Vec<u8>>,
    ) -> Result<(StatusCode, Bytes), InboxFailure> {
        let stream = UnixStream::connect(&self.socket)
            .await
            .map_err(|_| InboxFailure::Unavailable)?;
        match stream.peer_cred() {
            Ok(cred) if cred.uid() == self.server_uid => {}
            _ => {
                tracing::warn!("notification inbox socket is served by an unexpected user");
                return Err(InboxFailure::Unavailable);
            }
        }
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|_| InboxFailure::Unavailable)?;
        let driver = tokio::spawn(connection);
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "notifications");
        if body.is_some() {
            request = request.header(header::CONTENT_TYPE, "application/json");
        }
        let request = request
            .body(Full::new(Bytes::from(body.unwrap_or_default())))
            .map_err(|_| InboxFailure::Unavailable)?;
        let result = async {
            let response = sender
                .send_request(request)
                .await
                .map_err(|_| InboxFailure::Unavailable)?;
            let status = response.status();
            let bytes = Limited::new(response.into_body(), MAX_RESPONSE_BYTES)
                .collect()
                .await
                .map_err(|_| InboxFailure::Unavailable)?
                .to_bytes();
            Ok((status, bytes))
        }
        .await;
        drop(sender);
        driver.abort();
        result
    }

    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: String,
        body: Option<Vec<u8>>,
    ) -> Result<T, InboxFailure> {
        let (status, bytes) = tokio::time::timeout(self.timeout, self.exchange(method, path, body))
            .await
            .map_err(|_| InboxFailure::Unavailable)??;
        answer(status, &bytes)
    }
}

/// Maps one service answer: `200` with a well-formed body, or a contract error code.
pub fn answer<T: serde::de::DeserializeOwned>(
    status: StatusCode,
    body: &[u8],
) -> Result<T, InboxFailure> {
    if status == StatusCode::OK {
        return serde_json::from_slice(body).map_err(|_| InboxFailure::Unavailable);
    }
    let code = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| value["error"]["code"].as_str().map(str::to_owned));
    let failure = match (status, code.as_deref()) {
        (StatusCode::UNPROCESSABLE_ENTITY, Some("invalid_snapshot")) => {
            InboxFailure::InvalidSnapshot
        }
        (StatusCode::UNPROCESSABLE_ENTITY, Some("invalid_cursor")) => InboxFailure::InvalidCursor,
        (StatusCode::UNPROCESSABLE_ENTITY, Some("invalid_group_version")) => {
            InboxFailure::InvalidVersion
        }
        (StatusCode::UNPROCESSABLE_ENTITY, Some("idempotency_key_reused")) => {
            InboxFailure::KeyReused
        }
        (StatusCode::NOT_FOUND, Some("not_found")) => InboxFailure::NotFound,
        _ => {
            tracing::info!(
                status = status.as_u16(),
                "notification inbox refused the call"
            );
            InboxFailure::Unavailable
        }
    };
    Err(failure)
}

fn inbox_path(recipient: PrincipalId, tail: &str) -> String {
    format!("/v1/inboxes/{}/{tail}", recipient.as_uuid().hyphenated())
}

impl NotificationInbox for SocketInbox {
    fn count(&self, recipient: PrincipalId) -> InboxFuture<'_, WireCount> {
        Box::pin(self.call(Method::GET, inbox_path(recipient, "count"), None))
    }

    fn page<'a>(
        &'a self,
        recipient: PrincipalId,
        limit: u8,
        cursor: Option<&'a str>,
    ) -> InboxFuture<'a, WirePage> {
        let mut path = inbox_path(recipient, &format!("groups?limit={limit}"));
        if let Some(cursor) = cursor {
            path.push_str("&cursor=");
            path.extend(form_urlencoded::byte_serialize(cursor.as_bytes()));
        }
        Box::pin(self.call(Method::GET, path, None))
    }

    fn command<'a>(
        &'a self,
        recipient: PrincipalId,
        command_key: &'a str,
        command: &'a InboxCommand,
    ) -> InboxFuture<'a, Value> {
        let (tail, body) = match command {
            InboxCommand::Seen { up_to } => (
                "seen",
                json!({ "command_key": command_key, "up_to_snapshot": format!("s{up_to}") }),
            ),
            InboxCommand::Read {
                group_id,
                observed_version,
            } => (
                "read",
                json!({
                    "command_key": command_key,
                    "group_id": group_id,
                    "observed_group_version": observed_version,
                }),
            ),
            InboxCommand::MarkAllRead { up_to } => (
                "mark-all-read",
                json!({ "command_key": command_key, "up_to_snapshot": format!("s{up_to}") }),
            ),
        };
        let path = inbox_path(recipient, tail);
        Box::pin(async move {
            let body = serde_json::to_vec(&body).map_err(|_| InboxFailure::Unavailable)?;
            self.call(Method::POST, path, Some(body)).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(pairs: &[(&str, &str)]) -> Result<Option<InboxTransport>, String> {
        InboxTransport::from_lookup(|key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
        })
    }

    #[test]
    fn the_inbox_transport_needs_a_socket_and_its_server_user() {
        assert_eq!(lookup(&[]), Ok(None));
        // The relay's own server-uid variable alone never configures the Gateway.
        assert_eq!(
            lookup(&[("VINK_NOTIFICATIONS_SERVER_UID", "501")]),
            Ok(None)
        );
        assert!(lookup(&[("VINK_NOTIFICATIONS_INBOX_SOCKET", "/run/n/inbox.sock")]).is_err());
        assert!(
            lookup(&[
                ("VINK_NOTIFICATIONS_INBOX_SOCKET", "/run/n/inbox.sock"),
                ("VINK_NOTIFICATIONS_SERVER_UID", "abc"),
            ])
            .is_err()
        );
        assert_eq!(
            lookup(&[
                ("VINK_NOTIFICATIONS_INBOX_SOCKET", "/run/n/inbox.sock"),
                ("VINK_NOTIFICATIONS_SERVER_UID", "501"),
            ]),
            Ok(Some(InboxTransport {
                socket: PathBuf::from("/run/n/inbox.sock"),
                server_uid: 501
            }))
        );
    }

    #[test]
    fn snapshots_are_the_service_spelling_only() {
        assert_eq!(parse_snapshot("s0"), Some(0));
        assert_eq!(parse_snapshot("s42"), Some(42));
        for bad in [
            "",
            "s",
            "42",
            "s-1",
            "s01",
            "s4 2",
            "S4",
            "s1234567890123456789",
        ] {
            assert_eq!(parse_snapshot(bad), None, "{bad}");
        }
    }

    #[test]
    fn contract_errors_are_told_apart_from_an_outage() {
        let error = |status: StatusCode, code: &str| {
            answer::<Value>(
                status,
                json!({"error": {"code": code, "message": "x"}})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap_err()
        };
        assert_eq!(
            error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_snapshot"),
            InboxFailure::InvalidSnapshot
        );
        assert_eq!(
            error(StatusCode::UNPROCESSABLE_ENTITY, "idempotency_key_reused"),
            InboxFailure::KeyReused
        );
        assert_eq!(
            error(StatusCode::NOT_FOUND, "not_found"),
            InboxFailure::NotFound
        );
        // A socket of the other scope, a refused caller, a database outage or a malformed 200 are
        // all outages: never an empty inbox.
        for (status, code) in [
            (StatusCode::NOT_FOUND, "unknown_route"),
            (StatusCode::UNAUTHORIZED, "caller_unauthenticated"),
            (StatusCode::SERVICE_UNAVAILABLE, "dependency_unavailable"),
            (StatusCode::UNPROCESSABLE_ENTITY, "something_new"),
        ] {
            assert_eq!(error(status, code), InboxFailure::Unavailable, "{code}");
        }
        assert_eq!(
            answer::<WireCount>(StatusCode::OK, b"{\"groups\":[]}").unwrap_err(),
            InboxFailure::Unavailable
        );
    }

    #[tokio::test]
    async fn calls_cross_a_real_socket_and_a_foreign_server_user_is_never_trusted() {
        use std::os::unix::fs::MetadataExt;

        let dir = std::env::temp_dir().join(format!("vink-ni-{}", Uuid::now_v7().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("b.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let router = axum::Router::new()
            .route(
                "/v1/inboxes/{recipient}/count",
                axum::routing::get(
                    |axum::extract::Path(recipient): axum::extract::Path<String>| async move {
                        axum::Json(json!({
                            "status": "fresh",
                            "unseen_group_count": if recipient.ends_with('b') { 2 } else { 0 },
                            "as_of_snapshot": "s7",
                            "max_staleness_seconds": 60,
                        }))
                    },
                ),
            )
            .route(
                "/v1/inboxes/{recipient}/seen",
                axum::routing::post(|body: Bytes| async move {
                    let request: Value = serde_json::from_slice(&body).unwrap();
                    axum::Json(json!({ "seen_snapshot": request["up_to_snapshot"] }))
                }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, router).await });
        let owner = std::fs::metadata(&path).unwrap().uid();
        let inbox = SocketInbox::new(InboxTransport {
            socket: path.clone(),
            server_uid: owner,
        });
        let recipient = PrincipalId::parse("22222222-2222-4222-8222-22222222222b").unwrap();
        let count = inbox.count(recipient).await.unwrap();
        assert_eq!(count.unseen_group_count, 2);
        assert_eq!(parse_snapshot(&count.as_of_snapshot), Some(7));
        let seen = inbox
            .command(recipient, "k-1", &InboxCommand::Seen { up_to: 7 })
            .await
            .unwrap();
        assert_eq!(seen["seen_snapshot"], "s7");
        let impostor = SocketInbox::new(InboxTransport {
            socket: path.clone(),
            server_uid: owner.wrapping_add(1),
        });
        assert_eq!(
            impostor.count(recipient).await.unwrap_err(),
            InboxFailure::Unavailable
        );
        server.abort();
        std::fs::remove_dir_all(&dir).unwrap();
        let gone = SocketInbox::new(InboxTransport {
            socket: path,
            server_uid: owner,
        });
        assert_eq!(
            gone.count(recipient).await.unwrap_err(),
            InboxFailure::Unavailable
        );
    }
}
