//! The App's client for the private Identity contract v1 (`vink.identity.v1`, owned by
//! `services/vink-identity/contracts/rpc/v1`). The App keeps its own representation of the wire
//! types here and never imports the Identity crate; `tests` decode the provider's fixtures.
//!
//! Transport: HTTP/1.1 on the Unix socket `VINK_IDENTITY_SOCKET`. Before sending anything the
//! client checks that the process serving the socket runs as `VINK_IDENTITY_SERVER_UID`, so a
//! socket planted by another user is never trusted. Every call has a deadline shorter than the
//! server's own and a response size limit.

use std::path::PathBuf;
use std::time::Duration;

use axum::body::Bytes;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::{BodyExt, Full, Limited};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use tokio::net::UnixStream;

/// Below the server's 2 s deadline, so the client gives up first.
pub const CALL_TIMEOUT: Duration = Duration::from_millis(1500);
/// Longest credential the contract accepts, in bytes.
pub const MAX_CREDENTIAL_BYTES: usize = 4096;
const MAX_RESPONSE_BYTES: usize = 16 * 1024;

/// What the Identity service decided about a presented credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verification {
    /// An authentication fact for `authority`/`subject`. It is not a session and grants nothing
    /// until the App maps the subject to a principal (not implemented).
    Authenticated {
        authority: String,
        subject: String,
    },
    Rejected,
}

/// Why no decision was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallError {
    /// Identity answered that no authority is configured: nobody can be authenticated.
    NoAuthority,
    /// Identity could not be asked or did not give a contract answer; nothing was decided.
    Unavailable(Failure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Connect,
    ServerIdentity,
    Timeout,
    Transport,
    CallerRefused,
    DependencyUnavailable,
    UnexpectedStatus,
    MalformedResponse,
}

impl Failure {
    pub fn label(self) -> &'static str {
        match self {
            Failure::Connect => "connect",
            Failure::ServerIdentity => "server_identity",
            Failure::Timeout => "timeout",
            Failure::Transport => "transport",
            Failure::CallerRefused => "caller_refused",
            Failure::DependencyUnavailable => "dependency_unavailable",
            Failure::UnexpectedStatus => "unexpected_status",
            Failure::MalformedResponse => "malformed_response",
        }
    }
}

/// Availability reported by `GET /identity/v1/capabilities`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authority {
    Available,
    NotConfigured,
}

// --- The App's representation of the wire types (consumer side). ---

#[derive(Debug, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum WireVerification {
    Authenticated {
        subject: WireSubject,
        #[allow(dead_code)]
        authenticated_at: String,
    },
    Rejected,
}

#[derive(Debug, Deserialize)]
struct WireSubject {
    authority: String,
    id: String,
}

#[derive(Debug, Deserialize)]
struct WireCapabilities {
    contract: String,
    authority: WireAvailability,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireAvailability {
    Available,
    NotConfigured,
}

#[derive(Debug, Deserialize)]
struct WireError {
    error: WireErrorBody,
}

#[derive(Debug, Deserialize)]
struct WireErrorBody {
    code: String,
}

#[derive(Debug, Clone)]
pub struct IdentityClient {
    socket: PathBuf,
    server_uid: u32,
    timeout: Duration,
}

impl IdentityClient {
    pub fn new(socket: PathBuf, server_uid: u32) -> Self {
        Self::with_timeout(socket, server_uid, CALL_TIMEOUT)
    }

    pub fn with_timeout(socket: PathBuf, server_uid: u32, timeout: Duration) -> Self {
        Self {
            socket,
            server_uid,
            timeout,
        }
    }

    /// `POST /identity/v1/verify-credential`. The credential is sent once and never logged.
    pub async fn verify(&self, credential: &str) -> Result<Verification, CallError> {
        let body = serde_json::to_vec(&serde_json::json!({ "credential": credential }))
            .map_err(|_| CallError::Unavailable(Failure::Transport))?;
        let (status, bytes) = self
            .call(Method::POST, "/identity/v1/verify-credential", body)
            .await?;
        match status {
            StatusCode::OK => match serde_json::from_slice::<WireVerification>(&bytes) {
                Ok(WireVerification::Authenticated { subject, .. }) => {
                    Ok(Verification::Authenticated {
                        authority: subject.authority,
                        subject: subject.id,
                    })
                }
                Ok(WireVerification::Rejected) => Ok(Verification::Rejected),
                Err(_) => Err(CallError::Unavailable(Failure::MalformedResponse)),
            },
            status => Err(error_for(status, &bytes)),
        }
    }

