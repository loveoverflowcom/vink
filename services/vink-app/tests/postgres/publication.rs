//! Work publication persistence and API tests (PUB-RUNTIME-01, #158) on real PostgreSQL.
//! Covers snapshot capture with immutable metadata binding, request-public (submit) with durable
//! ticket creation, set-private with pointer deletion, idempotency receipts, stale generation /
//! version conflicts, privacy enforcement (old ETag returns 404 not 304), two-connection lock
//! ordering, and schema upgrade compatibility.

use axum::http::StatusCode;
use serde_json::json;
use sqlx::Acquire;
use std::sync::Arc;
use uuid::Uuid;

use vink_app::domain::ids::{PrincipalId, WorkId};
use vink_app::domain::principal::SuspensionReason;
use vink_app::persistence::{reinstate_principal, suspend_principal};

use super::*;

fn publication_state_path(work: Uuid) -> String {
    format!("/api/v1/works/{work}/publication/state")
}

fn snapshots_path(work: Uuid) -> String {
    format!("/api/v1/works/{work}/snapshots")
}

fn request_public_path(work: Uuid) -> String {
    format!("/api/v1/works/{work}/publication/request-public")
}

fn set_private_path(work: Uuid) -> String {
    format!("/api/v1/works/{work}/publication/set-private")
}

