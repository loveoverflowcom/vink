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
use time::OffsetDateTime;
use tokio::net::UnixStream;

/// Below the server's 2 s deadline, so the client gives up first.
pub const CALL_TIMEOUT: Duration = Duration::from_millis(1500);
/// Sign-in, logout and other committing operations: below the server's 5 s write deadline. Rare
/// and user-initiated, so a slow commit is waited for rather than reported as an outage.
pub const WRITE_TIMEOUT: Duration = Duration::from_millis(4500);
/// Longest credential the contract accepts, in bytes.
pub const MAX_CREDENTIAL_BYTES: usize = 4096;
// Byte limits of the Identity RPC fields, before policy validation.
const MAX_USERNAME_FIELD_BYTES: usize = 64;
const MAX_PASSWORD_FIELD_BYTES: usize = 4096;
const MAX_CHALLENGE_FIELD_BYTES: usize = 256;
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

/// A sign-in challenge: the handle is a client-held secret (it goes into an HttpOnly cookie).
#[derive(Clone, PartialEq, Eq)]
pub struct IssuedChallenge {
    pub handle: String,
    pub expires_at: OffsetDateTime,
}

impl std::fmt::Debug for IssuedChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedChallenge")
            .field("handle", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Why a challenge was not issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartError {
    InvalidEmail,
    RateLimited,
    Call(CallError),
}

/// A new session for an authenticated subject. The credential is handed to the client once.
#[derive(Clone, PartialEq, Eq)]
pub struct IssuedSession {
    pub authority: String,
    pub subject: String,
    pub credential: String,
    pub expires_at: OffsetDateTime,
}

impl std::fmt::Debug for IssuedSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedSession")
            .field("authority", &self.authority)
            .field("subject", &self.subject)
            .field("credential", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Identity's reason for not completing a challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeRefusal {
    /// Wrong code; another attempt is allowed.
    InvalidCode,
    /// Expired, used, out of attempts or unknown: request a new code.
    ChallengeClosed,
    AccountDisabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignIn {
    Authenticated(IssuedSession),
    Refused(ChallengeRefusal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordRegisterOutcome {
    Authenticated(IssuedSession),
    UsernameTaken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordRegisterError {
    InvalidUsername,
    InvalidPassword,
    RateLimited,
    Call(CallError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordLoginOutcome {
    Authenticated(IssuedSession),
    InvalidCredentials,
    AccountDisabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordLoginError {
    InvalidUsername,
    InvalidPassword,
    RateLimited,
    Call(CallError),
}

/// Why an enrollment challenge was not issued. Every variant means no credential was added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollStartError {
    /// The session is not (or no longer) valid.
    CredentialRejected,
    /// The account already has a username/password: nothing to add, nothing was mailed.
    AlreadyEnrolled,
    AccountDisabled,
    /// The account has no verified address to prove control with.
    NoVerifiedAddress,
    RateLimited,
    Call(CallError),
}

/// Identity's decision on one enrollment attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollOutcome {
    /// The credential belongs to the session's own account now.
    Enrolled,
    Refused(EnrollRefusal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollRefusal {
    /// Wrong code; the challenge accepts another attempt.
    InvalidCode,
    /// Expired, used, exhausted, another account's or outdated: request a new code.
    ChallengeClosed,
    /// The username is taken. The code is not spent.
    UsernameTaken,
    AlreadyEnrolled,
    AccountDisabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollError {
    InvalidUsername,
    InvalidPassword,
    CredentialRejected,
    Call(CallError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollStatusError {
    CredentialRejected,
    AccountDisabled,
    Call(CallError),
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

#[derive(Deserialize)]
struct WireChallenge {
    challenge: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum WireSignIn {
    Authenticated {
        subject: WireSubject,
        session: WireIssuedSession,
    },
    Rejected {
        reason: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum WirePasswordRegister {
    Authenticated {
        subject: WireSubject,
        session: WireIssuedSession,
    },
    Rejected {
        reason: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum WirePasswordSignIn {
    Authenticated {
        subject: WireSubject,
        session: WireIssuedSession,
    },
    Rejected {
        reason: String,
    },
}

#[derive(Deserialize)]
struct WireIssuedSession {
    credential: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}

impl std::fmt::Debug for WireIssuedSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WireIssuedSession(<redacted>)")
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum WireEnroll {
    Enrolled,
    Rejected { reason: String },
}

#[derive(Deserialize)]
struct WireEnrollmentStatus {
    enrolled: bool,
}

#[derive(Debug, Deserialize)]
struct WireRevoked {
    revoked: bool,
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
    write_timeout: Duration,
}

impl IdentityClient {
    pub fn new(socket: PathBuf, server_uid: u32) -> Self {
        Self {
            write_timeout: WRITE_TIMEOUT,
            ..Self::with_timeout(socket, server_uid, CALL_TIMEOUT)
        }
    }

    /// One timeout for every call (tests use a short one).
    pub fn with_timeout(socket: PathBuf, server_uid: u32, timeout: Duration) -> Self {
        Self {
            socket,
            server_uid,
            timeout,
            write_timeout: timeout,
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

    /// `POST /identity/v1/sign-in/challenges`. Identity sends the code; the answer never says
    /// whether an account exists.
    pub async fn start_sign_in(&self, email: &str) -> Result<IssuedChallenge, StartError> {
        let body = serde_json::to_vec(&serde_json::json!({ "email": email }))
            .map_err(|_| StartError::Call(CallError::Unavailable(Failure::Transport)))?;
        let (status, bytes) = self
            .call_within(
                self.write_timeout,
                Method::POST,
                "/identity/v1/sign-in/challenges",
                body,
            )
            .await
            .map_err(StartError::Call)?;
        match (status.as_u16(), error_code(&bytes).as_str()) {
            (202, _) => serde_json::from_slice::<WireChallenge>(&bytes)
                .map(|wire| IssuedChallenge {
                    handle: wire.challenge,
                    expires_at: wire.expires_at,
                })
                .map_err(|_| StartError::Call(CallError::Unavailable(Failure::MalformedResponse))),
            (422, "invalid_email") => Err(StartError::InvalidEmail),
            (429, "rate_limited") => Err(StartError::RateLimited),
            _ => Err(StartError::Call(error_for(status, &bytes))),
        }
    }

    /// `POST /identity/v1/sign-in/complete`. A refusal is an answer, not an outage.
    pub async fn complete_sign_in(&self, handle: &str, code: &str) -> Result<SignIn, CallError> {
        let body = serde_json::to_vec(&serde_json::json!({ "challenge": handle, "code": code }))
            .map_err(|_| CallError::Unavailable(Failure::Transport))?;
        let (status, bytes) = self
            .call_within(
                self.write_timeout,
                Method::POST,
                "/identity/v1/sign-in/complete",
                body,
            )
            .await?;
        if status != StatusCode::OK {
            return Err(error_for(status, &bytes));
        }
        let malformed = CallError::Unavailable(Failure::MalformedResponse);
        match serde_json::from_slice::<WireSignIn>(&bytes).map_err(|_| malformed)? {
            WireSignIn::Authenticated { subject, session } => {
                Ok(SignIn::Authenticated(IssuedSession {
                    authority: subject.authority,
                    subject: subject.id,
                    credential: session.credential,
                    expires_at: session.expires_at,
                }))
            }
            WireSignIn::Rejected { reason } => Ok(SignIn::Refused(match reason.as_str() {
                "invalid_code" => ChallengeRefusal::InvalidCode,
                "challenge_closed" => ChallengeRefusal::ChallengeClosed,
                "account_disabled" => ChallengeRefusal::AccountDisabled,
                // An unknown reason is outside the contract: nothing is decided.
                _ => return Err(malformed),
            })),
        }
    }

    /// `POST /identity/v1/password/register`.
    pub async fn register_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<PasswordRegisterOutcome, PasswordRegisterError> {
        if !fits_wire_field(username, MAX_USERNAME_FIELD_BYTES) {
            return Err(PasswordRegisterError::InvalidUsername);
        }
        if !fits_wire_field(password, MAX_PASSWORD_FIELD_BYTES) {
            return Err(PasswordRegisterError::InvalidPassword);
        }
        let body = serde_json::to_vec(&serde_json::json!({
            "username": username,
            "password": password,
        }))
        .map_err(|_| PasswordRegisterError::Call(CallError::Unavailable(Failure::Transport)))?;
        let (status, bytes) = self
            .call_within(
                self.write_timeout,
                Method::POST,
                "/identity/v1/password/register",
                body,
            )
            .await
            .map_err(PasswordRegisterError::Call)?;
        match (status.as_u16(), error_code(&bytes).as_str()) {
            (200, _) => {
                let malformed =
                    PasswordRegisterError::Call(CallError::Unavailable(Failure::MalformedResponse));
                match serde_json::from_slice::<WirePasswordRegister>(&bytes)
                    .map_err(|_| malformed)?
                {
                    WirePasswordRegister::Authenticated { subject, session } => {
                        Ok(PasswordRegisterOutcome::Authenticated(IssuedSession {
                            authority: subject.authority,
                            subject: subject.id,
                            credential: session.credential,
                            expires_at: session.expires_at,
                        }))
                    }
                    WirePasswordRegister::Rejected { reason } if reason == "username_taken" => {
                        Ok(PasswordRegisterOutcome::UsernameTaken)
                    }
                    _ => Err(malformed),
                }
            }
            (422, "invalid_username") => Err(PasswordRegisterError::InvalidUsername),
            (422, "invalid_password") => Err(PasswordRegisterError::InvalidPassword),
            (409, "username_taken") => Ok(PasswordRegisterOutcome::UsernameTaken),
            (429, "rate_limited") => Err(PasswordRegisterError::RateLimited),
            _ => Err(PasswordRegisterError::Call(error_for(status, &bytes))),
        }
    }

    /// `POST /identity/v1/password/sign-in`.
    pub async fn login_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<PasswordLoginOutcome, PasswordLoginError> {
        if !fits_wire_field(username, MAX_USERNAME_FIELD_BYTES) {
            return Err(PasswordLoginError::InvalidUsername);
        }
        if !fits_wire_field(password, MAX_PASSWORD_FIELD_BYTES) {
            return Err(PasswordLoginError::InvalidPassword);
        }
        let body = serde_json::to_vec(&serde_json::json!({
            "username": username,
            "password": password,
        }))
        .map_err(|_| PasswordLoginError::Call(CallError::Unavailable(Failure::Transport)))?;
        let (status, bytes) = self
            .call_within(
                self.write_timeout,
                Method::POST,
                "/identity/v1/password/sign-in",
                body,
            )
            .await
            .map_err(PasswordLoginError::Call)?;
        match (status.as_u16(), error_code(&bytes).as_str()) {
            (200, _) => {
                let malformed =
                    PasswordLoginError::Call(CallError::Unavailable(Failure::MalformedResponse));
                match serde_json::from_slice::<WirePasswordSignIn>(&bytes).map_err(|_| malformed)? {
                    WirePasswordSignIn::Authenticated { subject, session } => {
                        Ok(PasswordLoginOutcome::Authenticated(IssuedSession {
                            authority: subject.authority,
                            subject: subject.id,
                            credential: session.credential,
                            expires_at: session.expires_at,
                        }))
                    }
                    WirePasswordSignIn::Rejected { reason } => match reason.as_str() {
                        "invalid_credentials" => Ok(PasswordLoginOutcome::InvalidCredentials),
                        "account_disabled" => Ok(PasswordLoginOutcome::AccountDisabled),
                        _ => Err(malformed),
                    },
                }
            }
            (422, "invalid_username") => Err(PasswordLoginError::InvalidUsername),
            (422, "invalid_password") => Err(PasswordLoginError::InvalidPassword),
            (429, "rate_limited") => Err(PasswordLoginError::RateLimited),
            _ => Err(PasswordLoginError::Call(error_for(status, &bytes))),
        }
    }

    /// `POST /identity/v1/password/enroll/challenges`: a code goes to the verified address of the
    /// account `credential` names. The account is never a parameter.
    pub async fn start_password_enrollment(
        &self,
        credential: &str,
    ) -> Result<IssuedChallenge, EnrollStartError> {
        let body = serde_json::to_vec(&serde_json::json!({ "credential": credential }))
            .map_err(|_| EnrollStartError::Call(CallError::Unavailable(Failure::Transport)))?;
        let (status, bytes) = self
            .call_within(
                self.write_timeout,
                Method::POST,
                "/identity/v1/password/enroll/challenges",
                body,
            )
            .await
            .map_err(EnrollStartError::Call)?;
        match (status.as_u16(), error_code(&bytes).as_str()) {
            (202, _) => serde_json::from_slice::<WireChallenge>(&bytes)
                .map(|wire| IssuedChallenge {
                    handle: wire.challenge,
                    expires_at: wire.expires_at,
                })
                .map_err(|_| {
                    EnrollStartError::Call(CallError::Unavailable(Failure::MalformedResponse))
                }),
            (403, "credential_rejected") => Err(EnrollStartError::CredentialRejected),
            (403, "account_disabled") => Err(EnrollStartError::AccountDisabled),
            (409, "already_enrolled") => Err(EnrollStartError::AlreadyEnrolled),
            (409, "address_unavailable") => Err(EnrollStartError::NoVerifiedAddress),
            (429, "rate_limited") => Err(EnrollStartError::RateLimited),
            _ => Err(EnrollStartError::Call(error_for(status, &bytes))),
        }
    }

    /// `POST /identity/v1/password/enroll`. Identity owns the password policy; the App bounds
    /// only what it sends over the transport.
    pub async fn enroll_password(
        &self,
        credential: &str,
        handle: &str,
        code: &str,
        username: &str,
        password: &str,
    ) -> Result<EnrollOutcome, EnrollError> {
        if !fits_wire_field(handle, MAX_CHALLENGE_FIELD_BYTES) {
            return Ok(EnrollOutcome::Refused(EnrollRefusal::ChallengeClosed));
        }
        if !fits_wire_field(username, MAX_USERNAME_FIELD_BYTES) {
            return Err(EnrollError::InvalidUsername);
        }
        if !fits_wire_field(password, MAX_PASSWORD_FIELD_BYTES) {
            return Err(EnrollError::InvalidPassword);
        }
        let body = serde_json::to_vec(&serde_json::json!({
            "credential": credential,
            "challenge": handle,
            "code": code,
            "username": username,
            "password": password,
        }))
        .map_err(|_| EnrollError::Call(CallError::Unavailable(Failure::Transport)))?;
        let (status, bytes) = self
            .call_within(
                self.write_timeout,
                Method::POST,
                "/identity/v1/password/enroll",
                body,
            )
            .await
            .map_err(EnrollError::Call)?;
        let malformed = EnrollError::Call(CallError::Unavailable(Failure::MalformedResponse));
        match (status.as_u16(), error_code(&bytes).as_str()) {
            (200, _) => {
                match serde_json::from_slice::<WireEnroll>(&bytes).map_err(|_| malformed)? {
                    WireEnroll::Enrolled => Ok(EnrollOutcome::Enrolled),
                    WireEnroll::Rejected { reason } => {
                        Ok(EnrollOutcome::Refused(match reason.as_str() {
                            "invalid_code" => EnrollRefusal::InvalidCode,
                            "challenge_closed" => EnrollRefusal::ChallengeClosed,
                            "username_taken" => EnrollRefusal::UsernameTaken,
                            "already_enrolled" => EnrollRefusal::AlreadyEnrolled,
                            "account_disabled" => EnrollRefusal::AccountDisabled,
                            // An unknown reason is outside the contract: nothing is decided.
                            _ => return Err(malformed),
                        }))
                    }
                }
            }
            (403, "credential_rejected") => Err(EnrollError::CredentialRejected),
            (403, "account_disabled") => Ok(EnrollOutcome::Refused(EnrollRefusal::AccountDisabled)),
            (422, "invalid_username") => Err(EnrollError::InvalidUsername),
            (422, "invalid_password") => Err(EnrollError::InvalidPassword),
            _ => Err(EnrollError::Call(error_for(status, &bytes))),
        }
    }

    /// The status of the account named by the presenting credential. This sends no mail.
    pub async fn password_enrollment_status(
        &self,
        credential: &str,
    ) -> Result<bool, EnrollStatusError> {
        let body = serde_json::to_vec(&serde_json::json!({ "credential": credential }))
            .map_err(|_| EnrollStatusError::Call(CallError::Unavailable(Failure::Transport)))?;
        let (status, bytes) = self
            .call(Method::POST, "/identity/v1/password/enroll/status", body)
            .await
            .map_err(EnrollStatusError::Call)?;
        match (status.as_u16(), error_code(&bytes).as_str()) {
            (200, _) => serde_json::from_slice::<WireEnrollmentStatus>(&bytes)
                .map(|response| response.enrolled)
                .map_err(|_| {
                    EnrollStatusError::Call(CallError::Unavailable(Failure::MalformedResponse))
                }),
            (403, "credential_rejected") => Err(EnrollStatusError::CredentialRejected),
            (403, "account_disabled") => Err(EnrollStatusError::AccountDisabled),
            _ => Err(EnrollStatusError::Call(error_for(status, &bytes))),
        }
    }

    /// `POST /identity/v1/sessions/revoke` (logout). `false`: nothing active was revoked.
    pub async fn revoke(&self, credential: &str) -> Result<bool, CallError> {
        let body = serde_json::to_vec(&serde_json::json!({ "credential": credential }))
            .map_err(|_| CallError::Unavailable(Failure::Transport))?;
        let (status, bytes) = self
            .call_within(
                self.write_timeout,
                Method::POST,
                "/identity/v1/sessions/revoke",
                body,
            )
            .await?;
        if status != StatusCode::OK {
            return Err(error_for(status, &bytes));
        }
        serde_json::from_slice::<WireRevoked>(&bytes)
            .map(|wire| wire.revoked)
            .map_err(|_| CallError::Unavailable(Failure::MalformedResponse))
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
        self.call_within(self.timeout, method, path, body).await
    }

    async fn call_within(
        &self,
        limit: Duration,
        method: Method,
        path: &str,
        body: Vec<u8>,
    ) -> Result<(StatusCode, Bytes), CallError> {
        tokio::time::timeout(limit, self.exchange(method, path, body))
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

/// Matches Identity's non-empty, byte-bounded wire fields. Identity applies the credential policy.
fn fits_wire_field(value: &str, max_bytes: usize) -> bool {
    (1..=max_bytes).contains(&value.len())
}

fn error_code(bytes: &[u8]) -> String {
    serde_json::from_slice::<WireError>(bytes)
        .map(|error| error.error.code)
        .unwrap_or_default()
}

/// Error answers by status and `code`; anything outside the contract is malformed.
fn error_for(status: StatusCode, bytes: &[u8]) -> CallError {
    let code = error_code(bytes);
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
    fn email_code_fixtures_decode_into_the_app_representation() {
        let challenge: WireChallenge =
            serde_json::from_slice(&fixture("challenge.issued.json")).unwrap();
        assert!(challenge.challenge.starts_with("vink_c1."));
        match serde_json::from_slice::<WireSignIn>(&fixture("sign_in.authenticated.json")).unwrap()
        {
            WireSignIn::Authenticated { subject, session } => {
                assert_eq!(subject.authority, "vink-identity");
                assert!(session.credential.starts_with("vink_s1_"));
                assert!(!format!("{session:?}").contains("vink_s1_"));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            serde_json::from_slice::<WireSignIn>(&fixture("sign_in.rejected.json")).unwrap(),
            WireSignIn::Rejected { ref reason } if reason == "invalid_code"
        ));
        // Additive v1 fields on verify are ignored by this consumer; the outcome is unchanged.
        assert!(matches!(
            serde_json::from_slice::<WireVerification>(&fixture(
                "verify.authenticated.session.json"
            ))
            .unwrap(),
            WireVerification::Authenticated { .. }
        ));
        assert!(matches!(
            serde_json::from_slice::<WireVerification>(&fixture("verify.rejected.revoked.json"))
                .unwrap(),
            WireVerification::Rejected
        ));
        let revoked: WireRevoked =
            serde_json::from_slice(&fixture("session.revoked.json")).unwrap();
        assert!(revoked.revoked);
        let caps: WireCapabilities =
            serde_json::from_slice(&fixture("capabilities.email_code.json")).unwrap();
        assert!(matches!(caps.authority, WireAvailability::Available));
        for (name, code) in [
            ("error.invalid_email.json", "invalid_email"),
            ("error.rate_limited.json", "rate_limited"),
        ] {
            assert_eq!(error_code(&fixture(name)), code);
        }
    }

    #[test]
    fn password_fixtures_decode_into_the_app_representation() {
        match serde_json::from_slice::<WirePasswordRegister>(&fixture(
            "password.register.authenticated.json",
        ))
        .unwrap()
        {
            WirePasswordRegister::Authenticated { subject, session } => {
                assert_eq!(subject.authority, "vink-identity");
                assert!(session.credential.starts_with("vink_s1_"));
                assert!(!format!("{session:?}").contains("vink_s1_"));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            serde_json::from_slice::<WirePasswordRegister>(&fixture("password.register.rejected.username_taken.json")).unwrap(),
            WirePasswordRegister::Rejected { ref reason } if reason == "username_taken"
        ));

        match serde_json::from_slice::<WirePasswordSignIn>(&fixture(
            "password.sign_in.authenticated.json",
        ))
        .unwrap()
        {
            WirePasswordSignIn::Authenticated { subject, session } => {
                assert_eq!(subject.authority, "vink-identity");
                assert!(session.credential.starts_with("vink_s1_"));
                assert!(!format!("{session:?}").contains("vink_s1_"));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            serde_json::from_slice::<WirePasswordSignIn>(&fixture("password.sign_in.rejected.invalid_credentials.json")).unwrap(),
            WirePasswordSignIn::Rejected { ref reason } if reason == "invalid_credentials"
        ));

        let caps: WireCapabilities =
            serde_json::from_slice(&fixture("capabilities.password.json")).unwrap();
        assert!(matches!(caps.authority, WireAvailability::Available));

        for (name, code) in [
            ("error.invalid_username.json", "invalid_username"),
            ("error.invalid_password.json", "invalid_password"),
            ("error.username_taken.json", "username_taken"),
        ] {
            assert_eq!(error_code(&fixture(name)), code);
        }
    }

    #[test]
    fn enrollment_fixtures_decode_into_the_app_representation() {
        assert!(matches!(
            serde_json::from_slice::<WireEnroll>(&fixture("password.enroll.enrolled.json"))
                .unwrap(),
            WireEnroll::Enrolled
        ));
        for (name, expected) in [
            ("password.enroll.status.enrolled.json", true),
            ("password.enroll.status.unenrolled.json", false),
        ] {
            let status = serde_json::from_slice::<WireEnrollmentStatus>(&fixture(name)).unwrap();
            assert_eq!(status.enrolled, expected, "{name}");
        }
        for (name, expected) in [
            (
                "password.enroll.rejected.username_taken.json",
                "username_taken",
            ),
            ("password.enroll.rejected.invalid_code.json", "invalid_code"),
        ] {
            assert!(matches!(
                serde_json::from_slice::<WireEnroll>(&fixture(name)).unwrap(),
                WireEnroll::Rejected { ref reason } if reason == expected
            ));
        }
        for (name, code) in [
            ("error.already_enrolled.json", "already_enrolled"),
            ("error.account_disabled.json", "account_disabled"),
            ("error.credential_rejected.json", "credential_rejected"),
            ("error.address_unavailable.json", "address_unavailable"),
        ] {
            assert_eq!(error_code(&fixture(name)), code);
        }
        // The wire request carries the session and the proof, never an account.
        let request: serde_json::Value =
            serde_json::from_slice(&fixture("password.enroll.request.json")).unwrap();
        let mut keys: Vec<_> = request.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            ["challenge", "code", "credential", "password", "username"]
        );
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
