use super::{ORIGIN, app, backend_pid, call, json, wait_blocked_by};
use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::collections::BTreeSet;
use tower::ServiceExt;
use uuid::Uuid;
use vink_app::application::Vink;
use vink_app::application::follow::{FollowError, SetFollowInput};
use vink_app::domain::handle::Intent;
use vink_app::domain::ids::PrincipalId;
use vink_app::persistence::{PgStore, handle};

const OWN_SESSION: (&str, &str) = ("cookie", "__Host-vink_session=fixture-own-session-1");
const OTHER_OWN_SESSION: (&str, &str) = ("cookie", "__Host-vink_session=fixture-own-session-2");

async fn seed_author(pool: &PgPool, id: Uuid, pen_name: &str, handle_name: &str) -> PrincipalId {
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, $2)")
        .bind(id)
        .bind(pen_name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO principal_bindings (principal_id, authority, subject_id) VALUES ($1, 'vink-identity', $2)",
    )
    .bind(id)
    .bind(format!("sub-{id}"))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO author_handle_names (principal_id, handle) VALUES ($1, $2)")
        .bind(id)
        .bind(handle_name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO author_profiles (principal_id, current_handle, setup_complete) VALUES ($1, $2, true)",
    )
    .bind(id)
    .bind(handle_name)
    .execute(pool)
    .await
    .unwrap();
    PrincipalId::from_uuid(id)
}

