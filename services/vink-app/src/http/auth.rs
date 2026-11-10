//! Sign-in API (D-05/S-03 spike): the browser-facing side of the native email-code authority.
//!
//! The browser never reaches Identity. These routes forward to it through the App's client and
//! keep every secret out of page scripts: the challenge handle and the session credential travel
//! only in `__Host-` cookies (`Secure`, `HttpOnly`, `Path=/`); response bodies never contain them.
//! Unsafe methods inherit the API's same-origin check, and all answers are `no-store`.
//!
//! Authentication (Identity) is kept apart from authorization (the App): completing a challenge
//! yields a session for a *subject*; the App then reads its own binding and suspension list.
//! A new subject gets no principal until `POST /auth/principal` provisions one with a pen name.
//!
//! Abuse protection: asking for and answering codes are counted per client network
//! ([`crate::domain::abuse`]) in the App database before Identity is called; a refused or
//! uncountable request never reaches Identity. Identity adds its own per-address, per-account and
//! global ceilings. Production activation is explicit (`VINK_AUTH_SIGN_IN`).

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, FromRequest, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tower_http::set_header::SetResponseHeaderLayer;

use super::{Actor, ApiError, AppState};
use crate::application::Vink;
use crate::application::account::AccountError;
use crate::application::ports::{RateLimitStore, StoreError};
use crate::domain::abuse::{
    Admission, Network, RateLimit, SIGN_IN_CHALLENGE, SIGN_IN_COMPLETE, client_address,
};
use crate::domain::principal::{Standing, SubjectKey};
use crate::identity::client::{CallError, ChallengeRefusal, SignIn, StartError};
use crate::identity::{IdentityClient, Resolution, SESSION_COOKIE, bind, presented_credential};

