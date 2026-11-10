//! Real PostgreSQL evidence for the operator producer, result adapter and worker. SQL fault and
//! time-travel fixtures are labelled; none is an evaluator or a production authority.
use super::outbox::{MODERATOR, database_url, make_due, post, setup, spawn_worker};
use super::*;
use vink_app::application::discussion::{
    DiscussionError, ModerateComment, ModerationAction, OwnCommentsQuery, OwnStatus,
};
use vink_app::application::post_review::{
    RequestReview, ReviewAuthority, ReviewResult, ReviewWrite,
};
use vink_app::domain::discussion::admission::PostReviewFinding;
use vink_app::domain::discussion::lifecycle::ModerationVersion;
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::discussion::outbox::{Completion, HandlerResult, OutboxPolicy, Transient};
use vink_app::domain::discussion::post_review::CONSUMER;
use vink_app::domain::ids::CommentId;
use vink_app::outbox_worker::{OutboxSettings, OutboxWorker};
use vink_app::persistence::outbox::{
    self as queue, BoxFuture, Claimed, EffectHandler, OutboxEvent,
};
use vink_app::persistence::post_review::{self as reviews, OperatorPostReview};

fn request(comment: CommentId, version: i64) -> RequestReview {
    RequestReview {
        key: Uuid::now_v7(),
        comment,
        expected: ModerationVersion::new(version).unwrap(),
        note: GrantNote::parse("Lan vận hành: kiểm lại báo cáo chính sách").unwrap(),
    }
}
fn result(ticket: Uuid, finding: PostReviewFinding) -> ReviewResult {
    ReviewResult {
        ticket,
        key: Uuid::now_v7(),
        finding,
        note: GrantNote::parse("Lan vận hành: kết quả đối chiếu chính sách").unwrap(),
    }
}
async fn moderate(
    vink: &Vink<PgStore>,
    comment: CommentId,
    version: i64,
    action: ModerationAction,
) -> Result<vink_app::domain::discussion::lifecycle::CommentState, DiscussionError> {
    vink.moderate_comment(
        principal(MODERATOR),
        ModerateComment {
            comment_id: comment,
            expected_version: version,
            action,
            reason: (action != ModerationAction::Approve)
                .then(|| "Kiểm lại theo chính sách".into()),
        },
    )
    .await
}
async fn approved(pool: &PgPool, vink: &Vink<PgStore>) -> CommentId {
    let id = post(vink, "Nội dung cần kiểm lại, không ghi vào log").await;
    moderate(vink, id, 1, ModerationAction::Approve)
        .await
        .unwrap();
    // Drain the unrelated shadow obligation before inspecting this workflow.
    OutboxWorker::production(pool.clone(), OutboxSettings::default())
        .run_once()
        .await
        .unwrap();
    id
}
async fn enqueue(vink: &Vink<PgStore>, id: CommentId) -> Uuid {
    let ReviewWrite::Accepted(ticket) = vink
        .request_review(ReviewAuthority::operator(), true, &request(id, 2))
        .await
        .unwrap()
    else {
        panic!("request not accepted");
    };
    ticket
}
async fn claim(pool: &PgPool, policy: OutboxPolicy) -> Claimed {
    let mut batch = queue::claim(pool, &[CONSUMER], 1, policy).await.unwrap();
    assert_eq!(batch.claimed.len(), 1);
    batch.claimed.remove(0)
}
async fn finish(pool: &PgPool, claimed: &Claimed, policy: OutboxPolicy) -> Completion {
    queue::complete(pool, claimed, &OperatorPostReview, policy, 0)
        .await
        .unwrap()
}
async fn receipt(pool: &PgPool, ticket: Uuid) -> Option<(String, i64, i64)> {
    sqlx::query_as("SELECT outcome, observed_version, resulting_version FROM comment_post_review_receipts WHERE ticket_id = $1").bind(ticket).fetch_optional(pool).await.unwrap()
}
async fn state(pool: &PgPool, id: CommentId) -> (String, i64) {
    sqlx::query_as("SELECT review, moderation_version FROM comments WHERE id = $1")
        .bind(id.as_uuid())
        .fetch_one(pool)
        .await
        .unwrap()
}
fn once_policy() -> OutboxPolicy {
    OutboxPolicy::new(1, 30_000_000, 1_000_000, 10_000_000, 3_600_000_000).unwrap()
}

