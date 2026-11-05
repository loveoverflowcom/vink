//! HTTP adapter: API v1, health probes and the Leptos SSR host on one Axum router.

mod api;
mod conditional;
pub mod contract;
mod error;
mod health;
mod ssr;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::extract::{ConnectInfo, FromRef, FromRequestParts};
use axum::http::request::Parts;
use leptos::config::LeptosOptions;
use tower_http::trace::TraceLayer;

pub use error::ApiError;

use crate::application::Vink;
use crate::domain::ids::PrincipalId;
use crate::identity::{IdentityClient, IdentityResolver, Resolution};
use crate::persistence::PgStore;

#[derive(Clone)]
pub struct AppState {
    pub vink: Arc<Vink<PgStore>>,
    pub identity: Arc<dyn IdentityResolver>,
    /// The Identity process, probed by readiness (informational: public reading never waits on it).
    pub identity_probe: Option<IdentityClient>,
    /// Readiness refuses a database role that can change schema or roles (production profile).
    pub require_restricted_role: bool,
    pub public_origin: Arc<str>,
    pub leptos: LeptosOptions,
}

impl FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos.clone()
    }
}

impl AppState {
    pub fn new(
        vink: Vink<PgStore>,
        identity: Arc<dyn IdentityResolver>,
        public_origin: &str,
        site_root: &Path,
    ) -> Self {
        let leptos = LeptosOptions::builder()
            .output_name("vink")
            .site_root(site_root.to_string_lossy().into_owned())
            .site_pkg_dir("pkg")
            .build();
        Self {
            vink: Arc::new(vink),
            identity,
            identity_probe: None,
            require_restricted_role: false,
            public_origin: Arc::from(public_origin),
            leptos,
        }
    }

    pub fn with_identity_probe(mut self, client: Option<IdentityClient>) -> Self {
        self.identity_probe = client;
        self
    }

    pub fn with_restricted_role_required(mut self, required: bool) -> Self {
        self.require_restricted_role = required;
        self
    }
}

/// The authenticated principal of a request, as decided by the server-side resolver. When the
/// identity dependency cannot decide, the request fails with `dependency_unavailable` instead of
/// being treated as anonymous or as anyone.
pub struct Actor(pub Option<PrincipalId>);