/// Holds the challenge handle between "send me a code" and "here is the code". `Strict`: the
/// completion is a same-origin request, so no cross-site navigation ever carries it.
pub const CHALLENGE_COOKIE: &str = "__Host-vink_sign_in";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/sign-in/challenges", post(start_sign_in))
        .route("/auth/sign-in/complete", post(complete_sign_in))
        .route("/auth/session", get(session_state).delete(sign_out))
        .route("/auth/session/account", get(current_account))
        .route("/auth/principal", post(provision_principal))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartSignInRequest {
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChallengeSent {
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompleteSignInRequest {
    pub code: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisionPrincipalRequest {
    pub pen_name: String,
}

/// What the server decided about the presented session. Never contains a credential.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionState {
    pub state: SessionStateKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStateKind {
    Anonymous,
    /// Signed in, no Vink principal yet: provision one with a pen name.
    NeedsPrincipal,
    Suspended,
    Active,
}

#[derive(Debug, Serialize)]
struct CurrentAccount {
    principal_id: String,
    display_name: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvisionedPrincipal {
    pub principal_id: String,
    /// `false`: the subject already had this principal (a retry); the pen name was not changed.
    pub created: bool,
}

pub type AdmitFuture<'a> = Pin<Box<dyn Future<Output = Result<Admission, StoreError>> + Send + 'a>>;

/// Counts one sign-in request of a network and decides. Shared by every App process when backed
/// by the App database ([`Vink`]); replaceable in tests.
pub trait SignInLimiter: Send + Sync + 'static {
    fn admit<'a>(&'a self, limit: &'a RateLimit, network: Network) -> AdmitFuture<'a>;
}

impl<S> SignInLimiter for Vink<S>
where
    S: RateLimitStore + 'static,
{
    fn admit<'a>(&'a self, limit: &'a RateLimit, network: Network) -> AdmitFuture<'a> {
        Box::pin(async move { Vink::admit(self, limit, &network).await })
    }
}

/// The sign-in client, only while new sessions may be started.
fn sign_in_client(state: &AppState) -> Result<&IdentityClient, ApiError> {
    state
        .sign_in
        .as_ref()
        .filter(|_| state.sign_in_enabled)
        .ok_or(ApiError::AuthorityNotConfigured)
}

/// The client network of a request: the TCP peer, or behind a trusted proxy the right-most
/// untrusted `X-Forwarded-For` hop.
fn network(state: &AppState, parts: &Parts) -> Option<Network> {
    let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>()?.0.ip();
    let forwarded: Vec<&str> = parts
        .headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect();
    Some(Network::of(client_address(
        peer,
        &forwarded,
        &state.trusted_proxies,
    )))
}

/// Counts this request against `limit`. Fails closed: an unknown peer or an unreachable counter
/// is `503`, never an unlimited request.
async fn admit(state: &AppState, parts: &Parts, limit: &RateLimit) -> Result<(), ApiError> {
    let Some(network) = network(state, parts) else {
        tracing::error!("sign-in request without a peer address; refused");
        return Err(ApiError::DependencyUnavailable);
    };
    match state.limiter.admit(limit, network).await {
        Ok(Admission::Admitted) => Ok(()),
        Ok(Admission::Refused {
            retry_after_seconds,
        }) => {
            tracing::warn!(
                bucket = limit.bucket,
                "sign-in request refused: network ceiling"
            );
            Err(ApiError::RateLimited {
                retry_after_seconds: Some(retry_after_seconds),
            })
        }
        Err(_) => {
            tracing::warn!(
                bucket = limit.bucket,
                "sign-in rate limit cannot be counted"
            );
            Err(ApiError::DependencyUnavailable)
        }
    }
}

async fn json<T: serde::de::DeserializeOwned>(
    parts: Parts,
    body: axum::body::Body,
) -> Result<T, ApiError> {
    let request = axum::extract::Request::from_parts(parts, body);
    json_body(<Json<T> as FromRequest<()>>::from_request(request, &()).await)
}

fn cookie(name: &str, value: &str, same_site: &str, max_age: i64) -> Option<HeaderValue> {
    HeaderValue::from_str(&format!(
        "{name}={value}; Path=/; Secure; HttpOnly; SameSite={same_site}; Max-Age={max_age}"
    ))
    .ok()
}

fn clear(name: &str) -> HeaderValue {
    cookie(name, "", "Lax", 0).expect("static cookie")
}

fn seconds_until(at: OffsetDateTime) -> i64 {
    (at - OffsetDateTime::now_utc()).whole_seconds().max(0)
}

/// Exactly one cookie named `name`; zero or several is none.
fn single_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut found = None;
    for value in headers.get_all(header::COOKIE) {
        let Ok(text) = value.to_str() else { continue };
        for pair in text.split(';') {
            if let Some((key, value)) = pair.trim().split_once('=')
                && key == name
            {
                if found.is_some() {
                    return None;
                }
                found = Some(value);
            }
        }
    }
    found.filter(|value| !value.is_empty())
}

fn json_body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    body.map(|Json(body)| body)
        .map_err(|rejection| ApiError::Validation {
            field: "body",
            message: rejection.body_text(),
        })
}

fn unavailable(error: CallError) -> ApiError {
    match error {
        CallError::NoAuthority => ApiError::AuthorityNotConfigured,
        CallError::Unavailable(failure) => {
            tracing::warn!(
                reason = failure.label(),
                "identity could not answer sign-in"
            );
            ApiError::DependencyUnavailable
        }
    }
}

/// `POST /auth/sign-in/challenges`: Identity issues a code (mailed by its worker); the handle
/// goes into a cookie.
async fn start_sign_in(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> Result<Response, ApiError> {
    let client = sign_in_client(&state)?;
    let (parts, body) = request.into_parts();
    admit(&state, &parts, &SIGN_IN_CHALLENGE).await?;
    let request: StartSignInRequest = json(parts, body).await?;
    let challenge = match client.start_sign_in(request.email.trim()).await {
        Ok(challenge) => challenge,
        Err(StartError::InvalidEmail) => {
            return Err(ApiError::Validation {
                field: "email",
                message: "email must be a mailbox address".into(),
            });
        }
        Err(StartError::RateLimited) => {
            return Err(ApiError::RateLimited {
                retry_after_seconds: None,
            });
        }
        Err(StartError::Call(error)) => return Err(unavailable(error)),
    };
    let set = cookie(
        CHALLENGE_COOKIE,
        &challenge.handle,
        "Strict",
        seconds_until(challenge.expires_at),
    )
    .ok_or(ApiError::DependencyUnavailable)?;
    Ok((
        StatusCode::ACCEPTED,
        [(header::SET_COOKIE, set)],
        Json(ChallengeSent {
            expires_at: challenge.expires_at,
        }),
    )
        .into_response())
}

/// `POST /auth/sign-in/complete`: answers the challenge in the cookie. On success the session
/// credential is set as a cookie and the App reports whether a principal is bound. A suspended
/// principal or an undecidable directory revokes the fresh session and sets no cookie. A session
/// the browser presented before is revoked once the new one is delivered, so signing in again
/// never leaves an orphaned session behind.
async fn complete_sign_in(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> Result<Response, ApiError> {
    let client = sign_in_client(&state)?;
    let (parts, body) = request.into_parts();
    admit(&state, &parts, &SIGN_IN_COMPLETE).await?;
    let previous = single_cookie(&parts.headers, SESSION_COOKIE).map(str::to_string);
    let headers = parts.headers.clone();
    let request: CompleteSignInRequest = json(parts, body).await?;
    let code: String = request
        .code
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ApiError::Validation {
            field: "code",
            message: "code must be the six digits from the email".into(),
        });
    }
    let Some(handle) = single_cookie(&headers, CHALLENGE_COOKIE) else {
        return Ok(closed());
    };
    let session = match client.complete_sign_in(handle, &code).await {
        Ok(SignIn::Authenticated(session)) => session,
        Ok(SignIn::Refused(ChallengeRefusal::InvalidCode)) => return Err(ApiError::CodeRejected),
        Ok(SignIn::Refused(ChallengeRefusal::ChallengeClosed)) => return Ok(closed()),
        Ok(SignIn::Refused(ChallengeRefusal::AccountDisabled)) => {
            return Ok(with_cleared(ApiError::AccountDisabled.into_response()));
        }
        Err(error) => return Err(unavailable(error)),
    };
    // From here a valid session exists in Identity, held only in this function.
    let revoke = || async {
        if client.revoke(&session.credential).await.is_err() {
            // The credential never left the server; it cannot be presented by anyone.
            tracing::warn!("could not revoke an undelivered session; it expires on its own");
        }
    };
    let Some(subject) = SubjectKey::new(&session.authority, &session.subject) else {
        revoke().await;
        return Err(ApiError::DependencyUnavailable);
    };
    let lookup = state.directory.lookup(&subject).await;
    let (kind, principal) = match bind(subject, lookup) {
        Resolution::Principal(id) => (SessionStateKind::Active, Some(id.to_string())),
        Resolution::Unbound(_) => (SessionStateKind::NeedsPrincipal, None),
        Resolution::Suspended(_) => {
            revoke().await;
            return Ok(with_cleared(ApiError::PrincipalSuspended.into_response()));
        }
        Resolution::Anonymous | Resolution::Unavailable => {
            revoke().await;
            return Err(ApiError::DependencyUnavailable);
        }
    };
    let Some(set) = cookie(
        SESSION_COOKIE,
        &session.credential,
        "Lax",
        seconds_until(session.expires_at),
    ) else {
        revoke().await;
        return Err(ApiError::DependencyUnavailable);
    };
    let mut response = Json(SessionState {
        state: kind,
        principal_id: principal,
    })
    .into_response();
    let headers = response.headers_mut();
    headers.append(header::SET_COOKIE, set);
    headers.append(header::SET_COOKIE, clear(CHALLENGE_COOKIE));
    if let Some(previous) = previous.filter(|previous| *previous != session.credential)
        && client.revoke(&previous).await.is_err()
    {
        // The browser no longer holds it; it expires on its own.
        tracing::warn!("could not revoke the session replaced by a new sign-in");
    }
    Ok(response)
}

fn closed() -> Response {
    with_cleared(ApiError::ChallengeClosed.into_response())
}

fn with_cleared(mut response: Response) -> Response {
    response
        .headers_mut()
        .append(header::SET_COOKIE, clear(CHALLENGE_COOKIE));
    response
}

async fn resolve(state: &AppState, parts: &Parts) -> Resolution {
    let peer = parts
        .extensions
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|info| info.0);
    state.identity.resolve(parts, peer).await
}

