//! The relay's transport to the Notification Service (#347, T-07 CHOICE-21 A): HTTP/1.1 on the
//! private ingest socket `VINK_NOTIFICATIONS_INGEST_SOCKET`. Before sending anything the client
//! checks that the process serving the socket runs as `VINK_NOTIFICATIONS_SERVER_UID`, so a socket
//! planted by another user is never trusted; the service in turn answers only its configured relay
//! user. The App keeps its own representation of the wire types and never imports the service.
//!
//! One call carries a whole relay batch: `POST /v1/ingest` with `{"events": [{"event_id",
//! "envelope"}]}` (the envelope is the stored canonical wire text, byte for byte) and answers
//! `{"results": [{"event_id", "status"}]}` after the service committed each event's receipt and
//! effect. Anything else (refused connection, wrong server user, non-200, malformed answer) is
//! [`SinkFailure::Unavailable`]: the relay retries the batch and the receipts make it one effect.

use std::path::PathBuf;
use std::time::Duration;

use axum::body::Bytes;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::{BodyExt, Full, Limited};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;
use uuid::Uuid;

use crate::domain::notification::IngestStatus;
use crate::notification_relay::{
    BoxFuture, IngestReceipt, NotificationSink, Outgoing, RelaySettings, SinkFailure,
};

/// `sun_path` is 104 bytes on macOS and 108 on Linux, including the terminator.
pub const MAX_SOCKET_PATH: usize = 100;
/// A full batch of results is ~10 KiB.
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// Where the relay sends, read from the worker's environment. Both variables or neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayTransport {
    pub socket: PathBuf,
    pub server_uid: u32,
}

