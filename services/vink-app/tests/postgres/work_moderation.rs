//! Work moderation and review-withdrawn reader integration tests (#159, O-17@r2).
//!
//! Covers:
//! 1. Full flow: request-public -> worker evaluation -> approved -> publication & reader endpoints.
//! 2. Rejection of active publication -> pointer removed, review_withdrawn notice, reader 200 with author name, publication 404.
//! 3. Discussion access on review-withdrawn work: guest & author reads allowed, own comment delete allowed, new submissions blocked.
//! 4. Set-private cleans up review-withdrawn notice and withdraws author name from URL.
//! 5. Review cases lifecycle: claims, notes, releases, version fencing, takeovers and conclusions.
//! 6. Moderator grants (Works capability) and admin provisioning.
//! 7. Automation mode transitions (enabled <-> paused), job rerouting and review case creation.
//! 8. Bug 1 & Bug 2 regressions: set-private withdraws jobs/cases, re-request-public handles conflicts idempotently.

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use vink_app::domain::ids::{CaseId, PrincipalId, RevisionId};
use vink_app::evaluation_worker::EvaluationWorker;
use vink_app::persistence::moderation::add_admin_seat;

use super::*;

const SAMPLE_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

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

fn reader_read_path(work: Uuid) -> String {
    format!("/api/v1/works/{work}/reader")
}

fn discussion_public_path(work: Uuid) -> String {
    format!("/api/v1/works/{work}/discussion/public")
}

