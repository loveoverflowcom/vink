//! Notifications v1 producers and relay on real PostgreSQL (#346; contract
//! `contracts/http/v1/notifications.md` §1): activation cutover, the five producing transitions
//! and their invalidations, source atomicity, and at-least-once delivery with a fenced ACK.
//!
//! `Receiver` is a labelled fixture of the Notification Service's ingestion (#347 owns the real
//! one): it keeps a receipt per `event_id` with its digest and counts effects, so a duplicate is
//! `already_applied` and a different payload under the same ID is `payload_mismatch`. It proves
//! the App side only; a two-process run needs #347. "Time travel" moves `available_at` into the
//! past with SQL instead of sleeping through a lease or a backoff.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::watch;
use vink_app::application::discussion::{ModerateComment, ModerationAction, SubmitComment};
use vink_app::application::endorsement::SetWorkEndorsement;
use vink_app::domain::discussion::outbox::{OutboxPolicy, StaleReason};
use vink_app::domain::ids::{CaseId, CommentId, RevisionId};
use vink_app::domain::notification::{
    IngestStatus, RelayCompletion, RelayDeadReason, RelayFailure, RelayResult, verify_wire,
};
use vink_app::domain::principal::SuspensionReason;
use vink_app::evaluation_worker::EvaluationWorker;
use vink_app::notification_relay::{
    BoxFuture, IngestReceipt, NotificationRelay, NotificationSink, Outgoing, RelaySettings,
    SinkFailure,
};
use vink_app::persistence::moderation::add_admin_seat;
use vink_app::persistence::notification::{self, RequeueOutcome};
use vink_app::persistence::{reinstate_principal, suspend_principal};

use super::outbox::{MODERATOR, post, setup};
use super::*;

fn policy() -> OutboxPolicy {
    // 3 attempts, 30 s lease, 1 s → 10 s backoff, 1 h deadline.
    OutboxPolicy::new(3, 30_000_000, 1_000_000, 10_000_000, 3_600_000_000).unwrap()
}

fn settings() -> RelaySettings {
    RelaySettings {
        policy: policy(),
        send_timeout: Duration::from_secs(5),
        poll_min: Duration::from_millis(20),
        poll_max: Duration::from_millis(100),
        drain: Duration::from_secs(5),
        ..RelaySettings::default()
    }
}