/// `GET /auth/session`: what the server decides about the presented credential, right now.
async fn session_state(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> Result<Json<SessionState>, ApiError> {
    let (parts, _) = request.into_parts();
    let (state, principal) = match resolve(&state, &parts).await {
        Resolution::Principal(id) => (SessionStateKind::Active, Some(id)),
        Resolution::Anonymous => (SessionStateKind::Anonymous, None),
        Resolution::Unbound(_) => (SessionStateKind::NeedsPrincipal, None),
        Resolution::Suspended(id) => (SessionStateKind::Suspended, Some(id)),
        Resolution::Unavailable => return Err(ApiError::DependencyUnavailable),
    };
    Ok(Json(SessionState {
        state,
        principal_id: principal.map(|id| id.to_string()),
    }))
}

async fn current_account(
    State(state): State<AppState>,
    Actor(actor): Actor,
) -> Result<Json<CurrentAccount>, ApiError> {
    let principal = actor.ok_or(ApiError::Unauthenticated)?;
    let display_name = state
        .vink
        .principal_display_name(principal)
        .await
        .map_err(|_| ApiError::DependencyUnavailable)?;
    Ok(Json(CurrentAccount {
        principal_id: principal.to_string(),
        display_name,
    }))
}

/// `DELETE /auth/session`: logout. Succeeds only when Identity revoked the session (or there was
/// nothing to revoke); during an outage the cookie stays so the person can retry, and the session
/// cannot authorize anything meanwhile because it cannot be verified.
async fn sign_out(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> Result<Response, ApiError> {
    let (parts, _) = request.into_parts();
    if let (Some(credential), Some(client)) = (presented_credential(&parts), state.sign_in.as_ref())
    {
        match client.revoke(credential).await {
            Ok(_) | Err(CallError::NoAuthority) => {}
            Err(error) => return Err(unavailable(error)),
        }
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .append(header::SET_COOKIE, clear(SESSION_COOKIE));
    Ok(response)
}

/// `POST /auth/principal`: gives an authenticated, unbound subject its Vink principal. Idempotent:
/// a bound subject gets its existing principal back (`200`, pen name unchanged).
async fn provision_principal(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> Result<Response, ApiError> {
    let (parts, body) = request.into_parts();
    let subject = match resolve(&state, &parts).await {
        Resolution::Unbound(subject) => subject,
        Resolution::Principal(id) => {
            return Ok(Json(ProvisionedPrincipal {
                principal_id: id.to_string(),
                created: false,
            })
            .into_response());
        }
        Resolution::Suspended(_) => return Err(ApiError::PrincipalSuspended),
        Resolution::Anonymous => return Err(ApiError::Unauthenticated),
        Resolution::Unavailable => return Err(ApiError::DependencyUnavailable),
    };
    // Authentication is decided before the body is inspected.
    let request = axum::extract::Request::from_parts(parts, body);
    let body = <Json<ProvisionPrincipalRequest> as axum::extract::FromRequest<()>>::from_request(
        request,
        &(),
    )
    .await;
    let pen_name = json_body(body)?.pen_name;
    let provisioned = state
        .vink
        .provision_principal(&subject, &pen_name)
        .await
        .map_err(|error| match error {
            AccountError::InvalidPenName => ApiError::Validation {
                field: "pen_name",
                message: error.to_string(),
            },
            AccountError::Unavailable => ApiError::DependencyUnavailable,
        })?;
    if provisioned.bound.standing == Standing::Suspended {
        return Err(ApiError::PrincipalSuspended);
    }
    let status = if provisioned.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    if provisioned.created {
        tracing::info!(principal = %provisioned.bound.principal, "principal provisioned");
    }
    Ok((
        status,
        Json(ProvisionedPrincipal {
            principal_id: provisioned.bound.principal.to_string(),
            created: provisioned.created,
        }),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;

    fn fixture(name: &str) -> Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("contracts/http/v1/fixtures")
            .join(name);
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
    }

    #[test]
    fn auth_shapes_and_errors_match_their_fixtures() {
        use time::macros::datetime;
        let shapes = [
            (
                "auth.challenge.response.json",
                serde_json::to_value(ChallengeSent {
                    expires_at: datetime!(2026-11-07 10:10:00 UTC),
                })
                .unwrap(),
            ),
            (
                "auth.session.needs_principal.json",
                serde_json::to_value(SessionState {
                    state: SessionStateKind::NeedsPrincipal,
                    principal_id: None,
                })
                .unwrap(),
            ),
            (
                "auth.session.active.json",
                serde_json::to_value(SessionState {
                    state: SessionStateKind::Active,
                    principal_id: Some("018f3c2e-0000-7000-8000-00000000c001".into()),
                })
                .unwrap(),
            ),
            (
                "auth.session.account.json",
                serde_json::to_value(CurrentAccount {
                    principal_id: "11111111-1111-4111-8111-111111111111".into(),
                    display_name: "An Nhiên".into(),
                })
                .unwrap(),
            ),
            (
                "auth.principal.response.json",
                serde_json::to_value(ProvisionedPrincipal {
                    principal_id: "018f3c2e-0000-7000-8000-00000000c001".into(),
                    created: true,
                })
                .unwrap(),
            ),
        ];
        for (name, value) in shapes {
            assert_eq!(value, fixture(name), "{name}");
        }
        let _: StartSignInRequest =
            serde_json::from_value(fixture("auth.challenge.request.json")).unwrap();
        let _: CompleteSignInRequest =
            serde_json::from_value(fixture("auth.complete.request.json")).unwrap();
        let _: ProvisionPrincipalRequest =
            serde_json::from_value(fixture("auth.principal.request.json")).unwrap();
        // A client cannot name its subject, principal or role.
        for (name, field) in [
            ("auth.complete.request.json", "subject"),
            ("auth.principal.request.json", "principal_id"),
            ("auth.principal.request.json", "role"),
        ] {
            let mut raw = fixture(name);
            raw[field] = Value::from("admin");
            let decoded = match name {
                "auth.complete.request.json" => {
                    serde_json::from_value::<CompleteSignInRequest>(raw).is_ok()
                }
                _ => serde_json::from_value::<ProvisionPrincipalRequest>(raw).is_ok(),
            };
            assert!(!decoded, "{name} accepted {field}");
        }
        for (name, error, status) in [
            (
                "error.authority_not_configured.json",
                ApiError::AuthorityNotConfigured,
                501,
            ),
            (
                "error.rate_limited.json",
                ApiError::RateLimited {
                    retry_after_seconds: Some(60),
                },
                429,
            ),
            ("error.code_rejected.json", ApiError::CodeRejected, 422),
            (
                "error.challenge_closed.json",
                ApiError::ChallengeClosed,
                409,
            ),
            (
                "error.account_disabled.json",
                ApiError::AccountDisabled,
                403,
            ),
            (
                "error.principal_suspended.json",
                ApiError::PrincipalSuspended,
                403,
            ),
        ] {
            assert_eq!(error.status().as_u16(), status, "{name}");
            assert_eq!(
                serde_json::to_value(error.envelope()).unwrap(),
                fixture(name),
                "{name}"
            );
        }
    }

    #[test]
    fn cookies_are_host_prefixed_secure_and_http_only() {
        let set = cookie(SESSION_COOKIE, "vink_s1_x", "Lax", 60).unwrap();
        assert_eq!(
            set,
            "__Host-vink_session=vink_s1_x; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=60"
        );
        assert_eq!(
            clear(CHALLENGE_COOKIE),
            "__Host-vink_sign_in=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0"
        );
        // A value that could break the header is refused, not written.
        assert!(cookie(SESSION_COOKIE, "a\r\nSet-Cookie: x=y", "Lax", 1).is_none());
        let mut headers = HeaderMap::new();
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("a=1; __Host-vink_sign_in=h1"),
        );
        assert_eq!(single_cookie(&headers, CHALLENGE_COOKIE), Some("h1"));
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("__Host-vink_sign_in=h2"),
        );
        assert_eq!(single_cookie(&headers, CHALLENGE_COOKIE), None);
    }
}