fn discussion_comments_path(work: Uuid) -> String {
    format!("/api/v1/works/{work}/discussion/comments")
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn test_work_moderation_full_flow_and_review_withdrawn_page(pool: PgPool) {
    let app = app(&pool).await;
    let author_id = dev_seed::SAMPLE_AUTHOR;
    let other_id = dev_seed::OTHER_PRINCIPAL;
    let work_id = dev_seed::SAMPLE_WORK;

    // Step 1: Capture a snapshot of the draft
    let (status, _, body) = call(
        &app,
        "POST",
        &snapshots_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "cmd-snap-1",
            "expected_draft_version": 1,
            "expected_metadata_version": 1,
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "capture snapshot failed: {body}"
    );
    let snap_body = json(&body);
    let rev_id_str = snap_body["revision_id"].as_str().unwrap().to_string();

    // Step 2: Request public
    let (status, _, body) = call(
        &app,
        "POST",
        &request_public_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "cmd-pub-1",
            "revision_id": rev_id_str,
            "expected_generation": 2,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "request public failed: {body}");

    // Verify evaluation job is open in DB
    let job_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM work_evaluation_jobs WHERE work_id = $1 AND state = 'open'",
    )
    .bind(work_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(job_count, 1);

    // Step 3: Run evaluation worker with default/approve scenario
    sqlx::query("UPDATE work_evaluation_jobs SET scenario = 'approve' WHERE work_id = $1")
        .bind(work_id)
        .execute(&pool)
        .await
        .unwrap();
    let ran = EvaluationWorker::run_once(&pool, "test-worker")
        .await
        .unwrap();
    assert!(ran, "evaluation worker should process the open job");

    // Verify job is applied and decision is approved
    let job_state: String =
        sqlx::query_scalar("SELECT state FROM work_evaluation_jobs WHERE work_id = $1")
            .bind(work_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job_state, "applied");

    // Public pointer is published!
    let (status, cache, body) =
        call(&app, "GET", &public_read_path(work_id), None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("private, no-cache"));
    assert_eq!(json(&body)["title"], dev_seed::SAMPLE_TITLE);

    // Reader endpoint returns published representation
    let (status, _, body) = call(&app, "GET", &reader_read_path(work_id), None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    let reader_body = json(&body);
    assert_eq!(reader_body["status"], "published");
    assert_eq!(reader_body["title"], dev_seed::SAMPLE_TITLE);

    // Step 4: Moderator / Admin rejects the currently published revision!
    // Provision admin seat for other_id
    {
        let mut conn = pool.acquire().await.unwrap();
        add_admin_seat(
            &mut conn,
            "operator-test",
            PrincipalId::from_uuid(other_id),
            Some("test provision"),
        )
        .await
        .unwrap();
    }

    // Admin creates review case or we conclude directly via conclude_case
    let _now = time::OffsetDateTime::now_utc();
    let rev_id = RevisionId::parse(&rev_id_str).unwrap();
    let case_id = CaseId::from_uuid(Uuid::now_v7());

    let (actual_digest, dec_ver): (String, i64) = sqlx::query_as(
        "SELECT r.content_hash, m.decision_version
           FROM revisions r
           JOIN work_revision_moderation m ON m.work_id = r.work_id AND m.revision_id = r.id
          WHERE r.id = $1",
    )
    .bind(rev_id.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();

    // Create a review case for this revision in DB
    sqlx::query(
        "INSERT INTO moderation_cases (
             id, subject_kind, work_id, revision_id, content_digest, decision_version,
             target_content_owner, target_work_owner, origin, version, state,
             created_at, updated_at
         )
         VALUES ($1, 'work_revision', $2, $3, $4, $5, $6, $6, 'evaluator_uncertain', 1, 'queued', now(), now())",
    )
    .bind(case_id.as_uuid())
    .bind(work_id)
    .bind(rev_id.as_uuid())
    .bind(actual_digest)
    .bind(dec_ver)
    .bind(author_id)
    .execute(&pool)
    .await
    .unwrap();

    // Admin claims the case
    let (status, _, body) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/works/cases/{case_id}/claim"),
        Some(other_id),
        Some(ORIGIN),
        Some(json!({
            "expected_version": 1,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "claim case failed: {body}");

    // Conclude case as rejected
    let (status, _, body) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/works/cases/{case_id}/conclude"),
        Some(other_id),
        Some(ORIGIN),
        Some(json!({
            "expected_version": 2,
            "verdict": "rejected",
            "reason": "safety_violation",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "conclude case failed: {body}");

    // Step 5: Verify review-withdrawn state!
    // 5a. GET /publication returns 404 (before checking ETag)
    let (status, cache, body) =
        call(&app, "GET", &public_read_path(work_id), None, None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert_eq!(json(&body)["error"]["code"], "not_found");

    // 5b. GET /reader returns 200 with review_withdrawn, work_id, author_name ("Tác giả Mẫu")
    let (status, cache, body) =
        call(&app, "GET", &reader_read_path(work_id), None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    let reader_body = json(&body);
    assert_eq!(reader_body["status"], "review_withdrawn");
    assert_eq!(reader_body["work_id"], work_id.to_string());
    assert_eq!(
        reader_body["author_name"],
        "Tác giả mẫu (dữ liệu phát triển)"
    );

    // 5c. Discussion thread on review-withdrawn work:
    // Reading public discussion succeeds
    let (status, _, _) = call(
        &app,
        "GET",
        &discussion_public_path(work_id),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Submitting a new comment fails at server (404 Not Found)
    let (status, _, body) = call(
        &app,
        "POST",
        &discussion_comments_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "cmd-cmt-1",
            "body": "Bình luận mới sau khi gỡ",
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "new comment must fail: {body}"
    );
    assert_eq!(json(&body)["error"]["code"], "not_found");

    // Step 6: Author sets work to private
    let (status, _, body) = call(
        &app,
        "POST",
        &set_private_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "cmd-priv-1",
            "expected_generation": 3,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "set private failed: {body}");

    // Now GET /reader returns 404 (author name is withdrawn)
    let (status, _, _) = call(&app, "GET", &reader_read_path(work_id), None, None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn test_review_cases_lifecycle_and_grants(pool: PgPool) {
    let app = app(&pool).await;
    let admin_id = dev_seed::OTHER_PRINCIPAL;
    let mod_id = Uuid::now_v7();
    let work_id = dev_seed::SAMPLE_WORK;

    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, 'Moderator Test')")
        .bind(mod_id)
        .execute(&pool)
        .await
        .unwrap();

    // 1. Provision admin
    {
        let mut conn = pool.acquire().await.unwrap();
        add_admin_seat(
            &mut conn,
            "operator-test",
            PrincipalId::from_uuid(admin_id),
            Some("test admin"),
        )
        .await
        .unwrap();
    }

    // 2. Admin creates a Works moderation grant for mod_id
    let (status, _, body) = call(
        &app,
        "POST",
        "/api/v1/moderation/grants",
        Some(admin_id),
        Some(ORIGIN),
        Some(json!({
            "grantee_kind": "principal",
            "grantee_id": mod_id.to_string(),
            "capability": "works",
            "scope_kind": "global",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create grant failed: {body}");
    let grant_id = json(&body)["grant_id"].as_str().unwrap().to_string();

    // 3. Insert a review case
    let case_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO moderation_cases (
             id, subject_kind, work_id, revision_id, content_digest, decision_version,
             target_content_owner, target_work_owner, origin, version, state,
             created_at, updated_at
         )
         VALUES ($1, 'work_revision', $2, $3, $4, 1, $5, $5, 'evaluator_uncertain', 1, 'queued', now(), now())",
    )
    .bind(case_id)
    .bind(work_id)
    .bind(dev_seed::SAMPLE_REVISION)
    .bind(SAMPLE_HASH)
    .bind(dev_seed::SAMPLE_AUTHOR)
    .execute(&pool)
    .await
    .unwrap();

    // 4. Moderator views queue
    let (status, _, body) = call(
        &app,
        "GET",
        "/api/v1/moderation/works/queue",
        Some(mod_id),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "view queue failed: {body}");
    let queue = json(&body);
    assert!(
        queue
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == case_id.to_string())
    );

    // 5. Moderator claims case
    let (status, _, body) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/works/cases/{case_id}/claim"),
        Some(mod_id),
        Some(ORIGIN),
        Some(json!({
            "expected_version": 1,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "claim failed: {body}");
    assert_eq!(json(&body)["version"], 2);

    // 6. Version conflict check: claiming with old version fails with 409
    let (status, _, body) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/works/cases/{case_id}/claim"),
        Some(mod_id),
        Some(ORIGIN),
        Some(json!({
            "expected_version": 1,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["error"]["code"], "case_version_stale");

    // 7. Add note
    let (status, _, body) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/works/cases/{case_id}/note"),
        Some(mod_id),
        Some(ORIGIN),
        Some(json!({
            "expected_version": 2,
            "note": "Ghi chú nội bộ kiểm duyệt",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "add note failed: {body}");
    assert_eq!(json(&body)["version"], 3);

    // 8. Release case
    let (status, _, body) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/works/cases/{case_id}/release"),
        Some(mod_id),
        Some(ORIGIN),
        Some(json!({
            "expected_version": 3,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "release failed: {body}");
    assert_eq!(json(&body)["version"], 4);

    // 9. Admin revokes grant
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/grants/{grant_id}/revoke"),
        Some(admin_id),
        Some(ORIGIN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn test_automation_pause_reroutes_open_jobs(pool: PgPool) {
    let app = app(&pool).await;
    let admin_id = dev_seed::OTHER_PRINCIPAL;
    let author_id = dev_seed::SAMPLE_AUTHOR;
    let work_id = dev_seed::SAMPLE_WORK;

    // 1. Provision admin
    {
        let mut conn = pool.acquire().await.unwrap();
        add_admin_seat(
            &mut conn,
            "operator-test",
            PrincipalId::from_uuid(admin_id),
            Some("test admin"),
        )
        .await
        .unwrap();
    }

    // 2. Submit a work snapshot and request public while automation is live
    let (status, _, body) = call(
        &app,
        "POST",
        &snapshots_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "cmd-snap-pause",
            "expected_draft_version": 1,
            "expected_metadata_version": 1,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let rev_id_str = json(&body)["revision_id"].as_str().unwrap().to_string();

    let (status, _, _body) = call(
        &app,
        "POST",
        &request_public_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "cmd-pub-pause",
            "revision_id": rev_id_str,
            "expected_generation": 2,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Verify job is open
    let job_state: String =
        sqlx::query_scalar("SELECT state FROM work_evaluation_jobs WHERE work_id = $1")
            .bind(work_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job_state, "open");

    // 3. Admin pauses automation
    let (status, _, body) = call(
        &app,
        "POST",
        "/api/v1/moderation/automation/mode",
        Some(admin_id),
        Some(ORIGIN),
        Some(json!({
            "expected_version": 1,
            "mode": "paused",
            "reason": "safety_maintenance",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "pause automation failed: {body}");
    let auto_body = json(&body);
    assert_eq!(auto_body["mode"], "paused");
    assert_eq!(auto_body["version"], 2);

    // 4. Verify evaluation job was rerouted and a human review case was created
    let job_state: String =
        sqlx::query_scalar("SELECT state FROM work_evaluation_jobs WHERE work_id = $1")
            .bind(work_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(job_state, "rerouted");

    let case_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM moderation_cases WHERE work_id = $1 AND origin = 'automation_paused'",
    )
    .bind(work_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(case_count, 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn test_bug1_set_private_withdraws_jobs_and_bug2_request_public_on_conflict(pool: PgPool) {
    let app = app(&pool).await;
    let author_id = dev_seed::SAMPLE_AUTHOR;
    let work_id = dev_seed::SAMPLE_WORK;

    // 1. Capture snapshot
    let (status, _, body) = call(
        &app,
        "POST",
        &snapshots_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "cmd-snap-bug",
            "expected_draft_version": 1,
            "expected_metadata_version": 1,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let rev_id_str = json(&body)["revision_id"].as_str().unwrap().to_string();

    // 2. Request public -> creates ticket and job
    let (status, _, _) = call(
        &app,
        "POST",
        &request_public_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "cmd-pub-bug-1",
            "revision_id": rev_id_str,
            "expected_generation": 2,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Bug 1: set-private withdraws the open evaluation job and closes open cases
    let (status, _, body) = call(
        &app,
        "POST",
        &set_private_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "cmd-priv-bug-1",
            "expected_generation": 3,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "set-private failed: {body}");

    let job_state: String =
        sqlx::query_scalar("SELECT state FROM work_evaluation_jobs WHERE work_id = $1")
            .bind(work_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        job_state, "withdrawn",
        "open job must be withdrawn on set-private"
    );

    // Bug 2: Re-request public for the SAME revision_id
    // This previously hit primary key violation on work_revision_moderation.
    // With ON CONFLICT DO UPDATE, it succeeds cleanly!
    let (status, _, body) = call(
        &app,
        "POST",
        &request_public_path(work_id),
        Some(author_id),
        Some(ORIGIN),
        Some(json!({
            "command_key": "cmd-pub-bug-2",
            "revision_id": rev_id_str,
            "expected_generation": 4,
        })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "re-request-public must succeed without PK conflict: {body}"
    );

    // A new open job exists for the new evaluation ticket
    let open_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM work_evaluation_jobs WHERE work_id = $1 AND state = 'open'",
    )
    .bind(work_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(open_count, 1);
}

// ---------------------------------------------------------------------------
// Regressions of the #159 review (claim fence, pause, retries, page, authority, transactions).
// ---------------------------------------------------------------------------

async fn new_principal(pool: &PgPool, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seat_admin(pool: &PgPool, id: Uuid) {
    let mut conn = pool.acquire().await.unwrap();
    assert!(
        add_admin_seat(&mut conn, "operator-test", PrincipalId::from_uuid(id), None)
            .await
            .unwrap()
    );
}

async fn snapshot(app: &axum::Router, work: Uuid, key: &str, draft_version: i64) -> String {
    let (status, _, body) = call(
        app,
        "POST",
        &snapshots_path(work),
        Some(dev_seed::SAMPLE_AUTHOR),
        Some(ORIGIN),
        Some(json!({
            "command_key": key,
            "expected_draft_version": draft_version,
            "expected_metadata_version": 1,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "snapshot failed: {body}");
    json(&body)["revision_id"].as_str().unwrap().to_owned()
}

async fn request_public(
    app: &axum::Router,
    work: Uuid,
    rev: &str,
    generation: i64,
    key: &str,
) -> i64 {
    let (status, _, body) = call(
        app,
        "POST",
        &request_public_path(work),
        Some(dev_seed::SAMPLE_AUTHOR),
        Some(ORIGIN),
        Some(json!({
            "command_key": key,
            "revision_id": rev,
            "expected_generation": generation,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "request public failed: {body}");
    json(&body)["generation"].as_i64().unwrap()
}

async fn set_scenario(pool: &PgPool, work: Uuid, scenario: &str) {
    sqlx::query(
        "UPDATE work_evaluation_jobs SET scenario = $2 WHERE work_id = $1 AND state = 'open'",
    )
    .bind(work)
    .bind(scenario)
    .execute(pool)
    .await
    .unwrap();
}

async fn scalar_i64(pool: &PgPool, sql: &'static str, work: Uuid) -> i64 {
    sqlx::query_scalar(sql)
        .bind(work)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn open_case(pool: &PgPool, work: Uuid) -> (Uuid, i64) {
    sqlx::query_as(
        "SELECT id, version FROM moderation_cases
          WHERE work_id = $1 AND state IN ('queued', 'claimed', 'escalated')",
    )
    .bind(work)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn case_state(pool: &PgPool, case: Uuid) -> (String, i64) {
    sqlx::query_as("SELECT state, version FROM moderation_cases WHERE id = $1")
        .bind(case)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn post(
    app: &axum::Router,
    path: &str,
    actor: Uuid,
    body: serde_json::Value,
) -> (StatusCode, String) {
    let (status, _, body) = call(app, "POST", path, Some(actor), Some(ORIGIN), Some(body)).await;
    (status, body)
}

async fn reader_status(app: &axum::Router, work: Uuid) -> (StatusCode, serde_json::Value) {
    let (status, _, body) = call(app, "GET", &reader_read_path(work), None, None, None).await;
    let value = if status == StatusCode::OK {
        json(&body)
    } else {
        serde_json::Value::Null
    };
    (status, value)
}

/// O-17@r2: the page at the old URL keeps the discussion rules, survives a pending
/// resubmission (the store used to clear it on request-public) and yields to the next approval.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn review_withdrawn_page_survives_a_pending_resubmission(pool: PgPool) {
    let app = app(&pool).await;
    let work = dev_seed::SAMPLE_WORK;
    let reader = dev_seed::OTHER_PRINCIPAL;
    let admin = new_principal(&pool, "Quản trị").await;
    seat_admin(&pool, admin).await;

    let r2 = snapshot(&app, work, "snapshot-r2", 1).await;
    let generation = request_public(&app, work, &r2, 2, "publish-r2").await;
    set_scenario(&pool, work, "approve").await;
    assert!(EvaluationWorker::run_once(&pool, "w").await.unwrap());
    assert_eq!(reader_status(&app, work).await.1["status"], "published");

    // A reader's comment waits in premoderation when the revision is rejected.
    let (status, body) = post(
        &app,
        &discussion_comments_path(work),
        reader,
        json!({ "command_key": "comment-1", "body": "Bài thơ đẹp quá." }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let comment = json(&body)["comment_id"].as_str().unwrap().to_owned();

    // The evaluator approved r2; an admin opens an intervention case on it (MOD-13). The author
    // holds no seat and could never review their own work.
    let open_path = format!("/api/v1/moderation/works/{work}/revisions/{r2}/cases");
    let (status, _) = post(&app, &open_path, dev_seed::SAMPLE_AUTHOR, json!({})).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = post(&app, &open_path, admin, json!({})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let case = json(&body)["case_id"].as_str().unwrap().to_owned();
    let (status, body) = post(&app, &open_path, admin, json!({})).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "one open case per revision: {body}"
    );
    let cases = format!("/api/v1/moderation/works/cases/{case}");
    assert_eq!(
        post(
            &app,
            &format!("{cases}/claim"),
            admin,
            json!({ "expected_version": 1 })
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, body) = post(
        &app,
        &format!("{cases}/conclude"),
        admin,
        json!({ "expected_version": 2, "verdict": "rejected", "reason": "Vi phạm chính sách" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        reader_status(&app, work).await.1["status"],
        "review_withdrawn"
    );

    // The comment writer still sees and may remove their own comment; nothing new is accepted.
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/api/v1/works/{work}/discussion/me"),
        Some(reader),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains(&comment),
        "own pending comment stays visible to its writer"
    );
    let (status, _, body) = call(
        &app,
        "DELETE",
        &format!("/api/v1/works/{work}/discussion/comments/{comment}"),
        Some(reader),
        Some(ORIGIN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "own removal stays allowed: {body}");
    let (status, _) = post(
        &app,
        &discussion_comments_path(work),
        reader,
        json!({ "command_key": "comment-2", "body": "Bình luận mới." }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A pending r3 keeps the page; its approval replaces it.
    let vink = vink_app::application::Vink::new(vink_app::persistence::PgStore::new(pool.clone()));
    vink.save_draft(
        author(),
        save(
            WorkId::from_uuid(work),
            1,
            "Bản sửa",
            doc(&[&["Một khổ mới,", "đã sửa."]]),
        ),
    )
    .await
    .unwrap();
    let r3 = snapshot(&app, work, "snapshot-r3", 2).await;
    let generation = request_public(&app, work, &r3, generation, "publish-r3").await;
    assert!(generation > 0);
    let (status, page) = reader_status(&app, work).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        page["status"], "review_withdrawn",
        "a pending resubmission keeps the page"
    );
    set_scenario(&pool, work, "approve").await;
    assert!(EvaluationWorker::run_once(&pool, "w").await.unwrap());
    let (_, page) = reader_status(&app, work).await;
    assert_eq!(page["status"], "published");
    assert_eq!(page["revision_id"], r3);

    // r2's history: submitted (1), evaluator approval (2), admin rejection (3).
    let history: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT decision_version, moderation, provenance_kind FROM work_revision_decision_history
          WHERE revision_id = $1 ORDER BY decision_version",
    )
    .bind(Uuid::parse_str(&r2).unwrap())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        history,
        vec![
            (1, "pending".to_owned(), None),
            (2, "approved".to_owned(), Some("ai_approved".to_owned())),
            (3, "rejected".to_owned(), Some("admin".to_owned())),
        ]
    );
}

/// The finisher re-checks its claim at commit: a reclaim (new generation) or a pause committed
/// between claim and finish leaves decisions and pointers untouched.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn stale_claim_or_pause_before_finish_changes_nothing(pool: PgPool) {
    use vink_app::domain::moderation::EvaluatorOutcome;
    use vink_app::persistence::evaluation::{JobOutcome, claim_job, finish_job};

    let app = app(&pool).await;
    let work = dev_seed::SAMPLE_WORK;
    let admin = new_principal(&pool, "Quản trị").await;
    seat_admin(&pool, admin).await;
    let r2 = snapshot(&app, work, "snapshot-r2", 1).await;
    request_public(&app, work, &r2, 2, "publish-r2").await;
    let published = "SELECT count(*) FROM publications WHERE work_id = $1 AND revision_id <> '44444444-4444-4444-8444-444444444444'";

    // Reclaimed by another worker after the lease: the first finisher is stale.
    let first = claim_job(&pool, "w1", 30, None).await.unwrap().unwrap();
    sqlx::query("UPDATE work_evaluation_jobs SET lease_expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();
    let second = claim_job(&pool, "w2", 30, None).await.unwrap().unwrap();
    assert_eq!(second.id, first.id, "a reclaim keeps the evaluation id");
    assert_eq!(second.claim_generation, first.claim_generation + 1);
    let outcome = finish_job(&pool, "w1", &first, Some(EvaluatorOutcome::Approve), 0)
        .await
        .unwrap();
    assert_eq!(outcome, JobOutcome::Stale);
    assert_eq!(scalar_i64(&pool, published, work).await, 0);

    // A pause commits between the second claim and its finish: rerouted, case opened, no effect.
    let (status, body) = post(
        &app,
        "/api/v1/moderation/automation/mode",
        admin,
        json!({ "expected_version": 1, "mode": "paused", "reason": "Bảo trì" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let outcome = finish_job(&pool, "w2", &second, Some(EvaluatorOutcome::Approve), 0)
        .await
        .unwrap();
    assert_eq!(outcome, JobOutcome::Stale);
    assert_eq!(scalar_i64(&pool, published, work).await, 0);
    let state: String = sqlx::query_scalar("SELECT state FROM work_evaluation_jobs WHERE id = $1")
        .bind(first.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "rerouted");
    assert_eq!(
        scalar_i64(&pool, "SELECT count(*) FROM moderation_cases WHERE work_id = $1 AND origin = 'automation_paused' AND decision_version = 1", work).await,
        1
    );
    assert!(
        claim_job(&pool, "w3", 30, None).await.unwrap().is_none(),
        "nothing is claimed while paused"
    );
    assert_eq!(
        scalar_i64(&pool, "SELECT count(*) FROM work_revision_moderation WHERE work_id = $1 AND moderation = 'pending' AND provenance_kind IS NULL", work).await,
        1,
        "the revision stays pending for people"
    );
}

/// MOD-14: a submission while automation is paused goes straight to the human queue.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn paused_submission_goes_to_the_human_queue(pool: PgPool) {
    let app = app(&pool).await;
    let work = dev_seed::SAMPLE_WORK;
    let admin = new_principal(&pool, "Quản trị").await;
    seat_admin(&pool, admin).await;
    let (status, body) = post(
        &app,
        "/api/v1/moderation/automation/mode",
        admin,
        json!({ "expected_version": 1, "mode": "paused", "reason": "Bảo trì" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let r2 = snapshot(&app, work, "snapshot-r2", 1).await;
    request_public(&app, work, &r2, 2, "publish-r2").await;
    assert_eq!(
        scalar_i64(
            &pool,
            "SELECT count(*) FROM work_evaluation_jobs WHERE work_id = $1",
            work
        )
        .await,
        0
    );
    assert_eq!(
        scalar_i64(&pool, "SELECT count(*) FROM moderation_cases WHERE work_id = $1 AND origin = 'automation_paused' AND state = 'queued'", work).await,
        1
    );
    assert_eq!(
        scalar_i64(
            &pool,
            "SELECT count(*) FROM moderation_automation_audit WHERE mode = 'paused'",
            work
        )
        .await,
        1
    );
}

/// MOD-08: a retryable failure opens a fresh evaluation id for the same submission; after three
/// attempts the revision stays pending and goes to people.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn retryable_failures_use_fresh_evaluation_ids_then_go_to_people(pool: PgPool) {
    let app = app(&pool).await;
    let work = dev_seed::SAMPLE_WORK;
    let r2 = snapshot(&app, work, "snapshot-r2", 1).await;
    request_public(&app, work, &r2, 2, "publish-r2").await;
    set_scenario(&pool, work, "timeout").await;
    for _ in 0..3 {
        assert!(EvaluationWorker::run_once(&pool, "w").await.unwrap());
    }
    assert!(
        !EvaluationWorker::run_once(&pool, "w").await.unwrap(),
        "no fourth attempt"
    );
    let jobs: Vec<(Uuid, String, i32)> = sqlx::query_as(
        "SELECT id, state, attempts FROM work_evaluation_jobs WHERE work_id = $1 ORDER BY attempts",
    )
    .bind(work)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        jobs.len(),
        3,
        "each business retry has its own evaluation id"
    );
    assert!(jobs.iter().all(|(_, state, _)| state == "failed"));
    assert_eq!(jobs.iter().map(|j| j.2).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(
        scalar_i64(&pool, "SELECT count(*) FROM moderation_cases WHERE work_id = $1 AND origin = 'retries_exhausted' AND decision_version = 1", work).await,
        1
    );
    assert_eq!(
        scalar_i64(&pool, "SELECT count(*) FROM work_revision_moderation WHERE work_id = $1 AND moderation = 'pending'", work).await,
        1
    );
}

/// A stale note is not stored; a claimant whose grant expired is refused at conclude and the
/// case returns to the queue in that refusal; an admin then decides on the exact snapshot.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn stale_note_and_lost_authority_are_persisted_honestly(pool: PgPool) {
    let app = app(&pool).await;
    let work = dev_seed::SAMPLE_WORK;
    let admin = new_principal(&pool, "Quản trị").await;
    let moderator = new_principal(&pool, "Người duyệt").await;
    seat_admin(&pool, admin).await;
    let (status, body) = post(
        &app,
        "/api/v1/moderation/grants",
        admin,
        json!({ "grantee_kind": "principal", "grantee_id": moderator, "capability": "works", "scope_kind": "work", "scope_work_id": work }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let grant = json(&body)["grant_id"].as_str().unwrap().to_owned();

    let r2 = snapshot(&app, work, "snapshot-r2", 1).await;
    request_public(&app, work, &r2, 2, "publish-r2").await;
    set_scenario(&pool, work, "needs_admin").await;
    assert!(EvaluationWorker::run_once(&pool, "w").await.unwrap());
    let (case, version) = open_case(&pool, work).await;
    assert_eq!(version, 1);
    let cases = format!("/api/v1/moderation/works/cases/{case}");

    // The reviewer reads the exact submitted snapshot.
    let (status, _, body) = call(&app, "GET", &cases, Some(moderator), None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let detail = json(&body);
    assert_eq!(detail["revision_id"], r2);
    assert!(detail["document"]["blocks"].is_array());
    assert_eq!(detail["decision_version"], 2);

    assert_eq!(
        post(
            &app,
            &format!("{cases}/claim"),
            moderator,
            json!({ "expected_version": 1 })
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, _) = post(
        &app,
        &format!("{cases}/note"),
        moderator,
        json!({ "expected_version": 1, "note": "cũ" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        scalar_i64(&pool, "SELECT count(*) FROM moderation_case_notes n JOIN moderation_cases c ON c.id = n.case_id WHERE c.work_id = $1", work).await,
        0,
        "a refused note leaves no row"
    );
    assert_eq!(
        post(
            &app,
            &format!("{cases}/note"),
            moderator,
            json!({ "expected_version": 2, "note": "Đã đọc" })
        )
        .await
        .0,
        StatusCode::OK
    );

    sqlx::query(
        "UPDATE moderation_grants SET expires_at = now() - interval '1 second' WHERE id = $1::uuid",
    )
    .bind(&grant)
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = post(
        &app,
        &format!("{cases}/conclude"),
        moderator,
        json!({ "expected_version": 3, "verdict": "approved", "reason": "Ổn" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(case_state(&pool, case).await, ("queued".to_owned(), 4));

    assert_eq!(
        post(
            &app,
            &format!("{cases}/claim"),
            admin,
            json!({ "expected_version": 4 })
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, body) = post(
        &app,
        &format!("{cases}/conclude"),
        admin,
        json!({ "expected_version": 5, "verdict": "approved", "reason": "Phù hợp" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, page) = reader_status(&app, work).await;
    assert_eq!(page["revision_id"], r2);
}

/// Group grants work through the API; recusal hides one's own work; removing a member returns
/// their claim to the queue in the same commit.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn group_grants_recusal_and_membership_removal(pool: PgPool) {
    let app = app(&pool).await;
    let work = dev_seed::SAMPLE_WORK;
    let author_id = dev_seed::SAMPLE_AUTHOR;
    let admin = new_principal(&pool, "Quản trị").await;
    let moderator = new_principal(&pool, "Người duyệt").await;
    seat_admin(&pool, admin).await;

    let (status, body) = post(
        &app,
        "/api/v1/moderation/groups",
        admin,
        json!({ "name": "Ban biên tập" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let group = json(&body)["group_id"].as_str().unwrap().to_owned();
    for member in [moderator, author_id] {
        let (status, body) = post(
            &app,
            &format!("/api/v1/moderation/groups/{group}/members"),
            admin,
            json!({ "principal_id": member }),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    }
    let (status, body) = post(
        &app,
        "/api/v1/moderation/grants",
        admin,
        json!({ "grantee_kind": "group", "grantee_id": group, "capability": "works", "scope_kind": "global" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let r2 = snapshot(&app, work, "snapshot-r2", 1).await;
    request_public(&app, work, &r2, 2, "publish-r2").await;
    set_scenario(&pool, work, "needs_admin").await;
    assert!(EvaluationWorker::run_once(&pool, "w").await.unwrap());
    let (case, _) = open_case(&pool, work).await;
    let cases = format!("/api/v1/moderation/works/cases/{case}");

    // The author holds the group grant but never sees or takes a case on their own work.
    let (status, _, body) = call(
        &app,
        "GET",
        "/api/v1/moderation/works/queue",
        Some(author_id),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains(&case.to_string()));
    let (status, _, body) = call(&app, "GET", &cases, Some(author_id), None, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json(&body)["error"]["code"], "recused");
    assert_eq!(
        post(
            &app,
            &format!("{cases}/claim"),
            author_id,
            json!({ "expected_version": 1 })
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    assert_eq!(
        post(
            &app,
            &format!("{cases}/claim"),
            moderator,
            json!({ "expected_version": 1 })
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, _, body) = call(
        &app,
        "DELETE",
        &format!("/api/v1/moderation/groups/{group}/members/{moderator}"),
        Some(admin),
        Some(ORIGIN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(case_state(&pool, case).await, ("queued".to_owned(), 3));
    assert_eq!(
        scalar_i64(&pool, "SELECT count(*) FROM moderation_directory_audit WHERE change IN ('group_created', 'joined', 'left', 'granted') AND $1::uuid IS NOT NULL", work).await,
        5
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn admin_cli_returns_connection_before_closing_pool(pool: PgPool) {
    use sqlx::ConnectOptions;
    use std::process::{Command, Stdio};
    seeded(&pool).await;
    for op in ["add", "list", "remove"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_vink-app"));
        command.arg("admin").arg(op);
        if op != "list" {
            command.arg(dev_seed::OTHER_PRINCIPAL.to_string());
        }
        let mut child = command
            .env("VINK_PROFILE", "local")
            .env(
                "DATABASE_URL",
                pool.connect_options().to_url_lossy().as_str(),
            )
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await;
        if result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(
            result
                .expect("admin CLI must exit after reporting its result")
                .success()
        );
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn hidden_reader_retains_only_discussion_and_republication_restores_body(pool: PgPool) {
    let app = app(&pool).await;
    let work = dev_seed::SAMPLE_WORK;
    let writer = dev_seed::OTHER_PRINCIPAL;
    let (status, _, body) = call(
        &app,
        "POST",
        &discussion_comments_path(work),
        Some(writer),
        Some(ORIGIN),
        Some(json!({"body":"Lời riêng của độc giả", "command_key":"hidden-comment"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let comment = json(&body)["comment_id"].as_str().unwrap().to_owned();
    let (status, _, body) = call(
        &app,
        "POST",
        &set_private_path(work),
        Some(dev_seed::SAMPLE_AUTHOR),
        Some(ORIGIN),
        Some(json!({"command_key":"hide-public", "expected_generation":2})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, cache, body) = call(&app, "GET", &reader_read_path(work), None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert_eq!(json(&body), json!({"status":"hidden", "work_id":work}));
    assert_eq!(
        call(&app, "GET", &public_read_path(work), None, None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, "GET", &discussion_public_path(work), None, None, None)
            .await
            .0,
        StatusCode::OK
    );
    let (status, _, body) = call(
        &app,
        "POST",
        &discussion_comments_path(work),
        Some(writer),
        Some(ORIGIN),
        Some(json!({"body":"Không ghi mới", "command_key":"hidden-new"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(json(&body)["error"]["code"], "hidden_read_only");
    let (status, _, body) = call(
        &app,
        "DELETE",
        &format!("/api/v1/works/{work}/discussion/comments/{comment}"),
        Some(writer),
        Some(ORIGIN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, body) = call(&app, "POST", &request_public_path(work), Some(dev_seed::SAMPLE_AUTHOR), Some(ORIGIN), Some(json!({"command_key":"restore-hidden", "revision_id":dev_seed::SAMPLE_REVISION, "expected_generation":3}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _, body) = call(&app, "GET", &reader_read_path(work), None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["status"], "published");
}

async fn reject_for_appeal(pool: &PgPool, app: &Router) -> String {
    let work = dev_seed::SAMPLE_WORK;
    let author = dev_seed::SAMPLE_AUTHOR;
    let (status, _, body) = call(app, "POST", &snapshots_path(work), Some(author), Some(ORIGIN), Some(json!({"command_key":"appeal-snapshot", "expected_draft_version":1,"expected_metadata_version":1}))).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let revision = json(&body)["revision_id"].as_str().unwrap().to_owned();
    let (status, _, body) = call(
        app,
        "POST",
        &request_public_path(work),
        Some(author),
        Some(ORIGIN),
        Some(
            json!({"command_key":"appeal-publish", "expected_generation":2,"revision_id":revision}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    sqlx::query("UPDATE work_evaluation_jobs SET scenario = 'reject' WHERE work_id = $1")
        .bind(work)
        .execute(pool)
        .await
        .unwrap();
    assert!(
        EvaluationWorker::run_once(pool, "appeal-worker")
            .await
            .unwrap()
    );
    revision
}
async fn file_appeal(app: &Router, revision: &str) -> Value {
    let (status, cache, body) = call(
        app,
        "POST",
        &format!(
            "/api/v1/works/{}/revisions/{revision}/appeals",
            dev_seed::SAMPLE_WORK
        ),
        Some(dev_seed::SAMPLE_AUTHOR),
        Some(ORIGIN),
        Some(json!({"expected_decision_version":2})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    json(&body)
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn work_appeal_deduplicates_and_only_admin_concludes_with_history(pool: PgPool) {
    let app = app(&pool).await;
    let revision = reject_for_appeal(&pool, &app).await;
    let (a, b) = tokio::join!(file_appeal(&app, &revision), file_appeal(&app, &revision));
    assert_eq!(a["appeal"]["id"], b["appeal"]["id"]);
    assert_ne!(a["replayed"], b["replayed"]);
    assert_eq!(a["appeal"]["notice"], "not_sent");
    let golden: Value = serde_json::from_str(include_str!(
        "../../contracts/http/v1/fixtures/appeal.open.json"
    ))
    .unwrap();
    let mut wire = a["appeal"].clone();
    // Only generated identifiers/time differ; all public fields must match the consumer fixture.
    for key in ["id", "work_id", "revision_id", "created_at"] {
        wire[key] = golden[key].clone();
    }
    assert_eq!(wire, golden);
    let id = a["appeal"]["id"].as_str().unwrap();
    // Real pre-activation backout must refuse as soon as an appeal is owed.
    let mut conn = pool.acquire().await.unwrap();
    let refused = sqlx::raw_sql(include_str!(
        "../../migrations-rollback/20261118000000_work_appeals.down.sql"
    ))
    .execute(&mut *conn)
    .await
    .unwrap_err();
    assert!(refused.to_string().contains("roll forward"));
    sqlx::query("ROLLBACK").execute(&mut *conn).await.unwrap();
    drop(conn);
    let path = format!("/api/v1/moderation/appeals/{id}/conclude");
    let request = json!({"expected_decision_version":2,"verdict":"reverse","reason":"Bản thảo phù hợp chính sách"});
    assert_eq!(
        call(
            &app,
            "POST",
            &path,
            Some(dev_seed::OTHER_PRINCIPAL),
            Some(ORIGIN),
            Some(request.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    add_admin_seat(
        &mut pool.acquire().await.unwrap(),
        "test",
        PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL),
        None,
    )
    .await
    .unwrap();
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("/api/v1/moderation/appeals/{id}"),
        Some(dev_seed::OTHER_PRINCIPAL),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(json(&body)["document"].is_object());
    let (status, _, body) = call(
        &app,
        "POST",
        &path,
        Some(dev_seed::OTHER_PRINCIPAL),
        Some(ORIGIN),
        Some(request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(json(&body)["state"], "reversed");
    assert_eq!(
        call(
            &app,
            "POST",
            &path,
            Some(dev_seed::OTHER_PRINCIPAL),
            Some(ORIGIN),
            Some(request)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let row:(i64,String,Option<Uuid>)=sqlx::query_as("SELECT decision_version,provenance_kind,appeal_id FROM work_revision_decision_history WHERE appeal_id=$1").bind(Uuid::parse_str(id).unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!((row.0, row.1.as_str()), (3, "admin"));
    assert!(row.2.is_some());
    let (status, _, body) = call(
        &app,
        "POST",
        &format!(
            "/api/v1/works/{}/revisions/{revision}/appeals",
            dev_seed::SAMPLE_WORK
        ),
        Some(dev_seed::SAMPLE_AUTHOR),
        Some(ORIGIN),
        Some(json!({"expected_decision_version":3})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["error"]["code"], "final_decision");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn appeal_waits_for_seat_removal_before_taking_the_principal(pool: PgPool) {
    let app = app(&pool).await;
    let revision = reject_for_appeal(&pool, &app).await;
    let appeal = file_appeal(&app, &revision).await;
    let path = format!(
        "/api/v1/moderation/appeals/{}/conclude",
        appeal["appeal"]["id"].as_str().unwrap()
    );
    let admin = dev_seed::OTHER_PRINCIPAL;
    add_admin_seat(
        &mut pool.acquire().await.unwrap(),
        "test",
        PrincipalId::from_uuid(admin),
        None,
    )
    .await
    .unwrap();

    let mut removal = pool.begin().await.unwrap();
    let blocker = backend_pid(&mut removal).await;
    sqlx::query("DELETE FROM moderation_admin_seats WHERE principal_id = $1")
        .bind(admin)
        .execute(&mut *removal)
        .await
        .unwrap();
    let concluding = tokio::spawn(async move {
        call(
            &app,
            "POST",
            &path,
            Some(admin),
            Some(ORIGIN),
            Some(
                json!({"expected_decision_version": 2, "verdict": "reverse", "reason": "Xem lại"}),
            ),
        )
        .await
    });
    wait_blocked_by(&pool, blocker).await;
    sqlx::query("SET LOCAL lock_timeout = '2s'")
        .execute(&mut *removal)
        .await
        .unwrap();
    // The removal audit takes this FK lock after deleting the seat.
    sqlx::query("SELECT id FROM principals WHERE id = $1 FOR KEY SHARE")
        .bind(admin)
        .execute(&mut *removal)
        .await
        .unwrap();
    removal.commit().await.unwrap();
    let (status, _, body) = concluding.await.unwrap();
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(json(&body)["error"]["code"], "not_admin");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn private_closes_appeal_and_refuses_late_conclusion_and_snapshot_reads(pool: PgPool) {
    let app = app(&pool).await;
    let revision = reject_for_appeal(&pool, &app).await;
    let a = file_appeal(&app, &revision).await;
    let id = a["appeal"]["id"].as_str().unwrap();
    add_admin_seat(
        &mut pool.acquire().await.unwrap(),
        "test",
        PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL),
        None,
    )
    .await
    .unwrap();
    let (status, _, body) = call(
        &app,
        "POST",
        &set_private_path(dev_seed::SAMPLE_WORK),
        Some(dev_seed::SAMPLE_AUTHOR),
        Some(ORIGIN),
        Some(json!({"command_key":"withdraw-appeal","expected_generation":3})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let path = format!("/api/v1/moderation/appeals/{id}");
    assert_eq!(
        call(
            &app,
            "GET",
            &path,
            Some(dev_seed::OTHER_PRINCIPAL),
            None,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &(path + "/conclude"),
            Some(dev_seed::OTHER_PRINCIPAL),
            Some(ORIGIN),
            Some(json!({"expected_decision_version":2,"verdict":"reverse","reason":"Quá muộn"}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let row: (String, String) =
        sqlx::query_as("SELECT state,close_reason FROM work_appeals WHERE id=$1")
            .bind(Uuid::parse_str(id).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row, ("closed".into(), "withdrawn".into()));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn appeal_reserves_decision_and_concurrent_conclusions_apply_once(pool: PgPool) {
    let app = app(&pool).await;
    let revision = reject_for_appeal(&pool, &app).await;
    let path = format!(
        "/api/v1/works/{}/revisions/{revision}/appeals",
        dev_seed::SAMPLE_WORK
    );
    let (status, _, body) = call(
        &app,
        "POST",
        &path,
        Some(dev_seed::SAMPLE_AUTHOR),
        Some(ORIGIN),
        Some(json!({"expected_decision_version":1})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json(&body)["error"]["code"], "stale_decision");
    assert_eq!(
        call(
            &app,
            "POST",
            &path,
            Some(dev_seed::OTHER_PRINCIPAL),
            Some(ORIGIN),
            Some(json!({"expected_decision_version":2}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let filed = file_appeal(&app, &revision).await;
    let id = filed["appeal"]["id"].as_str().unwrap();
    add_admin_seat(
        &mut pool.acquire().await.unwrap(),
        "test",
        PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL),
        None,
    )
    .await
    .unwrap();
    let open = format!(
        "/api/v1/moderation/works/{}/revisions/{revision}/cases",
        dev_seed::SAMPLE_WORK
    );
    let (_, _, opened) = call(
        &app,
        "POST",
        &open,
        Some(dev_seed::OTHER_PRINCIPAL),
        Some(ORIGIN),
        None,
    )
    .await;
    let case = json(&opened)["case_id"].as_str().unwrap().to_owned();
    let (_, _, claimed) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/works/cases/{case}/claim"),
        Some(dev_seed::OTHER_PRINCIPAL),
        Some(ORIGIN),
        Some(json!({"expected_version":1})),
    )
    .await;
    let (status,_,body)=call(&app,"POST",&format!("/api/v1/moderation/works/cases/{case}/conclude"),Some(dev_seed::OTHER_PRINCIPAL),Some(ORIGIN),Some(json!({"expected_version":json(&claimed)["version"],"verdict":"approved","reason":"Must use appeal"}))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(json(&body)["error"]["code"], "appeal_open");
    let conclude = format!("/api/v1/moderation/appeals/{id}/conclude");
    let request = json!({"expected_decision_version":2,"verdict":"uphold","reason":"Giữ quyết định có căn cứ"});
    let (a, b) = tokio::join!(
        call(
            &app,
            "POST",
            &conclude,
            Some(dev_seed::OTHER_PRINCIPAL),
            Some(ORIGIN),
            Some(request.clone())
        ),
        call(
            &app,
            "POST",
            &conclude,
            Some(dev_seed::OTHER_PRINCIPAL),
            Some(ORIGIN),
            Some(request)
        )
    );
    assert!(
        matches!(
            (a.0, b.0),
            (StatusCode::OK, StatusCode::CONFLICT) | (StatusCode::CONFLICT, StatusCode::OK)
        ),
        "{} {}",
        a.2,
        b.2
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM work_revision_decision_history WHERE appeal_id=$1",
    )
    .bind(Uuid::parse_str(id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn appeal_on_old_revision_never_overwrites_new_public_intent(pool: PgPool) {
    let app = app(&pool).await;
    let old = reject_for_appeal(&pool, &app).await;
    let filed = file_appeal(&app, &old).await;
    let id = filed["appeal"]["id"].as_str().unwrap();
    let work = dev_seed::SAMPLE_WORK;
    let (status,_,body)=call(&app,"PUT",&format!("/api/v1/drafts/{work}"),Some(dev_seed::SAMPLE_AUTHOR),Some(ORIGIN),Some(json!({"expected_version":1,"title":"Bản mới đang chờ", "document":{"schema_version":1,"blocks":[{"lines":["Nội dung mới"]}]}}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status,_,body)=call(&app,"POST",&snapshots_path(work),Some(dev_seed::SAMPLE_AUTHOR),Some(ORIGIN),Some(json!({"command_key":"appeal-new-snapshot","expected_draft_version":2,"expected_metadata_version":1}))).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let new_revision = json(&body)["revision_id"].as_str().unwrap().to_owned();
    let (status,_,body)=call(&app,"POST",&request_public_path(work),Some(dev_seed::SAMPLE_AUTHOR),Some(ORIGIN),Some(json!({"command_key":"appeal-new-intent","expected_generation":3,"revision_id":new_revision}))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    add_admin_seat(
        &mut pool.acquire().await.unwrap(),
        "test",
        PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL),
        None,
    )
    .await
    .unwrap();
    let (status, _, body) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/appeals/{id}/conclude"),
        Some(dev_seed::OTHER_PRINCIPAL),
        Some(ORIGIN),
        Some(json!({"expected_decision_version":2,"verdict":"reverse","reason":"Bản cũ phù hợp"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (intent, pointer): (Uuid, Option<Uuid>) = sqlx::query_as(
        "SELECT intent_revision_id,pointer_revision_id FROM work_publications WHERE work_id=$1",
    )
    .bind(work)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(intent.to_string(), new_revision);
    assert_ne!(
        pointer.map(|id| id.to_string()).as_deref(),
        Some(old.as_str())
    );
}
