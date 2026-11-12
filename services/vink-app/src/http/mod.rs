//! HTTP adapter: API v1, health probes and the Leptos SSR host on one Axum router.

mod api;
mod appeals;
mod auth;
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

pub use auth::{AdmitFuture, SignInLimiter};
pub use error::ApiError;

use crate::application::Vink;
use crate::domain::abuse::TrustedProxies;
use crate::domain::ids::PrincipalId;
use crate::identity::{IdentityClient, IdentityResolver, PrincipalDirectory};
use crate::persistence::PgStore;

#[derive(Clone)]
pub struct AppState {
    pub vink: Arc<Vink<PgStore>>,
    pub identity: Arc<dyn IdentityResolver>,
    /// The App's subject → principal bindings and suspensions, as read by sign-in (the same
    /// store the resolver reads; replaceable in tests).
    pub directory: Arc<dyn PrincipalDirectory>,
    /// Identity client for the sign-in routes; `None` answers `501 authority_not_configured`.
    pub sign_in: Option<IdentityClient>,
    /// Whether the sign-in routes may start new sessions (`VINK_AUTH_SIGN_IN`). Logout and the
    /// verification of existing sessions never depend on it.
    pub sign_in_enabled: bool,
    /// Rate limits of the sign-in routes; by default the App database.
    pub limiter: Arc<dyn SignInLimiter>,
    /// Proxies whose `X-Forwarded-For` names the client network (`VINK_TRUSTED_PROXIES`).
    pub trusted_proxies: Arc<TrustedProxies>,
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
        let vink = Arc::new(vink);
        Self {
            directory: vink.clone(),
            limiter: vink.clone(),
            vink,
            identity,
            sign_in: None,
            sign_in_enabled: true,
            trusted_proxies: Arc::default(),
            identity_probe: None,
            require_restricted_role: false,
            public_origin: Arc::from(public_origin),
            leptos,
        }
    }

    pub fn with_sign_in(mut self, client: Option<IdentityClient>) -> Self {
        self.sign_in = client;
        self
    }

    pub fn with_sign_in_enabled(mut self, enabled: bool) -> Self {
        self.sign_in_enabled = enabled;
        self
    }

    pub fn with_limiter(mut self, limiter: Arc<dyn SignInLimiter>) -> Self {
        self.limiter = limiter;
        self
    }

    pub fn with_trusted_proxies(mut self, proxies: TrustedProxies) -> Self {
        self.trusted_proxies = Arc::new(proxies);
        self
    }

    pub fn with_directory(mut self, directory: Arc<dyn PrincipalDirectory>) -> Self {
        self.directory = directory;
        self
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

/// The authenticated principal of a request, as decided by the server-side resolver. Only a
/// bound, unsuspended principal acts; an unbound subject or a suspended principal is `None` like
/// an anonymous request (`GET /api/v1/auth/session` tells them apart). When a dependency cannot
/// decide, the request fails with `dependency_unavailable` instead of being treated as anyone.
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
        let resolution = state.identity.resolve(parts, peer).await;
        match resolution {
            crate::identity::Resolution::Unavailable => Err(ApiError::DependencyUnavailable),
            other => Ok(Self(other.principal())),
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

        // #120 DISC-02/03: the public projection resolves no actor, so a database outage is its
        // only failure, and it is a 503, never an empty discussion. The private overlay refuses a
        // guest before touching storage.
        for path in [
            "/api/v1/works/33333333-3333-4333-8333-333333333333/discussion/public",
            "/api/v1/works/33333333-3333-4333-8333-333333333333/discussion/public/roots/0192f3a1-0000-7000-8000-000000000001/replies",
        ] {
            let response = router
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let body = String::from_utf8(bytes.to_vec()).unwrap();
            assert!(body.contains("dependency_unavailable"), "{body}");
            assert!(
                !body.contains("roots") && !body.contains("replies\""),
                "{body}"
            );
        }
        let (status, body) = get(
            &router,
            "/api/v1/works/33333333-3333-4333-8333-333333333333/discussion/me",
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(body.contains(r#""code":"unauthenticated""#), "{body}");

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
        for path in ["/api/v1/drafts", "/api/v1/auth/session/account"] {
            let (status, body) = get(&router, path).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
            assert!(body.contains(r#""code":"unauthenticated""#), "{body}");
        }
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

    /// Catalog reads (TAG-PR-03) need no actor: with the database down they say so (503, never an
    /// empty catalog), validate first, and are never stored. Private draft tags fail closed.
    #[tokio::test]
    async fn tag_routes_validate_report_outages_and_fail_closed() {
        let router = outage_router();
        let origin = Some("http://127.0.0.1:3000");
        for path in [
            "/api/v1/tags",
            "/api/v1/tags?facet=form",
            "/api/v1/tags/suggestions?q=h%E1%BB%93i&facet=theme&limit=5",
            "/api/v1/tags/slugs/k%C3%BD-%E1%BB%A9c",
        ] {
            let (status, cache, body) = send(&router, "GET", path, None, "").await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}: {body}");
            assert!(body.contains("dependency_unavailable"), "{body}");
            assert!(!body.contains(r#""tags""#) && !body.contains("suggestions"));
            assert_eq!(cache.as_deref(), Some("no-store"), "{path}");
        }
        for (path, field) in [
            ("/api/v1/tags?facet=genre", "facet"),
            ("/api/v1/tags?sort=label", "query"),
            ("/api/v1/tags/suggestions", "q"),
            ("/api/v1/tags/suggestions?q=%20-%20", "q"),
            ("/api/v1/tags/suggestions?q=ma&limit=11", "limit"),
            ("/api/v1/tags/suggestions?q=ma&limit=x", "query"),
            ("/api/v1/tags/suggestions?q=ma&facet=Theme", "facet"),
            ("/api/v1/tags/suggestions?q=ma&actor_id=1", "query"),
        ] {
            let (status, cache, body) = send(&router, "GET", path, None, "").await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path}: {body}");
            assert!(
                body.contains(&format!(r#""field":"{field}""#)),
                "{path}: {body}"
            );
            assert_eq!(cache.as_deref(), Some("no-store"));
        }
        // A malformed slug cannot name a tag.
        let (status, _, _) = send(
            &router,
            "GET",
            "/api/v1/tags/slugs/k%C3%BD--%E1%BB%A9c",
            None,
            "",
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        // There is no catalog write route, whatever the Origin or a forged identity header says.
        for (method, path) in [
            ("POST", "/api/v1/tags"),
            ("PUT", "/api/v1/tags/slugs/k%C3%BD-%E1%BB%A9c"),
            ("DELETE", "/api/v1/tags/slugs/k%C3%BD-%E1%BB%A9c"),
        ] {
            let (status, _, _) = send(&router, method, path, origin, r#"{"label":"Ký ức"}"#).await;
            assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{method} {path}");
        }
        let tags = "/api/v1/drafts/33333333-3333-4333-8333-333333333333/tags";
        let save = r#"{"expected_version":1,"form":null,"themes":[]}"#;
        for (method, body) in [
            ("GET", ""),
            ("PUT", save),
            // Authentication precedes body validation.
            (
                "PUT",
                r#"{"actor_id":"22222222-2222-4222-8222-222222222222"}"#,
            ),
        ] {
            let (status, cache, response) = send(&router, method, tags, origin, body).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{method}: {response}");
            assert!(
                response.contains(r#""code":"unauthenticated""#),
                "{response}"
            );
            assert_eq!(cache.as_deref(), Some("no-store"));
        }
        let (status, _, _) = send(&router, "PUT", tags, None, save).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "Origin is required for writes"
        );
    }

    /// Public search (TAG-PR-04): every malformed parameter is refused before the database is
    /// touched (the outage router would answer `503` otherwise), with the field to fix; a valid
    /// search during an outage is `503`, never an empty page, a validator or a stored error.
    #[tokio::test]
    async fn work_search_validates_before_reading_and_reports_outages() {
        let router = outage_router();
        let nine_tags = format!("/api/v1/works?{}", ["tag=a"; 9].join("&"));
        for (path, field) in [
            ("/api/v1/works?sort=newest", "query"),
            ("/api/v1/works?q=a&q=b", "query"),
            ("/api/v1/works?limit=3&limit=3", "query"),
            ("/api/v1/works?cursor=&cursor=", "query"),
            ("/api/v1/works?actor_id=1", "query"),
            ("/api/v1/works?q=%21%21%21", "q"),
            ("/api/v1/works?q=%25_", "q"),
            ("/api/v1/works?q=ma%00", "q"),
            ("/api/v1/works?tag=k%C3%BD--%E1%BB%A9c", "tag"),
            ("/api/v1/works?tag=%E2%80%A6", "tag"),
            ("/api/v1/works?tag=a&tag=b&tag=c&tag=d&tag=e", "tag"),
            (nine_tags.as_str(), "tag"),
            ("/api/v1/works?limit=0", "limit"),
            ("/api/v1/works?limit=25", "limit"),
            ("/api/v1/works?limit=x", "limit"),
            ("/api/v1/works?cursor=zz", "cursor"),
            // `q` is checked first, then `tag`, `limit` and `cursor`.
            ("/api/v1/works?cursor=zz&limit=0&tag=-&q=%21", "q"),
            ("/api/v1/works?cursor=zz&limit=0&tag=-", "tag"),
            ("/api/v1/works?cursor=zz&limit=0", "limit"),
        ] {
            let (status, cache, body) = send(&router, "GET", path, None, "").await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path}: {body}");
            assert!(
                body.contains(&format!(r#""field":"{field}""#)),
                "{path}: {body}"
            );
            assert_eq!(cache.as_deref(), Some("no-store"), "{path}");
        }
        for path in [
            "/api/v1/works?q=m%C6%B0a&tag=k%C3%BD-%E1%BB%A9c&tag=h%E1%BB%93i-%E1%BB%A9c&limit=24",
            // Blank values are absent, as an empty native form field submits them.
            "/api/v1/works?q=&tag=&limit=&cursor=",
            "/api/v1/works?q=+m%C6%B0a+",
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
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let body = String::from_utf8_lossy(&body);
            assert!(body.contains("dependency_unavailable"), "{body}");
            assert!(!body.contains(r#""works""#), "{body}");
        }
    }

    /// Catalog governance authority is constructed by the operator command only (#97 CHOICE-02
    /// A). No HTTP source may name it, so no route can change the catalog.
    #[test]
    fn http_never_holds_curator_authority() {
        // Spelled in pieces so that this test does not match itself.
        let authority = concat!("Curator", "Authority");
        let change = concat!("change_", "catalog");
        for (file, source) in [
            ("api.rs", include_str!("api.rs")),
            ("auth.rs", include_str!("auth.rs")),
            ("conditional.rs", include_str!("conditional.rs")),
            ("contract.rs", include_str!("contract.rs")),
            ("error.rs", include_str!("error.rs")),
            ("health.rs", include_str!("health.rs")),
            ("mod.rs", include_str!("mod.rs")),
            ("ssr.rs", include_str!("ssr.rs")),
        ] {
            assert!(!source.contains(authority), "{file} names {authority}");
            assert!(!source.contains(change), "{file} calls {change}");
        }
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
    use crate::application::ports::BoundPrincipal;
    use crate::domain::principal::{Standing, SubjectKey};
    use crate::identity::{
        DirectoryFuture, DirectoryUnavailable, IdentityClient, IdentityService, PrincipalDirectory,
    };
    use crate::persistence::lazy_pool;

    const BOUND: &str = "22222222-2222-4222-8222-222222222222";
    const SUSPENDED: &str = "33333333-3333-4333-8333-333333333333";

    /// The App's binding table as the tests need it, keyed by subject id.
    struct Directory;

    impl PrincipalDirectory for Directory {
        fn lookup<'a>(&'a self, subject: &'a SubjectKey) -> DirectoryFuture<'a> {
            let principal = |raw| PrincipalId::parse(raw).unwrap();
            let answer = match subject.id() {
                "bound-subject" => Ok(Some(BoundPrincipal {
                    principal: principal(BOUND),
                    standing: Standing::Active,
                })),
                "suspended-subject" => Ok(Some(BoundPrincipal {
                    principal: principal(SUSPENDED),
                    standing: Standing::Suspended,
                })),
                "directory-down" => Err(DirectoryUnavailable),
                _ => Ok(None),
            };
            Box::pin(std::future::ready(answer))
        }
    }

    fn authenticated(subject: &str) -> Value {
        json!({
            "outcome": "authenticated",
            "subject": { "authority": "vink-identity", "id": subject },
            "authenticated_at": "2026-11-07T10:01:00Z",
            "session": { "id": "s", "expires_at": "2026-12-07T10:01:00Z" }
        })
    }

    static REVOKED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    /// Revocations of tests that must not race the exact assertions on [`REVOKED`].
    static REPLACED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    async fn fake_challenge(axum::Json(body): axum::Json<Value>) -> axum::response::Response {
        use axum::response::IntoResponse;
        match body["email"].as_str().unwrap_or_default() {
            "invalid" => (
                StatusCode::UNPROCESSABLE_ENTITY,
                axum::Json(json!({ "error": { "code": "invalid_email", "message": "x" } })),
            )
                .into_response(),
            "limited@vi-du.vn" => (
                StatusCode::TOO_MANY_REQUESTS,
                axum::Json(json!({ "error": { "code": "rate_limited", "message": "x" } })),
            )
                .into_response(),
            "down@vi-du.vn" => (StatusCode::BAD_GATEWAY, "proxy").into_response(),
            _ => (
                StatusCode::ACCEPTED,
                axum::Json(json!({
                    "challenge": "vink_c1.handle-for-tests",
                    "expires_at": "2099-01-01T00:00:00Z"
                })),
            )
                .into_response(),
        }
    }

    /// The challenge cookie names the subject the fake authenticates; the code decides refusals.
    async fn fake_complete(axum::Json(body): axum::Json<Value>) -> axum::response::Response {
        use axum::response::IntoResponse;
        let subject = body["challenge"].as_str().unwrap_or_default().to_string();
        let rejected = |reason: &str| {
            axum::Json(json!({ "outcome": "rejected", "reason": reason })).into_response()
        };
        match body["code"].as_str().unwrap_or_default() {
            "000000" => rejected("invalid_code"),
            "111111" => rejected("challenge_closed"),
            "222222" => rejected("account_disabled"),
            "333333" => rejected("promoted_to_admin"),
            _ => axum::Json(json!({
                "outcome": "authenticated",
                "subject": { "authority": "vink-identity", "id": subject },
                "authenticated_at": "2026-11-07T10:01:00Z",
                "session": {
                    "id": "s",
                    "credential": format!("vink_s1_for-{subject}"),
                    "expires_at": "2099-01-01T00:00:00Z"
                }
            }))
            .into_response(),
        }
    }

    async fn fake_revoke(axum::Json(body): axum::Json<Value>) -> axum::response::Response {
        use axum::response::IntoResponse;
        let credential = body["credential"].as_str().unwrap_or_default().to_string();
        if credential == "unavailable" {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(
                    json!({ "error": { "code": "dependency_unavailable", "message": "x" } }),
                ),
            )
                .into_response();
        }
        if credential.starts_with("vink_s1_isolated-") {
            REPLACED.lock().unwrap().push(credential);
        } else {
            REVOKED.lock().unwrap().push(credential);
        }
        axum::Json(json!({ "revoked": true })).into_response()
    }

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
            "revoked" => {
                axum::Json(json!({ "outcome": "rejected", "reason": "revoked" })).into_response()
            }
            "authenticated" => axum::Json(json!({
                "outcome": "authenticated",
                "subject": { "authority": "test-fixture", "id": "fixture-subject-0001" },
                "authenticated_at": "2026-11-02T00:00:00Z"
            }))
            .into_response(),
            "bound" => axum::Json(authenticated("bound-subject")).into_response(),
            "suspended" => axum::Json(authenticated("suspended-subject")).into_response(),
            "directory-down" => axum::Json(authenticated("directory-down")).into_response(),
            "bad-subject" => axum::Json(authenticated("has space")).into_response(),
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
            .route("/identity/v1/sign-in/challenges", post(fake_challenge))
            .route("/identity/v1/sign-in/complete", post(fake_complete))
            .route("/identity/v1/sessions/revoke", post(fake_revoke))
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

    /// Admits everything; the PostgreSQL limiter is `tests/postgres/abuse.rs`.
    struct Unlimited;

    impl SignInLimiter for Unlimited {
        fn admit<'a>(
            &'a self,
            _: &'a crate::domain::abuse::RateLimit,
            _: crate::domain::abuse::Network,
        ) -> AdmitFuture<'a> {
            Box::pin(std::future::ready(Ok(
                crate::domain::abuse::Admission::Admitted,
            )))
        }
    }

    /// Refuses (`Some`) or cannot count (`None`), and records the networks it was asked about.
    struct Scripted {
        refuse: Option<u32>,
        seen: std::sync::Mutex<Vec<crate::domain::abuse::Network>>,
    }

    impl SignInLimiter for Scripted {
        fn admit<'a>(
            &'a self,
            _: &'a crate::domain::abuse::RateLimit,
            network: crate::domain::abuse::Network,
        ) -> AdmitFuture<'a> {
            self.seen.lock().unwrap().push(network);
            let answer = match self.refuse {
                Some(seconds) => Ok(crate::domain::abuse::Admission::Refused {
                    retry_after_seconds: seconds,
                }),
                None => Err(crate::application::ports::StoreError::Unavailable(
                    "counter down".into(),
                )),
            };
            Box::pin(std::future::ready(answer))
        }
    }

    const PEER: ([u8; 4], u16) = ([203, 0, 113, 9], 50_000);

    fn state(socket: &Path, server_uid: u32) -> AppState {
        let pool = lazy_pool("postgres://vink:unused@127.0.0.1:1/vink").unwrap();
        let client = IdentityClient::with_timeout(
            socket.to_path_buf(),
            server_uid,
            Duration::from_millis(400),
        );
        AppState::new(
            Vink::new(PgStore::new(pool)),
            Arc::new(IdentityService::new(client.clone(), Arc::new(Directory))),
            "http://127.0.0.1:3000",
            Path::new("target/site"),
        )
        .with_directory(Arc::new(Directory))
        .with_sign_in(Some(client.clone()))
        .with_identity_probe(Some(client))
        .with_limiter(Arc::new(Unlimited))
    }

    fn with_peer(state: AppState, peer: SocketAddr) -> Router {
        router(state).layer(axum::Extension(ConnectInfo(peer)))
    }

    fn app(socket: &Path, server_uid: u32) -> Router {
        with_peer(state(socket, server_uid), SocketAddr::from(PEER))
    }

    async fn call(
        router: &Router,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> (StatusCode, axum::http::HeaderMap, String) {
        let mut request = Request::builder().method(method).uri(path);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, headers, String::from_utf8_lossy(&body).into_owned())
    }

    fn set_cookies(headers: &axum::http::HeaderMap) -> Vec<String> {
        headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .collect()
    }

    const ORIGIN: (&str, &str) = ("origin", "http://127.0.0.1:3000");

    #[tokio::test]
    async fn bindings_and_suspensions_decide_after_identity_authenticates() {
        let fake = fake_identity("bindings").await;
        let router = app(&fake.socket, fake.uid);
        for (credential, drafts_status, state) in [
            ("authenticated", 401, Some("needs_principal")),
            ("suspended", 401, Some("suspended")),
            ("revoked", 401, Some("anonymous")),
            ("directory-down", 503, None),
            ("bad-subject", 503, None),
        ] {
            for carrier in [
                (
                    header::AUTHORIZATION.as_str(),
                    format!("Bearer {credential}"),
                ),
                (
                    header::COOKIE.as_str(),
                    format!("__Host-vink_session={credential}"),
                ),
            ] {
                let headers = [(carrier.0, carrier.1.as_str())];
                let (got, body) = drafts(&router, &headers).await;
                assert_eq!(
                    got.as_u16(),
                    drafts_status,
                    "{credential} {}: {body}",
                    carrier.0
                );
                let (got, _, body) =
                    call(&router, "GET", "/api/v1/auth/session", &headers, None).await;
                match state {
                    Some(state) => {
                        assert_eq!(got, StatusCode::OK, "{credential}");
                        assert!(body.contains(&format!(r#""state":"{state}""#)), "{body}");
                    }
                    None => assert_eq!(got, StatusCode::SERVICE_UNAVAILABLE, "{credential}"),
                }
            }
        }
        // A bound, active principal acts (the session endpoint needs no database).
        let (got, _, body) = call(
            &router,
            "GET",
            "/api/v1/auth/session",
            &[("cookie", "__Host-vink_session=bound")],
            None,
        )
        .await;
        assert_eq!(got, StatusCode::OK);
        assert!(body.contains(BOUND), "{body}");
        // A suspended principal cannot provision itself a fresh principal.
        let (got, _, body) = call(
            &router,
            "POST",
            "/api/v1/auth/principal",
            &[ORIGIN, ("cookie", "__Host-vink_session=suspended")],
            Some(r#"{"pen_name":"Tên mới"}"#),
        )
        .await;
        assert_eq!(got, StatusCode::FORBIDDEN, "{body}");
        assert!(body.contains("principal_suspended"), "{body}");
        let (got, _, _) = call(
            &router,
            "POST",
            "/api/v1/auth/principal",
            &[ORIGIN],
            Some(r#"{"pen_name":"Tên mới"}"#),
        )
        .await;
        assert_eq!(got, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn sign_in_keeps_secrets_in_host_cookies_and_never_grants_by_itself() {
        let fake = fake_identity("sign-in").await;
        let router = app(&fake.socket, fake.uid);
        // Cross-site or origin-less unsafe requests are refused before Identity is asked.
        let (got, _, _) = call(
            &router,
            "POST",
            "/api/v1/auth/sign-in/challenges",
            &[],
            Some(r#"{"email":"a@vi-du.vn"}"#),
        )
        .await;
        assert_eq!(got, StatusCode::FORBIDDEN);

        let (got, headers, body) = call(
            &router,
            "POST",
            "/api/v1/auth/sign-in/challenges",
            &[ORIGIN],
            Some(r#"{"email":"a@vi-du.vn"}"#),
        )
        .await;
        assert_eq!(got, StatusCode::ACCEPTED, "{body}");
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert!(
            !body.contains("vink_c1"),
            "the handle never reaches page scripts: {body}"
        );
        let cookies = set_cookies(&headers);
        assert_eq!(cookies.len(), 1, "{cookies:?}");
        assert!(cookies[0].starts_with("__Host-vink_sign_in=vink_c1.handle-for-tests; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age="), "{cookies:?}");

        for (email, status, code) in [
            ("invalid", 422, "validation_failed"),
            ("limited@vi-du.vn", 429, "rate_limited"),
            ("down@vi-du.vn", 503, "dependency_unavailable"),
        ] {
            let (got, _, body) = call(
                &router,
                "POST",
                "/api/v1/auth/sign-in/challenges",
                &[ORIGIN],
                Some(&format!(r#"{{"email":"{email}"}}"#)),
            )
            .await;
            assert_eq!(got.as_u16(), status, "{email}: {body}");
            assert!(body.contains(code), "{body}");
        }
        // A forged subject in the body is refused.
        let (got, _, _) = call(
            &router,
            "POST",
            "/api/v1/auth/sign-in/complete",
            &[ORIGIN, ("cookie", "__Host-vink_sign_in=new-subject")],
            Some(r#"{"code":"123456","subject":"bound-subject"}"#),
        )
        .await;
        assert_eq!(got, StatusCode::UNPROCESSABLE_ENTITY);

        let complete = |cookie: &'static str, code: &'static str| {
            let router = router.clone();
            async move {
                call(
                    &router,
                    "POST",
                    "/api/v1/auth/sign-in/complete",
                    &[ORIGIN, ("cookie", cookie)],
                    Some(&format!(r#"{{"code":"{code}"}}"#)),
                )
                .await
            }
        };
        // New subject: a session cookie, no principal, the challenge cookie cleared.
        let (got, headers, body) = complete("__Host-vink_sign_in=new-subject", "123 456").await;
        assert_eq!(got, StatusCode::OK, "{body}");
        assert_eq!(body, r#"{"state":"needs_principal"}"#);
        let cookies = set_cookies(&headers);
        assert!(cookies.iter().any(|c| c.starts_with(
            "__Host-vink_session=vink_s1_for-new-subject; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age="
        )), "{cookies:?}");
        assert!(
            cookies
                .iter()
                .any(|c| c.starts_with("__Host-vink_sign_in=; ")),
            "{cookies:?}"
        );
        assert!(!body.contains("vink_s1"));
        // Existing, active principal.
        let (got, _, body) = complete("__Host-vink_sign_in=bound-subject", "123456").await;
        assert_eq!(got, StatusCode::OK);
        assert!(
            body.contains(r#""state":"active""#) && body.contains(BOUND),
            "{body}"
        );

        // Refusals keep their meaning; a wrong code keeps the challenge for another attempt.
        for (code, status, error, clears) in [
            ("000000", 422, "code_rejected", false),
            ("111111", 409, "challenge_closed", true),
            ("222222", 403, "account_disabled", true),
            ("333333", 503, "dependency_unavailable", false),
            ("12345", 422, "validation_failed", false),
        ] {
            let (got, headers, body) = complete("__Host-vink_sign_in=new-subject", code).await;
            assert_eq!(got.as_u16(), status, "{code}: {body}");
            assert!(body.contains(error), "{code}: {body}");
            let cookies = set_cookies(&headers);
            assert!(
                !cookies.iter().any(|c| c.starts_with("__Host-vink_session")),
                "{code}"
            );
            assert_eq!(!cookies.is_empty(), clears, "{code}: {cookies:?}");
        }
        // No challenge cookie: closed, Identity not asked for a session.
        let (got, _, _) = complete("theme=dark", "123456").await;
        assert_eq!(got, StatusCode::CONFLICT);

        // Suspended principal or an undecidable directory: the fresh session is revoked and
        // never delivered.
        REVOKED.lock().unwrap().clear();
        let (got, headers, body) =
            complete("__Host-vink_sign_in=suspended-subject", "123456").await;
        assert_eq!(got, StatusCode::FORBIDDEN, "{body}");
        assert!(body.contains("principal_suspended"));
        assert!(
            !set_cookies(&headers)
                .iter()
                .any(|c| c.starts_with("__Host-vink_session"))
        );
        let (got, headers, _) = complete("__Host-vink_sign_in=directory-down", "123456").await;
        assert_eq!(got, StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            !set_cookies(&headers)
                .iter()
                .any(|c| c.starts_with("__Host-vink_session"))
        );
        assert_eq!(
            REVOKED.lock().unwrap().as_slice(),
            [
                "vink_s1_for-suspended-subject",
                "vink_s1_for-directory-down"
            ]
        );
    }

    #[tokio::test]
    async fn logout_clears_the_cookie_only_after_identity_revoked() {
        let fake = fake_identity("logout").await;
        let router = app(&fake.socket, fake.uid);
        let (got, headers, _) = call(
            &router,
            "DELETE",
            "/api/v1/auth/session",
            &[ORIGIN, ("cookie", "__Host-vink_session=vink_s1_logout-me")],
            None,
        )
        .await;
        assert_eq!(got, StatusCode::NO_CONTENT);
        assert_eq!(
            set_cookies(&headers),
            ["__Host-vink_session=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0"]
        );
        assert!(
            REVOKED
                .lock()
                .unwrap()
                .iter()
                .any(|c| c == "vink_s1_logout-me")
        );
        // Identity down: the cookie stays so logout can be retried.
        let (got, headers, _) = call(
            &router,
            "DELETE",
            "/api/v1/auth/session",
            &[ORIGIN, ("cookie", "__Host-vink_session=unavailable")],
            None,
        )
        .await;
        assert_eq!(got, StatusCode::SERVICE_UNAVAILABLE);
        assert!(set_cookies(&headers).is_empty());
        // Nothing presented: idempotent success. Cross-site: refused.
        let (got, _, _) = call(&router, "DELETE", "/api/v1/auth/session", &[ORIGIN], None).await;
        assert_eq!(got, StatusCode::NO_CONTENT);
        let (got, _, _) = call(
            &router,
            "DELETE",
            "/api/v1/auth/session",
            &[
                ("origin", "https://evil.example"),
                ("cookie", "__Host-vink_session=x"),
            ],
            None,
        )
        .await;
        assert_eq!(got, StatusCode::FORBIDDEN);
    }

    /// D-05 abuse protection at the App edge: a refused, uncountable or unattributable request
    /// never reaches Identity (whose answers below would be 202/503 otherwise).
    #[tokio::test]
    async fn sign_in_is_counted_per_network_before_identity_and_fails_closed() {
        let fake = fake_identity("limits").await;
        let challenge = |router: Router| async move {
            call(
                &router,
                "POST",
                "/api/v1/auth/sign-in/challenges",
                &[ORIGIN],
                Some(r#"{"email":"down@vi-du.vn"}"#),
            )
            .await
        };
        let complete = |router: Router| async move {
            call(
                &router,
                "POST",
                "/api/v1/auth/sign-in/complete",
                &[ORIGIN, ("cookie", "__Host-vink_sign_in=new-subject")],
                Some(r#"{"code":"123456"}"#),
            )
            .await
        };
        let refusing = Arc::new(Scripted {
            refuse: Some(42),
            seen: Default::default(),
        });
        let router = with_peer(
            state(&fake.socket, fake.uid).with_limiter(refusing.clone()),
            SocketAddr::from(PEER),
        );
        for (got, headers, body) in [
            challenge(router.clone()).await,
            complete(router.clone()).await,
        ] {
            assert_eq!(got, StatusCode::TOO_MANY_REQUESTS, "{body}");
            assert!(body.contains(r#""code":"rate_limited""#), "{body}");
            assert_eq!(headers[header::RETRY_AFTER], "42");
            assert_eq!(headers[header::CACHE_CONTROL], "no-store");
            assert!(set_cookies(&headers).is_empty());
        }
        assert_eq!(refusing.seen.lock().unwrap().len(), 2);

        // The counter is down: 503, never an unlimited request.
        let down = Arc::new(Scripted {
            refuse: None,
            seen: Default::default(),
        });
        let router = with_peer(
            state(&fake.socket, fake.uid).with_limiter(down),
            SocketAddr::from(PEER),
        );
        for (got, _, body) in [
            challenge(router.clone()).await,
            complete(router.clone()).await,
        ] {
            assert_eq!(got, StatusCode::SERVICE_UNAVAILABLE, "{body}");
            assert!(body.contains("dependency_unavailable"), "{body}");
        }
        // No peer address: cannot be attributed, refused.
        let (got, _, _) = challenge(super::router(state(&fake.socket, fake.uid))).await;
        assert_eq!(got, StatusCode::SERVICE_UNAVAILABLE);
        // Identity's own ceiling (per address) passes through without a Retry-After guess.
        let (got, headers, body) = call(
            &app(&fake.socket, fake.uid),
            "POST",
            "/api/v1/auth/sign-in/challenges",
            &[ORIGIN],
            Some(r#"{"email":"limited@vi-du.vn"}"#),
        )
        .await;
        assert_eq!(got, StatusCode::TOO_MANY_REQUESTS, "{body}");
        assert!(headers.get(header::RETRY_AFTER).is_none());
    }

    /// A forged `X-Forwarded-For` cannot choose the bucket of a direct client; behind a trusted
    /// proxy the right-most untrusted hop is the client, whatever was prepended.
    #[tokio::test]
    async fn forwarded_for_selects_the_bucket_only_behind_a_trusted_proxy() {
        use crate::domain::abuse::{Network, TrustedProxies};
        let fake = fake_identity("proxies").await;
        let ask = |peer: SocketAddr, proxies: &str| {
            let recorder = Arc::new(Scripted {
                refuse: Some(1),
                seen: Default::default(),
            });
            let router = with_peer(
                state(&fake.socket, fake.uid)
                    .with_limiter(recorder.clone())
                    .with_trusted_proxies(TrustedProxies::parse(proxies).unwrap()),
                peer,
            );
            async move {
                call(
                    &router,
                    "POST",
                    "/api/v1/auth/sign-in/challenges",
                    &[ORIGIN, ("x-forwarded-for", "198.51.100.1, 192.0.2.66")],
                    Some(r#"{"email":"a@vi-du.vn"}"#),
                )
                .await;
                recorder.seen.lock().unwrap()[0]
            }
        };
        let ip = |raw: &str| raw.parse::<std::net::IpAddr>().unwrap();
        let direct = SocketAddr::from(PEER);
        assert_eq!(ask(direct, "").await, Network::of(ip("203.0.113.9")));
        assert_eq!(
            ask(direct, "10.0.0.0/8").await,
            Network::of(ip("203.0.113.9")),
            "an untrusted peer's header is ignored"
        );
        let proxy = SocketAddr::from(([10, 0, 0, 5], 40_000));
        assert_eq!(
            ask(proxy, "10.0.0.0/8").await,
            Network::of(ip("192.0.2.66"))
        );
    }

    /// Production activation stays explicit: with sign-in disabled no session can start, but an
    /// existing one is still verified and can log out. Signing in again revokes the session the
    /// browser held before.
    #[tokio::test]
    async fn disabled_sign_in_keeps_logout_and_a_new_sign_in_replaces_the_old_session() {
        let fake = fake_identity("activation").await;
        let closed = with_peer(
            state(&fake.socket, fake.uid).with_sign_in_enabled(false),
            SocketAddr::from(PEER),
        );
        for (path, body) in [
            (
                "/api/v1/auth/sign-in/challenges",
                r#"{"email":"a@vi-du.vn"}"#,
            ),
            ("/api/v1/auth/sign-in/complete", r#"{"code":"123456"}"#),
        ] {
            let (got, _, body) = call(&closed, "POST", path, &[ORIGIN], Some(body)).await;
            assert_eq!(got, StatusCode::NOT_IMPLEMENTED, "{path}: {body}");
            assert!(body.contains("authority_not_configured"), "{body}");
        }
        let (got, _, _) = call(
            &closed,
            "DELETE",
            "/api/v1/auth/session",
            &[
                ORIGIN,
                ("cookie", "__Host-vink_session=vink_s1_isolated-logout"),
            ],
            None,
        )
        .await;
        assert_eq!(got, StatusCode::NO_CONTENT);
        let (got, _, body) = call(
            &closed,
            "GET",
            "/api/v1/auth/session",
            &[("cookie", "__Host-vink_session=bound")],
            None,
        )
        .await;
        assert_eq!(got, StatusCode::OK);
        assert!(body.contains(r#""state":"active""#), "{body}");

        let (got, headers, _) = call(
            &app(&fake.socket, fake.uid),
            "POST",
            "/api/v1/auth/sign-in/complete",
            &[
                ORIGIN,
                (
                    "cookie",
                    "__Host-vink_sign_in=bound-subject; __Host-vink_session=vink_s1_isolated-old",
                ),
            ],
            Some(r#"{"code":"123456"}"#),
        )
        .await;
        assert_eq!(got, StatusCode::OK);
        assert!(
            set_cookies(&headers)
                .iter()
                .any(|c| c.starts_with("__Host-vink_session=vink_s1_for-bound-subject"))
        );
        let replaced = REPLACED.lock().unwrap().clone();
        assert!(
            replaced.contains(&"vink_s1_isolated-logout".to_string())
                && replaced.contains(&"vink_s1_isolated-old".to_string()),
            "{replaced:?}"
        );
    }

    #[tokio::test]
    async fn sign_in_is_closed_without_an_identity_service() {
        let pool = lazy_pool("postgres://vink:unused@127.0.0.1:1/vink").unwrap();
        let router = router(AppState::new(
            Vink::new(PgStore::new(pool)),
            Arc::new(crate::identity::NoAuthority),
            "http://127.0.0.1:3000",
            Path::new("target/site"),
        ));
        for (method, path, body) in [
            (
                "POST",
                "/api/v1/auth/sign-in/challenges",
                Some(r#"{"email":"a@vi-du.vn"}"#),
            ),
            (
                "POST",
                "/api/v1/auth/sign-in/complete",
                Some(r#"{"code":"123456"}"#),
            ),
        ] {
            let (got, _, body) = call(&router, method, path, &[ORIGIN], body).await;
            assert_eq!(got, StatusCode::NOT_IMPLEMENTED, "{path}: {body}");
            assert!(body.contains("authority_not_configured"));
        }
        let (got, _, body) = call(
            &router,
            "GET",
            "/api/v1/auth/session",
            &[("cookie", "__Host-vink_session=x")],
            None,
        )
        .await;
        assert_eq!(
            (got, body.as_str()),
            (StatusCode::OK, r#"{"state":"anonymous"}"#)
        );
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
            ("suspended", 401, "unauthenticated"),
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