    /// `GET /identity/v1/capabilities`, for readiness reporting.
    pub async fn authority(&self) -> Result<Authority, CallError> {
        let (status, bytes) = self
            .call(Method::GET, "/identity/v1/capabilities", Vec::new())
            .await?;
        if status != StatusCode::OK {
            return Err(error_for(status, &bytes));
        }
        match serde_json::from_slice::<WireCapabilities>(&bytes) {
            Ok(caps) if caps.contract == "vink.identity.v1" => Ok(match caps.authority {
                WireAvailability::Available => Authority::Available,
                WireAvailability::NotConfigured => Authority::NotConfigured,
            }),
            _ => Err(CallError::Unavailable(Failure::MalformedResponse)),
        }
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Vec<u8>,
    ) -> Result<(StatusCode, Bytes), CallError> {
        tokio::time::timeout(self.timeout, self.exchange(method, path, body))
            .await
            .unwrap_or(Err(CallError::Unavailable(Failure::Timeout)))
    }

    async fn exchange(
        &self,
        method: Method,
        path: &str,
        body: Vec<u8>,
    ) -> Result<(StatusCode, Bytes), CallError> {
        let unavailable = CallError::Unavailable;
        let stream = UnixStream::connect(&self.socket)
            .await
            .map_err(|_| unavailable(Failure::Connect))?;
        match stream.peer_cred() {
            Ok(cred) if cred.uid() == self.server_uid => {}
            _ => return Err(unavailable(Failure::ServerIdentity)),
        }
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|_| unavailable(Failure::Transport))?;
        let driver = tokio::spawn(connection);
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "identity")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::new(Bytes::from(body)))
            .map_err(|_| unavailable(Failure::Transport))?;
        let result = async {
            let response = sender
                .send_request(request)
                .await
                .map_err(|_| unavailable(Failure::Transport))?;
            let status = response.status();
            let bytes = Limited::new(response.into_body(), MAX_RESPONSE_BYTES)
                .collect()
                .await
                .map_err(|_| unavailable(Failure::MalformedResponse))?
                .to_bytes();
            Ok((status, bytes))
        }
        .await;
        drop(sender);
        driver.abort();
        result
    }
}

/// Error answers by status and `code`; anything outside the contract is malformed.
fn error_for(status: StatusCode, bytes: &[u8]) -> CallError {
    let code = serde_json::from_slice::<WireError>(bytes)
        .map(|error| error.error.code)
        .unwrap_or_default();
    match (status.as_u16(), code.as_str()) {
        (501, "authority_not_configured") => CallError::NoAuthority,
        (503, "dependency_unavailable" | "deadline_exceeded") => {
            CallError::Unavailable(Failure::DependencyUnavailable)
        }
        (401, "caller_unauthenticated") => CallError::Unavailable(Failure::CallerRefused),
        (_, "") => CallError::Unavailable(Failure::MalformedResponse),
        _ => CallError::Unavailable(Failure::UnexpectedStatus),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Provider fixtures, read as files: the App never links the Identity crate.
    fn fixture(name: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../vink-identity/contracts/rpc/v1/fixtures")
            .join(name);
        std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    #[test]
    fn provider_fixtures_decode_into_the_app_representation() {
        let authenticated: WireVerification =
            serde_json::from_slice(&fixture("verify.authenticated.json")).unwrap();
        assert!(matches!(
            authenticated,
            WireVerification::Authenticated { ref subject, .. } if !subject.id.is_empty() && !subject.authority.is_empty()
        ));
        assert!(matches!(
            serde_json::from_slice::<WireVerification>(&fixture("verify.rejected.json")).unwrap(),
            WireVerification::Rejected
        ));
        let caps: WireCapabilities =
            serde_json::from_slice(&fixture("capabilities.not_configured.json")).unwrap();
        assert_eq!(caps.contract, "vink.identity.v1");
        assert!(matches!(caps.authority, WireAvailability::NotConfigured));
    }

    #[test]
    fn provider_error_fixtures_map_to_distinct_outcomes() {
        let cases = [
            (
                501,
                "error.authority_not_configured.json",
                CallError::NoAuthority,
            ),
            (
                503,
                "error.dependency_unavailable.json",
                CallError::Unavailable(Failure::DependencyUnavailable),
            ),
            (
                503,
                "error.deadline_exceeded.json",
                CallError::Unavailable(Failure::DependencyUnavailable),
            ),
            (
                401,
                "error.caller_unauthenticated.json",
                CallError::Unavailable(Failure::CallerRefused),
            ),
            (
                400,
                "error.malformed_request.json",
                CallError::Unavailable(Failure::UnexpectedStatus),
            ),
            (
                404,
                "error.unsupported_version.json",
                CallError::Unavailable(Failure::UnexpectedStatus),
            ),
        ];
        for (status, name, expected) in cases {
            let status = StatusCode::from_u16(status).unwrap();
            assert_eq!(error_for(status, &fixture(name)), expected, "{name}");
        }
        // A 501 that is not the contract's code is not mistaken for "no authority".
        assert_eq!(
            error_for(StatusCode::NOT_IMPLEMENTED, b"<html>proxy</html>"),
            CallError::Unavailable(Failure::MalformedResponse)
        );
    }
}
