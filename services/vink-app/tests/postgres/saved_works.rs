//! Exercise the private HTTP surface and force lock waits on a real PostgreSQL connection.
use super::*;
use crate::conditional_reads::publish;
use crate::follow::get_with_headers;
use time::OffsetDateTime;
use vink_app::{
    application::social_retirement::SocialRetirement,
    domain::{
        ids::RevisionId,
        saved_work::{SaveIntent, SavedGeneration},
    },
};

const LIST: &str = "/api/v1/me/saved-works";
const OWNER: Uuid = dev_seed::OTHER_PRINCIPAL;
const WORK: Uuid = dev_seed::SAMPLE_WORK;
const AUTHOR: Uuid = dev_seed::SAMPLE_AUTHOR;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    actor: Option<Uuid>,
    headers: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(actor) = actor {
        req = req.header("x-test-principal", actor.to_string());
    }
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.headers()[header::CACHE_CONTROL], "private, no-store");
    assert!(res.headers().get(header::ETAG).is_none());
    let status = res.status();
    (
        status,
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap(),
    )
}

async fn change(app: &Router, method: &str, work: Uuid, g: i64) -> (StatusCode, Value) {
    request(
        app,
        method,
        &format!("{LIST}/{work}?expected_generation={g}"),
        Some(OWNER),
        &[
            ("origin", ORIGIN),
            ("x-vink-expected-principal", &OWNER.to_string()),
        ],
    )
    .await
}

