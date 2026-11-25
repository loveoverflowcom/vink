//! HTTP intake for work views (`POST /api/v1/works/{work_id}/views`, #343, `VIEW-01..06`,
//! `VIEW-08`, `VIEW-10..11`).
//!
//! Validates input, ignores speculative prefetch/prerender requests, enforces the in-memory
//! per-network [`INGEST_LIMIT`], resolves member vs `__Host-vink_guest` identity without renewing
//! unexpired guest tokens (`CHOICE-03`), and answers uniformly with `204 No Content` (`no-store`)
//! across `Counted`, `Skipped`, `Replayed`, and `Ignored` so existence and private states never
//! leak.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::body::to_bytes;
use axum::extract::{ConnectInfo, Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use time::OffsetDateTime;

use super::{ApiError, AppState, client_network, cookie, single_cookie};
use crate::application::work_view::{RecordWorkView, Viewer};
use crate::domain::abuse::Network;
use crate::domain::ids::WorkId;
use crate::domain::work_view::{
    GUEST_TOKEN_TTL, GuestToken, INGEST_LIMIT, ViewEventId, is_speculative,
};
use crate::identity::Resolution;

/// First-party browser guest cookie (`CHOICE-03`), matching [`GUEST_TOKEN_TTL`] without renewal.
pub const GUEST_COOKIE: &str = "__Host-vink_guest";

const MAX_VIEW_BODY_BYTES: usize = 1024;
const MAX_LIMITER_ENTRIES: usize = 16_384;
const PURPOSE_HEADERS: [&str; 3] = ["sec-purpose", "purpose", "x-purpose"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordWorkViewRequest {
    event_id: String,
}

#[derive(Default)]
pub struct IngestLimiter {
    state: Mutex<LimiterState>,
}

#[derive(Default)]
struct LimiterState {
    windows: HashMap<[u8; 32], Window>,
    next_prune_at: Option<Instant>,
}

struct Window {
    started: Instant,
    seen: u32,
}

impl IngestLimiter {
    pub fn admit(&self, network: &Network) -> bool {
        self.admit_at(network, Instant::now())
    }

    pub fn admit_at(&self, network: &Network, now: Instant) -> bool {
        let window_duration = Duration::from_secs(u64::from(INGEST_LIMIT.window_seconds));
        let key = network.key("work_view.ingest");
        let mut guard = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if guard.next_prune_at.is_none_or(|at| now >= at)
            || guard.windows.len() >= MAX_LIMITER_ENTRIES
        {
            guard
                .windows
                .retain(|_, w| now.saturating_duration_since(w.started) < window_duration);
            guard.next_prune_at = Some(now + window_duration);
        }
        if !guard.windows.contains_key(&key)
            && guard.windows.len() >= MAX_LIMITER_ENTRIES
            && let Some(oldest_key) = guard
                .windows
                .iter()
                .min_by_key(|(_, w)| w.started)
                .map(|(k, _)| *k)
        {
            guard.windows.remove(&oldest_key);
        }
        let entry = guard.windows.entry(key).or_insert(Window {
            started: now,
            seen: 0,
        });
        if now.saturating_duration_since(entry.started) >= window_duration {
            entry.started = now;
            entry.seen = 0;
        }
        if !INGEST_LIMIT.admits(entry.seen) {
            return false;
        }
        entry.seen = entry.seen.saturating_add(1);
        true
    }
}

fn no_content(issue_guest_cookie: Option<GuestToken>) -> Response {
    let mut response = (
        StatusCode::NO_CONTENT,
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
    )
        .into_response();
    if let Some(token) = issue_guest_cookie
        && let Some(value) = cookie(
            GUEST_COOKIE,
            &token.expose(),
            "Lax",
            GUEST_TOKEN_TTL.whole_seconds(),
        )
    {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

pub(super) async fn record_work_view(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    request: Request,
) -> Result<Response, ApiError> {
    let work_id = WorkId::parse(&raw).ok_or(ApiError::NotFound)?;
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_VIEW_BODY_BYTES)
        .await
        .map_err(|_| ApiError::Validation {
            field: "body",
            message: "invalid or oversized JSON request body".into(),
        })?;
    let body: RecordWorkViewRequest =
        serde_json::from_slice(&bytes).map_err(|_| ApiError::Validation {
            field: "body",
            message: "invalid or oversized JSON request body".into(),
        })?;
    if ViewEventId::parse(&body.event_id).is_none() {
        return Err(ApiError::Validation {
            field: "event_id",
            message: "invalid event_id".into(),
        });
    }

    let speculative = is_speculative(
        PURPOSE_HEADERS
            .iter()
            .flat_map(|name| parts.headers.get_all(*name).iter())
            .filter_map(|value| value.to_str().ok()),
    );
    if speculative {
        return Ok(no_content(None));
    }

    let Some(digest_key) = state.work_view_digest_key.as_ref() else {
        tracing::error!("work view intake disabled: VINK_WORK_VIEW_DIGEST_KEY is unconfigured");
        return Err(ApiError::DependencyUnavailable);
    };

    let peer = parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0);
    let Some(network) = client_network(&state, &parts.headers, peer) else {
        tracing::error!("work view request without a peer address; refused");
        return Err(ApiError::DependencyUnavailable);
    };
    if !state.work_view_limiter.admit(&network) {
        return Ok(no_content(None));
    }

    let actor = match state.identity.resolve(&parts, peer).await {
        Resolution::Unavailable => return Err(ApiError::DependencyUnavailable),
        other => other.principal(),
    };
    let (viewer, issue_guest_cookie) = match actor {
        Some(principal) => (Viewer::Member(principal), None),
        None => match single_cookie(&parts.headers, GUEST_COOKIE).and_then(GuestToken::parse) {
            Some(token) => (Viewer::Guest(token.expose()), None),
            None => {
                let fresh = GuestToken::generate();
                (Viewer::Guest(fresh.expose()), Some(fresh))
            }
        },
    };

    state
        .vink
        .record_work_view(
            digest_key,
            viewer,
            RecordWorkView {
                work_id,
                event_id: body.event_id,
            },
            OffsetDateTime::now_utc(),
        )
        .await?;

    Ok(no_content(issue_guest_cookie))
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
    use std::path::Path;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use axum::body::Body;
    use axum::extract::ConnectInfo;
    use axum::http::{Request, StatusCode, header};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;
    use crate::application::Vink;
    use crate::domain::work_view::ViewDigestKey;
    use crate::http::{AppState, router};
    use crate::identity::{IdentityResolver, NoAuthority, Resolution, ResolveFuture};
    use crate::persistence::lazy_pool;

    const WORK: &str = "33333333-3333-4333-8333-333333333333";
    const EVENT_V7: &str = "0193a000-0000-7000-8000-000000000001";
    const KEY: ViewDigestKey = ViewDigestKey::from_bytes([0x2a; 32]);

    fn peer(ip: &str) -> SocketAddr {
        SocketAddr::new(ip.parse::<IpAddr>().unwrap(), 4000)
    }

    fn post_view(
        path: &str,
        origin: Option<&str>,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Request<Body> {
        let mut req = Request::post(path).header(header::CONTENT_TYPE, "application/json");
        if let Some(origin) = origin {
            req = req.header(header::ORIGIN, origin);
        }
        for (name, val) in headers {
            req = req.header(*name, *val);
        }
        req.body(Body::from(body.to_string())).unwrap()
    }

    #[test]
    fn ingest_limiter_caps_per_network_and_resets_after_window() {
        let limiter = IngestLimiter::default();
        let net_a = Network::of(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let net_b = Network::of(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let v6_a = Network::of(IpAddr::V6("2001:db8:1:2::1".parse::<Ipv6Addr>().unwrap()));
        let v6_same_prefix =
            Network::of(IpAddr::V6("2001:db8:1:2::99".parse::<Ipv6Addr>().unwrap()));

        let t0 = Instant::now();
        for n in 0..INGEST_LIMIT.requests {
            assert!(limiter.admit_at(&net_a, t0), "request {n} admitted");
        }
        assert!(!limiter.admit_at(&net_a, t0), "request 121 dropped");
        assert!(limiter.admit_at(&net_b, t0), "distinct IPv4 unaffected");

        for n in 0..INGEST_LIMIT.requests {
            assert!(
                limiter.admit_at(&v6_a, t0),
                "v6 request {n} within /64 admitted"
            );
        }
        assert!(
            !limiter.admit_at(&v6_same_prefix, t0),
            "same IPv6 /64 prefix shares bucket"
        );

        let after_window = t0 + Duration::from_secs(u64::from(INGEST_LIMIT.window_seconds));
        assert!(limiter.admit_at(&net_a, after_window));
    }

    #[tokio::test]
    async fn view_intake_validates_enforces_origin_ignores_speculative_and_fails_closed() {
        let pool = lazy_pool("postgres://vink:unused@127.0.0.1:1/vink").unwrap();
        let state_without_key = AppState::new(
            Vink::new(crate::persistence::PgStore::new(pool.clone())),
            Arc::new(NoAuthority),
            "http://127.0.0.1:3000",
            Path::new("target/site"),
        );
        let state_with_key = state_without_key
            .clone()
            .with_work_view_digest_key(Some(KEY));
        let app_no_key =
            router(state_without_key).layer(axum::Extension(ConnectInfo(peer("127.0.0.1"))));
        let app = router(state_with_key).layer(axum::Extension(ConnectInfo(peer("127.0.0.1"))));
        let valid_body = format!(r#"{{"event_id":"{EVENT_V7}"}}"#);
        let path = format!("/api/v1/works/{WORK}/views");

        // Origin checks come before everything else.
        let resp = app
            .clone()
            .oneshot(post_view(&path, None, &[], &valid_body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        let resp = app
            .clone()
            .oneshot(post_view(
                "/api/v1/works/not-a-uuid/views",
                Some("http://127.0.0.1:3000"),
                &[],
                &valid_body,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // Invalid JSON, extra fields, and non-UUIDv7 event IDs fail 422 no-store before DB.
        for bad in [
            r#"{}"#,
            r#"{"event_id":"not-a-uuid"}"#,
            r#"{"event_id":"44444444-4444-4444-8444-444444444444"}"#,
            &format!(r#"{{"event_id":"{EVENT_V7}","extra":1}}"#),
        ] {
            let resp = app
                .clone()
                .oneshot(post_view(&path, Some("http://127.0.0.1:3000"), &[], bad))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
            assert_eq!(resp.headers()[header::CACHE_CONTROL], "no-store");
            assert!(resp.headers().get(header::SET_COOKIE).is_none());
        }

        // Speculative requests return 204 silently without DB or Set-Cookie.
        for (hdr, val) in [
            ("Sec-Purpose", "prefetch"),
            ("Purpose", "prefetch"),
            ("X-Purpose", "prerender"),
        ] {
            let resp = app
                .clone()
                .oneshot(post_view(
                    &path,
                    Some("http://127.0.0.1:3000"),
                    &[(hdr, val)],
                    &valid_body,
                ))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::NO_CONTENT, "{hdr}: {val}");
            assert_eq!(resp.headers()[header::CACHE_CONTROL], "no-store");
            assert!(resp.headers().get(header::SET_COOKIE).is_none());
        }

        // Unconfigured key fails closed 503 without DB or Set-Cookie.
        let resp = app_no_key
            .oneshot(post_view(
                &path,
                Some("http://127.0.0.1:3000"),
                &[],
                &valid_body,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.headers()[header::CACHE_CONTROL], "no-store");
        assert!(resp.headers().get(header::SET_COOKIE).is_none());

        // Configured key against unreachable DB returns 503 no-store without Set-Cookie.
        let resp = app
            .oneshot(post_view(
                &path,
                Some("http://127.0.0.1:3000"),
                &[],
                &valid_body,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.headers()[header::CACHE_CONTROL], "no-store");
        assert!(resp.headers().get(header::SET_COOKIE).is_none());
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        assert!(String::from_utf8_lossy(&bytes).contains("dependency_unavailable"));
    }

    #[tokio::test]
    async fn identity_unavailable_fails_closed_instead_of_falling_back_to_guest() {
        struct UnavailableIdentity;
        impl IdentityResolver for UnavailableIdentity {
            fn resolve<'a>(
                &'a self,
                _parts: &'a axum::http::request::Parts,
                _peer: Option<SocketAddr>,
            ) -> ResolveFuture<'a> {
                Box::pin(std::future::ready(Resolution::Unavailable))
            }
        }

        let pool = lazy_pool("postgres://vink:unused@127.0.0.1:1/vink").unwrap();
        let state = AppState::new(
            Vink::new(crate::persistence::PgStore::new(pool)),
            Arc::new(UnavailableIdentity),
            "http://127.0.0.1:3000",
            Path::new("target/site"),
        )
        .with_work_view_digest_key(Some(KEY));
        let app = router(state).layer(axum::Extension(ConnectInfo(peer("127.0.0.1"))));
        let path = format!("/api/v1/works/{WORK}/views");
        let resp = app
            .oneshot(post_view(
                &path,
                Some("http://127.0.0.1:3000"),
                &[],
                &format!(r#"{{"event_id":"{EVENT_V7}"}}"#),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.headers()[header::CACHE_CONTROL], "no-store");
        assert!(resp.headers().get(header::SET_COOKIE).is_none());
    }
}
