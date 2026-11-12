//! HTTP/1.1 on the private Unix socket. Health answers any process that can reach the socket;
//! contract v1 answers only connections whose OS peer user is the configured caller. No header,
//! body field or network address is evidence of a caller or of an end user.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::accounts::{
    ChallengeOutcome, EmailChangeOutcome, EmailChangeStart, EnrollmentOutcome, EnrollmentRejection,
    EnrollmentStart, PasswordRegistrationError, PasswordSignInRejection, SignInOutcome,
};
use crate::authority::{self, Authority, AuthorityError};
use crate::contract::{
    CONTRACT, Capabilities, ChallengeIssued, ChallengeRejection, ContractError,
    CredentialRejection, EmailChangeChallengeRequest, EmailChangeCompleteRequest,
    EmailChangeResponse, EnrollPasswordRejection, EnrollPasswordRequest, EnrollPasswordResponse,
    IssuedSession, MAX_BODY_BYTES, MAX_PASSWORD_BODY_BYTES, Operations,
    PasswordEnrollmentSessionRequest, PasswordEnrollmentStatus, RegisterPasswordRejection,
    RegisterPasswordRequest, RegisterPasswordResponse, RevokeSessionRequest, RevokeSessionResponse,
    SignInChallengeRequest, SignInCompleteRequest, SignInPasswordRejection, SignInPasswordRequest,
    SignInPasswordResponse, SignInResponse, VerifyCredentialRequest,
};
use crate::delivery;
use crate::email_code::{ChallengeHandle, Code, EmailAddress};
use crate::password::{Password, Username};
use crate::persistence::{SchemaState, role_is_privileged, schema_state};

/// Server-side bound on one contract call, body upload included.
pub const DEADLINE: Duration = Duration::from_secs(2);
/// Bound on the email-code operations that commit (sign-in, revoke, email change). They are rare
/// and user-initiated; a commit on a busy disk was observed to take 1.5 s, which must not turn a
/// valid sign-in into "unavailable". `verify-credential`, on every request, keeps [`DEADLINE`].
pub const WRITE_DEADLINE: Duration = Duration::from_secs(5);

fn writes(path: &str) -> bool {
    if path == "/identity/v1/password/enroll/status" {
        return false;
    }
    [
        "/identity/v1/sign-in/",
        "/identity/v1/sessions/",
        "/identity/v1/email-change/",
        "/identity/v1/password/",
    ]
    .iter()
    .any(|prefix| path.starts_with(prefix))
}

/// The OS user of the connecting process, read from the socket (`SO_PEERCRED`/`getpeereid`) by
/// [`crate::server`] and attached to each request as `ConnectInfo<Peer>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Peer {
    pub uid: Option<u32>,
}

#[derive(Clone)]
pub struct IdentityState {
    inner: Arc<Inner>,
}

struct Inner {
    pool: PgPool,
    authority: Authority,
    caller_uid: Option<u32>,
    deadline: Duration,
    write_deadline: Duration,
    /// Label of the transport the delivery worker uses (`local_outbox`, `smtp`), reported by
    /// readiness; `None` without the email-code authority.
    mail_transport: Option<&'static str>,
}

impl IdentityState {
    pub fn new(pool: PgPool, authority: Authority, caller_uid: Option<u32>) -> Self {
        let mut state = Self::with_deadline(pool, authority, caller_uid, DEADLINE);
        if let Some(inner) = Arc::get_mut(&mut state.inner) {
            inner.write_deadline = WRITE_DEADLINE;
        }
        state
    }

    /// One deadline for every call (tests use a short one).
    pub fn with_deadline(
        pool: PgPool,
        authority: Authority,
        caller_uid: Option<u32>,
        deadline: Duration,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                pool,
                authority,
                caller_uid,
                deadline,
                write_deadline: deadline,
                mail_transport: None,
            }),
        }
    }

    /// Reports the delivery worker's transport in readiness.
    pub fn with_mail_transport(mut self, label: &'static str) -> Self {
        if let Some(inner) = Arc::get_mut(&mut self.inner) {
            inner.mail_transport = Some(label);
        }
        self
    }
}

impl IntoResponse for ContractError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::CallerUnauthenticated => StatusCode::UNAUTHORIZED,
            Self::MalformedRequest => StatusCode::BAD_REQUEST,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedVersion | Self::NotFound => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::AuthorityNotConfigured => StatusCode::NOT_IMPLEMENTED,
            Self::DependencyUnavailable | Self::DeadlineExceeded => StatusCode::SERVICE_UNAVAILABLE,
            Self::InvalidEmail | Self::InvalidUsername | Self::InvalidPassword => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::CredentialRejected => StatusCode::FORBIDDEN,
            Self::AddressUnavailable | Self::UsernameTaken | Self::AlreadyEnrolled => {
                StatusCode::CONFLICT
            }
            Self::AccountDisabled => StatusCode::FORBIDDEN,
        };
        (status, Json(self.envelope())).into_response()
    }
}

pub fn router(state: IdentityState) -> Router {
    let contract = Router::new()
        .route("/identity/v1/capabilities", get(capabilities))
        .route("/identity/v1/verify-credential", post(verify_credential))
        .route("/identity/v1/sign-in/challenges", post(sign_in_challenge))
        .route("/identity/v1/sign-in/complete", post(sign_in_complete))
        .route("/identity/v1/sessions/revoke", post(revoke_session))
        .route(
            "/identity/v1/email-change/challenges",
            post(email_change_challenge),
        )
        .route(
            "/identity/v1/email-change/complete",
            post(email_change_complete),
        )
        .route("/identity/v1/password/register", post(register_password))
        .route("/identity/v1/password/sign-in", post(sign_in_password))
        .route(
            "/identity/v1/password/enroll/challenges",
            post(enroll_password_challenge),
        )
        .route("/identity/v1/password/enroll", post(enroll_password))
        .route(
            "/identity/v1/password/enroll/status",
            post(password_enrollment_status),
        )
        .route("/identity/{version}", any(unknown_operation))
        .route("/identity/{version}/{*rest}", any(unknown_operation))
        .method_not_allowed_fallback(method_not_allowed)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate_caller,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), deadline));
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .merge(contract)
        .fallback(not_found)
        .with_state(state)
}