impl RelayTransport {
    /// `Ok(None)` when neither variable is set: the relay stays off (`no_transport`).
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Option<Self>, String> {
        let value = |key: &str| get(key).filter(|v| !v.trim().is_empty());
        match (
            value("VINK_NOTIFICATIONS_INGEST_SOCKET"),
            value("VINK_NOTIFICATIONS_SERVER_UID"),
        ) {
            (None, None) => Ok(None),
            (Some(_), None) => Err(
                "VINK_NOTIFICATIONS_SERVER_UID: required with VINK_NOTIFICATIONS_INGEST_SOCKET (user id that must serve the socket)"
                    .into(),
            ),
            (None, Some(_)) => {
                Err("VINK_NOTIFICATIONS_SERVER_UID: set without VINK_NOTIFICATIONS_INGEST_SOCKET".into())
            }
            (Some(socket), Some(_)) if socket.len() > MAX_SOCKET_PATH => Err(format!(
                "VINK_NOTIFICATIONS_INGEST_SOCKET: longer than {MAX_SOCKET_PATH} bytes, the Unix socket path limit"
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

#[derive(Serialize)]
struct WireEvent<'a> {
    event_id: Uuid,
    envelope: &'a str,
}

#[derive(Serialize)]
struct WireRequest<'a> {
    events: Vec<WireEvent<'a>>,
}

#[derive(Deserialize)]
struct WireResult {
    event_id: Uuid,
    status: String,
}

#[derive(Deserialize)]
struct WireResponse {
    results: Vec<WireResult>,
}

/// The production [`NotificationSink`]: one connection per call, bounded by `timeout`.
#[derive(Debug, Clone)]
pub struct SocketSink {
    socket: PathBuf,
    server_uid: u32,
    timeout: Duration,
}

impl SocketSink {
    /// The call bound stays below the relay's own send timeout, so the relay sees a transport
    /// failure rather than its timeout when the service stalls.
    pub fn new(transport: RelayTransport, settings: RelaySettings) -> Self {
        let timeout = settings
            .send_timeout
            .saturating_sub(Duration::from_millis(500))
            .max(Duration::from_millis(500));
        Self {
            socket: transport.socket,
            server_uid: transport.server_uid,
            timeout,
        }
    }

    async fn exchange(&self, body: Vec<u8>) -> Result<(StatusCode, Bytes), SinkFailure> {
        let stream = UnixStream::connect(&self.socket)
            .await
            .map_err(|_| SinkFailure::Unavailable)?;
        match stream.peer_cred() {
            Ok(cred) if cred.uid() == self.server_uid => {}
            _ => {
                tracing::warn!("notification socket is served by an unexpected user; not sending");
                return Err(SinkFailure::Unavailable);
            }
        }
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|_| SinkFailure::Unavailable)?;
        let driver = tokio::spawn(connection);
        let request = Request::builder()
            .method(Method::POST)
            .uri("/v1/ingest")
            .header(header::HOST, "notifications")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::new(Bytes::from(body)))
            .map_err(|_| SinkFailure::Unavailable)?;
        let result = async {
            let response = sender
                .send_request(request)
                .await
                .map_err(|_| SinkFailure::Unavailable)?;
            let status = response.status();
            let bytes = Limited::new(response.into_body(), MAX_RESPONSE_BYTES)
                .collect()
                .await
                .map_err(|_| SinkFailure::Unavailable)?
                .to_bytes();
            Ok((status, bytes))
        }
        .await;
        drop(sender);
        driver.abort();
        result
    }
}

/// Maps the service's answer to receipts; an unknown status or a non-200 fails the whole call.
pub fn receipts(status: StatusCode, body: &[u8]) -> Result<Vec<IngestReceipt>, SinkFailure> {
    if status != StatusCode::OK {
        tracing::info!(
            status = status.as_u16(),
            "notification service refused the call"
        );
        return Err(SinkFailure::Unavailable);
    }
    let response: WireResponse =
        serde_json::from_slice(body).map_err(|_| SinkFailure::Unavailable)?;
    response
        .results
        .into_iter()
        .map(|result| {
            IngestStatus::from_code(&result.status)
                .map(|status| IngestReceipt {
                    event_id: result.event_id,
                    status,
                })
                .ok_or(SinkFailure::Unavailable)
        })
        .collect()
}

impl NotificationSink for SocketSink {
    fn deliver<'a>(
        &'a self,
        events: &'a [Outgoing],
    ) -> BoxFuture<'a, Result<Vec<IngestReceipt>, SinkFailure>> {
        Box::pin(async move {
            let request = WireRequest {
                events: events
                    .iter()
                    .map(|event| WireEvent {
                        event_id: event.event_id,
                        envelope: &event.wire,
                    })
                    .collect(),
            };
            let body = serde_json::to_vec(&request).map_err(|_| SinkFailure::Unavailable)?;
            let (status, bytes) = tokio::time::timeout(self.timeout, self.exchange(body))
                .await
                .map_err(|_| SinkFailure::Unavailable)??;
            receipts(status, &bytes)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(pairs: &[(&str, &str)]) -> Result<Option<RelayTransport>, String> {
        let env: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        RelayTransport::from_lookup(|key| env.get(key).cloned())
    }

    #[test]
    fn the_transport_needs_both_variables_or_neither() {
        assert_eq!(lookup(&[]), Ok(None));
        assert!(
            lookup(&[("VINK_NOTIFICATIONS_INGEST_SOCKET", "/run/n/ingest.sock")])
                .unwrap_err()
                .contains("required")
        );
        assert!(
            lookup(&[("VINK_NOTIFICATIONS_SERVER_UID", "1000")])
                .unwrap_err()
                .contains("without")
        );
        assert!(
            lookup(&[
                ("VINK_NOTIFICATIONS_INGEST_SOCKET", "/run/n/ingest.sock"),
                ("VINK_NOTIFICATIONS_SERVER_UID", "app")
            ])
            .unwrap_err()
            .contains("numeric")
        );
        assert_eq!(
            lookup(&[
                ("VINK_NOTIFICATIONS_INGEST_SOCKET", "/run/n/ingest.sock"),
                ("VINK_NOTIFICATIONS_SERVER_UID", "1000")
            ]),
            Ok(Some(RelayTransport {
                socket: PathBuf::from("/run/n/ingest.sock"),
                server_uid: 1000
            }))
        );
    }

    #[test]
    fn the_contract_answer_maps_to_receipts_and_anything_else_fails_the_call() {
        let body = br#"{"results":[{"event_id":"019a9b10-0001-7000-8000-000000000101","status":"applied"},{"event_id":"019a9b10-0002-7000-8000-000000000102","status":"already_applied"}]}"#;
        let answered = receipts(StatusCode::OK, body).unwrap();
        assert_eq!(answered.len(), 2);
        assert_eq!(answered[1].status, IngestStatus::AlreadyApplied);
        assert_eq!(
            receipts(StatusCode::UNAUTHORIZED, br#"{"error":{}}"#),
            Err(SinkFailure::Unavailable)
        );
        let unknown = br#"{"results":[{"event_id":"019a9b10-0001-7000-8000-000000000101","status":"maybe"}]}"#;
        assert_eq!(
            receipts(StatusCode::OK, unknown),
            Err(SinkFailure::Unavailable)
        );
        assert_eq!(
            receipts(StatusCode::OK, b"[]"),
            Err(SinkFailure::Unavailable)
        );
    }

    /// A receiver fixture on a real Unix socket: answers `applied` for each event and records
    /// the envelopes exactly as received.
    #[tokio::test]
    async fn a_batch_crosses_a_real_socket_and_a_foreign_server_user_is_never_trusted() {
        use std::os::unix::fs::MetadataExt;
        use std::sync::{Arc, Mutex};

        let dir = std::env::temp_dir().join(format!("vink-nt-{}", Uuid::now_v7().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("i.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let seen: Arc<Mutex<Vec<String>>> = Arc::default();
        let recorded = seen.clone();
        let router = axum::Router::new().route(
            "/v1/ingest",
            axum::routing::post(move |body: Bytes| {
                let recorded = recorded.clone();
                async move {
                    let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
                    let mut results = Vec::new();
                    for event in request["events"].as_array().unwrap() {
                        recorded
                            .lock()
                            .unwrap()
                            .push(event["envelope"].as_str().unwrap().into());
                        results.push(
                            serde_json::json!({"event_id": event["event_id"], "status": "applied"}),
                        );
                    }
                    axum::Json(serde_json::json!({ "results": results }))
                }
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, router).await });
        let owner = std::fs::metadata(&path).unwrap().uid();
        let wire = r#"{"event_id":"019a9b10-0001-7000-8000-000000000101","payload_digest":"ab","x":"Thấm"}"#;
        let outgoing = [Outgoing {
            event_id: Uuid::parse_str("019a9b10-0001-7000-8000-000000000101").unwrap(),
            payload_digest: "ab".into(),
            wire: wire.into(),
        }];
        let sink = SocketSink::new(
            RelayTransport {
                socket: path.clone(),
                server_uid: owner,
            },
            RelaySettings::default(),
        );
        let answered = sink.deliver(&outgoing).await.unwrap();
        assert_eq!(
            answered,
            vec![IngestReceipt {
                event_id: outgoing[0].event_id,
                status: IngestStatus::Applied
            }]
        );
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            [wire.to_string()],
            "sent byte for byte"
        );
        // Same socket, but the relay expects another server user: nothing is sent.
        let impostor = SocketSink::new(
            RelayTransport {
                socket: path.clone(),
                server_uid: owner.wrapping_add(1),
            },
            RelaySettings::default(),
        );
        assert_eq!(
            impostor.deliver(&outgoing).await,
            Err(SinkFailure::Unavailable)
        );
        assert_eq!(seen.lock().unwrap().len(), 1);
        server.abort();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn an_absent_socket_is_unavailable_not_a_panic() {
        let sink = SocketSink::new(
            RelayTransport {
                socket: std::env::temp_dir()
                    .join(format!("vink-no-{}.sock", Uuid::now_v7().simple())),
                server_uid: 0,
            },
            RelaySettings::default(),
        );
        let outgoing = [Outgoing {
            event_id: Uuid::now_v7(),
            payload_digest: "00".repeat(32),
            wire: "{}".into(),
        }];
        assert_eq!(sink.deliver(&outgoing).await, Err(SinkFailure::Unavailable));
    }
}