async fn activate(pool: &PgPool) {
    notification::set_activation(pool, true, "kiểm thử #346: bật thông báo")
        .await
        .unwrap();
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
struct Stored {
    id: Uuid,
    event_type: String,
    dedup_key: String,
    activation_epoch: i64,
    envelope: String,
    payload_digest: String,
}

impl Stored {
    fn json(&self) -> Value {
        serde_json::from_str(&self.envelope).unwrap()
    }
}

async fn events(pool: &PgPool) -> Vec<Stored> {
    sqlx::query_as(
        "SELECT id, event_type, dedup_key, activation_epoch, envelope, payload_digest
           FROM notification_events ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

fn types(events: &[Stored]) -> Vec<(String, String)> {
    events
        .iter()
        .map(|e| {
            (
                e.event_type.clone(),
                e.json()["semantic_kind"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn recipients(event: &Stored) -> Vec<String> {
    event.json()["recipients"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap().to_owned())
        .collect()
}

async fn moderate(
    vink: &Vink<PgStore>,
    comment: CommentId,
    expected: i64,
    action: ModerationAction,
) -> Result<(), vink_app::application::discussion::DiscussionError> {
    vink.moderate_comment(
        Some(PrincipalId::from_uuid(MODERATOR)),
        ModerateComment {
            comment_id: comment,
            expected_version: expected,
            action,
            reason: (action != ModerationAction::Approve)
                .then(|| "Lý do kiểm thử hiển thị cho người viết.".to_string()),
        },
    )
    .await
    .map(|_| ())
}

async fn submit(
    vink: &Vink<PgStore>,
    actor: Uuid,
    target: Option<CommentId>,
    body: &str,
) -> CommentId {
    vink.submit_comment(
        principal(actor),
        SubmitComment {
            work_id: sample_work(),
            target,
            body: body.into(),
            command_key: format!("k-{}", Uuid::now_v7().simple()),
        },
    )
    .await
    .unwrap()
    .comment_id
}

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

/// The stored digest is the SHA-256 of the stored canonical text, which the receiver recomputes
/// from the wire form; no body, display name or reason is present.
fn assert_minimal(event: &Stored, forbidden: &[&str]) {
    let wire =
        vink_app::domain::notification::wire(&event.envelope, &event.payload_digest).unwrap();
    assert_eq!(
        verify_wire(&wire),
        Some((event.id, event.payload_digest.clone(), true))
    );
    for text in forbidden {
        assert!(
            !event.envelope.contains(text),
            "{text} leaked into {}",
            event.envelope
        );
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn producers_write_nothing_until_activation_and_stamp_the_epoch_after_it(pool: PgPool) {
    let vink = setup(&pool).await;
    let before = post(&vink, "Lời bình trước khi bật thông báo").await;
    moderate(&vink, before, 1, ModerationAction::Approve)
        .await
        .unwrap();
    assert!(
        events(&pool).await.is_empty(),
        "no backfill and no event before activation"
    );

    activate(&pool).await;
    let body = "Lời bình sau khi bật thông báo";
    let after = post(&vink, body).await;
    // Submission itself owes no notification (only `comment.submitted` to its own consumer).
    assert!(events(&pool).await.is_empty());
    moderate(&vink, after, 1, ModerationAction::Approve)
        .await
        .unwrap();
    let owed = events(&pool).await;
    assert_eq!(
        types(&owed),
        [("comment.root.approved".into(), "approved".into())]
    );
    let event = &owed[0];
    assert_eq!(event.activation_epoch, 1);
    let json = event.json();
    assert_eq!(recipients(event), [dev_seed::SAMPLE_AUTHOR.to_string()]);
    assert_eq!(json["actor_id"], dev_seed::OTHER_PRINCIPAL.to_string());
    assert_eq!(json["source_owner"], "discussion");
    assert_eq!(json["source_order"], 2);
    assert_eq!(json["publication_stamp"]["readable"], true);
    assert_eq!(json["target"]["kind"], "work_comment_thread");
    assert_eq!(json["target"]["comment_id"], after.as_uuid().to_string());
    assert_minimal(event, &[body, "Người viết khác", "Lý do"]);
    let delivery: (String, i32) =
        sqlx::query_as("SELECT state, attempts FROM notification_deliveries WHERE event_id = $1")
            .bind(event.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(delivery, ("ready".into(), 0));

    // Deactivation is a cutover too: nothing is owed while inactive.
    notification::set_activation(&pool, false, "kiểm thử #346: tắt")
        .await
        .unwrap();
    let paused = post(&vink, "Lời bình lúc tắt").await;
    moderate(&vink, paused, 1, ModerationAction::Approve)
        .await
        .unwrap();
    assert_eq!(events(&pool).await.len(), 1);
    // Reactivation starts a new epoch.
    activate(&pool).await;
    let again = post(&vink, "Lời bình lúc bật lại").await;
    moderate(&vink, again, 1, ModerationAction::Approve)
        .await
        .unwrap();
    let owed = events(&pool).await;
    assert_eq!(owed.len(), 2);
    assert_eq!(owed[1].activation_epoch, 2);
    let audits: i64 = sqlx::query_scalar("SELECT count(*) FROM notification_activation_changes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audits, 3);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn approval_while_unreadable_is_never_owed_and_republication_does_not_backfill(pool: PgPool) {
    let vink = setup(&pool).await;
    activate(&pool).await;
    let hidden = post(&vink, "Duyệt lúc bài đang ẩn").await;
    // The public pointer is gone at the approval's commit (the stamp's `readable` fact). The
    // author's set-private path is covered with the work-decision flow below.
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    moderate(&vink, hidden, 1, ModerationAction::Approve)
        .await
        .unwrap();
    assert!(
        events(&pool).await.is_empty(),
        "COM-11: nothing owed while unreadable"
    );
    // Republication (a new readable epoch) does not owe the earlier approval.
    sqlx::query(
        "INSERT INTO publications (work_id, revision_id, generation, published_at)
         VALUES ($1, $2, 1, now())",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(dev_seed::SAMPLE_REVISION)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE work_publications SET public_epoch = public_epoch + 1 WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    let visible = post(&vink, "Duyệt lúc bài công khai lại").await;
    moderate(&vink, visible, 1, ModerationAction::Approve)
        .await
        .unwrap();
    let owed = events(&pool).await;
    assert_eq!(owed.len(), 1);
    assert_eq!(
        owed[0].json()["target"]["comment_id"],
        visible.as_uuid().to_string()
    );
    let epoch: i64 =
        sqlx::query_scalar("SELECT public_epoch FROM work_publications WHERE work_id = $1")
            .bind(dev_seed::SAMPLE_WORK)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(owed[0].json()["publication_stamp"]["epoch"], epoch);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn reply_recipients_are_the_direct_target_and_root_writers_without_the_actor(pool: PgPool) {
    let vink = setup(&pool).await;
    activate(&pool).await;
    let c = dev_seed::OTHER_PRINCIPAL;
    let b = new_principal(&pool, "Người trả lời B").await;
    let a = new_principal(&pool, "Người trả lời A").await;
    // A reply needs a public target, so each comment is approved before it is answered.
    let root = submit(&vink, c, None, "Gốc của C").await;
    moderate(&vink, root, 1, ModerationAction::Approve)
        .await
        .unwrap();
    let to_root = submit(&vink, b, Some(root), "B trả lời gốc").await;
    moderate(&vink, to_root, 1, ModerationAction::Approve)
        .await
        .unwrap();
    let to_reply = submit(&vink, a, Some(to_root), "A trả lời B").await;
    moderate(&vink, to_reply, 1, ModerationAction::Approve)
        .await
        .unwrap();
    let own_thread = submit(&vink, c, Some(to_root), "C trả lời B trong chuỗi của mình").await;
    moderate(&vink, own_thread, 1, ModerationAction::Approve)
        .await
        .unwrap();
    let owed = events(&pool).await;
    assert_eq!(owed.len(), 4);
    assert_eq!(owed[0].event_type, "comment.root.approved");
    assert_eq!(recipients(&owed[0]), [dev_seed::SAMPLE_AUTHOR.to_string()]);
    // B → root of C: only C (the work owner is not a reply recipient).
    assert_eq!(owed[1].event_type, "comment.reply.approved");
    assert_eq!(recipients(&owed[1]), [c.to_string()]);
    assert_eq!(owed[1].json()["target"]["reply_context_kind"], "root");
    assert_eq!(
        owed[1].json()["target"]["reply_target_id"],
        root.as_uuid().to_string()
    );
    // A → reply of B under root of C: B and C in one envelope, ascending.
    let mut expected = vec![b.to_string(), c.to_string()];
    expected.sort();
    assert_eq!(recipients(&owed[2]), expected);
    assert_eq!(owed[2].json()["target"]["reply_context_kind"], "reply");
    assert_eq!(
        owed[2].json()["target"]["reply_target_id"],
        to_root.as_uuid().to_string()
    );
    assert_eq!(owed[2].json()["actor_id"], a.to_string());
    // C answers B inside C's own root: only B (self excluded).
    assert_eq!(recipients(&owed[3]), [b.to_string()]);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn adverse_decisions_reach_the_writer_and_own_removal_only_invalidates(pool: PgPool) {
    let vink = setup(&pool).await;
    activate(&pool).await;
    let rejected = post(&vink, "Sẽ bị từ chối sau khi duyệt").await;
    moderate(&vink, rejected, 1, ModerationAction::Approve)
        .await
        .unwrap();
    moderate(&vink, rejected, 2, ModerationAction::Reject)
        .await
        .unwrap();
    let owed = events(&pool).await;
    assert_eq!(
        types(&owed),
        [
            ("comment.root.approved".into(), "approved".into()),
            ("comment.moderation.adverse".into(), "rejected".into()),
            ("notification.invalidation".into(), "invalidate".into()),
        ]
    );
    assert_eq!(
        recipients(&owed[1]),
        [dev_seed::OTHER_PRINCIPAL.to_string()]
    );
    assert_eq!(owed[1].json()["actor_id"], Value::Null);
    assert_eq!(
        owed[1].json()["target"]["kind"],
        "comment_moderation_decision"
    );
    assert_eq!(owed[1].json()["target"]["moderation_version"], 3);
    assert!(recipients(&owed[2]).is_empty());
    assert_eq!(owed[2].json()["target"]["kind"], "comment");
    assert_minimal(&owed[1], &["Lý do", &MODERATOR.to_string()]);

    // The writer's own removal tells nobody and invalidates the public contribution.
    let own = post(&vink, "Tự gỡ sau khi duyệt").await;
    moderate(&vink, own, 1, ModerationAction::Approve)
        .await
        .unwrap();
    vink.remove_own_comment(other(), sample_work(), own)
        .await
        .unwrap();
    let owed = events(&pool).await;
    assert_eq!(
        types(&owed[3..]),
        [
            ("comment.root.approved".into(), "approved".into()),
            ("notification.invalidation".into(), "invalidate".into()),
        ]
    );

    // Removing a pending comment: the writer is told; nothing was public.
    let pending = post(&vink, "Bị gỡ khi còn chờ").await;
    moderate(&vink, pending, 1, ModerationAction::Remove)
        .await
        .unwrap();
    let owed = events(&pool).await;
    assert_eq!(
        types(&owed[5..]),
        [("comment.moderation.adverse".into(), "removed".into())]
    );
    // A repeated or stale decision changes nothing and owes nothing.
    let _ = moderate(&vink, pending, 1, ModerationAction::Remove).await;
    let _ = moderate(&vink, pending, 2, ModerationAction::Remove).await;
    assert_eq!(events(&pool).await.len(), 6);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn an_enqueue_failure_rolls_the_decision_back(pool: PgPool) {
    let vink = setup(&pool).await;
    activate(&pool).await;
    let comment = post(&vink, "Duyệt khi hàng đợi thông báo lỗi").await;
    sqlx::raw_sql(
        "CREATE FUNCTION test_fail_notification() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'injected enqueue failure'; END $$;
         CREATE TRIGGER test_fail_notification BEFORE INSERT ON notification_events
             FOR EACH ROW EXECUTE FUNCTION test_fail_notification();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        moderate(&vink, comment, 1, ModerationAction::Approve)
            .await
            .is_err()
    );
    let (review, version): (String, i64) =
        sqlx::query_as("SELECT review, moderation_version FROM comments WHERE id = $1")
            .bind(comment.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (review.as_str(), version),
        ("pending", 1),
        "the mutation rolled back"
    );
    let decisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM comment_moderation_decisions WHERE comment_id = $1",
    )
    .bind(comment.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(decisions, 0);
    sqlx::raw_sql("DROP TRIGGER test_fail_notification ON notification_events")
        .execute(&pool)
        .await
        .unwrap();
    moderate(&vink, comment, 1, ModerationAction::Approve)
        .await
        .unwrap();
    assert_eq!(events(&pool).await.len(), 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn tham_changes_are_owed_with_first_selection_from_the_retained_row(pool: PgPool) {
    let vink = setup(&pool).await;
    activate(&pool).await;
    let tham = |key: &str, expected: i64, endorsed: bool| SetWorkEndorsement {
        work_id: sample_work(),
        revision: dev_seed::SAMPLE_REVISION.to_string(),
        command_key: key.into(),
        expected_order: expected,
        endorsed,
    };
    const K1: &str = "019a9b40-0001-7000-8000-000000000001";
    for (key, expected, endorsed) in [
        (K1, 1, true),
        ("019a9b40-0002-7000-8000-000000000002", 2, true), // same state: nothing is owed
        ("019a9b40-0003-7000-8000-000000000003", 3, false),
        ("019a9b40-0004-7000-8000-000000000004", 4, true),
    ] {
        vink.set_work_endorsement(other(), tham(key, expected, endorsed))
            .await
            .unwrap();
    }
    // A replay of an accepted key owes nothing new.
    vink.set_work_endorsement(other(), tham(K1, 1, true))
        .await
        .unwrap();
    let owed = events(&pool).await;
    let summary: Vec<(String, Value, Value)> = owed
        .iter()
        .map(|e| {
            let json = e.json();
            (
                json["semantic_kind"].as_str().unwrap().to_owned(),
                json.get("first_selection").cloned().unwrap_or(Value::Null),
                json["source_order"].clone(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("add".into(), Value::Bool(true), Value::from(2)),
            ("remove".into(), Value::Null, Value::from(4)),
            ("add".into(), Value::Bool(false), Value::from(5)),
        ]
    );
    for event in &owed {
        assert_eq!(event.event_type, "work.endorsement.tham.changed");
        assert_eq!(recipients(event), [dev_seed::SAMPLE_AUTHOR.to_string()]);
        assert_eq!(
            event.json()["target"]["revision_id"],
            dev_seed::SAMPLE_REVISION.to_string()
        );
        assert_eq!(event.json()["publication_stamp"]["readable"], true);
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_standing_change_owes_a_principal_invalidation(pool: PgPool) {
    seed(&pool).await;
    activate(&pool).await;
    let target = PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL);
    let reason = SuspensionReason::parse("Kiểm thử #346").unwrap();
    suspend_principal(&pool, target, &reason).await.unwrap();
    suspend_principal(&pool, target, &reason).await.unwrap();
    reinstate_principal(&pool, target).await.unwrap();
    reinstate_principal(&pool, target).await.unwrap();
    let owed = events(&pool).await;
    let orders: Vec<Value> = owed
        .iter()
        .map(|e| e.json()["source_order"].clone())
        .collect();
    assert_eq!(orders, [Value::from(1), Value::from(2)]);
    for event in &owed {
        assert_eq!(event.json()["target"]["kind"], "principal");
        assert_eq!(event.json()["source_owner"], "principal");
        assert_minimal(event, &["Kiểm thử"]);
    }
}

fn path(work: Uuid, tail: &str) -> String {
    format!("/api/v1/works/{work}/{tail}")
}

/// P-11a/b through the real decision paths: evaluator approval, then a human rejection of the
/// readable revision (which also ends the readable epoch), then the author's set-private.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn work_decisions_follow_p11_through_the_real_paths(pool: PgPool) {
    let app = app(&pool).await;
    activate(&pool).await;
    let author_id = dev_seed::SAMPLE_AUTHOR;
    let admin = dev_seed::OTHER_PRINCIPAL;
    let work = dev_seed::SAMPLE_WORK;
    let (status, _, body) = call(
        &app,
        "POST",
        &path(work, "snapshots"),
        Some(author_id),
        Some(ORIGIN),
        Some(serde_json::json!({
            "command_key": "notif-snapshot-001", "expected_draft_version": 1, "expected_metadata_version": 1,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let revision = json(&body)["revision_id"].as_str().unwrap().to_owned();
    let (status, _, body) = call(
        &app,
        "POST",
        &path(work, "publication/request-public"),
        Some(author_id),
        Some(ORIGIN),
        Some(serde_json::json!({
            "command_key": "notif-request-public-001", "revision_id": revision, "expected_generation": 2,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        events(&pool)
            .await
            .iter()
            .all(|e| e.event_type != "work.moderation.decided"),
        "a pending decision owes nothing"
    );
    sqlx::query("UPDATE work_evaluation_jobs SET scenario = 'approve' WHERE work_id = $1")
        .bind(work)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        EvaluationWorker::run_once(&pool, "test-worker")
            .await
            .unwrap()
    );
    let decided: Vec<Stored> = events(&pool)
        .await
        .into_iter()
        .filter(|e| e.event_type == "work.moderation.decided")
        .collect();
    assert_eq!(decided.len(), 1);
    let approved = decided[0].json();
    assert_eq!(approved["semantic_kind"], "approved");
    assert_eq!(approved["action_required"], false);
    assert_eq!(approved["actor_id"], Value::Null);
    assert_eq!(recipients(&decided[0]), [author_id.to_string()]);
    assert_eq!(approved["target"]["revision_id"], revision);

    // A human rejects the readable revision.
    {
        let mut conn = pool.acquire().await.unwrap();
        add_admin_seat(
            &mut conn,
            "operator-test",
            PrincipalId::from_uuid(admin),
            Some("test"),
        )
        .await
        .unwrap();
    }
    let rev = RevisionId::parse(&revision).unwrap();
    let case = CaseId::from_uuid(Uuid::now_v7());
    let (digest, version): (String, i64) = sqlx::query_as(
        "SELECT r.content_hash, m.decision_version FROM revisions r
           JOIN work_revision_moderation m ON m.work_id = r.work_id AND m.revision_id = r.id
          WHERE r.id = $1",
    )
    .bind(rev.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO moderation_cases (
             id, subject_kind, work_id, revision_id, content_digest, decision_version,
             target_content_owner, target_work_owner, origin, version, state, created_at, updated_at)
         VALUES ($1, 'work_revision', $2, $3, $4, $5, $6, $6, 'evaluator_uncertain', 1, 'queued', now(), now())",
    )
    .bind(case.as_uuid())
    .bind(work)
    .bind(rev.as_uuid())
    .bind(digest)
    .bind(version)
    .bind(author_id)
    .execute(&pool)
    .await
    .unwrap();
    for (tail, body) in [
        ("claim", serde_json::json!({"expected_version": 1})),
        (
            "conclude",
            serde_json::json!({"expected_version": 2, "verdict": "rejected", "reason": "safety_violation"}),
        ),
    ] {
        let (status, _, body) = call(
            &app,
            "POST",
            &format!("/api/v1/moderation/works/cases/{case}/{tail}"),
            Some(admin),
            Some(ORIGIN),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{tail}: {body}");
    }
    let owed = events(&pool).await;
    let tail: Vec<(String, String)> = types(&owed)
        .into_iter()
        .filter(|(t, _)| t != "comment.root.approved")
        .skip_while(|(t, s)| !(t == "work.moderation.decided" && s == "rejected"))
        .collect();
    assert_eq!(
        tail,
        [
            ("work.moderation.decided".into(), "rejected".into()),
            ("notification.invalidation".into(), "invalidate".into()),
        ]
    );
    let rejected = owed
        .iter()
        .find(|e| {
            e.event_type == "work.moderation.decided" && e.json()["semantic_kind"] == "rejected"
        })
        .unwrap();
    assert_eq!(rejected.json()["action_required"], true);
    assert_minimal(rejected, &["safety_violation", &admin.to_string()]);
    let invalidated = owed.last().unwrap().json();
    assert_eq!(invalidated["target"]["kind"], "work");
    assert_eq!(invalidated["source_owner"], "publication_moderation");
}

/// The Notification Service's ingestion as #347 specifies it, labelled as a fixture.
#[derive(Default)]
struct Receiver {
    receipts: Mutex<HashMap<Uuid, String>>,
    effects: Mutex<Vec<Uuid>>,
    script: Mutex<VecDeque<Script>>,
    calls: Mutex<usize>,
}

#[derive(Debug, Clone, Copy)]
enum Script {
    /// Apply and commit, then lose the answer.
    LoseAnswer,
    /// Refuse the whole call without applying anything.
    Down,
    /// Answer this status for every event without applying anything.
    Answer(IngestStatus),
}

impl Receiver {
    fn script(&self, steps: &[Script]) {
        self.script.lock().unwrap().extend(steps.iter().copied());
    }

    fn effects(&self) -> Vec<Uuid> {
        self.effects.lock().unwrap().clone()
    }

    fn ingest(&self, event: &Outgoing) -> IngestStatus {
        let Some((id, digest, true)) = verify_wire(&event.wire) else {
            return IngestStatus::RejectedInvalid;
        };
        assert_eq!(
            (id, digest.as_str()),
            (event.event_id, event.payload_digest.as_str())
        );
        let mut receipts = self.receipts.lock().unwrap();
        match receipts.get(&id) {
            Some(stored) if *stored == digest => IngestStatus::AlreadyApplied,
            Some(_) => IngestStatus::PayloadMismatch,
            None => {
                receipts.insert(id, digest);
                self.effects.lock().unwrap().push(id);
                IngestStatus::Applied
            }
        }
    }
}

impl NotificationSink for Receiver {
    fn deliver<'a>(
        &'a self,
        events: &'a [Outgoing],
    ) -> BoxFuture<'a, Result<Vec<IngestReceipt>, SinkFailure>> {
        Box::pin(async move {
            *self.calls.lock().unwrap() += 1;
            let step = self.script.lock().unwrap().pop_front();
            match step {
                Some(Script::Down) => Err(SinkFailure::Unavailable),
                Some(Script::Answer(status)) => Ok(events
                    .iter()
                    .map(|e| IngestReceipt {
                        event_id: e.event_id,
                        status,
                    })
                    .collect()),
                Some(Script::LoseAnswer) => {
                    for event in events {
                        self.ingest(event);
                    }
                    Err(SinkFailure::Unavailable)
                }
                None => Ok(events
                    .iter()
                    .map(|e| IngestReceipt {
                        event_id: e.event_id,
                        status: self.ingest(e),
                    })
                    .collect()),
            }
        })
    }
}

async fn due_now(pool: &PgPool) {
    sqlx::query(
        "UPDATE notification_deliveries SET available_at = now() - interval '1 second'
          WHERE state IN ('ready', 'leased')",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn delivery_states(pool: &PgPool) -> Vec<(String, Option<String>, i32, i64)> {
    sqlx::query_as(
        "SELECT d.state, d.outcome, d.attempts, d.generation FROM notification_deliveries d
           JOIN notification_events e ON e.id = d.event_id ORDER BY e.created_at, e.id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Two approvals owe two events; returns their IDs.
async fn produce_two(pool: &PgPool) -> Vink<PgStore> {
    let vink = setup(pool).await;
    activate(pool).await;
    for body in ["Thứ nhất", "Thứ hai"] {
        let comment = post(&vink, body).await;
        moderate(&vink, comment, 1, ModerationAction::Approve)
            .await
            .unwrap();
    }
    vink
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_relay_acknowledges_once_and_survives_a_lost_answer(pool: PgPool) {
    produce_two(&pool).await;
    let receiver = Arc::new(Receiver::default());
    receiver.script(&[Script::LoseAnswer]);
    let relay = NotificationRelay::new(pool.clone(), receiver.clone(), settings()).unwrap();
    let pass = relay.run_once().await.unwrap();
    assert_eq!(pass.claimed, 2);
    assert!(pass.completions.iter().all(|(_, c)| matches!(
        c,
        RelayCompletion::Retry {
            error: "transport",
            ..
        }
    )));
    assert_eq!(
        receiver.effects().len(),
        2,
        "the receiver committed before the answer was lost"
    );
    // Nothing due before the backoff ends.
    assert_eq!(relay.run_once().await.unwrap().claimed, 0);
    due_now(&pool).await;
    let pass = relay.run_once().await.unwrap();
    assert!(pass.completions.iter().all(|(_, c)| *c
        == RelayCompletion::Acked {
            already_applied: true
        }));
    assert_eq!(receiver.effects().len(), 2, "a duplicate is one effect");
    assert_eq!(
        delivery_states(&pool).await,
        vec![("succeeded".into(), Some("already_applied".into()), 2, 2); 2]
    );
    // Acknowledged deliveries are never claimed again.
    due_now(&pool).await;
    assert_eq!(relay.run_once().await.unwrap().claimed, 0);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_late_ack_from_a_reclaimed_claim_writes_nothing(pool: PgPool) {
    let vink = setup(&pool).await;
    activate(&pool).await;
    let comment = post(&vink, "ACK trễ").await;
    moderate(&vink, comment, 1, ModerationAction::Approve)
        .await
        .unwrap();
    let old = notification::claim(&pool, 10, policy())
        .await
        .unwrap()
        .claimed
        .remove(0);
    // The old worker stalls past its lease; another claims (generation 2) and re-sends.
    due_now(&pool).await;
    let new = notification::claim(&pool, 10, policy()).await.unwrap();
    assert_eq!(new.reclaimed, 1);
    let new = new.claimed.into_iter().next().unwrap();
    assert_eq!((old.generation.get(), new.generation.get()), (1, 2));
    assert_eq!(
        new.payload_digest, old.payload_digest,
        "the same payload every attempt"
    );
    let applied = RelayResult::Ingested(IngestStatus::Applied);
    assert_eq!(
        notification::record(&pool, &old, applied, policy(), 0)
            .await
            .unwrap(),
        RelayCompletion::Stale(StaleReason::Reclaimed)
    );
    assert_eq!(delivery_states(&pool).await[0].0, "leased");
    assert_eq!(
        notification::record(&pool, &new, applied, policy(), 0)
            .await
            .unwrap(),
        RelayCompletion::Acked {
            already_applied: false
        }
    );
    // A retried ACK of the finished delivery writes nothing either.
    assert_eq!(
        notification::record(
            &pool,
            &new,
            RelayResult::Failed(RelayFailure::Timeout),
            policy(),
            0
        )
        .await
        .unwrap(),
        RelayCompletion::Stale(StaleReason::NotLeased)
    );
    assert_eq!(
        delivery_states(&pool).await,
        [("succeeded".into(), Some("applied".into()), 2, 2)]
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn mismatch_dead_letters_and_a_requeue_reopens_it(pool: PgPool) {
    produce_two(&pool).await;
    let receiver = Arc::new(Receiver::default());
    let stored = events(&pool).await;
    // The receiver already holds a different payload under the first event's ID.
    receiver
        .receipts
        .lock()
        .unwrap()
        .insert(stored[0].id, "0".repeat(64));
    let relay = NotificationRelay::new(pool.clone(), receiver.clone(), settings()).unwrap();
    relay.run_once().await.unwrap();
    assert_eq!(
        delivery_states(&pool).await,
        [
            ("dead".into(), Some("payload_mismatch".into()), 1, 1),
            ("succeeded".into(), Some("applied".into()), 1, 1),
        ]
    );
    let dead = notification::dead_letters(&pool, 10).await.unwrap();
    assert_eq!(dead.len(), 1);
    assert_eq!(dead[0].event_id, stored[0].id);
    // A finished delivery is never requeued; a dead letter is, once the cause is fixed.
    assert_eq!(
        notification::requeue(&pool, stored[1].id, "thử requeue sai")
            .await
            .unwrap(),
        RequeueOutcome::Refused(vink_app::domain::discussion::outbox::DeliveryState::Succeeded)
    );
    receiver.receipts.lock().unwrap().remove(&stored[0].id);
    assert_eq!(
        notification::requeue(&pool, stored[0].id, "vận hành: đã đối soát receipt")
            .await
            .unwrap(),
        RequeueOutcome::Requeued {
            previous_attempts: 1
        }
    );
    relay.run_once().await.unwrap();
    assert_eq!(
        delivery_states(&pool).await[0],
        ("succeeded".into(), Some("applied".into()), 1, 2)
    );
    let audit: Vec<(String, String)> =
        sqlx::query_as("SELECT kind, previous_outcome FROM notification_delivery_requeues")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(audit, [("requeue".into(), "payload_mismatch".into())]);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn transient_failures_back_off_and_end_as_dead_letters(pool: PgPool) {
    produce_two(&pool).await;
    let receiver = Arc::new(Receiver::default());
    receiver.script(&[
        Script::Down,
        Script::Answer(IngestStatus::TransientUnavailable),
        Script::Down,
    ]);
    let relay = NotificationRelay::new(pool.clone(), receiver.clone(), settings()).unwrap();
    for _ in 0..3 {
        relay.run_once().await.unwrap();
        due_now(&pool).await;
    }
    assert_eq!(
        delivery_states(&pool).await,
        vec![
            (
                "dead".into(),
                Some(RelayDeadReason::RetryExhausted.code().into()),
                3,
                3
            );
            2
        ]
    );
    assert!(receiver.effects().is_empty());
    assert_eq!(*receiver.calls.lock().unwrap(), 3);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn replay_resends_acknowledged_events_after_a_receiver_restore(pool: PgPool) {
    produce_two(&pool).await;
    let receiver = Arc::new(Receiver::default());
    let relay = NotificationRelay::new(pool.clone(), receiver.clone(), settings()).unwrap();
    relay.run_once().await.unwrap();
    assert_eq!(receiver.effects().len(), 2);
    // The Notification database is restored to before the second effect.
    let stored = events(&pool).await;
    receiver.receipts.lock().unwrap().remove(&stored[1].id);
    let since: time::OffsetDateTime = sqlx::query_scalar("SELECT now() - interval '1 hour'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        notification::replay(&pool, since, "vận hành: phục hồi Notification", 100)
            .await
            .unwrap(),
        2
    );
    relay.run_once().await.unwrap();
    let outcomes: Vec<Option<String>> = delivery_states(&pool)
        .await
        .into_iter()
        .map(|d| d.1)
        .collect();
    assert_eq!(
        outcomes,
        [Some("already_applied".into()), Some("applied".into())]
    );
    assert_eq!(
        receiver.effects().len(),
        3,
        "only the lost effect is applied again"
    );
    let replays: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notification_delivery_requeues WHERE kind = 'replay'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(replays, 2);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_relay_loop_stops_cleanly_and_a_restart_delivers_the_backlog(pool: PgPool) {
    let vink = produce_two(&pool).await;
    let receiver = Arc::new(Receiver::default());
    let run = |receiver: Arc<Receiver>| {
        let (stop, stopped) = watch::channel(false);
        let relay = NotificationRelay::new(pool.clone(), receiver, settings()).unwrap();
        (stop, tokio::spawn(relay.run(stopped)))
    };
    let wait_for = |n: usize, receiver: Arc<Receiver>| async move {
        tokio::time::timeout(Duration::from_secs(20), async {
            while receiver.effects().len() < n {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("relay delivered");
    };
    let (stop, task) = run(receiver.clone());
    wait_for(2, receiver.clone()).await;
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("relay stopped")
        .unwrap();
    // Work committed while the relay is down stays owed and is delivered after a restart.
    let late = post(&vink, "Trong lúc relay dừng").await;
    moderate(&vink, late, 1, ModerationAction::Approve)
        .await
        .unwrap();
    assert_eq!(receiver.effects().len(), 2);
    let (stop, task) = run(receiver.clone());
    wait_for(3, receiver.clone()).await;
    stop.send(true).unwrap();
    task.await.unwrap();
    assert!(
        delivery_states(&pool)
            .await
            .iter()
            .all(|d| d.0 == "succeeded")
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn prune_keeps_owed_work_and_the_rollback_refuses_while_work_is_owed(pool: PgPool) {
    produce_two(&pool).await;
    let receiver = Arc::new(Receiver::default());
    receiver.script(&[Script::Answer(IngestStatus::RejectedInvalid)]);
    let relay = NotificationRelay::new(pool.clone(), receiver, settings()).unwrap();
    relay.run_once().await.unwrap();
    let stored = events(&pool).await;
    // Age everything past the replay horizon: dead letters keep their events.
    sqlx::query("ALTER TABLE notification_events DISABLE TRIGGER notification_events_immutable")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE notification_events SET created_at = now() - interval '40 days'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE notification_events ENABLE TRIGGER notification_events_immutable")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(notification::prune(&pool, 30, 100).await.unwrap(), 0);
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations-rollback/20261127000000_notification_outbox.down.sql"),
    )
    .unwrap();
    let refused = sqlx::raw_sql(sqlx::AssertSqlSafe(script.clone()))
        .execute(&pool)
        .await;
    assert!(refused.is_err(), "dead letters block the rollback");
    // Finish the dead letters (an operator would requeue them after a fix).
    for event in &stored {
        notification::requeue(&pool, event.id, "vận hành: hoàn tất trước rollback")
            .await
            .unwrap();
    }
    relay.run_once().await.unwrap();
    sqlx::query("UPDATE notification_deliveries SET finished_at = now() - interval '40 days'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(notification::prune(&pool, 30, 100).await.unwrap(), 2);
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(script))
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let applied: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = $1)")
            .bind(NOTIFICATION_OUTBOX_MIGRATION)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!applied);
    // This (newer) binary now reports the migration as pending until it is applied again.
    assert_eq!(schema_state(&pool).await.unwrap(), SchemaState::Pending);
}