/// Bounds the whole call, so a stalled upload or slow authority cannot hold the caller.
async fn deadline(State(state): State<IdentityState>, request: Request, next: Next) -> Response {
    let limit = if writes(request.uri().path()) {
        state.inner.write_deadline
    } else {
        state.inner.deadline
    };
    match tokio::time::timeout(limit, next.run(request)).await {
        Ok(response) => response,
        Err(_) => {
            tracing::warn!("contract call exceeded its deadline");
            ContractError::DeadlineExceeded.into_response()
        }
    }
}

/// Fails closed: no configured caller, an unknown peer or a different user is refused before
/// the body is read.
async fn authenticate_caller(
    State(state): State<IdentityState>,
    request: Request,
    next: Next,
) -> Response {
    // Only the accept loop inserts this; a client cannot set request extensions.
    let peer = request
        .extensions()
        .get::<ConnectInfo<Peer>>()
        .map(|info| info.0);
    let peer_uid = peer.and_then(|peer| peer.uid);
    match state.inner.caller_uid {
        Some(allowed) if peer_uid == Some(allowed) => next.run(request).await,
        Some(_) => {
            tracing::warn!(
                ?peer_uid,
                "contract call refused: peer is not the configured caller"
            );
            ContractError::CallerUnauthenticated.into_response()
        }
        None => {
            tracing::warn!("contract call refused: no caller is configured");
            ContractError::CallerUnauthenticated.into_response()
        }
    }
}

async fn capabilities(State(state): State<IdentityState>) -> Json<Capabilities> {
    let availability = state.inner.authority.availability();
    let email_code = state.inner.authority.email_code_availability();
    let password = state.inner.authority.password_availability();
    Json(Capabilities {
        contract: CONTRACT.into(),
        authority: availability,
        operations: Operations {
            verify_credential: availability,
            sign_in: email_code,
            revoke_session: availability,
            email_change: email_code,
            register_password: password,
            sign_in_password: password,
            enroll_password: state.inner.authority.enrollment_availability(),
        },
    })
}

/// Reads and decodes a bounded JSON body. The parse error is not logged or returned: it may quote
/// a credential, a code or an address.
async fn read_json<T: serde::de::DeserializeOwned>(body: Body) -> Result<T, ContractError> {
    read_json_with_limit(body, MAX_BODY_BYTES).await
}

async fn read_json_with_limit<T: serde::de::DeserializeOwned>(
    body: Body,
    limit: usize,
) -> Result<T, ContractError> {
    let bytes = match axum::body::to_bytes(body, limit).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let too_large = std::error::Error::source(&error)
                .is_some_and(|source| source.is::<http_body_util::LengthLimitError>());
            return Err(if too_large {
                ContractError::PayloadTooLarge
            } else {
                ContractError::MalformedRequest
            });
        }
    };
    serde_json::from_slice::<T>(&bytes).map_err(|_| ContractError::MalformedRequest)
}

impl From<AuthorityError> for ContractError {
    fn from(error: AuthorityError) -> Self {
        match error {
            AuthorityError::NotConfigured => ContractError::AuthorityNotConfigured,
            AuthorityError::Unavailable => ContractError::DependencyUnavailable,
        }
    }
}

impl From<crate::accounts::StoreError> for ContractError {
    fn from(_: crate::accounts::StoreError) -> Self {
        ContractError::DependencyUnavailable
    }
}

async fn verify_credential(State(state): State<IdentityState>, body: Body) -> Response {
    let request = match read_json::<VerifyCredentialRequest>(body).await {
        Ok(request) => request,
        Err(error) => return error.into_response(),
    };
    match state.inner.authority.verify(&request.credential).await {
        Ok(answer) => Json(answer).into_response(),
        Err(error) => ContractError::from(error).into_response(),
    }
}

fn issued(handle: &ChallengeHandle, expires_at: time::OffsetDateTime) -> Response {
    let body = ChallengeIssued {
        challenge: handle.encode(),
        expires_at,
    };
    (StatusCode::ACCEPTED, Json(body)).into_response()
}

