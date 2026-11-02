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

use crate::authority::{Authority, AuthorityError};
use crate::contract::{
    CONTRACT, Capabilities, ContractError, MAX_BODY_BYTES, Operations, VerifyCredentialRequest,
};
use crate::persistence::{SchemaState, role_is_privileged, schema_state};

/// Server-side bound on one contract call, body upload included.
pub const DEADLINE: Duration = Duration::from_secs(2);

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
}

impl IdentityState {
    pub fn new(pool: PgPool, authority: Authority, caller_uid: Option<u32>) -> Self {
        Self::with_deadline(pool, authority, caller_uid, DEADLINE)
    }

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
            }),
        }
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
        };
        (status, Json(self.envelope())).into_response()
    }
}

pub fn router(state: IdentityState) -> Router {
    let contract = Router::new()
        .route("/identity/v1/capabilities", get(capabilities))
        .route("/identity/v1/verify-credential", post(verify_credential))
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
    match tokio::time::timeout(state.inner.deadline, next.run(request)).await {
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
    Json(Capabilities {
        contract: CONTRACT.into(),
        authority: availability,
        operations: Operations {
            verify_credential: availability,
        },
    })
}

async fn verify_credential(State(state): State<IdentityState>, body: Body) -> Response {
    let bytes = match axum::body::to_bytes(body, MAX_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let too_large = std::error::Error::source(&error)
                .is_some_and(|source| source.is::<http_body_util::LengthLimitError>());
            return if too_large {
                ContractError::PayloadTooLarge
            } else {
                ContractError::MalformedRequest
            }
            .into_response();
        }
    };
    // The parse error is not logged or returned: it may quote the credential.
    let Ok(request) = serde_json::from_slice::<VerifyCredentialRequest>(&bytes) else {
        return ContractError::MalformedRequest.into_response();
    };
    match state.inner.authority.verify(&request.credential).await {
        Ok(answer) => Json(answer).into_response(),
        Err(AuthorityError::NotConfigured) => ContractError::AuthorityNotConfigured.into_response(),
        Err(AuthorityError::Unavailable) => ContractError::DependencyUnavailable.into_response(),
    }
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
/// cannot change schema or roles, and a caller is configured. The authority is reported, not
/// required: `not_configured` is the honest state.
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
    let (database, schema) = match schema {
        Some(schema) => ("ok", schema.label()),
        None => ("unavailable", "unknown"),
    };
    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({
            "status": if ready { "ready" } else { "not_ready" },
            "checks": {
                "database": database,
                "schema": schema,
                "role": role,
                "callers": if callers { "configured" } else { "not_configured" },
            },
            "authority": state.inner.authority.availability(),
            "production_authentication": "not_implemented",
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
}
