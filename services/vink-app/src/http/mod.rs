//! HTTP adapter: API v1, health probes and the Leptos SSR host on one Axum router.

mod api;
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
use crate::identity::IdentityResolver;
use crate::persistence::PgStore;

#[derive(Clone)]
pub struct AppState {
    pub vink: Arc<Vink<PgStore>>,
    pub identity: Arc<dyn IdentityResolver>,
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
            public_origin: Arc::from(public_origin),
            leptos,
        }
    }
}

/// The authenticated principal of a request, as decided by the server-side resolver.
pub struct Actor(pub Option<PrincipalId>);

impl FromRequestParts<AppState> for Actor {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0);
        Ok(Self(state.identity.resolve(parts, peer)))
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
    use axum::http::{Request, StatusCode};
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
        assert!(body.contains(r#""discourse":"not_implemented""#), "{body}");

        let (status, body) = get(&router, "/api/v1/works").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.contains("dependency_unavailable"), "{body}");

        let (status, body) = get(&router, "/").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.contains("Tạm thời chưa đọc được"), "{body}");
        assert!(!body.contains("Chưa có tác phẩm nào"), "{body}");

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
}
