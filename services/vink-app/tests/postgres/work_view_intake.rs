//! HTTP intake for work views (`POST /api/v1/works/{work_id}/views`, #343) on real PostgreSQL,
//! wired end-to-end through the Axum router, Identity/guest cookie mapping, `INGEST_LIMIT`,
//! publication changes (`request-public`, `set-private`, `O-17@r2` review rejection, `r2` upgrade),
//! public `Reader` ETag invariance, and private `GET /api/v1/author/works` readback (#342).

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::request::Parts;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;
use vink_app::domain::work_view::{GuestToken, INGEST_LIMIT, ViewDigestKey};
use vink_app::evaluation_worker::EvaluationWorker;
use vink_app::http::{AppState, router};
use vink_app::identity::{
    IdentityResolver, Resolution, ResolveFuture, SESSION_COOKIE, presented_credential,
};
use vink_app::persistence::moderation::add_admin_seat;

use super::work_view::{event_at, new_member};
use super::*;

const KEY: ViewDigestKey = ViewDigestKey::from_bytes([0x6b; 32]);
const GUEST_COOKIE: &str = "__Host-vink_guest";
const AUTHOR_WORKS: &str = "/api/v1/author/works?limit=100";

struct CookieIdentity {
    sessions: HashMap<String, Resolution>,
}

impl IdentityResolver for CookieIdentity {
    fn resolve<'a>(&'a self, parts: &'a Parts, _peer: Option<SocketAddr>) -> ResolveFuture<'a> {
        let answer = match presented_credential(parts) {
            Some("outage") => Resolution::Unavailable,
            Some(token) => self
                .sessions
                .get(token)
                .cloned()
                .unwrap_or(Resolution::Anonymous),
            None => Resolution::Anonymous,
        };
        Box::pin(std::future::ready(answer))
    }
}

/// A router whose identity resolves every request as a guest (no sessions).
pub(super) fn guest_app(pool: &PgPool) -> Router {
    let identity = Arc::new(CookieIdentity {
        sessions: HashMap::new(),
    });
    app_with_peer(pool, identity, Ipv4Addr::new(203, 0, 113, 23)).1
}

fn app_with_peer(
    pool: &PgPool,
    identity: Arc<dyn IdentityResolver>,
    peer_ip: Ipv4Addr,
) -> (AppState, Router) {
    let state = AppState::new(
        Vink::new(PgStore::new(pool.clone())),
        identity,
        ORIGIN,
        Path::new("target/site"),
    )
    .with_work_view_digest_key(Some(KEY));
    let app = router(state.clone()).layer(axum::Extension(ConnectInfo(SocketAddr::new(
        IpAddr::V4(peer_ip),
        4200,
    ))));
    (state, app)
}