/// Labelled failure source, never an evaluator. The real terminalization adapter must still
/// execute after the supervisor records a panic or timeout on the final attempt.
struct Fault(bool);
impl EffectHandler for Fault {
    fn consumer(&self) -> &str {
        CONSUMER
    }
    fn apply<'a>(
        &'a self,
        _tx: &'a mut PgConnection,
        _event: &'a OutboxEvent,
    ) -> BoxFuture<'a, Result<HandlerResult, sqlx::Error>> {
        Box::pin(async move {
            if self.0 {
                panic!("labelled post-review panic");
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok(HandlerResult::Retry(Transient::Timeout))
        })
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn supervised_timeout_and_panic_end_in_real_holds_on_the_last_attempt(pool: PgPool) {
    let vink = setup(&pool).await;
    for panic in [false, true] {
        let id = approved(&pool, &vink).await;
        let ticket = enqueue(&vink, id).await;
        let settings = OutboxSettings {
            policy: once_policy(),
            handler_timeout: Duration::from_millis(20),
            poll_min: Duration::from_millis(10),
            poll_max: Duration::from_millis(20),
            ..OutboxSettings::default()
        };
        let worker =
            OutboxWorker::with_handlers(pool.clone(), settings, vec![Arc::new(Fault(panic))]);
        let (stop, stopped) = tokio::sync::watch::channel(false);
        let task = tokio::spawn(worker.run(stopped));
        let until = Instant::now() + Duration::from_secs(10);
        while receipt(&pool, ticket).await.is_none() {
            assert!(Instant::now() < until);
            tokio::time::sleep(POLL).await;
        }
        stop.send(true).unwrap();
        task.await.unwrap();
        assert_eq!(state(&pool, id).await, ("pending".into(), 3));
        assert_eq!(receipt(&pool, ticket).await, Some(("held".into(), 2, 3)));
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn operator_producer_checks_state_switch_exact_replay_and_atomic_enqueue(pool: PgPool) {
    let vink = setup(&pool).await;
    let pending = post(&vink, "Chưa được duyệt").await;
    assert_eq!(
        vink.request_review(ReviewAuthority::operator(), true, &request(pending, 1))
            .await
            .unwrap(),
        ReviewWrite::Refused
    );
    let id = approved(&pool, &vink).await;
    let command = request(id, 2);
    assert_eq!(
        vink.request_review(ReviewAuthority::operator(), false, &command)
            .await
            .unwrap(),
        ReviewWrite::Disabled
    );
    // Labelled enqueue fault: the whole producer transaction must roll back.
    sqlx::raw_sql("CREATE FUNCTION fail_review_enqueue() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.consumer = 'operator_post_review.v1' THEN RAISE EXCEPTION 'fixture enqueue failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_review_enqueue BEFORE INSERT ON discussion_deliveries FOR EACH ROW EXECUTE FUNCTION fail_review_enqueue();").execute(&pool).await.unwrap();
    assert!(
        vink.request_review(ReviewAuthority::operator(), true, &command)
            .await
            .is_err()
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM comment_post_reviews")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
    sqlx::query("DROP TRIGGER fail_review_enqueue ON discussion_deliveries")
        .execute(&pool)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        vink.request_review(ReviewAuthority::operator(), true, &command),
        vink.request_review(ReviewAuthority::operator(), true, &command)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    let ticket = match a {
        ReviewWrite::Accepted(id) | ReviewWrite::Replayed(id) => id,
        _ => panic!("{a:?}"),
    };
    assert!(matches!(b,ReviewWrite::Accepted(id) | ReviewWrite::Replayed(id) if id == ticket));
    assert_eq!(
        vink.request_review(ReviewAuthority::operator(), false, &command)
            .await
            .unwrap(),
        ReviewWrite::Replayed(ticket)
    );
    let mut reused = command.clone();
    reused.expected = ModerationVersion::FIRST;
    assert_eq!(
        vink.request_review(ReviewAuthority::operator(), true, &reused)
            .await
            .unwrap(),
        ReviewWrite::Conflict
    );
    assert_eq!(
        vink.request_review(ReviewAuthority::operator(), true, &request(id, 2))
            .await
            .unwrap(),
        ReviewWrite::Conflict
    );
    let envelope: (String,String,i64,bool) = sqlx::query_as("SELECT origin,policy,expected_version,deadline_at = created_at + interval '15 minutes' FROM comment_post_reviews WHERE id=$1").bind(ticket).fetch_one(&pool).await.unwrap();
    assert_eq!(
        envelope,
        (
            "operator_rereview".into(),
            "operator-rereview.v1".into(),
            2,
            true
        )
    );
    assert!(
        sqlx::query("UPDATE comment_post_reviews SET expected_version=3")
            .execute(&pool)
            .await
            .is_err()
    );
    let binding_error = sqlx::query("INSERT INTO comment_post_review_results (ticket_id,command_key,comment_id,expected_version,payload_digest,origin,policy,finding,note) SELECT id,$2,comment_id,expected_version+1,payload_digest,origin,policy,'clear','fixture wrong binding' FROM comment_post_reviews WHERE id=$1").bind(ticket).bind(Uuid::now_v7()).execute(&pool).await.unwrap_err();
    assert_eq!(
        binding_error.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );
    let origin_error = sqlx::query("INSERT INTO comment_post_reviews (id,request_key,comment_id,expected_version,payload_digest,origin,policy,note,deadline_at) SELECT $2,$3,comment_id,expected_version,payload_digest,'admission_approved',policy,'fixture wrong origin',now()+interval '15 minutes' FROM comment_post_reviews WHERE id=$1").bind(ticket).bind(Uuid::now_v7()).bind(Uuid::now_v7()).execute(&pool).await.unwrap_err();
    assert_eq!(
        origin_error.as_database_error().unwrap().constraint(),
        Some("comment_post_reviews_origin_check")
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn structured_result_is_exactly_replayed_and_hold_projects_through_existing_reads(
    pool: PgPool,
) {
    let vink = setup(&pool).await;
    let id = approved(&pool, &vink).await;
    let ticket = enqueue(&vink, id).await;
    let answer = result(ticket, PostReviewFinding::Flagged);
    assert_eq!(
        vink.record_review_result(ReviewAuthority::operator(), &answer)
            .await
            .unwrap(),
        ReviewWrite::Accepted(ticket)
    );
    assert_eq!(
        vink.record_review_result(ReviewAuthority::operator(), &answer)
            .await
            .unwrap(),
        ReviewWrite::Replayed(ticket)
    );
    let mut reused = answer.clone();
    reused.finding = PostReviewFinding::Clear;
    assert_eq!(
        vink.record_review_result(ReviewAuthority::operator(), &reused)
            .await
            .unwrap(),
        ReviewWrite::Conflict
    );
    assert_eq!(
        vink.record_review_result(
            ReviewAuthority::operator(),
            &result(ticket, PostReviewFinding::Clear)
        )
        .await
        .unwrap(),
        ReviewWrite::Conflict
    );
    let policy = OutboxPolicy::DEFAULT;
    let held = claim(&pool, policy).await;
    assert!(matches!(
        finish(&pool, &held, policy).await,
        Completion::Succeed { .. }
    ));
    assert_eq!(state(&pool, id).await, ("pending".into(), 3));
    assert_eq!(receipt(&pool, ticket).await, Some(("held".into(), 2, 3)));
    // Lost commit response: neither effect nor receipt can be applied a second time.
    assert!(matches!(
        finish(&pool, &held, policy).await,
        Completion::Stale(_)
    ));
    assert_eq!(
        vink.record_review_result(ReviewAuthority::operator(), &answer)
            .await
            .unwrap(),
        ReviewWrite::Replayed(ticket)
    );
    let public = vink
        .public_discussion(sample_work(), None, None)
        .await
        .unwrap();
    assert!(!format!("{public:?}").contains("Nội dung cần kiểm lại"));
    let own = vink
        .own_comments(
            other(),
            sample_work(),
            &OwnCommentsQuery {
                cursor: None,
                limit: None,
                ids: Some(id.to_string()),
            },
        )
        .await
        .unwrap();
    assert_eq!(own.items[0].status, OwnStatus::Pending);
    let queue = vink
        .moderation_queue(principal(MODERATOR), None, None)
        .await
        .unwrap();
    assert!(queue.items.iter().any(|c| c.comment_id == id));
    let human_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM comment_moderation_decisions WHERE comment_id=$1")
            .bind(id.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(human_count, 1, "worker must not forge a moderator decision");
    assert!(
        sqlx::query("UPDATE comment_post_review_results SET finding='clear'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM comment_post_review_receipts")
            .execute(&pool)
            .await
            .is_err()
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn clear_human_reaffirmation_and_author_removal_preserve_their_distinct_outcomes(
    pool: PgPool,
) {
    let vink = setup(&pool).await;
    for expected in ["kept", "superseded", "removed"] {
        let id = approved(&pool, &vink).await;
        let ticket = enqueue(&vink, id).await;
        vink.record_review_result(
            ReviewAuthority::operator(),
            &result(ticket, PostReviewFinding::Clear),
        )
        .await
        .unwrap();
        match expected {
            "superseded" => {
                moderate(&vink, id, 2, ModerationAction::Approve)
                    .await
                    .unwrap();
            }
            "removed" => {
                vink.remove_own_comment(other(), sample_work(), id)
                    .await
                    .unwrap();
            }
            _ => {}
        }
        let held = claim(&pool, OutboxPolicy::DEFAULT).await;
        finish(&pool, &held, OutboxPolicy::DEFAULT).await;
        assert_eq!(receipt(&pool, ticket).await.unwrap().0, expected);
        if expected == "kept" {
            assert_eq!(state(&pool, id).await, ("approved".into(), 2));
        }
    }
    let id = approved(&pool, &vink).await;
    let ticket = enqueue(&vink, id).await;
    vink.record_review_result(
        ReviewAuthority::operator(),
        &result(ticket, PostReviewFinding::Flagged),
    )
    .await
    .unwrap();
    let held = claim(&pool, OutboxPolicy::DEFAULT).await;
    finish(&pool, &held, OutboxPolicy::DEFAULT).await;
    assert_eq!(
        moderate(&vink, id, 2, ModerationAction::Approve).await,
        Err(DiscussionError::ModerationConflict { current: 3 })
    );
    assert_eq!(
        moderate(&vink, id, 3, ModerationAction::Approve)
            .await
            .unwrap()
            .version()
            .get(),
        4
    );
    // rejected -> rejected also accepts a new intent and audits it.
    assert_eq!(
        moderate(&vink, id, 4, ModerationAction::Reject)
            .await
            .unwrap()
            .version()
            .get(),
        5
    );
    assert_eq!(
        moderate(&vink, id, 5, ModerationAction::Reject)
            .await
            .unwrap()
            .version()
            .get(),
        6
    );
    assert_eq!(receipt(&pool, ticket).await.unwrap().0, "held");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn reclaimed_worker_cannot_write_result_effect_receipt_failure_or_ack(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = approved(&pool, &vink).await;
    let ticket = enqueue(&vink, id).await;
    vink.record_review_result(
        ReviewAuthority::operator(),
        &result(ticket, PostReviewFinding::Flagged),
    )
    .await
    .unwrap();
    let policy = OutboxPolicy::DEFAULT;
    let (a, b) = tokio::join!(
        queue::claim(&pool, &[CONSUMER], 1, policy),
        queue::claim(&pool, &[CONSUMER], 1, policy)
    );
    let mut combined = a.unwrap().claimed;
    combined.extend(b.unwrap().claimed);
    assert_eq!(combined.len(), 1);
    let old = combined.remove(0);
    make_due(&pool).await;
    let new = claim(&pool, policy).await;
    assert!(new.generation > old.generation);
    assert!(matches!(
        finish(&pool, &old, policy).await,
        Completion::Stale(_)
    ));
    assert!(matches!(
        queue::record_failure(&pool, &old, Transient::Panic, once_policy(), 0)
            .await
            .unwrap(),
        Completion::Stale(_)
    ));
    assert!(receipt(&pool, ticket).await.is_none());
    assert_eq!(state(&pool, id).await, ("approved".into(), 2));
    finish(&pool, &new, policy).await;
    assert_eq!(receipt(&pool, ticket).await, Some(("held".into(), 2, 3)));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn all_three_exhaustion_boundaries_finalize_before_the_delivery_ends(pool: PgPool) {
    let vink = setup(&pool).await;
    for boundary in ["claim", "complete", "record_failure", "delivery_deadline"] {
        let id = approved(&pool, &vink).await;
        let ticket = enqueue(&vink, id).await;
        let policy = once_policy();
        let held = claim(&pool, policy).await;
        match boundary {
            "claim" => {
                make_due(&pool).await;
                assert_eq!(
                    queue::claim(&pool, &[CONSUMER], 1, policy)
                        .await
                        .unwrap()
                        .dead_lettered,
                    1
                );
            }
            "complete" => {
                assert!(matches!(
                    finish(&pool, &held, policy).await,
                    Completion::DeadLetter(_)
                ));
            }
            "delivery_deadline" => {
                sqlx::query("UPDATE discussion_deliveries SET deadline_from=now()-interval '8 days',available_at=now()-interval '1 second' WHERE event_id=$1").bind(ticket).execute(&pool).await.unwrap();
                assert_eq!(
                    queue::claim(&pool, &[CONSUMER], 1, OutboxPolicy::DEFAULT)
                        .await
                        .unwrap()
                        .dead_lettered,
                    1
                );
            }
            _ => {
                assert!(matches!(
                    queue::record_failure(&pool, &held, Transient::Timeout, policy, 0)
                        .await
                        .unwrap(),
                    Completion::DeadLetter(_)
                ));
            }
        }
        assert_eq!(
            receipt(&pool, ticket).await,
            Some(("held".into(), 2, 3)),
            "{boundary}"
        );
        assert_eq!(state(&pool, id).await, ("pending".into(), 3));
        let deadline: String =
            sqlx::query_scalar("SELECT deadline_at::text FROM comment_post_reviews WHERE id=$1")
                .bind(ticket)
                .fetch_one(&pool)
                .await
                .unwrap();
        let from: String = sqlx::query_scalar(
            "SELECT deadline_from::text FROM discussion_deliveries WHERE event_id=$1",
        )
        .bind(ticket)
        .fetch_one(&pool)
        .await
        .unwrap();
        queue::requeue(
            &pool,
            ticket,
            CONSUMER,
            "Lan: phục hồi sau lỗi, giữ nguyên deadline",
        )
        .await
        .unwrap();
        let after: (String,String)=sqlx::query_as("SELECT t.deadline_at::text,d.deadline_from::text FROM comment_post_reviews t JOIN discussion_deliveries d ON d.event_id=t.id WHERE t.id=$1").bind(ticket).fetch_one(&pool).await.unwrap();
        assert_eq!(after, (deadline, from));
        // New human intent after finalization wins even when a dead delivery is requeued.
        moderate(&vink, id, 3, ModerationAction::Approve)
            .await
            .unwrap();
        let batch = queue::claim(&pool, &[CONSUMER], 1, policy).await.unwrap();
        if let Some(again) = batch.claimed.first() {
            finish(&pool, again, policy).await;
        }
        assert_eq!(state(&pool, id).await, ("approved".into(), 4));
        assert_eq!(receipt(&pool, ticket).await.unwrap().0, "held");
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn db_failure_during_finalization_keeps_durable_debt_and_recovers(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = approved(&pool, &vink).await;
    let ticket = enqueue(&vink, id).await;
    let policy = once_policy();
    let held = claim(&pool, policy).await;
    // Labelled DB fault at the real comment effect, not a fake success adapter.
    sqlx::raw_sql("CREATE FUNCTION fail_review_hold() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.review='pending' AND OLD.review='approved' THEN RAISE EXCEPTION 'fixture hold failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_review_hold BEFORE UPDATE ON comments FOR EACH ROW EXECUTE FUNCTION fail_review_hold();").execute(&pool).await.unwrap();
    assert!(
        queue::record_failure(&pool, &held, Transient::Database, policy, 0)
            .await
            .is_err()
    );
    make_due(&pool).await;
    assert!(queue::claim(&pool, &[CONSUMER], 1, policy).await.is_err());
    assert!(receipt(&pool, ticket).await.is_none());
    let delivery: String =
        sqlx::query_scalar("SELECT state FROM discussion_deliveries WHERE event_id=$1")
            .bind(ticket)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(delivery, "leased");
    assert_eq!(state(&pool, id).await, ("approved".into(), 2));
    sqlx::query("DROP TRIGGER fail_review_hold ON comments")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        queue::claim(&pool, &[CONSUMER], 1, policy)
            .await
            .unwrap()
            .dead_lettered,
        1
    );
    assert_eq!(receipt(&pool, ticket).await, Some(("held".into(), 2, 3)));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn poisoned_binding_is_visible_debt_and_cannot_be_pruned_or_backed_out(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = approved(&pool, &vink).await;
    let ticket = enqueue(&vink, id).await;
    // Labelled corruption fixture; immutable event metadata cannot be edited by runtime code.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("ALTER TABLE discussion_events DISABLE TRIGGER discussion_events_immutable")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE discussion_events SET source_version=source_version+1 WHERE id=$1")
        .bind(ticket)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE discussion_events ENABLE TRIGGER discussion_events_immutable")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let held = claim(&pool, OutboxPolicy::DEFAULT).await;
    assert!(matches!(
        finish(&pool, &held, OutboxPolicy::DEFAULT).await,
        Completion::DeadLetter(_)
    ));
    assert!(receipt(&pool, ticket).await.is_none());
    assert_eq!(state(&pool, id).await, ("approved".into(), 2));
    assert!(
        reviews::status(&pool)
            .await
            .unwrap()
            .iter()
            .any(|(state, n, _)| state == "quarantined" && *n == 1)
    );
    queue::prune(&pool, -1, 1000).await.unwrap();
    assert_eq!(reviews::pending(&pool, 10).await.unwrap().len(), 1);
    let down =
        include_str!("../../migrations-rollback/20261115000000_comment_post_review.down.sql");
    let mut conn = pool.acquire().await.unwrap();
    assert!(sqlx::raw_sql(down).execute(&mut *conn).await.is_err());
    sqlx::query("ROLLBACK").execute(&mut *conn).await.unwrap();
    assert_eq!(reviews::pending(&pool, 10).await.unwrap().len(), 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn real_operator_cli_and_worker_restart_cover_a_crash_inside_the_effect(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = approved(&pool, &vink).await;
    let url = database_url(&pool);
    let cli = |args: &[String], enabled: bool| {
        std::process::Command::new(env!("CARGO_BIN_EXE_vink-app"))
            .env_clear()
            .env("DATABASE_URL", &url)
            .env("VINK_PROFILE", "local")
            .env(
                "VINK_POST_REVIEW_REQUESTS",
                if enabled { "enabled" } else { "disabled" },
            )
            .args(args)
            .output()
            .unwrap()
    };
    let output = cli(
        &[
            "post-review".into(),
            "request".into(),
            id.to_string(),
            "2".into(),
            Uuid::now_v7().to_string(),
            "Lan: hậu kiểm vận hành".into(),
        ],
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let ticket = Uuid::parse_str(text.trim().split('\t').nth(1).unwrap()).unwrap();
    let output = cli(
        &[
            "post-review".into(),
            "result".into(),
            ticket.to_string(),
            Uuid::now_v7().to_string(),
            "flagged".into(),
            "Lan: cần duyệt lại".into(),
        ],
        false,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Barrier INSIDE production effect: comment UPDATE has happened, receipt INSERT blocks.
    sqlx::raw_sql("CREATE FUNCTION pause_review_receipt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(884415); RETURN NEW; END $$; CREATE TRIGGER pause_review_receipt BEFORE INSERT ON comment_post_review_receipts FOR EACH ROW EXECUTE FUNCTION pause_review_receipt();").execute(&pool).await.unwrap();
    let mut barrier = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(884415)")
        .execute(&mut *barrier)
        .await
        .unwrap();
    let pid = backend_pid(&mut barrier).await;
    let mut worker = spawn_worker(&url, "local");
    wait_blocked_by(&pool, pid).await;
    worker.kill().unwrap();
    worker.wait().unwrap();
    barrier.rollback().await.unwrap();
    // The process was killed inside a transaction after effect, so it must all roll back.
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let waiting: bool=sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND application_name='vink-worker' AND xact_start IS NOT NULL)").fetch_one(&pool).await.unwrap();
        if !waiting {
            break;
        }
        assert!(Instant::now() < until);
        tokio::time::sleep(POLL).await;
    }
    assert!(receipt(&pool, ticket).await.is_none());
    assert_eq!(state(&pool, id).await, ("approved".into(), 2));
    make_due(&pool).await;
    let mut restarted = spawn_worker(&url, "local");
    let until = Instant::now() + Duration::from_secs(20);
    while receipt(&pool, ticket).await.is_none() {
        assert!(Instant::now() < until);
        tokio::time::sleep(POLL).await;
    }
    restarted.kill().unwrap();
    restarted.wait().unwrap();
    let mut logs = String::new();
    std::io::Read::read_to_string(restarted.stderr.as_mut().unwrap(), &mut logs).unwrap();
    let started: Value = logs
        .lines()
        .find_map(|line| {
            let row: Value = serde_json::from_str(line).ok()?;
            (row["fields"]["message"] == "vink-worker started").then_some(row)
        })
        .expect("structured worker startup");
    assert!(
        started["fields"]["consumers"]
            .as_str()
            .unwrap()
            .contains(CONSUMER)
    );
    assert_eq!(started["fields"]["post_review"], "operator_rereview.v1");
    assert_eq!(started["fields"]["admission_post_review"], "no_producer");
    assert!(!logs.contains("Nội dung cần kiểm lại"));
    assert_eq!(receipt(&pool, ticket).await, Some(("held".into(), 2, 3)));
    let public = vink
        .public_discussion(sample_work(), None, None)
        .await
        .unwrap();
    assert!(!format!("{public:?}").contains("Nội dung cần kiểm lại"));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn immutable_business_deadline_expires_before_first_claim_even_with_a_clear_result(
    pool: PgPool,
) {
    let vink = setup(&pool).await;
    let id = approved(&pool, &vink).await;
    let ticket = enqueue(&vink, id).await;
    vink.record_review_result(
        ReviewAuthority::operator(),
        &result(ticket, PostReviewFinding::Clear),
    )
    .await
    .unwrap();
    // Labelled time travel: preserve the envelope's deadline interval, move both instants.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("ALTER TABLE comment_post_reviews DISABLE TRIGGER comment_post_reviews_immutable")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE comment_post_reviews SET created_at=created_at-interval '16 minutes',deadline_at=deadline_at-interval '16 minutes' WHERE id=$1").bind(ticket).execute(&mut *tx).await.unwrap();
    sqlx::query("ALTER TABLE comment_post_reviews ENABLE TRIGGER comment_post_reviews_immutable")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        vink.record_review_result(
            ReviewAuthority::operator(),
            &result(ticket, PostReviewFinding::Flagged)
        )
        .await
        .unwrap(),
        ReviewWrite::Refused
    );
    assert!(reviews::pending(&pool, 10).await.unwrap()[0].overdue);
    let held = claim(&pool, OutboxPolicy::DEFAULT).await;
    finish(&pool, &held, OutboxPolicy::DEFAULT).await;
    assert_eq!(receipt(&pool, ticket).await, Some(("held".into(), 2, 3)));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn real_human_and_worker_commits_serialize_in_both_orders_without_deadlock(pool: PgPool) {
    let vink = setup(&pool).await;
    for remove in [false, true] {
        let id = approved(&pool, &vink).await;
        let ticket = enqueue(&vink, id).await;
        vink.record_review_result(
            ReviewAuthority::operator(),
            &result(ticket, PostReviewFinding::Flagged),
        )
        .await
        .unwrap();
        let held = claim(&pool, OutboxPolicy::DEFAULT).await;
        let mut barrier = pool.begin().await.unwrap();
        sqlx::query("SELECT 1 FROM comments WHERE id=$1 FOR UPDATE")
            .bind(id.as_uuid())
            .execute(&mut *barrier)
            .await
            .unwrap();
        let pid = backend_pid(&mut barrier).await;
        let human_vink = vink.clone();
        let human = tokio::spawn(async move {
            if remove {
                human_vink
                    .remove_own_comment(other(), sample_work(), id)
                    .await
                    .map(|_| ())
            } else {
                moderate(&human_vink, id, 2, ModerationAction::Approve)
                    .await
                    .map(|_| ())
            }
        });
        wait_blocked_by(&pool, pid).await;
        let worker_pool = pool.clone();
        let worker =
            tokio::spawn(async move { finish(&worker_pool, &held, OutboxPolicy::DEFAULT).await });
        wait_blocked(&pool, 2).await;
        barrier.commit().await.unwrap();
        human.await.unwrap().unwrap();
        worker.await.unwrap();
        assert_eq!(
            receipt(&pool, ticket).await.unwrap().0,
            if remove { "removed" } else { "superseded" }
        );
    }
    // Worker first, paused after its comment UPDATE and before receipt, while human waits.
    let id = approved(&pool, &vink).await;
    let ticket = enqueue(&vink, id).await;
    vink.record_review_result(
        ReviewAuthority::operator(),
        &result(ticket, PostReviewFinding::Flagged),
    )
    .await
    .unwrap();
    let held = claim(&pool, OutboxPolicy::DEFAULT).await;
    sqlx::raw_sql("CREATE FUNCTION gate_review_receipt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(884416); RETURN NEW; END $$; CREATE TRIGGER gate_review_receipt BEFORE INSERT ON comment_post_review_receipts FOR EACH ROW EXECUTE FUNCTION gate_review_receipt();").execute(&pool).await.unwrap();
    let mut barrier = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(884416)")
        .execute(&mut *barrier)
        .await
        .unwrap();
    let pid = backend_pid(&mut barrier).await;
    let worker_pool = pool.clone();
    let worker =
        tokio::spawn(async move { finish(&worker_pool, &held, OutboxPolicy::DEFAULT).await });
    wait_blocked_by(&pool, pid).await;
    let human_vink = vink.clone();
    let human =
        tokio::spawn(async move { moderate(&human_vink, id, 2, ModerationAction::Approve).await });
    wait_blocked(&pool, 2).await;
    barrier.rollback().await.unwrap();
    worker.await.unwrap();
    assert_eq!(
        human.await.unwrap(),
        Err(DiscussionError::ModerationConflict { current: 3 })
    );
    assert_eq!(receipt(&pool, ticket).await, Some(("held".into(), 2, 3)));
    moderate(&vink, id, 3, ModerationAction::Approve)
        .await
        .unwrap();
    assert_eq!(state(&pool, id).await, ("approved".into(), 4));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn runtime_role_can_produce_drain_and_inspect_without_ddl_and_backout_preserves_audit(
    pool: PgPool,
) {
    let vink = setup(&pool).await;
    let id = approved(&pool, &vink).await;
    let role = format!("vink_review_rt_{}", Uuid::now_v7().simple());
    // Explicit fixture credentials work with SCRAM as well as local trust. Changing only the
    // username would retain the harness password, not this new role's password.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE ROLE {role} LOGIN PASSWORD 'fixture' NOSUPERUSER NOCREATEDB NOCREATEROLE; GRANT USAGE ON SCHEMA public TO {role}; GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA public TO {role}; GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO {role};"))).execute(&pool).await.unwrap();
    let options = pool
        .connect_options()
        .as_ref()
        .clone()
        .username(&role)
        .password("fixture");
    let restricted = PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .unwrap();
    assert!(
        !vink_app::persistence::role_is_privileged(&restricted)
            .await
            .unwrap()
    );
    let operator = Vink::new(PgStore::new(restricted.clone()));
    let ticket = enqueue(&operator, id).await;
    operator
        .record_review_result(
            ReviewAuthority::operator(),
            &result(ticket, PostReviewFinding::Unresolved),
        )
        .await
        .unwrap();
    let held = claim(&restricted, OutboxPolicy::DEFAULT).await;
    finish(&restricted, &held, OutboxPolicy::DEFAULT).await;
    assert_eq!(receipt(&pool, ticket).await, Some(("held".into(), 2, 3)));
    assert!(
        reviews::status(&restricted)
            .await
            .unwrap()
            .iter()
            .any(|(s, n, _)| s == "held" && *n == 1)
    );
    assert!(
        sqlx::query("CREATE TABLE forbidden_review_fixture (id integer)")
            .execute(&restricted)
            .await
            .is_err()
    );
    restricted.close().await;
    let mut conn = pool.acquire().await.unwrap();
    assert!(
        sqlx::raw_sql(include_str!(
            "../../migrations-rollback/20261115000000_comment_post_review.down.sql"
        ))
        .execute(&mut *conn)
        .await
        .is_err()
    );
    sqlx::query("ROLLBACK").execute(&mut *conn).await.unwrap();
    assert_eq!(receipt(&pool, ticket).await.unwrap().0, "held");
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("REVOKE ALL ON ALL TABLES IN SCHEMA public FROM {role}; REVOKE ALL ON ALL SEQUENCES IN SCHEMA public FROM {role}; REVOKE ALL ON SCHEMA public FROM {role}; DROP ROLE {role};"))).execute(&pool).await.unwrap();
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn held_reply_disappears_from_counts_previews_context_and_endorsement_capability(
    pool: PgPool,
) {
    use vink_app::application::discussion::SubmitComment;
    use vink_app::domain::discussion::visibility::ReplyContext;
    let vink = setup(&pool).await;
    let root = approved(&pool, &vink).await;
    let mut target = root;
    let mut children = Vec::new();
    for text in ["Chữ của phản hồi cần giữ", "Trả lời còn công khai"] {
        let reply = vink
            .submit_comment(
                other(),
                SubmitComment {
                    work_id: sample_work(),
                    target: Some(target),
                    body: text.into(),
                    command_key: format!("key-{}", Uuid::now_v7().simple()),
                },
            )
            .await
            .unwrap()
            .comment_id;
        moderate(&vink, reply, 1, ModerationAction::Approve)
            .await
            .unwrap();
        children.push(reply);
        target = reply;
    }
    let ticket = enqueue(&vink, children[0]).await;
    vink.record_review_result(
        ReviewAuthority::operator(),
        &result(ticket, PostReviewFinding::Flagged),
    )
    .await
    .unwrap();
    let held = claim(&pool, OutboxPolicy::DEFAULT).await;
    finish(&pool, &held, OutboxPolicy::DEFAULT).await;
    let public = vink
        .public_discussion(sample_work(), None, None)
        .await
        .unwrap();
    assert_eq!(public.page().roots[0].public_replies, 1);
    assert_eq!(public.page().roots[0].preview.len(), 1);
    assert!(!format!("{public:?}").contains("Chữ của phản hồi cần giữ"));
    let replies = vink
        .public_replies(sample_work(), root, None, None)
        .await
        .unwrap();
    assert_eq!(replies.page().replies[0].id, children[1]);
    assert_eq!(replies.page().replies[0].context, ReplyContext::Unavailable);
    assert_eq!(replies.page().replies[0].context_author, None);
    let ids = children[0].to_string();
    assert_eq!(
        vink.endorsement_summaries(sample_work(), Some(&ids))
            .await
            .unwrap()
            .comments[0]
            .summary,
        None
    );
    let own = vink
        .own_endorsements(author(), sample_work(), Some(&ids))
        .await
        .unwrap();
    assert_eq!(
        own.comments[0].own.capability,
        vink_app::application::endorsement::Capability::Unavailable
    );
}

#[sqlx::test(migrations = false)]
async fn upgrade_and_unused_backout_preserve_comments_and_human_audit(pool: PgPool) {
    use sqlx::migrate::Migrator;
    use vink_app::persistence::MIGRATOR;
    let previous = Migrator::with_migrations(
        MIGRATOR
            .migrations
            .iter()
            .filter(|m| m.version < 20261115000000)
            .cloned()
            .collect(),
    );
    previous.run(&pool).await.unwrap();
    let vink = setup(&pool).await;
    let id = approved(&pool, &vink).await;
    MIGRATOR.run(&pool).await.unwrap();
    let mut conn = pool.acquire().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations-rollback/20261115000000_comment_post_review.down.sql"
    ))
    .execute(&mut *conn)
    .await
    .unwrap();
    drop(conn);
    assert_eq!(state(&pool, id).await, ("approved".into(), 2));
    MIGRATOR.run(&pool).await.unwrap();
    let ticket = enqueue(&vink, id).await;
    assert!(receipt(&pool, ticket).await.is_none());
    let human_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM comment_moderation_decisions WHERE comment_id=$1")
            .bind(id.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(human_count, 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn foreign_comment_schema_and_changed_text_are_quarantined_without_guessing_a_hold(
    pool: PgPool,
) {
    let vink = setup(&pool).await;
    for fault in ["comment", "schema", "digest"] {
        let id = approved(&pool, &vink).await;
        let unrelated = approved(&pool, &vink).await;
        let ticket = enqueue(&vink, id).await;
        vink.record_review_result(
            ReviewAuthority::operator(),
            &result(ticket, PostReviewFinding::Flagged),
        )
        .await
        .unwrap();
        // Labelled corruption fixtures. Runtime cannot edit immutable event provenance; no
        // product route edits comment text. The consumer must still refuse a mismatched body.
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("ALTER TABLE discussion_events DISABLE TRIGGER discussion_events_immutable")
            .execute(&mut *tx)
            .await
            .unwrap();
        match fault {
            "comment" => {
                sqlx::query("UPDATE discussion_events SET comment_id=$2 WHERE id=$1")
                    .bind(ticket)
                    .bind(unrelated.as_uuid())
                    .execute(&mut *tx)
                    .await
                    .unwrap();
            }
            "schema" => {
                sqlx::query("UPDATE discussion_events SET schema_version=2 WHERE id=$1")
                    .bind(ticket)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
            }
            _ => {
                sqlx::query(
                    "UPDATE comments SET body='Fixture: chữ bị sửa ngoài contract' WHERE id=$1",
                )
                .bind(id.as_uuid())
                .execute(&mut *tx)
                .await
                .unwrap();
            }
        }
        sqlx::query("ALTER TABLE discussion_events ENABLE TRIGGER discussion_events_immutable")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let held = claim(&pool, OutboxPolicy::DEFAULT).await;
        assert!(matches!(
            finish(&pool, &held, OutboxPolicy::DEFAULT).await,
            Completion::DeadLetter(_)
        ));
        assert!(receipt(&pool, ticket).await.is_none());
        assert_eq!(state(&pool, id).await, ("approved".into(), 2));
        assert_eq!(state(&pool, unrelated).await, ("approved".into(), 2));
    }
    assert_eq!(reviews::quarantines(&pool, 10).await.unwrap().len(), 3);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn an_event_without_a_ticket_is_visible_unresolved_operational_debt(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = approved(&pool, &vink).await;
    let event = Uuid::now_v7();
    // Labelled poison fixture, deliberately NOT a valid producer or review ticket.
    sqlx::query("INSERT INTO discussion_events (id,event_type,schema_version,comment_id,source_version) VALUES ($1,'comment.rereview_requested',1,$2,2)").bind(event).bind(id.as_uuid()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO discussion_deliveries (event_id,consumer) VALUES ($1,$2)")
        .bind(event)
        .bind(CONSUMER)
        .execute(&pool)
        .await
        .unwrap();
    let held = claim(&pool, OutboxPolicy::DEFAULT).await;
    assert!(matches!(
        finish(&pool, &held, OutboxPolicy::DEFAULT).await,
        Completion::DeadLetter(_)
    ));
    assert!(receipt(&pool, event).await.is_none());
    assert_eq!(state(&pool, id).await, ("approved".into(), 2));
    assert!(
        reviews::status(&pool)
            .await
            .unwrap()
            .iter()
            .any(|(s, n, _)| s == "quarantined" && *n == 1)
    );
    assert_eq!(
        reviews::quarantines(&pool, 10).await.unwrap(),
        [(event, "missing_ticket".into())]
    );
}