async fn page(app: &Router, owner: Uuid, cursor: Option<&str>) -> Value {
    let path = cursor.map_or_else(|| LIST.into(), |c| format!("{LIST}?cursor={c}"));
    let (status, page) = request(app, "GET", &path, Some(owner), &[]).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn api_ack_aba_and_private_cache(pool: PgPool) {
    let app = app(&pool).await;
    let public = format!("/api/v1/works/{WORK}/publication");
    let before = get_with_headers(&app, &public, None, &[]).await;
    assert_eq!(before.0, StatusCode::OK);
    assert!(before.2.is_some());
    let pair = format!("{LIST}/{WORK}");
    let initial = json!({"saved":false,"generation":1});
    assert_eq!(
        request(&app, "GET", &pair, Some(OWNER), &[]).await.1,
        initial
    );
    assert_eq!(
        request(&app, "GET", LIST, None, &[]).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (a, b) = tokio::join!(change(&app, "PUT", WORK, 1), change(&app, "PUT", WORK, 1));
    assert_eq!(a, b);
    assert_eq!(a, (StatusCode::OK, json!({"saved":true,"generation":2})));
    assert_eq!(
        request(&app, "GET", &pair, Some(AUTHOR), &[]).await.1,
        initial
    );
    assert_eq!(
        page(&app, AUTHOR, None).await,
        json!({"items":[],"next_cursor":null})
    );
    assert_eq!(
        get_with_headers(&app, &public, Some(OWNER), &[]).await,
        before
    );
    assert_eq!(
        get_with_headers(
            &app,
            &public,
            None,
            &[("if-none-match", before.2.as_ref().unwrap())]
        )
        .await
        .0,
        StatusCode::NOT_MODIFIED
    );
    let first = page(&app, OWNER, None).await;
    assert_eq!(first["items"].as_array().unwrap().len(), 1);
    assert_eq!(first["items"][0]["title"], dev_seed::SAMPLE_TITLE);
    assert_eq!(
        change(&app, "PUT", WORK, 2).await.1,
        json!({"saved":true,"generation":3})
    );
    assert_eq!(
        page(&app, OWNER, None).await["items"][0]["saved_at"],
        first["items"][0]["saved_at"]
    );
    assert_eq!(
        change(&app, "DELETE", WORK, 3).await.1,
        json!({"saved":false,"generation":4})
    );
    assert_eq!(
        change(&app, "DELETE", WORK, 3).await.1,
        json!({"saved":false,"generation":4})
    );
    assert_eq!(
        request(&app, "GET", &pair, Some(OWNER), &[]).await.1,
        json!({"saved":false,"generation":4})
    );
    for old in [1, 9] {
        let (s, v) = change(&app, "PUT", WORK, old).await;
        assert_eq!(s, StatusCode::CONFLICT);
        assert_eq!(v["error"]["field"], "expected_generation");
        assert_eq!(v["error"]["current_version"], 4);
    }
    assert_eq!(
        change(&app, "DELETE", WORK, 4).await.1,
        json!({"saved":false,"generation":5})
    );
    assert_eq!(
        change(&app, "PUT", WORK, 5).await.1,
        json!({"saved":true,"generation":6})
    );
    assert_eq!(
        change(&app, "DELETE", WORK, 3).await.0,
        StatusCode::CONFLICT
    );
    let no_work = Uuid::new_v4();
    assert_eq!(
        change(&app, "PUT", no_work, 1).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        change(&app, "DELETE", no_work, 1).await.1,
        json!({"saved":false,"generation":2})
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notification_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn owner_origin_and_query_validation(pool: PgPool) {
    let app = app(&pool).await;
    let pair = format!("{LIST}/{WORK}");
    let path = format!("{pair}?expected_generation=1");
    for origin in [None, Some("https://other.invalid")] {
        let mut headers = vec![];
        if let Some(origin) = origin {
            headers.push(("origin", origin));
        }
        assert_eq!(
            request(&app, "PUT", &path, Some(OWNER), &headers).await.0,
            StatusCode::FORBIDDEN
        );
    }
    let headers = [
        ("origin", ORIGIN),
        ("x-vink-expected-principal", &*OWNER.to_string()),
    ];
    let (s, v) = request(&app, "PUT", &path, Some(AUTHOR), &headers).await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("forbidden"))
    );
    assert_eq!(
        request(&app, "PUT", &path, None, &headers).await.0,
        StatusCode::UNAUTHORIZED
    );
    for (path, field) in [
        (pair.clone(), "expected_generation"),
        (
            format!("{pair}?expected_generation=1&expected_generation=1"),
            "expected_generation",
        ),
        (
            format!("{pair}?expected_generation=0"),
            "expected_generation",
        ),
        (format!("{pair}?owner_id={OWNER}"), "query"),
    ] {
        let (s, v) = request(&app, "PUT", &path, Some(OWNER), &headers).await;
        assert_eq!(
            (s, v["error"]["field"].as_str()),
            (StatusCode::UNPROCESSABLE_ENTITY, Some(field))
        );
    }
    for values in [
        vec![("origin", ORIGIN)],
        vec![("origin", ORIGIN), ("x-vink-expected-principal", "bad")],
    ] {
        assert_eq!(
            request(&app, "PUT", &path, Some(OWNER), &values).await.1["error"]["field"],
            "expected_principal"
        );
    }
    for suffix in [
        "?cursor=",
        "?cursor=bad",
        "?cursor=a&cursor=b",
        "?limit=99",
        "?principal_id=0",
    ] {
        assert_eq!(
            request(&app, "GET", &format!("{LIST}{suffix}"), Some(OWNER), &[])
                .await
                .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        request(&app, "GET", &format!("{LIST}/bad-id"), Some(OWNER), &[])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn publication_lifecycle_projects_only_live_metadata(pool: PgPool) {
    let app = app(&pool).await;
    let vink = Vink::new(PgStore::new(pool.clone()));
    change(&app, "PUT", WORK, 1).await;
    let original = page(&app, OWNER, None).await["items"][0].clone();
    vink.set_private(author(), sample_work(), "saved-set-private", 2)
        .await
        .unwrap();
    let neutral = |kind| json!({"kind":kind,"work_id":WORK,"generation":2});
    assert_eq!(
        page(&app, OWNER, None).await["items"],
        json!([neutral("hidden")])
    );
    let denied = change(&app, "PUT", WORK, 1).await;
    assert_eq!(denied, change(&app, "PUT", Uuid::new_v4(), 1).await);
    vink.request_public(
        author(),
        sample_work(),
        "saved-republish",
        RevisionId::from_uuid(dev_seed::SAMPLE_REVISION),
        3,
    )
    .await
    .unwrap();
    assert_eq!(page(&app, OWNER, None).await["items"][0], original);

    // No live admin-remove or author-delete endpoint. Use the persisted lifecycle shapes.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM publications WHERE work_id=$1")
        .bind(WORK)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE work_publications SET lifecycle='removed',intent_revision_id=NULL,intent_generation=NULL,pointer_revision_id=NULL,pointer_intent_generation=NULL WHERE work_id=$1")
        .bind(WORK).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(page(&app, OWNER, None).await["items"], json!([]));
    assert_eq!(
        request(&app, "GET", &format!("{LIST}/{WORK}"), Some(OWNER), &[])
            .await
            .1,
        json!({"saved":true,"generation":2})
    );
    let restored_revision = publish(
        &pool,
        WORK,
        AUTHOR,
        "Bản công khai mới",
        "Mưa cuối mùa",
        OffsetDateTime::now_utc(),
    )
    .await;
    sqlx::query(
        "UPDATE work_publications SET lifecycle='active',audience='public',
          intent_revision_id=$2,intent_generation=generation,pointer_revision_id=$2,
          pointer_intent_generation=generation WHERE work_id=$1",
    )
    .bind(WORK)
    .bind(restored_revision)
    .execute(&pool)
    .await
    .unwrap();
    let restored = page(&app, OWNER, None).await["items"][0].clone();
    assert_eq!(restored["title"], "Bản công khai mới");
    assert_eq!(restored["saved_at"], original["saved_at"]);
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM publications WHERE work_id=$1")
        .bind(WORK)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE work_publications SET generation=generation+1,lifecycle='author_deleted',audience='private',
        deletion_provenance='tombstone', tombstone_revision_id=pointer_revision_id, tombstone_intent_generation=pointer_intent_generation,
        pointer_revision_id=NULL,pointer_intent_generation=NULL,intent_revision_id=NULL,intent_generation=NULL WHERE work_id=$1")
        .bind(WORK).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        page(&app, OWNER, None).await["items"],
        json!([neutral("author_deleted")])
    );
    assert_eq!(change(&app, "DELETE", WORK, 2).await.0, StatusCode::OK);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn tied_cursor_skips_gone_survives_clear_and_rechecks_access(pool: PgPool) {
    let app = app(&pool).await;
    // 21 readable rows alternate with missing works. Identical times force UUID ordering.
    let mut expected = Vec::new();
    for n in 1..=42 {
        let work = Uuid::from_u128(n);
        if n % 2 == 0 {
            publish(
                &pool,
                work,
                AUTHOR,
                "Thơ",
                "Mây bay",
                OffsetDateTime::now_utc(),
            )
            .await;
            expected.push(work.to_string());
        }
        sqlx::query("INSERT INTO saved_works(principal_id,work_id,generation,saved_at) VALUES ($1,$2,2,'2026-11-24T00:00:00Z')")
            .bind(OWNER).bind(work).execute(&pool).await.unwrap();
    }
    let first = page(&app, OWNER, None).await;
    let ids = |p: &Value| {
        p["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["work_id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&first), expected[..20]);
    let cursor = first["next_cursor"].as_str().unwrap();
    let last = page(&app, OWNER, Some(cursor)).await;
    assert_eq!(ids(&last), expected[20..]);
    assert!(last["next_cursor"].is_null());
    change(&app, "DELETE", Uuid::from_u128(40), 2).await;
    change(&app, "PUT", Uuid::from_u128(40), 3).await;
    let refreshed = page(&app, OWNER, None).await;
    assert_eq!(ids(&refreshed)[0], expected[19]);
    assert_eq!(ids(&page(&app, OWNER, Some(cursor)).await), expected[20..]);
    let (position, _) = cursor.rsplit_once('.').unwrap();
    let wrong_mac = format!("{position}.00000000000000000000000000000000");
    for (owner, token) in [(AUTHOR, cursor.to_owned()), (OWNER, wrong_mac)] {
        assert_eq!(
            request(
                &app,
                "GET",
                &format!("{LIST}?cursor={token}"),
                Some(owner),
                &[]
            )
            .await
            .1["error"]["field"],
            "cursor"
        );
    }
    let work = Uuid::from_u128(42);
    let mut tx = pool.begin().await.unwrap();
    let revision: Uuid =
        sqlx::query_scalar("DELETE FROM publications WHERE work_id=$1 RETURNING revision_id")
            .bind(work)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO work_publications (work_id, audience, lifecycle, generation,
          hidden_revision_id, hidden_intent_generation, ever_public, public_epoch)
          VALUES ($1, 'private', 'active', 3, $2, 2, ARRAY[$2], 1)",
    )
    .bind(work)
    .bind(revision)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        page(&app, OWNER, Some(cursor)).await["items"],
        json!([
            {"kind":"hidden","work_id":work,"generation":2}
        ])
    );
    sqlx::query(
        "UPDATE work_publications SET lifecycle='removed',hidden_revision_id=NULL,
          hidden_intent_generation=NULL WHERE work_id=$1",
    )
    .bind(work)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        page(&app, OWNER, Some(cursor)).await,
        json!({"items":[],"next_cursor":null})
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn cap_serializes_two_public_works_and_counts_omitted_memberships(pool: PgPool) {
    let app = app(&pool).await;
    let another = Uuid::new_v4();
    publish(
        &pool,
        another,
        AUTHOR,
        "Thơ",
        "Hoa muộn",
        OffsetDateTime::now_utc(),
    )
    .await;
    sqlx::query(
        "INSERT INTO saved_works(principal_id,work_id,generation,saved_at)
        SELECT $1,gen_random_uuid(),2,now() FROM generate_series(1,9999)",
    )
    .bind(OWNER)
    .execute(&pool)
    .await
    .unwrap();
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM principals WHERE id=$1 FOR UPDATE")
        .bind(OWNER)
        .execute(&mut *lock)
        .await
        .unwrap();
    let spawn = |work| {
        let app = app.clone();
        tokio::spawn(async move { change(&app, "PUT", work, 1).await })
    };
    let (a, b) = (spawn(WORK), spawn(another));
    wait_blocked(&pool, 2).await;
    lock.commit().await.unwrap();
    let (a, b) = (a.await.unwrap(), b.await.unwrap());
    let (success, limit, winner, loser) = if a.0 == StatusCode::OK {
        (a, b, WORK, another)
    } else {
        (b, a, another, WORK)
    };
    assert_eq!(
        success,
        (StatusCode::OK, json!({"saved":true,"generation":2}))
    );
    assert_eq!(limit.0, StatusCode::CONFLICT);
    assert_eq!(limit.1["error"]["code"], "saved_works_limit_reached");
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM saved_works WHERE principal_id=$1 AND saved_at IS NOT NULL",
    )
    .bind(OWNER)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 10_000);
    assert_eq!(change(&app, "PUT", winner, 2).await.1["generation"], 3);
    assert_eq!(change(&app, "DELETE", winner, 3).await.1["generation"], 4);
    assert_eq!(change(&app, "PUT", loser, 1).await.0, StatusCode::OK);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn commit_waits_for_withdrawal_and_suspension(pool: PgPool) {
    let app = app(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let pid = backend_pid(&mut tx).await;
    sqlx::query("DELETE FROM publications WHERE work_id=$1")
        .bind(WORK)
        .execute(&mut *tx)
        .await
        .unwrap();
    let saving = {
        let app = app.clone();
        tokio::spawn(async move { change(&app, "PUT", WORK, 1).await })
    };
    wait_blocked_by(&pool, pid).await;
    tx.commit().await.unwrap();
    assert_eq!(saving.await.unwrap().0, StatusCode::NOT_FOUND);

    let mut tx = pool.begin().await.unwrap();
    let pid = backend_pid(&mut tx).await;
    sqlx::query(
        "INSERT INTO principal_suspensions(id,principal_id,reason) VALUES ($1,$2,'Tạm khóa')",
    )
    .bind(Uuid::new_v4())
    .bind(OWNER)
    .execute(&mut *tx)
    .await
    .unwrap();
    let clearing = {
        let app = app.clone();
        tokio::spawn(async move { change(&app, "DELETE", WORK, 1).await })
    };
    wait_blocked_by(&pool, pid).await;
    tx.commit().await.unwrap();
    let (status, body) = clearing.await.unwrap();
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "principal_suspended");
    assert_eq!(
        request(&app, "GET", LIST, Some(OWNER), &[]).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM saved_works")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn retirement_purges_only_its_owner_including_cleared_fences(pool: PgPool) {
    let vink = seeded(&pool).await;
    for actor in [AUTHOR, OWNER] {
        sqlx::query("INSERT INTO principal_bindings(authority,subject_id,principal_id) VALUES ('vink-identity',$1,$2)")
            .bind(actor.to_string()).bind(actor).execute(&pool).await.unwrap();
        let me = PrincipalId::from_uuid(actor);
        for (work, intent) in [
            (sample_work(), SaveIntent::Saved),
            (WorkId::from_uuid(Uuid::new_v4()), SaveIntent::NotSaved),
        ] {
            vink.set_saved_work(me, me, work, intent, SavedGeneration::FIRST)
                .await
                .unwrap();
        }
    }
    let app = app(&pool).await;
    let retirement = |actor| SocialRetirement::new(actor, Uuid::new_v4()).unwrap();
    vink.store()
        .fence_social_retirement(retirement(author().unwrap()))
        .await
        .unwrap();
    assert_eq!(
        page(&app, OWNER, None).await["items"],
        json!([{"work_id":WORK,"generation":2,"kind":"author_deleted"}])
    );
    assert_eq!(change(&app, "PUT", WORK, 1).await.0, StatusCode::NOT_FOUND);
    assert_eq!(change(&app, "DELETE", WORK, 2).await.0, StatusCode::OK);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM saved_works WHERE principal_id=$1")
        .bind(AUTHOR)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let command = retirement(other().unwrap());
    vink.store().fence_social_retirement(command).await.unwrap();
    vink.store().fence_social_retirement(command).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM saved_works")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        change(&app, "DELETE", WORK, 1).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(&app, "GET", LIST, Some(OWNER), &[]).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = false)]
async fn populated_schema_upgrades_once_without_public_changes(pool: PgPool) {
    use vink_app::persistence::{MIGRATOR, migrate};
    let previous = sqlx::migrate::Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|m| m.version < SAVED_WORKS_MIGRATION)
            .cloned()
            .collect(),
    );
    vink_store_postgres::migrate(&pool, &previous)
        .await
        .unwrap();
    let app = app(&pool).await;
    let path = format!("/api/v1/works/{WORK}/reader");
    let before = get_with_headers(&app, &path, None, &[]).await;
    assert_eq!(before.0, StatusCode::OK);
    assert_eq!(
        migrate(&pool).await.unwrap().applied,
        [SAVED_WORKS_MIGRATION, ACCOUNT_DELETION_MIGRATION]
    );
    assert!(migrate(&pool).await.unwrap().applied.is_empty());
    assert_eq!(change(&app, "PUT", WORK, 1).await.0, StatusCode::OK);
    assert_eq!(get_with_headers(&app, &path, None, &[]).await, before);
}
