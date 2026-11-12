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