impl FromRequestParts<AppState> for Actor {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0);
        match state.identity.resolve(parts, peer).await {
            Resolution::Principal(principal) => Ok(Self(Some(principal))),
            Resolution::Anonymous => Ok(Self(None)),
            Resolution::Unavailable => Err(ApiError::DependencyUnavailable),
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(health::routes())
        .nest("/api/v1", api::routes(state.clone()))
        .merge(ssr::routes(&state))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;
    use crate::identity::NoAuthority;
    use crate::persistence::lazy_pool;

    /// Nothing listens on port 1, so every database call fails like an outage.
    fn outage_router() -> Router {
        let pool = lazy_pool("postgres://vink:unused@127.0.0.1:1/vink").unwrap();
        router(AppState::new(
            Vink::new(PgStore::new(pool)),
            Arc::new(NoAuthority),
            "http://127.0.0.1:3000",
            Path::new("target/site"),
        ))
    }

    async fn get(router: &Router, path: &str) -> (StatusCode, String) {
        let response = router
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    #[tokio::test]
    async fn database_outage_is_reported_not_hidden() {
        let router = outage_router();
        let (status, _) = get(&router, "/health/live").await;
        assert_eq!(status, StatusCode::OK);

        let (status, body) = get(&router, "/health/ready").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.contains(r#""database":"unavailable""#), "{body}");
        assert!(
            body.contains(r#""ai_moderation":"not_implemented""#),
            "{body}"
        );
        // Discussion is Vink-owned (03/11/2026); no external forum is a readiness integration.
        assert!(!body.contains("discourse"), "{body}");

        let (status, body) = get(&router, "/api/v1/works").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.contains("dependency_unavailable"), "{body}");

        // An unverifiable state is never guessed "not modified", and the error is not stored.
        for path in [
            "/api/v1/works",
            "/api/v1/works/33333333-3333-4333-8333-333333333333/publication",
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::get(path)
                        .header(header::IF_NONE_MATCH, "*")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert!(response.headers().get(header::ETAG).is_none());
        }

        let (status, body) = get(&router, "/?q=%C4%90%20%26%20100%25").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.contains("Tạm thời chưa đọc được"), "{body}");
        assert!(!body.contains("Chưa có tác phẩm nào"), "{body}");
        assert!(!body.contains("Không có tác phẩm khớp"), "{body}");
        assert!(body.contains("value=\"Đ &amp; 100%\""), "{body}");
        let canonical = body
            .split("<link")
            .filter_map(|tag| tag.split_once('>').map(|(tag, _)| tag))
            .find(|tag| tag.contains("rel=\"canonical\""))
            .expect("public page has a canonical link");
        assert!(
            canonical.contains("href=\"http://127.0.0.1:3000/\""),
            "{canonical}"
        );
        assert!(!body.contains("working_drafts"), "{body}");

        let (status, body) = get(&router, "/tac-pham/33333333-3333-4333-8333-333333333333").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.contains("Tạm thời chưa đọc được"), "{body}");
    }

    #[tokio::test]
    async fn private_routes_are_closed_without_an_auth_authority() {
        let router = outage_router();
        let (status, body) = get(&router, "/api/v1/drafts").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(body.contains(r#""code":"unauthenticated""#), "{body}");
    }

    async fn send(
        router: &Router,
        method: &str,
        path: &str,
        origin: Option<&str>,
        body: &str,
    ) -> (StatusCode, Option<String>, String) {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            // Forged identity headers never select an actor.
            .header("x-user-id", "22222222-2222-4222-8222-222222222222");
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        let response = router
            .clone()
            .oneshot(request.body(Body::from(body.to_owned())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let cache = response
            .headers()
            .get(header::CACHE_CONTROL)
            .map(|value| value.to_str().unwrap().to_owned());
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, cache, String::from_utf8_lossy(&body).into_owned())
    }

    /// LVE routes with no auth authority and the database down: nothing fabricates a principal,
    /// a success or a zero count.
    #[tokio::test]
    async fn endorsement_routes_fail_closed_and_report_outages() {
        let router = outage_router();
        let origin = Some("http://127.0.0.1:3000");
        let work = "/api/v1/works/33333333-3333-4333-8333-333333333333";
        let comment = format!(
            "{work}/discussion/comments/0192f3a1-0000-7000-8000-000000000001/endorsement/me"
        );
        let endorse = r#"{"expected_version":1,"endorsed":true,"observed_revision_id":"44444444-4444-4444-8444-444444444444"}"#;
        for (method, path, origin, body) in [
            ("GET", format!("{work}/endorsements/me"), None, ""),
            ("PUT", format!("{work}/endorsement/me"), origin, endorse),
            (
                "PUT",
                comment.clone(),
                origin,
                r#"{"expected_version":1,"endorsed":true}"#,
            ),
            // Authentication precedes body validation.
            (
                "PUT",
                format!("{work}/endorsement/me"),
                origin,
                r#"{"actor_id":"x"}"#,
            ),
        ] {
            let (status, cache, body) = send(&router, method, &path, origin, body).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}: {body}");
            assert!(body.contains(r#""code":"unauthenticated""#), "{body}");
            assert_eq!(cache.as_deref(), Some("no-store"), "{path}");
        }
        let (status, _, _) = send(&router, "PUT", &comment, None, "{}").await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "Origin is required for writes"
        );
        // Public summaries need no principal; an outage is 503 (never zero) and is not stored.
        let (status, cache, body) =
            send(&router, "GET", &format!("{work}/endorsements"), None, "").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert!(body.contains("dependency_unavailable"), "{body}");
        assert!(!body.contains("count"), "{body}");
        assert_eq!(cache.as_deref(), Some("no-store"));
        // Input is validated before storage: these fail as 422/404 even while the database is down.
        let (status, _, body) = send(
            &router,
            "GET",
            &format!("{work}/endorsements?comment_ids=x"),
            None,
            "",
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert!(body.contains(r#""field":"comment_ids""#), "{body}");
        let (status, _, _) = send(
            &router,
            "GET",
            "/api/v1/works/not-a-work/endorsements",
            None,
            "",
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

/// The App against a fake Identity process on a real Unix socket: the production resolver,
/// client and router decide; only the server's answers are scripted.
#[cfg(test)]
mod identity_tests {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum::routing::{get, post};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;
    use crate::identity::{IdentityClient, IdentityService};
    use crate::persistence::lazy_pool;

    /// Answers by the presented credential, in the provider's wire format.
    async fn fake_verify(axum::Json(body): axum::Json<Value>) -> axum::response::Response {
        use axum::response::IntoResponse;
        let error = |status: u16, code: &str| {
            (
                StatusCode::from_u16(status).unwrap(),
                axum::Json(json!({ "error": { "code": code, "message": "fixed" } })),
            )
                .into_response()
        };
        match body["credential"].as_str().unwrap_or_default() {
            "rejected" => axum::Json(json!({ "outcome": "rejected" })).into_response(),
            "authenticated" => axum::Json(json!({
                "outcome": "authenticated",
                "subject": { "authority": "test-fixture", "id": "fixture-subject-0001" },
                "authenticated_at": "2026-11-02T00:00:00Z"
            }))
            .into_response(),
            "no-authority" => error(501, "authority_not_configured"),
            "unavailable" => error(503, "dependency_unavailable"),
            "caller" => error(401, "caller_unauthenticated"),
            "malformed" => (StatusCode::OK, "{\"outcome\":").into_response(),
            "unknown-outcome" => axum::Json(json!({ "outcome": "admin" })).into_response(),
            "slow" => {
                tokio::time::sleep(Duration::from_secs(3)).await;
                axum::Json(json!({ "outcome": "rejected" })).into_response()
            }
            _ => error(400, "malformed_request"),
        }
    }

    struct Fake {
        dir: PathBuf,
        socket: PathBuf,
        uid: u32,
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    async fn fake_identity(name: &str) -> Fake {
        let dir = std::env::temp_dir().join(format!("vink-app-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let socket = dir.join("id.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let app = Router::new()
            .route("/identity/v1/verify-credential", post(fake_verify))
            .route(
                "/identity/v1/capabilities",
                get(|| async {
                    axum::Json(json!({
                        "contract": "vink.identity.v1",
                        "authority": "not_configured",
                        "operations": { "verify_credential": "not_configured" }
                    }))
                }),
            );
        tokio::spawn(async move { axum::serve(listener, app).await });
        let uid = std::fs::metadata(&dir).unwrap().uid();
        Fake { dir, socket, uid }
    }

    fn app(socket: &Path, server_uid: u32) -> Router {
        let pool = lazy_pool("postgres://vink:unused@127.0.0.1:1/vink").unwrap();
        let client = IdentityClient::with_timeout(
            socket.to_path_buf(),
            server_uid,
            Duration::from_millis(400),
        );
        router(
            AppState::new(
                Vink::new(PgStore::new(pool)),
                Arc::new(IdentityService::new(client.clone())),
                "http://127.0.0.1:3000",
                Path::new("target/site"),
            )
            .with_identity_probe(Some(client)),
        )
    }

    async fn drafts(router: &Router, headers: &[(&str, &str)]) -> (StatusCode, String) {
        let mut request = Request::get("/api/v1/drafts");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = router
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    #[tokio::test]
    async fn identity_answers_never_grant_and_outages_stay_distinct() {
        let fake = fake_identity("answers").await;
        let router = app(&fake.socket, fake.uid);
        for (credential, status, code) in [
            ("rejected", 401, "unauthenticated"),
            // No principal binding exists: an authenticated subject is still nobody.
            ("authenticated", 401, "unauthenticated"),
            ("no-authority", 401, "unauthenticated"),
            ("unavailable", 503, "dependency_unavailable"),
            ("caller", 503, "dependency_unavailable"),
            ("malformed", 503, "dependency_unavailable"),
            ("unknown-outcome", 503, "dependency_unavailable"),
            ("slow", 503, "dependency_unavailable"),
        ] {
            let bearer = format!("Bearer {credential}");
            let forged = [
                (header::AUTHORIZATION.as_str(), bearer.as_str()),
                ("x-user-id", "11111111-1111-4111-8111-111111111111"),
                ("x-vink-principal", "11111111-1111-4111-8111-111111111111"),
            ];
            let (got, body) = drafts(&router, &forged).await;
            assert_eq!(got.as_u16(), status, "{credential}: {body}");
            assert!(
                body.contains(&format!(r#""code":"{code}""#)),
                "{credential}: {body}"
            );
        }
        // Endorsement own state resolves the actor the same way: an authenticated subject
        // without a principal binding is nobody, and an undecidable credential is an outage.
        for (credential, status) in [
            ("authenticated", 401),
            ("rejected", 401),
            ("unavailable", 503),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::get(
                        "/api/v1/works/33333333-3333-4333-8333-333333333333/endorsements/me",
                    )
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
                    .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status, "{credential}");
        }
        // No credential is anonymous, never an outage, and does not call Identity.
        let (got, body) = drafts(
            &router,
            &[("x-user-id", "11111111-1111-4111-8111-111111111111")],
        )
        .await;
        assert_eq!(got, StatusCode::UNAUTHORIZED, "{body}");
    }

    #[tokio::test]
    async fn a_socket_served_by_another_user_or_missing_is_unavailable() {
        let fake = fake_identity("server-uid").await;
        for router in [
            app(&fake.socket, fake.uid.wrapping_add(1)),
            app(&fake.dir.join("missing.sock"), fake.uid),
        ] {
            let (got, body) = drafts(&router, &[("authorization", "Bearer rejected")]).await;
            assert_eq!(got, StatusCode::SERVICE_UNAVAILABLE, "{body}");
            let (got, _) = drafts(&router, &[]).await;
            assert_eq!(got, StatusCode::UNAUTHORIZED);
            let ready = router
                .clone()
                .oneshot(Request::get("/health/ready").body(Body::empty()).unwrap())
                .await
                .unwrap();
            let body = ready.into_body().collect().await.unwrap().to_bytes();
            assert!(
                String::from_utf8_lossy(&body).contains(r#""identity":"unavailable""#),
                "{}",
                String::from_utf8_lossy(&body)
            );
        }
    }

    #[tokio::test]
    async fn readiness_reports_identity_without_authority() {
        let fake = fake_identity("ready").await;
        let response = app(&fake.socket, fake.uid)
            .oneshot(Request::get("/health/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["integrations"]["identity"], "no_authority");
        // Database outage: not ready, regardless of Identity.
        assert_eq!(body["checks"]["database"], "unavailable");
        assert_eq!(body["checks"]["role"], "unknown");
    }
}