fn public_read_path(work: Uuid) -> String {
    format!("/api/v1/works/{work}/publication")
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn test_publication_state_and_snapshot_capture_lifecycle(pool: PgPool) {
    let app = app(&pool).await;
    let author_id = dev_seed::SAMPLE_AUTHOR;
    let other_id = dev_seed::OTHER_PRINCIPAL;
    let work_id = dev_seed::SAMPLE_WORK;

    // 1. Author publication state access control
    let (status, _, body) = call(
        &app,
        "GET",
        &publication_state_path(work_id),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json(&body)["error"]["code"], "unauthenticated");

    let (status, cache, body) = call(
        &app,
        "GET",
        &publication_state_path(work_id),
        Some(other_id),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert_eq!(json(&body)["error"]["code"], "not_found");

    let (status, cache, body) = call(
        &app,
        "GET",
        &publication_state_path(work_id),
        Some(author_id),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    let state = json(&body);
    assert_eq!(state["work_id"], work_id.to_string());
    assert_eq!(state["audience"], "public");
    assert_eq!(state["generation"], 2);
    assert_eq!(
        state["pointer"]["revision_id"],
        dev_seed::SAMPLE_REVISION.to_string()
    );

    // 2. Snapshot capture with invalid / stale versions
    let (status, _, body) = call(
        &app,
        "POST",
        &snapshots_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "snap-00000001",
            "expected_draft_version": 99,
            "expected_metadata_version": 1
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["error"]["code"], "version_conflict");

    let (status, _, body) = call(
        &app,
        "POST",
        &snapshots_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "snap-00000001",
            "expected_draft_version": 1,
            "expected_metadata_version": 99
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["error"]["code"], "version_conflict");

    // 3. Successful snapshot capture
    let (status, cache, body) = call(
        &app,
        "POST",
        &snapshots_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "snap-00000001",
            "expected_draft_version": 1,
            "expected_metadata_version": 1
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    let snap_res = json(&body);
    assert_eq!(snap_res["outcome"], "saved");
    assert_eq!(snap_res["replayed"], false);
    let r2_str = snap_res["revision_id"].as_str().unwrap().to_string();
    let r2_uuid = Uuid::parse_str(&r2_str).unwrap();

    // Verify revision was inserted with bound metadata snapshot
    let bound_snapshot: Option<Uuid> =
        sqlx::query_scalar("SELECT metadata_snapshot_id FROM revisions WHERE id = $1")
            .bind(r2_uuid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        bound_snapshot.is_some(),
        "revision must have bound metadata_snapshot_id"
    );

    // Verify snapshot in work_metadata_snapshots
    let snap_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM work_metadata_snapshots WHERE id = $1 AND work_id = $2)",
    )
    .bind(bound_snapshot.unwrap())
    .bind(work_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(snap_exists, "work_metadata_snapshots row must exist");

    // 4. Snapshot replay with identical key and payload
    let (status, cache, body) = call(
        &app,
        "POST",
        &snapshots_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "snap-00000001",
            "expected_draft_version": 1,
            "expected_metadata_version": 1
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    let replay_res = json(&body);
    assert_eq!(replay_res["outcome"], "saved");
    assert_eq!(replay_res["replayed"], true);
    assert_eq!(replay_res["revision_id"], r2_str);

    // 5. Command key reuse with different payload
    let (status, _, body) = call(
        &app,
        "POST",
        &snapshots_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "snap-00000001",
            "expected_draft_version": 2,
            "expected_metadata_version": 1
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json(&body)["error"]["code"], "idempotency_key_reused");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn test_request_public_and_ticket_creation(pool: PgPool) {
    let app = app(&pool).await;
    let author_id = dev_seed::SAMPLE_AUTHOR;
    let work_id = dev_seed::SAMPLE_WORK;

    // First capture snapshot r2
    let (_, _, body) = call(
        &app,
        "POST",
        &snapshots_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "snap-000000r2",
            "expected_draft_version": 1,
            "expected_metadata_version": 1
        })),
    )
    .await;
    let r2_str = json(&body)["revision_id"].as_str().unwrap().to_string();
    let r2_uuid = Uuid::parse_str(&r2_str).unwrap();

    // 1. Submit request-public for r2
    let (status, cache, body) = call(
        &app,
        "POST",
        &request_public_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "pub-0000000r2",
            "revision_id": r2_str,
            "expected_generation": 2
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    let pub_res = json(&body);
    assert_eq!(pub_res["outcome"], "submitted");
    assert_eq!(pub_res["generation"], 3);
    assert_eq!(pub_res["replayed"], false);
    assert!(pub_res["ticket_id"].is_string());

    let ticket_uuid = Uuid::parse_str(pub_res["ticket_id"].as_str().unwrap()).unwrap();

    // Verify ticket in work_evaluation_jobs
    let job_state: String = sqlx::query_scalar(
        "SELECT state FROM work_evaluation_jobs WHERE id = $1 AND work_id = $2 AND revision_id = $3"
    )
    .bind(ticket_uuid)
    .bind(work_id)
    .bind(r2_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(job_state, "open");

    // Verify work_publications row updated
    let (gen_val, intent_rev, intent_gen): (i64, Option<Uuid>, Option<i64>) = sqlx::query_as(
        "SELECT generation, intent_revision_id, intent_generation FROM work_publications WHERE work_id = $1"
    )
    .bind(work_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(gen_val, 3);
    assert_eq!(intent_rev, Some(r2_uuid));
    assert_eq!(intent_gen, Some(3));

    // Verify receipt in work_publication_command_receipts
    let receipt_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM work_publication_command_receipts WHERE actor_id = $1 AND work_id = $2 AND command_key = 'pub-0000000r2'"
    )
    .bind(author_id)
    .bind(work_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(receipt_count, 1);

    // 2. Replay request-public with same command key
    let (status, cache, body) = call(
        &app,
        "POST",
        &request_public_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "pub-0000000r2",
            "revision_id": r2_str,
            "expected_generation": 2
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    let replay_res = json(&body);
    assert_eq!(replay_res["outcome"], "submitted");
    assert_eq!(replay_res["generation"], 3);
    assert_eq!(replay_res["replayed"], true);

    // 3. Command key reuse with different revision
    let (status, _, body) = call(
        &app,
        "POST",
        &request_public_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "pub-0000000r2",
            "revision_id": dev_seed::SAMPLE_REVISION.to_string(),
            "expected_generation": 2
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json(&body)["error"]["code"], "idempotency_key_reused");

    // 4. Stale generation conflict
    let (status, _, body) = call(
        &app,
        "POST",
        &request_public_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "pub-new-key",
            "revision_id": r2_str,
            "expected_generation": 2 // current is 3!
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["error"]["code"], "version_conflict");

    // 5. Standing check: suspended author fails closed
    let reason = SuspensionReason::parse("Vi phạm điều khoản thử nghiệm").unwrap();
    suspend_principal(&pool, PrincipalId::from_uuid(author_id), &reason)
        .await
        .unwrap();

    let (status, _, body) = call(
        &app,
        "POST",
        &request_public_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "pub-suspended",
            "revision_id": r2_str,
            "expected_generation": 3
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json(&body)["error"]["code"], "principal_suspended");

    // Reinstate
    reinstate_principal(&pool, PrincipalId::from_uuid(author_id))
        .await
        .unwrap();
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn test_r1_remains_readable_while_r2_is_submitted(pool: PgPool) {
    let app = app(&pool).await;
    let author_id = dev_seed::SAMPLE_AUTHOR;
    let work_id = dev_seed::SAMPLE_WORK;
    let r1 = dev_seed::SAMPLE_REVISION;

    // Verify initial public read of r1
    let (status, _, body) = call(&app, "GET", &public_read_path(work_id), None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    let r1_view = json(&body);
    assert_eq!(r1_view["work_id"], work_id.to_string());
    assert_eq!(r1_view["revision_id"], r1.to_string());

    // Capture and submit r2
    let (_, _, body) = call(
        &app,
        "POST",
        &snapshots_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "snap-000000r2",
            "expected_draft_version": 1,
            "expected_metadata_version": 1
        })),
    )
    .await;
    let r2_str = json(&body)["revision_id"].as_str().unwrap().to_string();

    let (status, _, _) = call(
        &app,
        "POST",
        &request_public_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "pub-0000000r2",
            "revision_id": r2_str,
            "expected_generation": 2
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Verify r1 remains readable on public read!
    let (status, _, body) = call(&app, "GET", &public_read_path(work_id), None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    let r1_still_readable = json(&body);
    assert_eq!(r1_still_readable["work_id"], work_id.to_string());
    assert_eq!(r1_still_readable["revision_id"], r1.to_string());

    // Public discovery also shows r1
    let (status, _, body) = call(&app, "GET", "/api/v1/works", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    let list = json(&body);
    let works = list["works"].as_array().unwrap();
    assert!(
        works
            .iter()
            .any(|item| item["work_id"] == work_id.to_string())
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn test_set_private_clears_pointer_and_enforces_privacy(pool: PgPool) {
    let app = app(&pool).await;
    let author_id = dev_seed::SAMPLE_AUTHOR;
    let work_id = dev_seed::SAMPLE_WORK;

    // Get r1 and its ETag first
    let req = Request::builder()
        .method("GET")
        .uri(public_read_path(work_id))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let r1_etag = resp
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Conditional GET before set_private returns 304
    let req = Request::builder()
        .method("GET")
        .uri(public_read_path(work_id))
        .header(header::IF_NONE_MATCH, &r1_etag)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);

    // Call set_private at generation 2
    let (status, cache, body) = call(
        &app,
        "POST",
        &set_private_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "priv-00000001",
            "expected_generation": 2
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    let priv_res = json(&body);
    assert_eq!(priv_res["outcome"], "made_private");
    assert_eq!(priv_res["generation"], 3);
    assert_eq!(priv_res["replayed"], false);

    // Verify pointer in publications table is deleted
    let pointer_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM publications WHERE work_id = $1)")
            .bind(work_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        !pointer_exists,
        "publications row must be deleted upon set_private"
    );

    // Verify work_publications audience is private
    let (aud, gen_val, ptr_rev): (String, i64, Option<Uuid>) = sqlx::query_as(
        "SELECT audience, generation, pointer_revision_id FROM work_publications WHERE work_id = $1"
    )
    .bind(work_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(aud, "private");
    assert_eq!(gen_val, 3);
    assert_eq!(ptr_rev, None);

    // Public read returns 404 NOT_FOUND!
    let (status, _, body) = call(&app, "GET", &public_read_path(work_id), None, None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    // Conditional GET with old ETag returns 404 NOT_FOUND, NOT 304!
    let req = Request::builder()
        .method("GET")
        .uri(public_read_path(work_id))
        .header(header::IF_NONE_MATCH, &r1_etag)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "old ETag must return 404 on private work"
    );

    // Public discovery does not show the work
    let (status, _, body) = call(&app, "GET", "/api/v1/works", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    let list = json(&body);
    let works = list["works"].as_array().unwrap();
    assert!(
        !works
            .iter()
            .any(|item| item["work_id"] == work_id.to_string()),
        "private work must not appear in discovery"
    );

    // Replay set_private
    let (status, cache, body) = call(
        &app,
        "POST",
        &set_private_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "priv-00000001",
            "expected_generation": 2
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    let replay_res = json(&body);
    assert_eq!(replay_res["outcome"], "made_private");
    assert_eq!(replay_res["generation"], 3);
    assert_eq!(replay_res["replayed"], true);

    // Reaffirm set_private with new key at generation 3
    let (status, cache, body) = call(
        &app,
        "POST",
        &set_private_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "priv-00000002",
            "expected_generation": 3
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    let reaffirm_res = json(&body);
    assert_eq!(reaffirm_res["outcome"], "reaffirmed");
    assert_eq!(reaffirm_res["generation"], 4);
    assert_eq!(reaffirm_res["replayed"], false);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn test_lock_ordering_concurrency_set_private_and_publications_share(pool: PgPool) {
    let vink = seeded(&pool).await;
    let author = dev_seed::SAMPLE_AUTHOR;
    let work_id = dev_seed::SAMPLE_WORK;

    // Tx1 acquires publications FOR SHARE (as reader / discussion / endorsement do)
    let mut conn1 = pool.acquire().await.unwrap();
    let mut tx1 = conn1.begin().await.unwrap();
    let _: Option<Uuid> =
        sqlx::query_scalar("SELECT revision_id FROM publications WHERE work_id = $1 FOR SHARE")
            .bind(work_id)
            .fetch_optional(&mut *tx1)
            .await
            .unwrap();

    let blocker_pid = backend_pid(&mut tx1).await;

    // Spawn Tx2: set_private via vink. It will attempt publications FOR UPDATE
    let vink_arc = Arc::new(vink);
    let vink_clone = vink_arc.clone();
    let handle = tokio::spawn(async move {
        vink_clone
            .set_private(
                Some(PrincipalId::from_uuid(author)),
                WorkId::from_uuid(work_id),
                "lock-test-key",
                2,
            )
            .await
    });

    // Verify Tx2 is blocked waiting on conn1's lock
    wait_blocked_by(&pool, blocker_pid).await;

    // Tx1 now takes principals lock (as discussion/endorsement do in their lock graph)
    let _: Option<Uuid> = sqlx::query_scalar("SELECT id FROM principals WHERE id = $1 FOR UPDATE")
        .bind(author)
        .fetch_optional(&mut *tx1)
        .await
        .unwrap();

    // Commit Tx1
    tx1.commit().await.unwrap();

    // Tx2 unblocks and completes cleanly without deadlock
    let outcome = handle.await.unwrap().unwrap();
    assert_eq!(outcome.outcome.as_str(), "made_private");
    assert_eq!(outcome.generation.get(), 3);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn test_schema_upgrade_and_legacy_compatibility(pool: PgPool) {
    let vink = seeded(&pool).await;
    let author = dev_seed::SAMPLE_AUTHOR;
    let work_id = dev_seed::SAMPLE_WORK;
    let r1 = dev_seed::SAMPLE_REVISION;

    // Verify database has legacy_bootstrap row
    let (mod_status, prov_kind): (String, String) = sqlx::query_as(
        "SELECT moderation, provenance_kind FROM work_revision_moderation WHERE work_id = $1 AND revision_id = $2"
    )
    .bind(work_id)
    .bind(r1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mod_status, "approved");
    assert_eq!(prov_kind, "legacy_bootstrap");

    // Verify work_publications exists with generation 2
    let (aud, gen_val): (String, i64) =
        sqlx::query_as("SELECT audience, generation FROM work_publications WHERE work_id = $1")
            .bind(work_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(aud, "public");
    assert_eq!(gen_val, 2);

    // Verify load_author_publication_state restores lawfully
    let facts = vink
        .load_author_publication_state(
            Some(PrincipalId::from_uuid(author)),
            WorkId::from_uuid(work_id),
        )
        .await
        .unwrap();

    assert_eq!(facts.publication.facts().generation.get(), 2);
    assert_eq!(facts.publication.facts().revisions.len(), 1);
    assert_eq!(
        facts.publication.facts().revisions[0].moderation,
        vink_app::domain::publication::Moderation::Approved
    );
    assert_eq!(facts.moderation.facts().revisions.len(), 1);
    assert_eq!(
        facts.moderation.facts().revisions[0].provenance,
        Some(vink_app::domain::moderation::DecisionProvenance::LegacyBootstrap)
    );
}

const PUBLICATION_MIGRATION: i64 = 20_261_116_000_000;
const MODERATION_MIGRATION: i64 = 20_261_117_000_000;

fn down_script() -> String {
    format!(
        "{}\n{}",
        include_str!("../../migrations-rollback/20261117000000_work_moderation_runtime.down.sql"),
        include_str!(
            "../../migrations-rollback/20261116000000_work_publication_persistence.down.sql"
        ),
    )
}

async fn run_down(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(sqlx::AssertSqlSafe(down_script()))
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

/// Upgrade through the real migration: a pointer written by the old code (generation 1, no paired
/// rows) is backfilled as a lawful public state with an honest `legacy_bootstrap` decision, the
/// author can read and withdraw it, and an unrelated work without a pointer becomes private.
#[sqlx::test(migrations = false)]
async fn upgrade_backfills_paired_state_for_old_pointers(pool: PgPool) {
    use sqlx::migrate::Migrator;
    use vink_app::persistence::{MIGRATOR, migrate};

    Migrator::with_migrations(
        MIGRATOR
            .migrations
            .iter()
            .filter(|m| m.version < PUBLICATION_MIGRATION)
            .cloned()
            .collect(),
    )
    .run(&pool)
    .await
    .unwrap();
    // The old schema has no paired tables: the seed writes the pointer alone.
    let vink = seeded(&pool).await;
    sqlx::query("UPDATE publications SET generation = 1 WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    let unpublished = Uuid::now_v7();
    sqlx::query("INSERT INTO works (id, owner_id) VALUES ($1, $2)")
        .bind(unpublished)
        .bind(dev_seed::SAMPLE_AUTHOR)
        .execute(&pool)
        .await
        .unwrap();

    let report = migrate(&pool).await.unwrap();
    assert_eq!(
        report.applied,
        vec![PUBLICATION_MIGRATION, MODERATION_MIGRATION]
    );

    let row: (String, i64, Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT audience, generation, intent_revision_id, pointer_revision_id
           FROM work_publications WHERE work_id = $1",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            "public".into(),
            2,
            Some(dev_seed::SAMPLE_REVISION),
            Some(dev_seed::SAMPLE_REVISION)
        )
    );
    let pointer_generation: i64 =
        sqlx::query_scalar("SELECT generation FROM publications WHERE work_id = $1")
            .bind(dev_seed::SAMPLE_WORK)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pointer_generation, 2);
    let private: (String, i64) =
        sqlx::query_as("SELECT audience, generation FROM work_publications WHERE work_id = $1")
            .bind(unpublished)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(private, ("private".into(), 1));

    let facts = vink
        .load_author_publication_state(
            Some(PrincipalId::from_uuid(dev_seed::SAMPLE_AUTHOR)),
            WorkId::from_uuid(dev_seed::SAMPLE_WORK),
        )
        .await
        .unwrap();
    assert_eq!(
        facts.moderation.facts().revisions[0].provenance,
        Some(vink_app::domain::moderation::DecisionProvenance::LegacyBootstrap)
    );
    // Seeding the upgraded database again changes nothing.
    dev_seed::apply(&pool).await.unwrap();
    let withdrawn = vink
        .set_private(
            Some(PrincipalId::from_uuid(dev_seed::SAMPLE_AUTHOR)),
            WorkId::from_uuid(dev_seed::SAMPLE_WORK),
            "upgrade-private-1",
            2,
        )
        .await
        .unwrap();
    assert_eq!(withdrawn.outcome.as_str(), "made_private");
    // A later seed never re-creates the pointer the author withdrew.
    dev_seed::apply(&pool).await.unwrap();
    let pointers: i64 = sqlx::query_scalar("SELECT count(*) FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pointers, 0);
}

/// The down script backs out unused schema only: once an author command was accepted it refuses
/// and leaves every row in place.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn backout_refuses_after_an_author_command_and_drops_unused_schema(pool: PgPool) {
    let vink = seeded(&pool).await;
    let accepted = vink
        .set_private(
            Some(PrincipalId::from_uuid(dev_seed::SAMPLE_AUTHOR)),
            WorkId::from_uuid(dev_seed::SAMPLE_WORK),
            "backout-private-1",
            2,
        )
        .await
        .unwrap();
    assert_eq!(accepted.generation.get(), 3);
    let refusal = run_down(&pool).await.unwrap_err().to_string();
    assert!(refusal.contains("roll forward"), "{refusal}");
    let receipts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM work_publication_command_receipts")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(receipts, 1);

    sqlx::query("DELETE FROM work_publication_command_receipts")
        .execute(&pool)
        .await
        .unwrap();
    run_down(&pool).await.unwrap();
    let table: Option<String> = sqlx::query_scalar("SELECT to_regclass('work_publications')::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(table, None);
    let latest: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(latest < PUBLICATION_MIGRATION);
}

/// A lost answer retried concurrently with the same key: exactly one command is applied and the
/// other attempt replays its receipt (never a stale refusal, a key error or a 5xx).
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn concurrent_retries_with_one_key_apply_once_and_replay(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    let author = Some(PrincipalId::from_uuid(dev_seed::SAMPLE_AUTHOR));
    let work = WorkId::from_uuid(dev_seed::SAMPLE_WORK);

    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let vink = vink.clone();
        tasks.spawn(async move {
            vink.capture_snapshot(author, work, "same-capture-key", 1, 1)
                .await
        });
    }
    let captures: Vec<_> = tasks
        .join_all()
        .await
        .into_iter()
        .map(Result::unwrap)
        .collect();
    assert_eq!(captures.iter().filter(|c| !c.replayed).count(), 1);
    assert!(
        captures
            .iter()
            .all(|c| c.revision_id == captures[0].revision_id)
    );
    let revisions: i64 = sqlx::query_scalar("SELECT count(*) FROM revisions WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(revisions, 2, "the seeded revision and exactly one capture");

    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let vink = vink.clone();
        let revision = captures[0].revision_id;
        tasks.spawn(async move {
            vink.request_public(author, work, "same-request-key", revision, 2)
                .await
        });
    }
    let requests: Vec<_> = tasks
        .join_all()
        .await
        .into_iter()
        .map(Result::unwrap)
        .collect();
    assert_eq!(requests.iter().filter(|r| !r.replayed).count(), 1);
    assert!(requests.iter().all(|r| r.generation.get() == 3));
    let jobs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM work_evaluation_jobs WHERE work_id = $1")
            .bind(dev_seed::SAMPLE_WORK)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(jobs, 1);
}