async fn get_with_headers(
    router: &Router,
    path: &str,
    actor: Option<Uuid>,
    headers: &[(&str, &str)],
) -> (StatusCode, Option<String>, Option<String>, String) {
    let mut req = Request::builder().method(Method::GET).uri(path);
    if let Some(id) = actor {
        req = req.header("x-vink-test-principal", id.to_string());
    }
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let resp = router
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let cache = resp
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let etag = resp
        .headers()
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        cache,
        etag,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn follow_body(cmd: Uuid, expected_order: i64, following: bool) -> String {
    json!({
        "command_id": cmd,
        "expected_order": expected_order,
        "following": following,
    })
    .to_string()
}

#[sqlx::test(migrations = "./migrations")]
async fn follow_auth_self_forged_fields_gone_and_suspended_boundaries(pool: PgPool) {
    let app = app(&pool).await;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    seed_author(&pool, a, "An", "an-may").await;
    seed_author(&pool, b, "Bình", "binh-yen").await;
    seed_author(&pool, c, "Châu", "chau-au").await;

    // Guest cannot call PUT /relationship/me (401 no-store).
    let (code, cache, body) = call(
        &app,
        "PUT",
        "/api/v1/authors/binh-yen/relationship/me",
        None,
        Some(ORIGIN),
        Some(follow_body(Uuid::new_v4(), 0, true)),
    )
    .await;
    assert_eq!(
        (code.as_u16(), cache.as_deref()),
        (401, Some("no-store")),
        "{body}"
    );

    // Client-supplied actor_id / follower_id rejected with 422 before mutation.
    for extra in ["actor_id", "follower_id"] {
        let forged = json!({
            "command_id": Uuid::new_v4(),
            "expected_order": 0,
            "following": true,
            extra: a
        })
        .to_string();
        let (code, cache, body) = call(
            &app,
            "PUT",
            "/api/v1/authors/binh-yen/relationship/me",
            Some(c),
            Some(ORIGIN),
            Some(forged),
        )
        .await;
        assert_eq!(
            (code.as_u16(), cache.as_deref()),
            (422, Some("no-store")),
            "{extra}: {body}"
        );
    }

    // Self-follow returns 403 self_follow.
    let (code, cache, body) = call(
        &app,
        "PUT",
        "/api/v1/authors/an-may/relationship/me",
        Some(a),
        Some(ORIGIN),
        Some(follow_body(Uuid::new_v4(), 0, true)),
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (403, Some("no-store")));
    assert_eq!(json(&body)["error"]["code"], "self_follow");

    // Viewer relationship on self returns `kind: "self"`, `can_follow: false`, own counts/hrefs.
    let (code, cache, body) = call(
        &app,
        "GET",
        "/api/v1/authors/an-may/relationship/me",
        Some(a),
        None,
        None,
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (200, Some("no-store")));
    assert_eq!(json(&body)["kind"], "self");
    assert_eq!(json(&body)["can_follow"], false);

    // Target with 0 works is still followable.
    let (code, _, body) = call(
        &app,
        "PUT",
        "/api/v1/authors/binh-yen/relationship/me",
        Some(a),
        Some(ORIGIN),
        Some(follow_body(Uuid::new_v4(), 0, true)),
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(json(&body)["following"], true);

    // Suspended actor -> 403.
    sqlx::query(
        "INSERT INTO principal_suspensions (id, principal_id, reason) VALUES ($1, $2, 'test')",
    )
    .bind(Uuid::new_v4())
    .bind(a)
    .execute(&pool)
    .await
    .unwrap();
    let (code, cache, _) = call(
        &app,
        "PUT",
        "/api/v1/authors/chau-au/relationship/me",
        Some(a),
        Some(ORIGIN),
        Some(follow_body(Uuid::new_v4(), 0, true)),
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (403, Some("no-store")));

    // Suspended target -> viewer sees can_follow=false; PUT returns 409 target_unavailable.
    sqlx::query("DELETE FROM principal_suspensions WHERE principal_id = $1")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO principal_suspensions (id, principal_id, reason) VALUES ($1, $2, 'test')",
    )
    .bind(Uuid::new_v4())
    .bind(c)
    .execute(&pool)
    .await
    .unwrap();
    let (code, _, body) = call(
        &app,
        "GET",
        "/api/v1/authors/chau-au/relationship/me",
        Some(a),
        None,
        None,
    )
    .await;
    assert_eq!(code, 200);
    assert_eq!(json(&body)["can_follow"], false);
    let (code, cache, body) = call(
        &app,
        "PUT",
        "/api/v1/authors/chau-au/relationship/me",
        Some(a),
        Some(ORIGIN),
        Some(follow_body(Uuid::new_v4(), 0, true)),
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (409, Some("no-store")));
    assert_eq!(json(&body)["error"]["code"], "target_unavailable");

    // Rename B so `binh-yen` becomes an alias -> public followers returns `state: "alias"` with no-store.
    let vink = Vink::new(PgStore::new(pool.clone()));
    vink.store()
        .set_handle(vink_app::domain::handle::HandleCommand {
            command_id: Uuid::new_v4(),
            actor: PrincipalId::from_uuid(b),
            expected_version: 0,
            intent: Intent::Rename,
            handle: "binh-moi".into(),
        })
        .await
        .unwrap();
    let (code, cache, _, body) =
        get_with_headers(&app, "/api/v1/authors/binh-yen/followers", None, &[]).await;
    assert_eq!((code.as_u16(), cache.as_deref()), (200, Some("no-store")));
    assert_eq!(json(&body)["state"], "alias");
    assert_eq!(json(&body)["href"], "/@binh-moi/nguoi-theo-doi");

    // Retire B -> old & new handles return 410 profile_gone (no-store) on all follow routes.
    let mut tx = pool.begin().await.unwrap();
    handle::retire(&mut tx, PrincipalId::from_uuid(b))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    for path in [
        "/api/v1/authors/binh-yen/followers",
        "/api/v1/authors/binh-moi/followers",
        "/api/v1/authors/binh-moi/following",
    ] {
        let (code, cache, _, body) = get_with_headers(&app, path, None, &[]).await;
        assert_eq!(
            (code.as_u16(), cache.as_deref()),
            (410, Some("no-store")),
            "{path}: {body}"
        );
        assert_eq!(json(&body)["error"]["code"], "profile_gone");
    }
    let (code, cache, body) = call(
        &app,
        "GET",
        "/api/v1/authors/binh-moi/relationship/me",
        Some(a),
        None,
        None,
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (410, Some("no-store")));
    assert_eq!(json(&body)["error"]["code"], "profile_gone");

    let (code, cache, body) = call(
        &app,
        "PUT",
        "/api/v1/authors/binh-moi/relationship/me",
        Some(a),
        Some(ORIGIN),
        Some(follow_body(Uuid::new_v4(), 1, false)),
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (410, Some("no-store")));
    assert_eq!(json(&body)["error"]["code"], "profile_gone");
}

#[sqlx::test(migrations = "./migrations")]
async fn follow_concurrency_aba_receipts_and_db_triggers(pool: PgPool) {
    let app = app(&pool).await;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    seed_author(&pool, a, "An", "an-con").await;
    seed_author(&pool, b, "Bình", "binh-con").await;

    // Two concurrent writes on the same (A -> B) pair at expected_order = 0:
    // exactly one 200 accepted (order=1) and one 409 version_conflict (observed.order=1).
    let cmd_race_1 = Uuid::new_v4();
    let cmd_race_2 = Uuid::new_v4();
    let (r1, r2) = tokio::join!(
        call(
            &app,
            "PUT",
            "/api/v1/authors/binh-con/relationship/me",
            Some(a),
            Some(ORIGIN),
            Some(follow_body(cmd_race_1, 0, true)),
        ),
        call(
            &app,
            "PUT",
            "/api/v1/authors/binh-con/relationship/me",
            Some(a),
            Some(ORIGIN),
            Some(follow_body(cmd_race_2, 0, true)),
        ),
    );
    let (winner, loser) = match (r1.0.as_u16(), r2.0.as_u16()) {
        (200, 409) => (r1, r2),
        (409, 200) => (r2, r1),
        other => panic!("expected (200, 409), got {other:?}"),
    };
    let winner_cmd = if winner.2 == r1.2 && r1.0 == 200 {
        cmd_race_1
    } else {
        cmd_race_2
    };
    let _ = loser.1;
    let wj = json(&winner.2);
    assert_eq!(wj["outcome"], "accepted");
    assert_eq!(wj["following"], true);
    assert_eq!(wj["pair_version"], 1);
    assert_eq!(wj["accepted_order"], 1);
    let lj = json(&loser.2);
    assert_eq!(lj["error"]["code"], "version_conflict");
    assert_eq!(lj["error"]["follow"]["following"], true);
    assert_eq!(lj["error"]["follow"]["accepted_order"], 1);

    // Opposite direction concurrently (Unfollow A->B at order 1 vs Follow B->A at order 0):
    // canonical lock order prevents deadlock.
    let (u_ab, f_ba) = tokio::join!(
        call(
            &app,
            "PUT",
            "/api/v1/authors/binh-con/relationship/me",
            Some(a),
            Some(ORIGIN),
            Some(follow_body(Uuid::new_v4(), 1, false)),
        ),
        call(
            &app,
            "PUT",
            "/api/v1/authors/an-con/relationship/me",
            Some(b),
            Some(ORIGIN),
            Some(follow_body(Uuid::new_v4(), 0, true)),
        ),
    );
    assert_eq!(u_ab.0, 200, "{}", u_ab.2);
    assert_eq!(f_ba.0, 200, "{}", f_ba.2);

    // ABA: A already went Follow (order 1, cmd winner_cmd) -> Unfollow (order 2).
    // Now replay `winner_cmd` (old Follow) -> MUST return `outcome: "replayed"` with committed
    // `(true, order 1)` and observed `(false, order 2)` WITHOUT reactivating edge!
    let (code, cache, body) = call(
        &app,
        "PUT",
        "/api/v1/authors/binh-con/relationship/me",
        Some(a),
        Some(ORIGIN),
        Some(follow_body(winner_cmd, 0, true)),
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (200, Some("no-store")));
    let rj = json(&body);
    assert_eq!(rj["outcome"], "replayed");
    assert_eq!(rj["following"], false);
    assert_eq!(rj["pair_version"], 2);
    assert_eq!(rj["accepted_order"], 2);
    assert_eq!(rj["committed"]["following"], true);
    assert_eq!(rj["committed"]["accepted_order"], 1);

    // Receipt GET reports both historical commit and current state.
    let (code, cache, body) = call(
        &app,
        "GET",
        &format!("/api/v1/me/follows/receipts/{winner_cmd}"),
        Some(a),
        None,
        None,
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (200, Some("no-store")));
    let rec = json(&body);
    assert_eq!(rec["committed"]["accepted_order"], 1);
    assert_eq!(rec["observed"]["following"], false);
    assert_eq!(rec["observed"]["accepted_order"], 2);

    // Reusing `winner_cmd` with a different intent -> 422 follow_key_reused.
    let (code, cache, body) = call(
        &app,
        "PUT",
        "/api/v1/authors/binh-con/relationship/me",
        Some(a),
        Some(ORIGIN),
        Some(follow_body(winner_cmd, 0, false)),
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (422, Some("no-store")));
    assert_eq!(json(&body)["error"]["code"], "follow_key_reused");

    // DB constraints & triggers refuse parity break, order regression, DELETE, and receipt mutation.
    assert!(
        sqlx::query(
            "UPDATE author_follows SET accepted_order = 1 WHERE follower_id = $1 AND followee_id = $2",
        )
        .bind(a)
        .bind(b)
        .execute(&pool)
        .await
        .is_err()
    );
    assert!(
        sqlx::query(
            "UPDATE author_follows SET active = true, version = 2, accepted_order = 3 WHERE follower_id = $1 AND followee_id = $2",
        )
        .bind(a)
        .bind(b)
        .execute(&pool)
        .await
        .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM author_follows")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE author_follow_receipts SET desired_following = false")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM author_follow_receipts")
            .execute(&pool)
            .await
            .is_err()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn follow_retire_lock_wait_both_orders_index_scan_and_count_list_equality(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let app = app(&pool).await;

    // Seed target T plus 26 followers so page count = 3 (12 + 12 + 2 = 26).
    let t = Uuid::new_v4();
    let t_id = seed_author(&pool, t, "Trang", "trang-tho").await;
    let mut followers = Vec::new();
    for i in 0..26u32 {
        let u = Uuid::new_v4();
        let handle = format!("doc-gia-{i:02}");
        seed_author(&pool, u, &format!("Độc giả {i:02}"), &handle).await;
        let (code, _, body) = call(
            &app,
            "PUT",
            "/api/v1/authors/trang-tho/relationship/me",
            Some(u),
            Some(ORIGIN),
            Some(follow_body(Uuid::new_v4(), 0, true)),
        )
        .await;
        assert_eq!(code, 200, "{body}");
        followers.push((u, handle));
    }

    // Verify EXPLAIN uses partial index `author_follows_followers_active_idx` when seqscan is off.
    let mut conn = pool.acquire().await.unwrap();
    let mut tx = sqlx::Connection::begin(&mut *conn).await.unwrap();
    sqlx::query("SET LOCAL enable_seqscan = off")
        .execute(&mut *tx)
        .await
        .unwrap();
    let plan: Value = sqlx::query_scalar(
        "EXPLAIN (FORMAT JSON)
         SELECT f.follower_id
           FROM author_follows f
          WHERE f.followee_id = $1 AND f.active
          ORDER BY f.active_since DESC, f.cursor_key ASC
          LIMIT 13",
    )
    .bind(t)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert!(
        plan.to_string()
            .contains("author_follows_followers_active_idx"),
        "expected partial active index scan in plan: {plan}"
    );
    let plan_following: Value = sqlx::query_scalar(
        "EXPLAIN (FORMAT JSON)
         SELECT f.followee_id
           FROM author_follows f
          WHERE f.follower_id = $1 AND f.active
          ORDER BY f.active_since DESC, f.cursor_key ASC
          LIMIT 13",
    )
    .bind(followers[0].0)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert!(
        plan_following
            .to_string()
            .contains("author_follows_following_active_idx"),
        "expected partial active following index scan in plan: {plan_following}"
    );
    tx.rollback().await.unwrap();

    // Paginate all followers and verify exact equality with profile & list count (26).
    let (code, _, prof_body) =
        call(&app, "GET", "/api/v1/authors/trang-tho", None, None, None).await;
    assert_eq!(code, 200);
    assert_eq!(json(&prof_body)["relationships"]["followers_count"], 26);

    let mut seen_handles = BTreeSet::new();
    let mut total_rows = 0usize;
    let mut cursor: Option<String> = None;
    let mut first_cursor: Option<String> = None;
    let mut pages = 0usize;
    loop {
        pages += 1;
        let path = match &cursor {
            Some(c) => format!("/api/v1/authors/trang-tho/followers?cursor={c}"),
            None => "/api/v1/authors/trang-tho/followers".into(),
        };
        let (code, _, _, body) = get_with_headers(&app, &path, None, &[]).await;
        assert_eq!(code, 200, "{body}");
        let v = json(&body);
        assert_eq!(v["count"], 26);
        let people = v["people"].as_array().unwrap();
        total_rows += people.len();
        for p in people {
            assert!(seen_handles.insert(p["handle"].as_str().unwrap().to_owned()));
        }
        match v["next_cursor"].as_str() {
            Some(next) => {
                if first_cursor.is_none() {
                    first_cursor = Some(next.to_owned());
                }
                cursor = Some(next.to_owned());
            }
            None => break,
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(total_rows, 26);
    assert_eq!(seen_handles.len(), 26);

    // Cross-target/cross-scope reuse of a real cursor triggers `state: "restart"`, never leakage.
    let cross = first_cursor.unwrap();
    let (code, cache, _, cross_body) = get_with_headers(
        &app,
        &format!("/api/v1/authors/doc-gia-00/followers?cursor={cross}"),
        None,
        &[],
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (200, Some("no-store")));
    assert_eq!(json(&cross_body)["state"], "restart");
    let (code, cache, _, cross_own) = get_with_headers(
        &app,
        &format!("/api/v1/me/relationships/followers?cursor={cross}"),
        Some(t),
        &[OWN_SESSION],
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (200, Some("no-store")));
    assert_eq!(json(&cross_own)["state"], "restart");

    // A private cursor paginates within the issuing authenticated session only.
    let own_path = "/api/v1/me/relationships/followers";
    let (code, cache, _, first_own) =
        get_with_headers(&app, own_path, Some(t), &[OWN_SESSION]).await;
    assert_eq!((code.as_u16(), cache.as_deref()), (200, Some("no-store")));
    let first_own = json(&first_own);
    assert_eq!(first_own["people"].as_array().unwrap().len(), 12);
    let own_cursor = first_own["next_cursor"].as_str().unwrap();
    let next = format!("{own_path}?cursor={own_cursor}");
    let (code, _, _, page) = get_with_headers(&app, &next, Some(t), &[OWN_SESSION]).await;
    assert_eq!(code, 200);
    let page = json(&page);
    assert_eq!(page["state"], "page");
    assert_eq!(page["people"].as_array().unwrap().len(), 12);
    for person in page["people"].as_array().unwrap() {
        assert!(!first_own["people"].as_array().unwrap().contains(person));
    }
    let (code, cache, _, page) = get_with_headers(&app, &next, Some(t), &[OTHER_OWN_SESSION]).await;
    assert_eq!((code.as_u16(), cache.as_deref()), (200, Some("no-store")));
    assert_eq!(json(&page)["state"], "restart");

    // Order 1: retire(T) locks principals(T) FOR UPDATE while concurrent set_follow waits and fails Gone.
    let contender = Uuid::new_v4();
    let contender_id = seed_author(&pool, contender, "Kiên", "kien-tri").await;
    let mut blocker = pool.begin().await.unwrap();
    let pid = backend_pid(&mut blocker).await;
    sqlx::query("SELECT 1 FROM principals WHERE id = $1 FOR UPDATE")
        .bind(t)
        .fetch_one(&mut *blocker)
        .await
        .unwrap();

    let store_waiter = store.clone();
    let task = tokio::spawn(async move {
        store_waiter
            .set_follow(
                contender_id,
                "trang-tho",
                SetFollowInput {
                    command_id: Uuid::new_v4(),
                    expected_order: 0,
                    following: true,
                },
            )
            .await
    });
    wait_blocked_by(&pool, pid).await;
    handle::retire(&mut blocker, t_id).await.unwrap();
    blocker.commit().await.unwrap();

    let res = task.await.unwrap();
    assert!(matches!(res, Err(FollowError::Gone)), "got {res:?}");

    // Every edge of retired T is now inactive with even version and incremented order.
    let active_left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM author_follows WHERE followee_id = $1 AND active")
            .bind(t)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(active_left, 0);
    // Remaining followers have updated following count (0) excluding retired T.
    let own = store
        .viewer_relationship(PrincipalId::from_uuid(followers[0].0), "doc-gia-00")
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(own).unwrap()["own_relationships"]["following_count"],
        0
    );

    // Partial batch retirement preserves version parity & accepted_order increment across calls.
    let batch_target = Uuid::new_v4();
    let batch_target_id = seed_author(&pool, batch_target, "Bách", "bach-batch").await;
    for i in 0..5u32 {
        let p = Uuid::new_v4();
        seed_author(&pool, p, &format!("Bạn {i}"), &format!("ban-{i:02}")).await;
        let (code, _, _) = call(
            &app,
            "PUT",
            "/api/v1/authors/bach-batch/relationship/me",
            Some(p),
            Some(ORIGIN),
            Some(follow_body(Uuid::new_v4(), 0, true)),
        )
        .await;
        assert_eq!(code, 200);
    }
    let mut btx = pool.begin().await.unwrap();
    assert_eq!(
        vink_app::persistence::follow::retire_edges_batch(&mut btx, batch_target_id, 2)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        vink_app::persistence::follow::retire_edges_batch(&mut btx, batch_target_id, 2)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        vink_app::persistence::follow::retire_edges_batch(&mut btx, batch_target_id, 2)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        vink_app::persistence::follow::retire_edges_batch(&mut btx, batch_target_id, 2)
            .await
            .unwrap(),
        0
    );
    btx.commit().await.unwrap();
    let parities: Vec<(bool, i64, i64)> = sqlx::query_as(
        "SELECT active, version, accepted_order FROM author_follows WHERE followee_id = $1",
    )
    .bind(batch_target)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(parities, vec![(false, 2, 2); 5]);

    // Order 2: set_follow holds pair principal locks first, while concurrent retire(C) waits cleanly.
    let mut lo_id = Uuid::new_v4();
    let mut hi_id = Uuid::new_v4();
    if lo_id > hi_id {
        std::mem::swap(&mut lo_id, &mut hi_id);
    }
    seed_author(&pool, lo_id, "Lộc", "loc-la").await;
    seed_author(&pool, hi_id, "Hiền", "hien-hoa").await;
    let mut follow_blocker = pool.begin().await.unwrap();
    let pid2 = backend_pid(&mut follow_blocker).await;
    sqlx::query("SELECT 1 FROM principals WHERE id = $1 FOR UPDATE")
        .bind(lo_id)
        .fetch_one(&mut *follow_blocker)
        .await
        .unwrap();
    sqlx::query("SELECT 1 FROM principals WHERE id = $1 FOR UPDATE")
        .bind(hi_id)
        .fetch_one(&mut *follow_blocker)
        .await
        .unwrap();

    let pool_retire = pool.clone();
    let retire_task = tokio::spawn(async move {
        let mut rtx = pool_retire.begin().await.unwrap();
        handle::retire(&mut rtx, PrincipalId::from_uuid(lo_id))
            .await
            .unwrap();
        rtx.commit().await.unwrap();
    });
    wait_blocked_by(&pool, pid2).await;
    follow_blocker.commit().await.unwrap();
    retire_task.await.unwrap();
    let after_retire = store
        .set_follow(
            contender_id,
            "loc-la",
            SetFollowInput {
                command_id: Uuid::new_v4(),
                expected_order: 0,
                following: true,
            },
        )
        .await;
    assert!(matches!(after_retire, Err(FollowError::Gone)));
}

#[sqlx::test(migrations = "./migrations")]
async fn follow_policy_matrix_rights_before_cursor_and_mutual_isolation(pool: PgPool) {
    let store = PgStore::new(pool.clone());
    let app = app(&pool).await;

    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let a_id = seed_author(&pool, a, "An", "an-pol").await;
    seed_author(&pool, b, "Bình", "binh-pol").await;
    let c_id = seed_author(&pool, c, "Châu", "chau-pol").await;

    // A & B follow each other (mutual); B also has C follow later -> public followers > 0.
    // A keeps following_list = "self" (default); B has followers_list = "everyone".
    for (actor, target_handle) in [(a, "binh-pol"), (b, "an-pol"), (c, "binh-pol")] {
        let (code, _, _) = call(
            &app,
            "PUT",
            &format!("/api/v1/authors/{target_handle}/relationship/me"),
            Some(actor),
            Some(ORIGIN),
            Some(follow_body(Uuid::new_v4(), 0, true)),
        )
        .await;
        assert_eq!(code, 200);
    }

    // Mutual isolation: A and B see `mutual: true` in viewer overlay; C sees `mutual: false`;
    // guest public endpoints NEVER contain `mutual`.
    let (_, _, avb) = call(
        &app,
        "GET",
        "/api/v1/authors/binh-pol/relationship/me",
        Some(a),
        None,
        None,
    )
    .await;
    assert_eq!(json(&avb)["mutual"], true);
    let (_, _, bva) = call(
        &app,
        "GET",
        "/api/v1/authors/an-pol/relationship/me",
        Some(b),
        None,
        None,
    )
    .await;
    assert_eq!(json(&bva)["mutual"], true);
    let (_, _, cvb) = call(
        &app,
        "GET",
        "/api/v1/authors/binh-pol/relationship/me",
        Some(c),
        None,
        None,
    )
    .await;
    assert_eq!(json(&cvb)["mutual"], false);
    let (_, _, cva) = call(
        &app,
        "GET",
        "/api/v1/authors/an-pol/relationship/me",
        Some(c),
        None,
        None,
    )
    .await;
    assert_eq!(json(&cva)["mutual"], false);

    // Cross-policy: A's public following is 403 (`following_list: self`), yet A appears in B's
    // public followers list (`followers_list: everyone`) AND owns `/me/relationships/following`.
    let (code, cache, _, body) = get_with_headers(
        &app,
        "/api/v1/authors/an-pol/following?cursor=bogus",
        None,
        &[],
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (403, Some("no-store")));
    assert_eq!(json(&body)["error"]["code"], "relationship_list_private");

    for (handle, direction, expected) in
        [("an-pol", "following", 403), ("binh-pol", "followers", 422)]
    {
        let (status, cache, _, _) = get_with_headers(
            &app,
            &format!("/api/v1/authors/{handle}/{direction}?cursor=a&cursor=b"),
            None,
            &[],
        )
        .await;
        assert_eq!(
            (status.as_u16(), cache.as_deref()),
            (expected, Some("no-store"))
        );
    }

    let (code, _, _, body) =
        get_with_headers(&app, "/api/v1/authors/binh-pol/followers", None, &[]).await;
    assert_eq!(code, 200);
    assert!(!body.contains("mutual"));
    assert!(
        json(&body)["people"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["handle"] == "an-pol")
    );

    let (code, cache, _, body) = get_with_headers(
        &app,
        "/api/v1/me/relationships/following",
        Some(a),
        &[OWN_SESSION],
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (200, Some("no-store")));
    let own_following = json(&body);
    assert_eq!(own_following["count"], 1);
    assert_eq!(own_following["people"][0]["author"]["handle"], "binh-pol");
    assert_eq!(own_following["people"][0]["mutual"], true);

    // Policy combination: B sets `followers_list = everyone, followers_count = self`
    // and enable following list (`everyone`) + `following_count = self` to test independent 4-policy matrix.
    sqlx::query(
        r#"UPDATE author_profiles SET policies = '{
            "pen_name":"everyone","avatar":"everyone","bio":"everyone","date_of_birth":"self",
            "followers_list":"everyone","followers_count":"self",
            "following_list":"everyone","following_count":"self"
        }'::jsonb WHERE principal_id = $1"#,
    )
    .bind(b)
    .execute(&pool)
    .await
    .unwrap();
    let (code, _, _, flist) =
        get_with_headers(&app, "/api/v1/authors/binh-pol/followers", None, &[]).await;
    assert_eq!(code, 200);
    assert!(json(&flist).get("count").is_none());
    let (code, _, _, prof) = get_with_headers(&app, "/api/v1/authors/binh-pol", None, &[]).await;
    assert_eq!(code, 200);
    assert_eq!(
        json(&prof)["relationships"]["followers_href"],
        "/@binh-pol/nguoi-theo-doi"
    );
    assert!(
        json(&prof)["relationships"]
            .get("followers_count")
            .is_none()
    );
    assert_eq!(
        json(&prof)["relationships"]["following_href"],
        "/@binh-pol/dang-theo-doi"
    );
    assert!(
        json(&prof)["relationships"]
            .get("following_count")
            .is_none()
    );

    // Capture valid public ETag while followers_list=everyone; also get a valid public cursor if possible.
    let (code, _, etag_opt, _) =
        get_with_headers(&app, "/api/v1/authors/binh-pol/followers", None, &[]).await;
    assert_eq!(code, 200);
    let etag = etag_opt.unwrap();
    // Same ETag returns 304 while list is still public.
    let (code, _, _, _) = get_with_headers(
        &app,
        "/api/v1/authors/binh-pol/followers",
        None,
        &[("if-none-match", &etag)],
    )
    .await;
    assert_eq!(code, 304);

    // Reverse: `followers_list = self, followers_count = everyone`!
    sqlx::query(
        r#"UPDATE author_profiles SET policies = '{
            "pen_name":"everyone","avatar":"everyone","bio":"everyone","date_of_birth":"self",
            "followers_list":"self","followers_count":"everyone",
            "following_list":"self","following_count":"everyone"
        }'::jsonb WHERE principal_id = $1"#,
    )
    .bind(b)
    .execute(&pool)
    .await
    .unwrap();
    // Stale valid ETag MUST receive 403 no-store (rights BEFORE If-None-Match, NEVER 304!).
    let (code, cache, _, body) = get_with_headers(
        &app,
        "/api/v1/authors/binh-pol/followers",
        None,
        &[("if-none-match", &etag)],
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (403, Some("no-store")));
    assert_eq!(json(&body)["error"]["code"], "relationship_list_private");

    // Profile exposes counts WITHOUT hrefs!
    let (code, _, _, prof) = get_with_headers(&app, "/api/v1/authors/binh-pol", None, &[]).await;
    assert_eq!(code, 200);
    assert_eq!(json(&prof)["relationships"]["followers_count"], 2);
    assert!(json(&prof)["relationships"].get("followers_href").is_none());
    assert_eq!(json(&prof)["relationships"]["following_count"], 1);
    assert!(json(&prof)["relationships"].get("following_href").is_none());

    // Cursor mismatch returns `state: "restart"` rather than leaking across scopes/directions.
    let (code, cache, _, body) = get_with_headers(
        &app,
        "/api/v1/me/relationships/followers?cursor=invalid-token",
        Some(a),
        &[OWN_SESSION],
    )
    .await;
    assert_eq!((code.as_u16(), cache.as_deref()), (200, Some("no-store")));
    assert_eq!(json(&body)["state"], "restart");

    // Batch membership API & `followed_principals_in` store handoff (#296/#295).
    let (code, cache, _, body) = get_with_headers(
        &app,
        "/api/v1/me/follows/membership?handles=binh-pol,an-pol,chau-pol,khong-ton-tai",
        Some(a),
        &[],
    )
    .await;
    assert_eq!(
        (code.as_u16(), cache.as_deref()),
        (200, Some("no-store")),
        "{body}"
    );
    let items = json(&body)["items"].as_array().unwrap().clone();
    // `an-pol` is self so omitted; `khong-ton-tai` missing so omitted; `binh-pol` followed + mutual; `chau-pol` not followed.
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["handle"], "binh-pol");
    assert_eq!(items[0]["following"], true);
    assert_eq!(items[0]["mutual"], true);
    assert_eq!(items[1]["handle"], "chau-pol");
    assert_eq!(items[1]["following"], false);

    let set = store
        .followed_principals_in(a_id, &[PrincipalId::from_uuid(b), c_id, a_id])
        .await
        .unwrap();
    assert!(set.contains(&PrincipalId::from_uuid(b)));
    assert!(!set.contains(&c_id));
    assert!(!set.contains(&a_id));
}