pub(super) async fn post_view_req(
    app: &Router,
    work_id: Uuid,
    event_id: &str,
    cookie_header: Option<&str>,
) -> (StatusCode, Vec<String>, Option<String>, String) {
    let mut builder = Request::post(format!("/api/v1/works/{work_id}/views"))
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie_header {
        builder = builder.header(header::COOKIE, cookie);
    }
    let response = app
        .clone()
        .oneshot(
            builder
                .body(Body::from(format!(r#"{{"event_id":"{event_id}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let cache = response
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let set_cookies: Vec<String> = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(str::to_string)
        .collect();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        set_cookies,
        cache,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

async fn author_work_views(app: &Router, work_id: WorkId, session_cookie: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::get(AUTHOR_WORKS)
                .header(header::COOKIE, format!("{SESSION_COOKIE}={session_cookie}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    json["works"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["work_id"] == work_id.as_uuid().to_string())
        .unwrap()["views"]
        .clone()
}

fn extract_guest_cookie(set_cookies: &[String]) -> Option<String> {
    assert_eq!(
        set_cookies.len(),
        1,
        "expected 1 Set-Cookie, got {set_cookies:?}"
    );
    let header_val = &set_cookies[0];
    assert!(
        header_val.contains("Path=/")
            && header_val.contains("Secure")
            && header_val.contains("HttpOnly")
            && header_val.contains("SameSite=Lax")
            && header_val.contains("Max-Age=86400"),
        "cookie attributes mismatch: {header_val}"
    );
    header_val
        .strip_prefix(&format!("{GUEST_COOKIE}="))
        .and_then(|rest| rest.split(';').next())
        .map(str::to_string)
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn http_view_intake_drives_guest_member_dedup_retry_privacy_and_author_readback(
    pool: PgPool,
) {
    let vink = seeded(&pool).await;
    let work = WorkId::from_uuid(dev_seed::SAMPLE_WORK);
    let reader_a_id = new_member(&pool, "[Mẫu] Độc giả A").await;
    let suspended_id = new_member(&pool, "[Mẫu] Độc giả đình chỉ").await;
    let moderator_id = new_member(&pool, "[Mẫu] Quản trị duyệt").await;

    let sessions = HashMap::from([
        (
            "sess-author".into(),
            Resolution::Principal(author().unwrap()),
        ),
        (
            "sess-reader-a".into(),
            Resolution::Principal(PrincipalId::from_uuid(reader_a_id)),
        ),
        (
            "sess-admin".into(),
            Resolution::Principal(PrincipalId::from_uuid(moderator_id)),
        ),
        (
            "sess-suspended".into(),
            Resolution::Suspended(PrincipalId::from_uuid(suspended_id)),
        ),
        (
            "sess-suspended-author".into(),
            Resolution::Suspended(author().unwrap()),
        ),
    ]);
    let (_, app) = app_with_peer(
        &pool,
        Arc::new(CookieIdentity { sessions }),
        Ipv4Addr::new(198, 51, 100, 10),
    );

    // Public Reader initial ETag before any views.
    let reader_path = format!("/api/v1/works/{}/reader", work.as_uuid());
    let reader_res = app
        .clone()
        .oneshot(Request::get(&reader_path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(reader_res.status(), StatusCode::OK);
    let initial_etag = reader_res.headers()[header::ETAG].clone();
    assert!(reader_res.headers().get(header::SET_COOKIE).is_none());

    // Author before any decided event -> not_started.
    let initial_views = author_work_views(&app, work, "sess-author").await;
    assert_eq!(initial_views["state"], "not_started");

    // 1. Author self-view -> 204, no guest cookie minted, +0 (note: first decided event starts counting).
    let now = OffsetDateTime::now_utc();
    let (status, cookies, cache, body) = post_view_req(
        &app,
        work.as_uuid(),
        &event_at(now),
        Some(&format!("{SESSION_COOKIE}=sess-author")),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert!(cookies.is_empty(), "member must not mint guest cookie");
    assert!(body.is_empty());
    let after_self = author_work_views(&app, work, "sess-author").await;
    assert_eq!(after_self["state"], "counted");
    assert_eq!(after_self["count"], 0);

    // 2. Guest without cookie -> 204 + mints __Host-vink_guest UUIDv4 (86400s), total -> 1.
    let retry_event = event_at(now);
    let (status, cookies, cache, _) = post_view_req(&app, work.as_uuid(), &retry_event, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(cache.as_deref(), Some("no-store"));
    let g1 = extract_guest_cookie(&cookies).expect("guest token issued");
    assert!(GuestToken::parse(&g1).is_some(), "expected UUIDv4: {g1}");
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        1
    );

    // 3. Same valid guest cookie within window -> 204, NO Set-Cookie (never renewed!), +0.
    let (status, cookies, _, _) = post_view_req(
        &app,
        work.as_uuid(),
        &event_at(now),
        Some(&format!("{GUEST_COOKIE}={g1}")),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        cookies.is_empty(),
        "valid unexpired guest cookie must never be renewed: {cookies:?}"
    );
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        1
    );

    // 4. Retry with SAME event_id even without cookie (lost ACK) -> 204, idempotent +0.
    let (status, _, _, _) = post_view_req(&app, work.as_uuid(), &retry_event, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        1
    );

    // 5. Malformed or duplicated guest cookies -> replaced with fresh valid cookie cleanly (+1).
    let (status, cookies, _, _) = post_view_req(
        &app,
        work.as_uuid(),
        &event_at(now),
        Some(&format!("{GUEST_COOKIE}=bad-token; {GUEST_COOKIE}={g1}")),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let g2 = extract_guest_cookie(&cookies).expect("fresh guest cookie");
    assert_ne!(g2, g1);
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        2
    );

    // 6. Member A views -> +1 (total 3), no guest cookie; second event by A -> +0.
    let (status, cookies, _, _) = post_view_req(
        &app,
        work.as_uuid(),
        &event_at(now),
        Some(&format!("{SESSION_COOKIE}=sess-reader-a")),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(cookies.is_empty());
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        3
    );
    let (status, _, _, _) = post_view_req(
        &app,
        work.as_uuid(),
        &event_at(now),
        Some(&format!("{SESSION_COOKIE}=sess-reader-a")),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        3
    );

    // 7. Public Reader ETag / 304 stays untouched after view increments.
    let conditional = app
        .clone()
        .oneshot(
            Request::get(&reader_path)
                .header(header::IF_NONE_MATCH, initial_etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(conditional.status(), StatusCode::NOT_MODIFIED);

    // 8. Author publishes revision r2 -> total stays 3 across revisions.
    let snap = vink
        .capture_snapshot(author(), work, "intake-snap-r2", 1, 1)
        .await
        .unwrap();
    let req_pub = vink
        .request_public(author(), work, "intake-pub-r2", snap.revision_id, 2)
        .await
        .unwrap();
    sqlx::query("UPDATE work_evaluation_jobs SET scenario = 'approve' WHERE work_id = $1 AND state = 'open'")
        .bind(work.as_uuid())
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        EvaluationWorker::run_once(&pool, "intake-worker")
            .await
            .unwrap()
    );
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        3
    );

    // 9. Work becomes private -> NotPublic and unknown work_id return indistinguishable 204 headers/body.
    let hide_gen = vink
        .set_private(author(), work, "intake-hide", req_pub.generation.get())
        .await
        .unwrap()
        .generation
        .get();
    let (s_hidden, c_hidden, cache_hidden, body_hidden) = post_view_req(
        &app,
        work.as_uuid(),
        &event_at(now),
        Some(&format!("{GUEST_COOKIE}={g2}")),
    )
    .await;
    let unknown_work = Uuid::now_v7();
    let (s_missing, c_missing, cache_missing, body_missing) = post_view_req(
        &app,
        unknown_work,
        &event_at(now),
        Some(&format!("{GUEST_COOKIE}={g2}")),
    )
    .await;
    assert_eq!(
        (s_hidden, &c_hidden, &cache_hidden, &body_hidden),
        (s_missing, &c_missing, &cache_missing, &body_missing)
    );
    assert_eq!(s_hidden, StatusCode::NO_CONTENT);
    // Author still reads retained count privately (OQ-1 = A).
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        3
    );

    // 10. Republish already-approved r2 (restores public immediately) -> admin rejects revision (O-17@r2 review withdrawal) -> +0 on view, retains count; republish r3 -> +1.
    vink.request_public(author(), work, "intake-repub", snap.revision_id, hide_gen)
        .await
        .unwrap();

    {
        let mut conn = pool.acquire().await.unwrap();
        assert!(
            add_admin_seat(
                &mut conn,
                "intake-test",
                PrincipalId::from_uuid(moderator_id),
                None,
            )
            .await
            .unwrap()
        );
    }
    let open_case = app
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/v1/moderation/works/{}/revisions/{}/cases",
                work.as_uuid(),
                snap.revision_id.as_uuid()
            ))
            .header(header::ORIGIN, ORIGIN)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, format!("{SESSION_COOKIE}=sess-admin"))
            .body(Body::from("{}"))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(open_case.status(), StatusCode::CREATED);
    let case_json: Value =
        serde_json::from_slice(&open_case.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let case_id = case_json["case_id"].as_str().unwrap();

    let claim_resp = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/moderation/works/cases/{case_id}/claim"))
                .header(header::ORIGIN, ORIGIN)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, format!("{SESSION_COOKIE}=sess-admin"))
                .body(Body::from(r#"{"expected_version":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(claim_resp.status(), StatusCode::OK);

    let conclude_resp = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/moderation/works/cases/{case_id}/conclude"))
                .header(header::ORIGIN, ORIGIN)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, format!("{SESSION_COOKIE}=sess-admin"))
                .body(Body::from(
                    r#"{"expected_version":2,"verdict":"rejected","reason":"Rút bản thử"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(conclude_resp.status(), StatusCode::OK);

    let (status, _, _, _) = post_view_req(&app, work.as_uuid(), &event_at(now), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        3
    );

    vink.save_draft(
        author(),
        save(
            work,
            1,
            "[Mẫu] Tập thơ sau rút",
            doc(&[&["Khổ mới chờ duyệt."]]),
        ),
    )
    .await
    .unwrap();
    let snap3 = vink
        .capture_snapshot(author(), work, "intake-snap-r3", 2, 1)
        .await
        .unwrap();
    let current_gen: i64 =
        sqlx::query_scalar("SELECT generation FROM work_publications WHERE work_id = $1")
            .bind(work.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    vink.request_public(
        author(),
        work,
        "intake-pub-r3",
        snap3.revision_id,
        current_gen,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE work_evaluation_jobs SET scenario = 'approve' WHERE work_id = $1 AND state = 'open'")
        .bind(work.as_uuid())
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        EvaluationWorker::run_once(&pool, "intake-worker-2")
            .await
            .unwrap()
    );
    let (status, _, _, _) = post_view_req(&app, work.as_uuid(), &event_at(now), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        4
    );

    // 11. Suspended principal still resolves as Viewer::Member (CHOICE-03): counts once by
    // PrincipalId without minting a guest cookie, deduplicates repeat views, and suspended author
    // self-views are skipped (+0).
    let (status, cookies, _, _) = post_view_req(
        &app,
        work.as_uuid(),
        &event_at(now),
        Some(&format!("{SESSION_COOKIE}=sess-suspended")),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        cookies.is_empty(),
        "suspended member must not mint guest cookie: {cookies:?}"
    );
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        5
    );
    let (status, cookies, _, _) = post_view_req(
        &app,
        work.as_uuid(),
        &event_at(now),
        Some(&format!("{SESSION_COOKIE}=sess-suspended")),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(cookies.is_empty());
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        5
    );
    let (status, cookies, _, _) = post_view_req(
        &app,
        work.as_uuid(),
        &event_at(now),
        Some(&format!("{SESSION_COOKIE}=sess-suspended-author")),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(cookies.is_empty());
    assert_eq!(
        author_work_views(&app, work, "sess-author").await["count"],
        5
    );

    // 12. Unconfigured/outage Identity fails closed 503 with same event_id, without minting guest cookie.
    let req_outage = Request::post(format!("/api/v1/works/{}/views", work.as_uuid()))
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("{SESSION_COOKIE}=outage"))
        .body(Body::from(format!(r#"{{"event_id":"{}"}}"#, event_at(now))))
        .unwrap();
    let outage_resp = app.clone().oneshot(req_outage).await.unwrap();
    assert_eq!(outage_resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(outage_resp.headers()[header::CACHE_CONTROL], "no-store");
    assert!(outage_resp.headers().get(header::SET_COOKIE).is_none());
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn http_ingest_limit_drops_over_ceiling_silently_without_touching_database(pool: PgPool) {
    let _ = seeded(&pool).await;
    let work = WorkId::from_uuid(dev_seed::SAMPLE_WORK);
    let empty_sessions = Arc::new(CookieIdentity {
        sessions: HashMap::new(),
    });
    let (state, app_peer_a) = app_with_peer(&pool, empty_sessions, Ipv4Addr::new(198, 51, 100, 50));
    let now = OffsetDateTime::now_utc();

    for _ in 0..INGEST_LIMIT.requests {
        let (status, _, _, _) =
            post_view_req(&app_peer_a, work.as_uuid(), &event_at(now), None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    // 121st request from same network returns silent 204 without Set-Cookie or DB insert.
    let (status, cookies, cache, _) =
        post_view_req(&app_peer_a, work.as_uuid(), &event_at(now), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert!(cookies.is_empty());

    let db_total: i64 = sqlx::query_scalar("SELECT views FROM work_view_totals WHERE work_id = $1")
        .bind(work.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_total, i64::from(INGEST_LIMIT.requests));

    // Distinct peer network with the same AppState limiter is still admitted (+1 -> 121).
    let app_peer_b = router(state).layer(axum::Extension(ConnectInfo(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(198, 51, 100, 51)),
        4200,
    ))));
    let (status, cookies, _, _) =
        post_view_req(&app_peer_b, work.as_uuid(), &event_at(now), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(cookies.len(), 1);
    let db_total_after: i64 =
        sqlx::query_scalar("SELECT views FROM work_view_totals WHERE work_id = $1")
            .bind(work.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(db_total_after, i64::from(INGEST_LIMIT.requests) + 1);
}