/// Issues a code for any valid address; the delivery worker mails it after commit. Account
/// existence is not read, so the answer cannot disclose it; only the throttles can refuse, and
/// they answer the same for every address.
async fn sign_in_challenge(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Response, ContractError> {
    let request: SignInChallengeRequest = read_json(body).await?;
    let store = state.inner.authority.email_code()?;
    let address = EmailAddress::parse(request.email.expose()).ok_or(ContractError::InvalidEmail)?;
    match store.start_sign_in(&address).await? {
        ChallengeOutcome::Issued { handle, expires_at } => Ok(issued(&handle, expires_at)),
        ChallengeOutcome::RateLimited(throttle) => {
            tracing::warn!(
                throttle = throttle.label(),
                "sign-in code refused: throttle"
            );
            Err(ContractError::RateLimited)
        }
    }
}

async fn sign_in_complete(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Json<SignInResponse>, ContractError> {
    let request: SignInCompleteRequest = read_json(body).await?;
    let store = state.inner.authority.email_code()?;
    let code = Code::parse(request.code.expose()).ok_or(ContractError::MalformedRequest)?;
    let Some(handle) = ChallengeHandle::parse(request.challenge.expose()) else {
        return Ok(Json(SignInResponse::Rejected {
            reason: ChallengeRejection::ChallengeClosed,
        }));
    };
    Ok(Json(match store.complete_sign_in(&handle, &code).await? {
        SignInOutcome::Authenticated(session) => {
            tracing::info!(
                session = %session.session_id,
                subject = %session.account_id,
                "session issued"
            );
            SignInResponse::Authenticated {
                subject: authority::subject(session.account_id),
                authenticated_at: session.authenticated_at,
                session: IssuedSession {
                    id: session.session_id.to_string(),
                    credential: session.credential.encode(),
                    expires_at: session.expires_at,
                },
            }
        }
        SignInOutcome::Rejected(reason) => SignInResponse::Rejected { reason },
    }))
}

async fn revoke_session(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Json<RevokeSessionResponse>, ContractError> {
    let request: RevokeSessionRequest = read_json(body).await?;
    let store = state.inner.authority.store()?;
    let revoked = store.revoke(request.credential.expose()).await?;
    if revoked {
        tracing::info!("session revoked (logout)");
    }
    Ok(Json(RevokeSessionResponse { revoked }))
}

async fn email_change_challenge(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Response, ContractError> {
    let request: EmailChangeChallengeRequest = read_json(body).await?;
    let store = state.inner.authority.email_code()?;
    let address = EmailAddress::parse(request.email.expose()).ok_or(ContractError::InvalidEmail)?;
    match store
        .start_email_change(request.credential.expose(), &address)
        .await?
    {
        EmailChangeStart::Issued(ChallengeOutcome::Issued { handle, expires_at }) => {
            Ok(issued(&handle, expires_at))
        }
        EmailChangeStart::Issued(ChallengeOutcome::RateLimited(throttle)) => {
            tracing::warn!(
                throttle = throttle.label(),
                "email-change code refused: throttle"
            );
            Err(ContractError::RateLimited)
        }
        EmailChangeStart::AddressUnavailable => Err(ContractError::AddressUnavailable),
        EmailChangeStart::CredentialRejected => Err(ContractError::CredentialRejected),
    }
}

async fn email_change_complete(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Json<EmailChangeResponse>, ContractError> {
    let request: EmailChangeCompleteRequest = read_json(body).await?;
    let store = state.inner.authority.email_code()?;
    let code = Code::parse(request.code.expose()).ok_or(ContractError::MalformedRequest)?;
    let Some(handle) = ChallengeHandle::parse(request.challenge.expose()) else {
        return Ok(Json(EmailChangeResponse::Rejected {
            reason: ChallengeRejection::ChallengeClosed,
        }));
    };
    match store
        .complete_email_change(request.credential.expose(), &handle, &code)
        .await?
    {
        EmailChangeOutcome::Changed => {
            tracing::info!("account address changed; other sessions revoked");
            Ok(Json(EmailChangeResponse::Changed))
        }
        EmailChangeOutcome::Rejected(reason) => Ok(Json(EmailChangeResponse::Rejected { reason })),
        EmailChangeOutcome::CredentialRejected => Err(ContractError::CredentialRejected),
    }
}

async fn register_password(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Response, ContractError> {
    let request: RegisterPasswordRequest =
        read_json_with_limit(body, MAX_PASSWORD_BODY_BYTES).await?;
    let store = state.inner.authority.password()?;
    let username =
        Username::parse(request.username.expose()).map_err(|_| ContractError::InvalidUsername)?;
    let password =
        Password::parse(request.password.expose()).map_err(|_| ContractError::InvalidPassword)?;

    match store.register_password(&username, &password).await? {
        Ok(session) => {
            tracing::info!(
                session = %session.session_id,
                subject = %session.account_id,
                "password account registered and session issued"
            );
            Ok(Json(RegisterPasswordResponse::Authenticated {
                subject: authority::subject(session.account_id),
                authenticated_at: session.authenticated_at,
                session: IssuedSession {
                    id: session.session_id.to_string(),
                    credential: session.credential.encode(),
                    expires_at: session.expires_at,
                },
            })
            .into_response())
        }
        Err(PasswordRegistrationError::UsernameTaken) => {
            Ok(Json(RegisterPasswordResponse::Rejected {
                reason: RegisterPasswordRejection::UsernameTaken,
            })
            .into_response())
        }
    }
}

async fn sign_in_password(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Response, ContractError> {
    let request: SignInPasswordRequest =
        read_json_with_limit(body, MAX_PASSWORD_BODY_BYTES).await?;
    let store = state.inner.authority.password()?;
    let username = match Username::parse(request.username.expose()) {
        Ok(u) => u,
        Err(_) => {
            crate::password::verify_sign_in(None, request.password.expose())
                .await
                .map_err(|_| ContractError::DependencyUnavailable)?;
            return Ok(Json(SignInPasswordResponse::Rejected {
                reason: SignInPasswordRejection::InvalidCredentials,
            })
            .into_response());
        }
    };
    match store
        .sign_in_password(&username, request.password.expose())
        .await?
    {
        Ok(session) => {
            tracing::info!(
                session = %session.session_id,
                subject = %session.account_id,
                "password sign-in authenticated and session issued"
            );
            Ok(Json(SignInPasswordResponse::Authenticated {
                subject: authority::subject(session.account_id),
                authenticated_at: session.authenticated_at,
                session: IssuedSession {
                    id: session.session_id.to_string(),
                    credential: session.credential.encode(),
                    expires_at: session.expires_at,
                },
            })
            .into_response())
        }
        Err(PasswordSignInRejection::InvalidCredentials) => {
            Ok(Json(SignInPasswordResponse::Rejected {
                reason: SignInPasswordRejection::InvalidCredentials,
            })
            .into_response())
        }
        Err(PasswordSignInRejection::AccountDisabled) => {
            Ok(Json(SignInPasswordResponse::Rejected {
                reason: SignInPasswordRejection::AccountDisabled,
            })
            .into_response())
        }
        Err(PasswordSignInRejection::RateLimited) => {
            tracing::warn!("password sign-in refused: rate limited");
            Err(ContractError::RateLimited)
        }
    }
}

/// Issues the code that proves control of the session account's own verified address before a
/// username/password is added. The account is the session's; no address or account is accepted.
async fn enroll_password_challenge(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Response, ContractError> {
    let request: PasswordEnrollmentSessionRequest = read_json(body).await?;
    let store = state.inner.authority.enrollment()?;
    match store
        .start_password_enrollment(request.credential.expose())
        .await?
    {
        EnrollmentStart::Issued(ChallengeOutcome::Issued { handle, expires_at }) => {
            Ok(issued(&handle, expires_at))
        }
        EnrollmentStart::Issued(ChallengeOutcome::RateLimited(throttle)) => {
            tracing::warn!(
                throttle = throttle.label(),
                "enrollment code refused: throttle"
            );
            Err(ContractError::RateLimited)
        }
        EnrollmentStart::AlreadyEnrolled => Err(ContractError::AlreadyEnrolled),
        EnrollmentStart::NoVerifiedAddress => Err(ContractError::AddressUnavailable),
        EnrollmentStart::AccountDisabled => Err(ContractError::AccountDisabled),
        EnrollmentStart::CredentialRejected => Err(ContractError::CredentialRejected),
    }
}

async fn password_enrollment_status(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Json<PasswordEnrollmentStatus>, ContractError> {
    let request: PasswordEnrollmentSessionRequest = read_json(body).await?;
    let store = state.inner.authority.enrollment()?;
    match store
        .password_enrollment_status(request.credential.expose())
        .await?
    {
        Ok(enrolled) => Ok(Json(PasswordEnrollmentStatus { enrolled })),
        Err(CredentialRejection::AccountDisabled) => Err(ContractError::AccountDisabled),
        Err(_) => Err(ContractError::CredentialRejected),
    }
}

/// Adds a username/password to the session's account. The request is parsed and the password
/// hashed before the store is called, so no transaction waits on Argon2. Nothing about the
/// account, session or subject appears in the answer.
async fn enroll_password(
    State(state): State<IdentityState>,
    body: Body,
) -> Result<Json<EnrollPasswordResponse>, ContractError> {
    let request: EnrollPasswordRequest =
        read_json_with_limit(body, MAX_PASSWORD_BODY_BYTES).await?;
    let store = state.inner.authority.enrollment()?;
    let code = Code::parse(request.code.expose()).ok_or(ContractError::MalformedRequest)?;
    let Some(handle) = ChallengeHandle::parse(request.challenge.expose()) else {
        return Ok(Json(EnrollPasswordResponse::Rejected {
            reason: EnrollPasswordRejection::ChallengeClosed,
        }));
    };
    let username =
        Username::parse(request.username.expose()).map_err(|_| ContractError::InvalidUsername)?;
    let password =
        Password::parse(request.password.expose()).map_err(|_| ContractError::InvalidPassword)?;
    let hash = crate::password::hash_credential(password)
        .await
        .map_err(|_| ContractError::DependencyUnavailable)?;
    let outcome = store
        .enroll_password(
            request.credential.expose(),
            &handle,
            &code,
            &username,
            &hash,
        )
        .await?;
    Ok(Json(match outcome {
        EnrollmentOutcome::Enrolled => {
            tracing::info!("password credential enrolled on an existing account");
            EnrollPasswordResponse::Enrolled
        }
        EnrollmentOutcome::Rejected(reason) => EnrollPasswordResponse::Rejected {
            reason: match reason {
                EnrollmentRejection::InvalidCode => EnrollPasswordRejection::InvalidCode,
                EnrollmentRejection::ChallengeClosed => EnrollPasswordRejection::ChallengeClosed,
                EnrollmentRejection::UsernameTaken => EnrollPasswordRejection::UsernameTaken,
                EnrollmentRejection::AlreadyEnrolled => EnrollPasswordRejection::AlreadyEnrolled,
                EnrollmentRejection::AccountDisabled => EnrollPasswordRejection::AccountDisabled,
            },
        },
        EnrollmentOutcome::CredentialRejected => return Err(ContractError::CredentialRejected),
    }))
}

/// Any other path under `/identity/`: a known version without such an operation, or an
/// unsupported version. A path that does not even decode is simply not found.
async fn unknown_operation(
    params: Result<Path<Vec<(String, String)>>, PathRejection>,
) -> ContractError {
    match params {
        Ok(Path(params)) if params.first().is_some_and(|(_, version)| version == "v1") => {
            ContractError::NotFound
        }
        Ok(_) => ContractError::UnsupportedVersion,
        Err(_) => ContractError::NotFound,
    }
}

async fn method_not_allowed() -> ContractError {
    ContractError::MethodNotAllowed
}

async fn not_found() -> ContractError {
    ContractError::NotFound
}

async fn live() -> Json<Value> {
    Json(json!({ "status": "live" }))
}

/// Ready only when the database answers with exactly the embedded schema, the connected role
/// cannot change schema or roles, and a caller is configured. The authority and the mail queue
/// are reported, not required: `not_configured` is the honest state, and a mail backlog is an
/// alert for operators, not a reason to stop verifying sessions.
async fn ready(State(state): State<IdentityState>) -> (StatusCode, Json<Value>) {
    let pool = &state.inner.pool;
    let probe = tokio::time::timeout(Duration::from_secs(2), async {
        Ok::<_, sqlx::Error>((schema_state(pool).await?, role_is_privileged(pool).await?))
    })
    .await;
    let (schema, privileged) = match probe {
        Ok(Ok((schema, privileged))) => (Some(schema), Some(privileged)),
        Ok(Err(_)) | Err(_) => (None, None),
    };
    let callers = state.inner.caller_uid.is_some();
    let ready = schema == Some(SchemaState::Current) && privileged == Some(false) && callers;
    let role = match privileged {
        Some(false) => "restricted",
        Some(true) => "privileged",
        None => "unknown",
    };
    let (database, schema_label) = match schema {
        Some(schema) => ("ok", schema.label()),
        None => ("unavailable", "unknown"),
    };
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let transport = match state.inner.authority.email_code_availability() {
        crate::contract::Availability::Available => {
            state.inner.mail_transport.unwrap_or("not_configured")
        }
        crate::contract::Availability::NotConfigured => "not_configured",
    };
    let queue = if transport != "not_configured" && schema == Some(SchemaState::Current) {
        tokio::time::timeout(Duration::from_secs(2), delivery::backlog(pool))
            .await
            .ok()
            .and_then(Result::ok)
    } else {
        None
    };
    (
        status,
        Json(json!({
            "status": if ready { "ready" } else { "not_ready" },
            "checks": {
                "database": database,
                "schema": schema_label,
                "role": role,
                "callers": if callers { "configured" } else { "not_configured" },
            },
            "authority": state.inner.authority.availability(),
            "mail_transport": transport,
            "mail_queue": queue.map(|q| json!({
                "pending": q.pending,
                "oldest_pending_seconds": q.oldest_pending_seconds,
                "failed_last_hour": q.failed_last_hour,
            })),
            // The local outbox reaches nobody outside this machine. With SMTP the code path is
            // production-capable, but D-05 is still Provisional and its gates are open
            // (docs/architecture/auth-authority.md §9), so it is not accepted either.
            "production_authentication": match transport {
                "smtp" => "not_accepted",
                _ => "not_implemented",
            },
        })),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use axum::http::{HeaderValue, Request as HttpRequest, header};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;
    use crate::authority::fixture::{self, Fixture};
    use crate::contract::{ErrorEnvelope, VerifyCredentialResponse};
    use crate::persistence::lazy_pool;

    const CALLER: u32 = 501;

    /// Nothing listens on port 1, so every database call fails like an outage.
    fn outage_pool() -> PgPool {
        lazy_pool("postgres://nobody:unused@127.0.0.1:1/vink_identity").unwrap()
    }

    fn app(authority: Authority, caller: Option<u32>, peer: Option<u32>) -> Router {
        router(IdentityState::with_deadline(
            outage_pool(),
            authority,
            caller,
            Duration::from_millis(300),
        ))
        .layer(axum::Extension(ConnectInfo(Peer { uid: peer })))
    }

    async fn send(router: &Router, request: HttpRequest<Body>) -> (StatusCode, Value) {
        let response = router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    fn verify(body: &str) -> HttpRequest<Body> {
        HttpRequest::post("/identity/v1/verify-credential")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    fn code(body: &Value) -> &str {
        body["error"]["code"].as_str().unwrap_or_default()
    }

    fn fixture(name: &str) -> Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("contracts/rpc/v1/fixtures")
            .join(name);
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
    }

    #[tokio::test]
    async fn without_an_authority_verification_is_not_configured_not_a_failure() {
        let router = app(Authority::NotConfigured, Some(CALLER), Some(CALLER));
        let (status, body) = send(&router, verify(r#"{"credential":"anything"}"#)).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(body, fixture("error.authority_not_configured.json"));

        let get = HttpRequest::get("/identity/v1/capabilities")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(&router, get).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, fixture("capabilities.not_configured.json"));
    }

    #[tokio::test]
    async fn the_caller_is_the_peer_user_and_nothing_else() {
        let forged = |request: HttpRequest<Body>| {
            let mut request = request;
            for (name, value) in [
                ("x-user-id", "11111111-1111-4111-8111-111111111111"),
                ("x-vink-principal", "11111111-1111-4111-8111-111111111111"),
                ("x-forwarded-for", "127.0.0.1"),
                ("authorization", "Bearer fixture-credential-accepted"),
            ] {
                request
                    .headers_mut()
                    .insert(name, HeaderValue::from_static(value));
            }
            request
        };
        let body = r#"{"credential":"fixture-credential-accepted"}"#;
        for (caller, peer) in [
            (None, Some(CALLER)),
            (Some(CALLER), Some(CALLER + 1)),
            (Some(CALLER), None),
        ] {
            let router = app(Authority::Fixture(Fixture::Answers), caller, peer);
            let (status, answer) = send(&router, forged(verify(body))).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{caller:?}/{peer:?}");
            assert_eq!(answer, fixture("error.caller_unauthenticated.json"));
            // Health stays reachable; it reveals no subject or credential.
            let live = HttpRequest::get("/health/live")
                .body(Body::empty())
                .unwrap();
            assert_eq!(send(&router, live).await.0, StatusCode::OK);
        }

        // An allowed caller's forged headers change nothing: the body decides.
        let router = app(
            Authority::Fixture(Fixture::Answers),
            Some(CALLER),
            Some(CALLER),
        );
        let (status, answer) = send(&router, forged(verify(r#"{"credential":"other"}"#))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(answer, fixture("verify.rejected.json"));
    }

    #[tokio::test]
    async fn fixture_answers_have_the_contract_shapes() {
        let router = app(
            Authority::Fixture(Fixture::Answers),
            Some(CALLER),
            Some(CALLER),
        );
        let accepted = format!(r#"{{"credential":"{}"}}"#, fixture::ACCEPTED);
        let (status, body) = send(&router, verify(&accepted)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, fixture("verify.authenticated.json"));
        let decoded: VerifyCredentialResponse = serde_json::from_value(body).unwrap();
        assert!(matches!(
            decoded,
            VerifyCredentialResponse::Authenticated { .. }
        ));

        let router = app(
            Authority::Fixture(Fixture::Unavailable),
            Some(CALLER),
            Some(CALLER),
        );
        let (status, body) = send(&router, verify(&accepted)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body, fixture("error.dependency_unavailable.json"));
    }

    #[tokio::test]
    async fn malformed_oversized_and_forged_subject_requests_are_refused() {
        let router = app(
            Authority::Fixture(Fixture::Answers),
            Some(CALLER),
            Some(CALLER),
        );
        let too_long = format!(r#"{{"credential":"{}"}}"#, "c".repeat(4097));
        for body in [
            "",
            "not json",
            "{}",
            r#"{"credential":""}"#,
            r#"{"credential":7}"#,
            &too_long,
            // A caller cannot choose the subject.
            r#"{"credential":"fixture-credential-accepted","subject":{"authority":"x","id":"admin"}}"#,
        ] {
            let (status, answer) = send(&router, verify(body)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert_eq!(answer, fixture("error.malformed_request.json"));
        }
        let oversized = format!(r#"{{"credential":"{}"}}"#, "c".repeat(9000));
        let (status, answer) = send(&router, verify(&oversized)).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(code(&answer), "payload_too_large");
    }

    #[tokio::test]
    async fn versions_paths_and_methods_are_explicit() {
        let router = app(Authority::NotConfigured, Some(CALLER), Some(CALLER));
        for (path, expected) in [
            ("/identity/v2/verify-credential", "unsupported_version"),
            ("/identity/v0", "unsupported_version"),
            ("/identity/v1/login", "not_found"),
            ("/admin", "not_found"),
        ] {
            let request = HttpRequest::post(path).body(Body::empty()).unwrap();
            let (status, body) = send(&router, request).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(code(&body), expected, "{path}");
        }
        let get = HttpRequest::get("/identity/v1/verify-credential")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(&router, get).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(code(&body), "method_not_allowed");

        // An unknown version is not disclosed to a process that is not the caller.
        let stranger = app(Authority::NotConfigured, Some(CALLER), Some(CALLER + 1));
        let request = HttpRequest::post("/identity/v2/x")
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(&stranger, request).await.0, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn a_stalled_upload_hits_the_deadline() {
        let router = app(
            Authority::Fixture(Fixture::Answers),
            Some(CALLER),
            Some(CALLER),
        );
        // One chunk, then the body never ends while `_sender` is alive.
        let (_sender, mut receiver) =
            tokio::sync::mpsc::channel::<Result<axum::body::Bytes, std::io::Error>>(1);
        _sender
            .try_send(Ok(axum::body::Bytes::from_static(b"{\"credential\":")))
            .unwrap();
        let stream = futures_util::stream::poll_fn(move |cx| receiver.poll_recv(cx));
        let request = HttpRequest::post("/identity/v1/verify-credential")
            .body(Body::from_stream(stream))
            .unwrap();
        let started = std::time::Instant::now();
        let (status, answer) = send(&router, request).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(answer, fixture("error.deadline_exceeded.json"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn readiness_is_honest_about_database_callers_and_authority() {
        let router = app(Authority::NotConfigured, None, Some(CALLER));
        let request = HttpRequest::get("/health/ready")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(&router, request).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["checks"]["database"], "unavailable");
        assert_eq!(body["checks"]["role"], "unknown");
        assert_eq!(body["checks"]["callers"], "not_configured");
        assert_eq!(body["authority"], "not_configured");
        assert_eq!(body["production_authentication"], "not_implemented");
    }

    #[test]
    fn errors_match_their_fixtures_and_never_echo_input() {
        for error in [
            ContractError::CallerUnauthenticated,
            ContractError::MalformedRequest,
            ContractError::PayloadTooLarge,
            ContractError::UnsupportedVersion,
            ContractError::NotFound,
            ContractError::MethodNotAllowed,
            ContractError::AuthorityNotConfigured,
            ContractError::DependencyUnavailable,
            ContractError::DeadlineExceeded,
            ContractError::InvalidEmail,
            ContractError::RateLimited,
            ContractError::CredentialRejected,
            ContractError::AddressUnavailable,
            ContractError::InvalidUsername,
            ContractError::InvalidPassword,
            ContractError::UsernameTaken,
            ContractError::AlreadyEnrolled,
            ContractError::AccountDisabled,
        ] {
            let name = format!("error.{}.json", error.code());
            let envelope: ErrorEnvelope = serde_json::from_value(fixture(&name)).unwrap();
            assert_eq!(envelope, error.envelope(), "{name}");
        }
        let example: VerifyCredentialRequest =
            serde_json::from_value(fixture("verify.request.json")).unwrap();
        assert!(!example.credential.expose().is_empty());
        let request: VerifyCredentialRequest =
            serde_json::from_str(r#"{"credential":"secret-marker"}"#).unwrap();
        assert!(!format!("{request:?}").contains("secret-marker"));
    }

    #[test]
    fn only_committing_email_code_operations_get_the_longer_deadline() {
        for path in [
            "/identity/v1/sign-in/challenges",
            "/identity/v1/sign-in/complete",
            "/identity/v1/sessions/revoke",
            "/identity/v1/email-change/challenges",
            "/identity/v1/email-change/complete",
        ] {
            assert!(writes(path), "{path}");
        }
        for path in [
            "/identity/v1/verify-credential",
            "/identity/v1/capabilities",
            "/identity/v2/sign-in/complete",
            "/health/ready",
        ] {
            assert!(!writes(path), "{path}");
        }
        assert!(WRITE_DEADLINE > DEADLINE);
    }

    fn post(path: &str, body: &str) -> HttpRequest<Body> {
        HttpRequest::post(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    /// The email-code authority over a database that never answers: everything decided before
    /// storage is observable without PostgreSQL (the real database is `tests/postgres.rs`).
    fn email_code_app() -> (Router, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("vink-outbox-http-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let store = crate::accounts::AccountStore::new(
            outage_pool(),
            crate::email_code::Policy::default(),
            std::sync::Arc::default(),
        );
        (
            app(Authority::new_email_code(store), Some(CALLER), Some(CALLER)),
            dir,
        )
    }

    #[tokio::test]
    async fn email_code_operations_are_closed_without_that_authority() {
        for authority in [
            Authority::NotConfigured,
            Authority::Fixture(Fixture::Answers),
        ] {
            let router = app(authority, Some(CALLER), Some(CALLER));
            for (path, body) in [
                (
                    "/identity/v1/sign-in/challenges",
                    r#"{"email":"a@vi-du.vn"}"#,
                ),
                (
                    "/identity/v1/sign-in/complete",
                    r#"{"challenge":"x","code":"123456"}"#,
                ),
                ("/identity/v1/sessions/revoke", r#"{"credential":"x"}"#),
                (
                    "/identity/v1/email-change/challenges",
                    r#"{"credential":"x","email":"a@vi-du.vn"}"#,
                ),
                (
                    "/identity/v1/email-change/complete",
                    r#"{"credential":"x","challenge":"x","code":"123456"}"#,
                ),
                (
                    "/identity/v1/password/register",
                    r#"{"username":"test_user","password":"valid_pass_123"}"#,
                ),
                (
                    "/identity/v1/password/sign-in",
                    r#"{"username":"test_user","password":"valid_pass_123"}"#,
                ),
                (
                    "/identity/v1/password/enroll/challenges",
                    r#"{"credential":"x"}"#,
                ),
                (
                    "/identity/v1/password/enroll",
                    r#"{"credential":"x","challenge":"x","code":"123456","username":"test_user","password":"valid_pass_123"}"#,
                ),
            ] {
                let (status, answer) = send(&router, post(path, body)).await;
                assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{path}");
                assert_eq!(answer, fixture("error.authority_not_configured.json"));
            }
        }
    }

    #[tokio::test]
    async fn email_code_requests_are_validated_before_storage_and_never_choose_a_subject() {
        let (router, dir) = email_code_app();
        let (status, body) = send(
            &router,
            HttpRequest::get("/identity/v1/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, fixture("capabilities.email_code.json"));

        let cases = [
            // Unknown fields (a forged subject, account or session) are refused.
            (
                "/identity/v1/sign-in/challenges",
                r#"{"email":"a@vi-du.vn","account":"018f3c2e-0000-7000-8000-00000000a001"}"#,
                StatusCode::BAD_REQUEST,
                "malformed_request",
            ),
            (
                "/identity/v1/sign-in/complete",
                r#"{"challenge":"x","code":"123456","subject":{"authority":"vink-identity","id":"admin"}}"#,
                StatusCode::BAD_REQUEST,
                "malformed_request",
            ),
            (
                "/identity/v1/sign-in/challenges",
                r#"{"email":"khong-phai-email"}"#,
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_email",
            ),
            (
                "/identity/v1/sign-in/complete",
                r#"{"challenge":"x","code":"12345a"}"#,
                StatusCode::BAD_REQUEST,
                "malformed_request",
            ),
            (
                "/identity/v1/email-change/challenges",
                r#"{"credential":"x","email":"@vi-du.vn"}"#,
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_email",
            ),
        ];
        for (path, body, status, code_name) in cases {
            let (got, answer) = send(&router, post(path, body)).await;
            assert_eq!((got, code(&answer)), (status, code_name), "{path} {body}");
        }
        // A valid address reaches storage, which is down: nothing is decided (the pool may give
        // up before or after the test deadline; both answers are 503 "not decided").
        let (status, answer) = send(
            &router,
            post(
                "/identity/v1/sign-in/challenges",
                r#"{"email":"a@vi-du.vn"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            ["dependency_unavailable", "deadline_exceeded"].contains(&code(&answer)),
            "{answer}"
        );

        // Not a handle / not a credential: decided without storage.
        let (status, answer) = send(
            &router,
            post(
                "/identity/v1/sign-in/complete",
                r#"{"challenge":"vink_c1.garbled","code":"123456"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(answer["outcome"], "rejected");
        assert_eq!(answer["reason"], "challenge_closed");
        let (status, answer) = send(
            &router,
            post(
                "/identity/v1/sessions/revoke",
                r#"{"credential":"not-ours"}"#,
            ),
        )
        .await;
        assert_eq!(
            (status, answer["revoked"].clone()),
            (StatusCode::OK, Value::Bool(false))
        );
        let (status, answer) = send(&router, verify(r#"{"credential":"not-ours"}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            answer,
            serde_json::json!({"outcome":"rejected","reason":"unknown"})
        );
        // A well-formed credential needs storage: an outage is never "rejected".
        let credential = crate::email_code::SessionCredential::generate()
            .unwrap()
            .encode();
        let (status, answer) = send(
            &router,
            verify(&format!(r#"{{"credential":"{credential}"}}"#)),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            ["dependency_unavailable", "deadline_exceeded"].contains(&code(&answer)),
            "{answer}"
        );
        // Nothing reached the outbox.
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn email_code_shapes_match_their_fixtures() {
        use crate::contract::*;
        use time::macros::datetime;
        let subject = Subject {
            authority: "vink-identity".into(),
            id: "018f3c2e-0000-7000-8000-00000000a001".into(),
        };
        let handle = fixture("challenge.issued.json")["challenge"]
            .as_str()
            .unwrap()
            .to_string();
        let credential = fixture("session.revoke.request.json")["credential"]
            .as_str()
            .unwrap()
            .to_string();
        let shapes: [(&str, Value); 9] = [
            (
                "challenge.issued.json",
                serde_json::to_value(ChallengeIssued {
                    challenge: handle,
                    expires_at: datetime!(2026-11-07 10:10:00 UTC),
                })
                .unwrap(),
            ),
            (
                "sign_in.authenticated.json",
                serde_json::to_value(SignInResponse::Authenticated {
                    subject: subject.clone(),
                    authenticated_at: datetime!(2026-11-07 10:01:00 UTC),
                    session: IssuedSession {
                        id: "018f3c2e-0000-7000-8000-00000000b001".into(),
                        credential,
                        expires_at: datetime!(2026-12-07 10:01:00 UTC),
                    },
                })
                .unwrap(),
            ),
            (
                "sign_in.rejected.json",
                serde_json::to_value(SignInResponse::Rejected {
                    reason: ChallengeRejection::InvalidCode,
                })
                .unwrap(),
            ),
            (
                "verify.authenticated.session.json",
                serde_json::to_value(VerifyCredentialResponse::Authenticated {
                    subject,
                    authenticated_at: datetime!(2026-11-07 10:01:00 UTC),
                    session: Some(SessionFacts {
                        id: "018f3c2e-0000-7000-8000-00000000b001".into(),
                        expires_at: datetime!(2026-12-07 10:01:00 UTC),
                    }),
                })
                .unwrap(),
            ),
            (
                "verify.rejected.revoked.json",
                serde_json::to_value(VerifyCredentialResponse::Rejected {
                    reason: Some(CredentialRejection::Revoked),
                })
                .unwrap(),
            ),
            (
                "verify.rejected.json",
                serde_json::to_value(VerifyCredentialResponse::Rejected { reason: None }).unwrap(),
            ),
            (
                "session.revoked.json",
                serde_json::to_value(RevokeSessionResponse { revoked: true }).unwrap(),
            ),
            (
                "email_change.changed.json",
                serde_json::to_value(EmailChangeResponse::Changed).unwrap(),
            ),
            (
                "email_change.rejected.json",
                serde_json::to_value(EmailChangeResponse::Rejected {
                    reason: ChallengeRejection::AddressUnavailable,
                })
                .unwrap(),
            ),
        ];
        for (name, value) in shapes {
            assert_eq!(value, fixture(name), "{name}");
        }
        // Request fixtures decode; the issued credential never reaches Debug.
        let _: SignInChallengeRequest =
            serde_json::from_value(fixture("sign_in.challenge.request.json")).unwrap();
        let _: SignInCompleteRequest =
            serde_json::from_value(fixture("sign_in.complete.request.json")).unwrap();
        let _: RevokeSessionRequest =
            serde_json::from_value(fixture("session.revoke.request.json")).unwrap();
        let _: EmailChangeChallengeRequest =
            serde_json::from_value(fixture("email_change.challenge.request.json")).unwrap();
        let change: EmailChangeCompleteRequest =
            serde_json::from_value(fixture("email_change.complete.request.json")).unwrap();
        assert!(!format!("{change:?}").contains("FIXTURE"));
        let issued: SignInResponse =
            serde_json::from_value(fixture("sign_in.authenticated.json")).unwrap();
        assert!(!format!("{issued:?}").contains("FIXTURE"));
    }

    #[test]
    fn password_shapes_match_their_fixtures() {
        use crate::contract::*;
        use time::macros::datetime;
        let subject = Subject {
            authority: "vink-identity".into(),
            id: "018f3c2e-0000-7000-8000-00000000a001".into(),
        };
        let credential = fixture("password.sign_in.authenticated.json")["session"]["credential"]
            .as_str()
            .unwrap()
            .to_string();

        let shapes: [(&str, Value); 5] = [
            (
                "password.register.authenticated.json",
                serde_json::to_value(RegisterPasswordResponse::Authenticated {
                    subject: subject.clone(),
                    authenticated_at: datetime!(2026-11-13 10:00:00 UTC),
                    session: IssuedSession {
                        id: "018f3c2e-0000-7000-8000-00000000b001".into(),
                        credential: credential.clone(),
                        expires_at: datetime!(2026-12-13 10:00:00 UTC),
                    },
                })
                .unwrap(),
            ),
            (
                "password.register.rejected.username_taken.json",
                serde_json::to_value(RegisterPasswordResponse::Rejected {
                    reason: RegisterPasswordRejection::UsernameTaken,
                })
                .unwrap(),
            ),
            (
                "password.sign_in.authenticated.json",
                serde_json::to_value(SignInPasswordResponse::Authenticated {
                    subject,
                    authenticated_at: datetime!(2026-11-13 10:00:00 UTC),
                    session: IssuedSession {
                        id: "018f3c2e-0000-7000-8000-00000000b001".into(),
                        credential,
                        expires_at: datetime!(2026-12-13 10:00:00 UTC),
                    },
                })
                .unwrap(),
            ),
            (
                "password.sign_in.rejected.invalid_credentials.json",
                serde_json::to_value(SignInPasswordResponse::Rejected {
                    reason: SignInPasswordRejection::InvalidCredentials,
                })
                .unwrap(),
            ),
            (
                "capabilities.password.json",
                serde_json::to_value(Capabilities {
                    contract: CONTRACT.into(),
                    authority: Availability::Available,
                    operations: Operations {
                        verify_credential: Availability::Available,
                        sign_in: Availability::NotConfigured,
                        revoke_session: Availability::Available,
                        email_change: Availability::NotConfigured,
                        register_password: Availability::Available,
                        sign_in_password: Availability::Available,
                        enroll_password: Availability::NotConfigured,
                    },
                })
                .unwrap(),
            ),
        ];
        for (name, value) in shapes {
            assert_eq!(value, fixture(name), "{name}");
        }

        let reg_req: RegisterPasswordRequest =
            serde_json::from_value(fixture("password.register.request.json")).unwrap();
        assert_eq!(reg_req.username.expose(), "author_01");
        assert!(!format!("{reg_req:?}").contains("valid_password_123"));

        let sign_in_req: SignInPasswordRequest =
            serde_json::from_value(fixture("password.sign_in.request.json")).unwrap();
        assert_eq!(sign_in_req.username.expose(), "author_01");
        assert!(!format!("{sign_in_req:?}").contains("valid_password_123"));
    }

    #[test]
    fn enrollment_shapes_match_their_fixtures() {
        use crate::contract::*;
        let shapes = [
            (
                "password.enroll.enrolled.json",
                serde_json::to_value(EnrollPasswordResponse::Enrolled).unwrap(),
            ),
            (
                "password.enroll.status.enrolled.json",
                serde_json::to_value(PasswordEnrollmentStatus { enrolled: true }).unwrap(),
            ),
            (
                "password.enroll.status.unenrolled.json",
                serde_json::to_value(PasswordEnrollmentStatus { enrolled: false }).unwrap(),
            ),
            (
                "password.enroll.rejected.username_taken.json",
                serde_json::to_value(EnrollPasswordResponse::Rejected {
                    reason: EnrollPasswordRejection::UsernameTaken,
                })
                .unwrap(),
            ),
            (
                "password.enroll.rejected.invalid_code.json",
                serde_json::to_value(EnrollPasswordResponse::Rejected {
                    reason: EnrollPasswordRejection::InvalidCode,
                })
                .unwrap(),
            ),
            (
                "capabilities.native.json",
                serde_json::to_value(Capabilities {
                    contract: CONTRACT.into(),
                    authority: Availability::Available,
                    operations: Operations {
                        verify_credential: Availability::Available,
                        sign_in: Availability::Available,
                        revoke_session: Availability::Available,
                        email_change: Availability::Available,
                        register_password: Availability::Available,
                        sign_in_password: Availability::Available,
                        enroll_password: Availability::Available,
                    },
                })
                .unwrap(),
            ),
        ];
        for (name, value) in shapes {
            assert_eq!(value, fixture(name), "{name}");
        }

        let challenge: PasswordEnrollmentSessionRequest =
            serde_json::from_value(fixture("password.enroll.challenge.request.json")).unwrap();
        let request: EnrollPasswordRequest =
            serde_json::from_value(fixture("password.enroll.request.json")).unwrap();
        assert_eq!(request.username.expose(), "author_01");
        for debug in [format!("{challenge:?}"), format!("{request:?}")] {
            assert!(!debug.contains("FIXTURE"), "{debug}");
            assert!(!debug.contains("123456"), "{debug}");
            assert!(!debug.contains("mật khẩu"), "{debug}");
            assert!(!debug.contains("author_01"), "{debug}");
        }
        // A caller cannot name the account, the subject or a verified flag.
        for field in ["account_id", "subject", "verified", "email"] {
            let mut raw = fixture("password.enroll.request.json");
            raw[field] = serde_json::Value::from("x");
            assert!(
                serde_json::from_value::<EnrollPasswordRequest>(raw).is_err(),
                "{field}"
            );
            let mut raw = fixture("password.enroll.challenge.request.json");
            raw[field] = serde_json::Value::from("x");
            assert!(
                serde_json::from_value::<PasswordEnrollmentSessionRequest>(raw).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn password_fields_accept_the_whole_o12_input_bound() {
        use crate::contract::*;
        // 4096 bytes is what Password::parse accepts before NFKC; the transport must not refuse
        // a field the policy still decides on, and must refuse one byte more.
        let at_bound = serde_json::json!({"username": "author_01", "password": "a".repeat(4096)});
        assert!(serde_json::from_value::<RegisterPasswordRequest>(at_bound.clone()).is_ok());
        assert!(serde_json::from_value::<SignInPasswordRequest>(at_bound).is_ok());
        let over = serde_json::json!({"username": "author_01", "password": "a".repeat(4097)});
        assert!(serde_json::from_value::<RegisterPasswordRequest>(over.clone()).is_err());
        assert!(serde_json::from_value::<SignInPasswordRequest>(over).is_err());
        // Vietnamese letters: 128 precomposed ones are 384 bytes, far above the old 256.
        let vietnamese = serde_json::json!({"username": "author_01", "password": "ệ".repeat(128)});
        assert!(serde_json::from_value::<RegisterPasswordRequest>(vietnamese).is_ok());
    }
}
