//! Discussion outbox on real PostgreSQL (DISC-04, #120): producer atomicity, claims, the
//! generation fence, retries, dead letters, requeue, retention and the `vink-worker` process.
//!
//! Labelled fixtures: `Fixture` handlers (`test.fixture.*` consumers) and their table
//! `outbox_fixture_effects` exist only here, to give one event a second effect or to script
//! failures; they are not a notification product. "Time travel" moves `available_at` or
//! `deadline_from` into the past with SQL instead of sleeping through a lease or a deadline.

use std::collections::VecDeque;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use sqlx::migrate::Migrator;
use tokio::sync::watch;
use vink_app::application::discussion::{
    ModerateComment, ModerationAction, OwnStatus, SubmitComment,
};
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::discussion::outbox::{
    Completion, DeadReason, HandlerResult, OutboxPolicy, SkipReason, StaleReason, Transient,
};
use vink_app::domain::ids::CommentId;
use vink_app::domain::principal::SuspensionReason;
use vink_app::outbox_worker::{OutboxSettings, OutboxWorker};
use vink_app::persistence::outbox::{
    self, AdmissionShadow, BoxFuture, Claimed, EffectHandler, OutboxEvent, RequeueOutcome,
};
use vink_app::persistence::{MIGRATOR, grant_moderator, suspend_principal};

use super::*;

const MODERATOR: Uuid = Uuid::from_u128(0x5555_5555_5555_4555_8555_5555_5555_5555);
const SHADOW: &str = "admission_shadow.v1";
const OUTBOX_MIGRATION: i64 = 20261114000000;

fn policy() -> OutboxPolicy {
    // 3 attempts, 30 s lease, 1 s → 10 s backoff, 1 h deadline.
    OutboxPolicy::new(3, 30_000_000, 1_000_000, 10_000_000, 3_600_000_000).unwrap()
}

fn settings() -> OutboxSettings {
    OutboxSettings {
        concurrency: vink_app::outbox_worker::MAX_CONCURRENCY,
        policy: policy(),
        ..OutboxSettings::default()
    }
}

async fn setup(pool: &PgPool) -> Vink<PgStore> {
    let vink = seeded(pool).await;
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, 'Kiểm duyệt viên')")
        .bind(MODERATOR)
        .execute(pool)
        .await
        .unwrap();
    let note = GrantNote::parse("Kiểm duyệt viên của bộ test").unwrap();
    grant_moderator(pool, PrincipalId::from_uuid(MODERATOR), &note)
        .await
        .unwrap();
    vink
}

fn submit(body: &str, key: &str) -> SubmitComment {
    SubmitComment {
        work_id: sample_work(),
        target: None,
        body: body.into(),
        command_key: key.into(),
    }
}

fn key() -> String {
    format!("k-{}", Uuid::now_v7().simple())
}

async fn post(vink: &Vink<PgStore>, body: &str) -> CommentId {
    let submitted = vink
        .submit_comment(other(), submit(body, &key()))
        .await
        .unwrap();
    assert_eq!(submitted.status, OwnStatus::Pending);
    submitted.comment_id
}

#[derive(Debug, PartialEq, Eq, sqlx::FromRow)]
struct Delivery {
    consumer: String,
    state: String,
    attempts: i32,
    generation: i64,
    outcome: Option<String>,
    last_error: Option<String>,
    expired_leases: i32,
}

async fn deliveries_of(pool: &PgPool, comment: CommentId) -> Vec<Delivery> {
    sqlx::query_as(
        "SELECT d.consumer, d.state, d.attempts, d.generation, d.outcome, d.last_error,
                d.expired_leases
           FROM discussion_deliveries d JOIN discussion_events e ON e.id = d.event_id
          WHERE e.comment_id = $1 ORDER BY d.consumer",
    )
    .bind(comment.as_uuid())
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn count(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn observations(pool: &PgPool, comment: CommentId) -> Vec<(String, String, i64, String)> {
    sqlx::query_as(
        "SELECT outcome, reason, observed_version, observed_review
           FROM comment_admission_observations WHERE comment_id = $1",
    )
    .bind(comment.as_uuid())
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Labelled fixture edit of stored events (immutable in production): runs `sql` with the
/// immutability trigger disabled for this one statement's transaction.
async fn edit_events(pool: &PgPool, sql: &'static str, comment: Option<CommentId>) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("ALTER TABLE discussion_events DISABLE TRIGGER discussion_events_immutable")
        .execute(&mut *tx)
        .await
        .unwrap();
    let query = sqlx::query(sql);
    match comment {
        Some(id) => query.bind(id.as_uuid()),
        None => query,
    }
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("ALTER TABLE discussion_events ENABLE TRIGGER discussion_events_immutable")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

/// Time travel: every open delivery is due now (expired lease or elapsed retry backoff).
async fn make_due(pool: &PgPool) {
    sqlx::query(
        "UPDATE discussion_deliveries SET available_at = now() - interval '1 second'
          WHERE state IN ('ready', 'leased')",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// The comment row exactly as stored, to prove a consumer never wrote it.
async fn comment_row(pool: &PgPool, comment: CommentId) -> (String, Option<String>, i64, String) {
    sqlx::query_as(
        "SELECT review, removal, moderation_version, updated_at::text FROM comments WHERE id = $1",
    )
    .bind(comment.as_uuid())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn claim_all(pool: &PgPool, consumers: &[&str]) -> Vec<Claimed> {
    outbox::claim(pool, consumers, 100, policy())
        .await
        .unwrap()
        .claimed
}

#[derive(Debug, Clone, Copy)]
enum Step {
    Apply,
    Retry,
    Fail,
    /// The effect is written, then the statement fails: the transaction must roll it back.
    WriteThenError,
    Sleep(Duration),
    Panic,
}

/// Labelled fixture consumer: scripted outcomes, effects counted in `outbox_fixture_effects`
/// (no unique key, so a duplicate effect would show).
struct Fixture {
    code: &'static str,
    script: Mutex<VecDeque<Step>>,
    otherwise: Step,
    calls: AtomicUsize,
}

impl Fixture {
    fn new(code: &'static str, script: &[Step], otherwise: Step) -> Arc<Self> {
        Arc::new(Self {
            code,
            script: Mutex::new(script.iter().copied().collect()),
            otherwise,
            calls: AtomicUsize::new(0),
        })
    }
}

impl EffectHandler for Fixture {
    fn consumer(&self) -> &str {
        self.code
    }

    fn apply<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        event: &'a OutboxEvent,
    ) -> BoxFuture<'a, Result<HandlerResult, sqlx::Error>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let step = self
            .script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(self.otherwise);
        Box::pin(async move {
            const WRITE: &str =
                "INSERT INTO outbox_fixture_effects (event_id, consumer) VALUES ($1, $2)";
            let code = self.code;
            match step {
                Step::Apply => {
                    sqlx::query(WRITE)
                        .bind(event.id)
                        .bind(code)
                        .execute(&mut *tx)
                        .await?;
                    Ok(HandlerResult::Applied)
                }
                Step::Retry => Ok(HandlerResult::Retry(Transient::Database)),
                Step::Fail => Ok(HandlerResult::Fail(DeadReason::InvalidPayload)),
                Step::WriteThenError => {
                    sqlx::query(WRITE)
                        .bind(event.id)
                        .bind(code)
                        .execute(&mut *tx)
                        .await?;
                    sqlx::query("SELECT 1 / 0").execute(&mut *tx).await?;
                    unreachable!("division by zero fails")
                }
                Step::Sleep(duration) => {
                    tokio::time::sleep(duration).await;
                    sqlx::query(WRITE)
                        .bind(event.id)
                        .bind(code)
                        .execute(&mut *tx)
                        .await?;
                    Ok(HandlerResult::Applied)
                }
                Step::Panic => panic!("fixture handler panic (labelled test fixture)"),
            }
        })
    }
}

async fn fixture_table(pool: &PgPool) {
    sqlx::query(
        "CREATE TABLE outbox_fixture_effects (event_id uuid NOT NULL, consumer text NOT NULL)",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Adds a delivery of `consumer` to `comment`'s event (labelled fixture: a second effect).
async fn add_delivery(pool: &PgPool, comment: CommentId, consumer: &str) {
    sqlx::query(
        "INSERT INTO discussion_deliveries (event_id, consumer)
         SELECT id, $2 FROM discussion_events WHERE comment_id = $1",
    )
    .bind(comment.as_uuid())
    .bind(consumer)
    .execute(pool)
    .await
    .unwrap();
}

async fn effects(pool: &PgPool, consumer: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM outbox_fixture_effects WHERE consumer = $1")
        .bind(consumer)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------------------- T1/T2

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_committed_comment_owes_its_consumers_and_a_rolled_back_one_owes_nothing(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = post(&vink, "Bình luận có sự kiện").await;
    let event: (String, i16, i64) = sqlx::query_as(
        "SELECT event_type, schema_version, source_version FROM discussion_events
          WHERE comment_id = $1",
    )
    .bind(id.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event, ("comment.submitted".into(), 1, 1));
    assert_eq!(
        deliveries_of(&pool, id).await,
        [Delivery {
            consumer: SHADOW.into(),
            state: "ready".into(),
            attempts: 0,
            generation: 0,
            outcome: None,
            last_error: None,
            expired_leases: 0,
        }]
    );
    // The event holds identifiers only: no body, author or reason column exists.
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns
          WHERE table_name = 'discussion_events' ORDER BY ordinal_position",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        columns,
        [
            "id",
            "event_type",
            "schema_version",
            "comment_id",
            "source_version",
            "created_at"
        ]
    );

    // Refused submissions roll back: a reused key with another payload writes no event.
    let reused = key();
    vink.submit_comment(other(), submit("Lần đầu", &reused))
        .await
        .unwrap();
    let before = count(&pool, "SELECT count(*) FROM discussion_events").await;
    let refused = vink
        .submit_comment(other(), submit("Nội dung khác", &reused))
        .await;
    assert!(refused.is_err());
    assert_eq!(
        count(&pool, "SELECT count(*) FROM discussion_events").await,
        before
    );

    // Enqueue failure (labelled fixture trigger): the comment and its receipt roll back too.
    sqlx::raw_sql(
        "CREATE FUNCTION outbox_fixture_refuse() RETURNS trigger LANGUAGE plpgsql AS
         $$ BEGIN RAISE EXCEPTION 'fixture: delivery insert refused'; END $$;
         CREATE TRIGGER outbox_fixture_refuse BEFORE INSERT ON discussion_deliveries
             FOR EACH ROW EXECUTE FUNCTION outbox_fixture_refuse();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let comments = count(&pool, "SELECT count(*) FROM comments").await;
    let receipts = count(&pool, "SELECT count(*) FROM discussion_command_receipts").await;
    let events = count(&pool, "SELECT count(*) FROM discussion_events").await;
    let failing = key();
    assert!(
        vink.submit_comment(other(), submit("Không được lưu", &failing))
            .await
            .is_err()
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM comments").await,
        comments
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM discussion_command_receipts").await,
        receipts
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM discussion_events").await,
        events
    );
    sqlx::query("DROP TRIGGER outbox_fixture_refuse ON discussion_deliveries")
        .execute(&pool)
        .await
        .unwrap();
    // The same command succeeds once enqueue works again, with exactly one event.
    let retried = vink
        .submit_comment(other(), submit("Không được lưu", &failing))
        .await
        .unwrap();
    assert!(!retried.replayed);
    assert_eq!(deliveries_of(&pool, retried.comment_id).await.len(), 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn replays_and_concurrent_submissions_never_duplicate_a_logical_event(pool: PgPool) {
    let vink = setup(&pool).await;
    let once = key();
    let first = vink
        .submit_comment(other(), submit("Một lần", &once))
        .await
        .unwrap();
    let again = vink
        .submit_comment(other(), submit("Một lần", &once))
        .await
        .unwrap();
    assert!(again.replayed);
    assert_eq!(again.comment_id, first.comment_id);
    assert_eq!(deliveries_of(&pool, first.comment_id).await.len(), 1);

    // Eight concurrent retries of one command: one comment, one event.
    let shared = key();
    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let vink = Vink::new(PgStore::new(pool.clone()));
            let command = submit("Cùng một lệnh", &shared);
            tokio::spawn(async move { vink.submit_comment(other(), command).await.unwrap() })
        })
        .collect();
    let mut ids = Vec::new();
    for task in tasks {
        ids.push(task.await.unwrap().comment_id);
    }
    ids.dedup();
    assert_eq!(ids.len(), 1);
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM discussion_events WHERE comment_id = $1")
            .bind(ids[0].as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 1);

    // Eight concurrent distinct commands: eight events, one per comment.
    let tasks: Vec<_> = (0..8)
        .map(|n| {
            let vink = Vink::new(PgStore::new(pool.clone()));
            let command = submit(&format!("Lệnh {n}"), &key());
            tokio::spawn(async move { vink.submit_comment(other(), command).await.unwrap() })
        })
        .collect();
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM (SELECT comment_id FROM discussion_events
                                    GROUP BY comment_id HAVING count(*) > 1) AS duplicated"
        )
        .await,
        0
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM discussion_events").await,
        count(&pool, "SELECT count(*) FROM comments").await
    );
}

// ---------------------------------------------------------------------------------------- T3/T4/T5

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn competing_workers_claim_disjoint_work_and_a_crash_is_reclaimed(pool: PgPool) {
    let vink = setup(&pool).await;
    let mut comments = Vec::new();
    for n in 0..6 {
        comments.push(post(&vink, &format!("Bình luận {n}")).await);
    }
    // Another worker is mid-claim on two rows: SKIP LOCKED passes them without waiting.
    let mut other_worker = pool.begin().await.unwrap();
    sqlx::query(
        "SELECT 1 FROM discussion_deliveries d JOIN discussion_events e ON e.id = d.event_id
          WHERE e.comment_id = ANY($1) FOR UPDATE OF d",
    )
    .bind(vec![comments[0].as_uuid(), comments[1].as_uuid()])
    .execute(&mut *other_worker)
    .await
    .unwrap();
    let started = Instant::now();
    let first = claim_all(&pool, &[SHADOW]).await;
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "a claim never waits on a locked row"
    );
    assert_eq!(first.len(), 4);
    other_worker.rollback().await.unwrap();

    // Two claims at once: disjoint, together everything that is left.
    let (a, b) = tokio::join!(claim_all(&pool, &[SHADOW]), claim_all(&pool, &[SHADOW]));
    let mut events: Vec<Uuid> = a.iter().chain(&b).map(|c| c.event.id).collect();
    assert_eq!(events.len(), 2);
    events.dedup();
    assert_eq!(events.len(), 2);
    // No transaction stays open between the claim and the effect.
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM pg_stat_activity
              WHERE datname = current_database() AND state LIKE 'idle in transaction%'"
        )
        .await,
        0
    );

    // Crash after claim: nothing ran; once the lease expires another worker reclaims it.
    let crashed = &first[0];
    make_due(&pool).await;
    let reclaimed = claim_all(&pool, &[SHADOW]).await;
    let mine = reclaimed
        .iter()
        .find(|c| c.event.id == crashed.event.id)
        .unwrap();
    assert!(mine.reclaimed);
    assert_eq!((mine.generation.get(), mine.attempts), (2, 2));
    let done = outbox::complete(&pool, mine, &AdmissionShadow, policy(), 0)
        .await
        .unwrap();
    assert_eq!(
        done,
        Completion::Succeed {
            already_applied: false
        }
    );
    let comment = CommentId::from_uuid(crashed.event.comment_id);
    let delivery = &deliveries_of(&pool, comment).await[0];
    assert_eq!(
        (
            delivery.state.as_str(),
            delivery.expired_leases,
            delivery.attempts
        ),
        ("succeeded", 1, 2)
    );
    assert_eq!(observations(&pool, comment).await.len(), 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_worker_that_lost_its_claim_writes_neither_effect_nor_outcome(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = post(&vink, "Bình luận bị tranh lease").await;
    let a = claim_all(&pool, &[SHADOW]).await.remove(0);
    assert_eq!(a.generation.get(), 1);
    // A stalls; its lease expires; B reclaims.
    make_due(&pool).await;
    let b = claim_all(&pool, &[SHADOW]).await.remove(0);
    assert_eq!(b.generation.get(), 2);

    // A comes back while B still holds the claim: every write of A is refused.
    assert_eq!(
        outbox::complete(&pool, &a, &AdmissionShadow, policy(), 0)
            .await
            .unwrap(),
        Completion::Stale(StaleReason::Reclaimed)
    );
    assert!(
        observations(&pool, id).await.is_empty(),
        "A's effect was not written"
    );
    for failure in [Transient::Database, Transient::Timeout, Transient::Panic] {
        assert_eq!(
            outbox::record_failure(&pool, &a, failure, policy(), 0)
                .await
                .unwrap(),
            Completion::Stale(StaleReason::Reclaimed)
        );
    }
    let failing = Fixture::new(SHADOW, &[Step::Fail], Step::Fail);
    assert_eq!(
        outbox::complete(&pool, &a, failing.as_ref(), policy(), 0)
            .await
            .unwrap(),
        Completion::Stale(StaleReason::Reclaimed),
        "A cannot dead-letter B's claim"
    );
    assert_eq!(
        failing.calls.load(Ordering::SeqCst),
        0,
        "the handler never ran for A"
    );
    let held = &deliveries_of(&pool, id).await[0];
    assert_eq!(
        (
            held.state.as_str(),
            held.generation,
            held.outcome.as_deref()
        ),
        ("leased", 2, None)
    );

    // B finishes; A's late result is still refused and nothing is duplicated.
    assert_eq!(
        outbox::complete(&pool, &b, &AdmissionShadow, policy(), 0)
            .await
            .unwrap(),
        Completion::Succeed {
            already_applied: false
        }
    );
    assert_eq!(
        outbox::complete(&pool, &a, &AdmissionShadow, policy(), 0)
            .await
            .unwrap(),
        Completion::Stale(StaleReason::NotLeased)
    );
    assert_eq!(observations(&pool, id).await.len(), 1);
    assert_eq!(deliveries_of(&pool, id).await[0].state, "succeeded");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn crashes_around_the_effect_and_lost_commit_responses_never_duplicate_it(pool: PgPool) {
    let vink = setup(&pool).await;
    fixture_table(&pool).await;
    let fixture = "test.fixture.crash";
    let id = post(&vink, "Bình luận cho fixture").await;
    add_delivery(&pool, id, fixture).await;

    // The effect is written and then the transaction fails before commit: nothing survives.
    let handler = Fixture::new(fixture, &[Step::WriteThenError], Step::Apply);
    let claimed = claim_all(&pool, &[fixture]).await.remove(0);
    let failed = outbox::complete(&pool, &claimed, handler.as_ref(), policy(), 0)
        .await
        .unwrap();
    assert!(matches!(
        failed,
        Completion::Retry {
            error: Transient::Database,
            ..
        }
    ));
    assert_eq!(effects(&pool, fixture).await, 0);
    let retry = deliveries_of(&pool, id).await;
    let row = retry.iter().find(|d| d.consumer == fixture).unwrap();
    assert_eq!(
        (row.state.as_str(), row.last_error.as_deref()),
        ("ready", Some("database"))
    );

    // Second attempt commits the effect and its outcome together.
    make_due(&pool).await;
    let claimed = claim_all(&pool, &[fixture]).await.remove(0);
    assert_eq!(
        outbox::complete(&pool, &claimed, handler.as_ref(), policy(), 0)
            .await
            .unwrap(),
        Completion::Succeed {
            already_applied: false
        }
    );
    // Lost commit response: the worker believes the commit failed and records a failure or
    // runs again with the same claim. The delivery is no longer leased; nothing is written.
    assert_eq!(
        outbox::record_failure(&pool, &claimed, Transient::Database, policy(), 0)
            .await
            .unwrap(),
        Completion::Stale(StaleReason::NotLeased)
    );
    assert_eq!(
        outbox::complete(&pool, &claimed, handler.as_ref(), policy(), 0)
            .await
            .unwrap(),
        Completion::Stale(StaleReason::NotLeased)
    );
    assert_eq!(effects(&pool, fixture).await, 1);

    // The shadow consumer's own receipt: re-applying the effect records nothing new.
    let shadow = claim_all(&pool, &[SHADOW]).await.remove(0);
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        AdmissionShadow.apply(&mut tx, &shadow.event).await.unwrap(),
        HandlerResult::Applied
    );
    assert_eq!(
        AdmissionShadow.apply(&mut tx, &shadow.event).await.unwrap(),
        HandlerResult::AlreadyApplied
    );
    tx.commit().await.unwrap();
    assert_eq!(
        outbox::complete(&pool, &shadow, &AdmissionShadow, policy(), 0)
            .await
            .unwrap(),
        Completion::Succeed {
            already_applied: true
        }
    );
    assert_eq!(observations(&pool, id).await.len(), 1);
    // The schema itself refuses a second observation of one comment and candidate.
    let duplicate = sqlx::query(
        "INSERT INTO comment_admission_observations
                (comment_id, candidate_policy, event_id, outcome, reason, observed_version,
                 observed_review)
         VALUES ($1, 'trusted-cohort.v1', $2, 'pending', 'trust_unknown', 1, 'pending')",
    )
    .bind(id.as_uuid())
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(
        duplicate
            .to_string()
            .contains("comment_admission_observations_pkey")
    );
}

// ---------------------------------------------------------------------------------------- T6/T7

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn one_effect_failing_retries_only_that_effect(pool: PgPool) {
    let vink = setup(&pool).await;
    fixture_table(&pool).await;
    let fixture = "test.fixture.second_effect";
    let id = post(&vink, "Hai hiệu ứng").await;
    add_delivery(&pool, id, fixture).await;
    let second = Fixture::new(fixture, &[Step::Retry], Step::Apply);
    let worker = OutboxWorker::with_handlers(
        pool.clone(),
        settings(),
        vec![Arc::new(AdmissionShadow), second.clone()],
    );
    assert_eq!(worker.run_once().await.unwrap(), 2);
    let after_first = deliveries_of(&pool, id).await;
    assert_eq!(after_first[0].consumer, SHADOW);
    assert_eq!(
        (after_first[0].state.as_str(), after_first[0].generation),
        ("succeeded", 1)
    );
    assert_eq!(
        (
            after_first[1].state.as_str(),
            after_first[1].last_error.as_deref()
        ),
        ("ready", Some("database"))
    );
    // Not due yet: nothing is claimed before the backoff ends.
    assert_eq!(worker.run_once().await.unwrap(), 0);
    make_due(&pool).await;
    assert_eq!(
        worker.run_once().await.unwrap(),
        1,
        "only the failed effect runs again"
    );
    let after_retry = deliveries_of(&pool, id).await;
    assert_eq!(
        (after_retry[0].state.as_str(), after_retry[0].generation),
        ("succeeded", 1)
    );
    assert_eq!(
        (after_retry[1].state.as_str(), after_retry[1].attempts),
        ("succeeded", 2)
    );
    assert_eq!(observations(&pool, id).await.len(), 1);
    assert_eq!(effects(&pool, fixture).await, 1);
    assert_eq!(worker.stats().succeeded.load(Ordering::Relaxed), 2);
    assert_eq!(worker.stats().retried.load(Ordering::Relaxed), 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn poison_exhaustion_and_deadlines_end_in_dead_letters_that_an_operator_requeues(
    pool: PgPool,
) {
    let vink = setup(&pool).await;
    fixture_table(&pool).await;
    let flaky = "test.fixture.flaky";
    let healthy = post(&vink, "Bình thường").await;
    let poison = post(&vink, "Sự kiện lạ").await;
    let exhausted = post(&vink, "Hết lượt").await;
    let late = post(&vink, "Quá hạn").await;
    // Poison: a schema this binary cannot read (labelled fixture edit of a stored event).
    edit_events(
        &pool,
        "UPDATE discussion_events SET schema_version = 99 WHERE comment_id = $1",
        Some(poison),
    )
    .await;
    add_delivery(&pool, exhausted, flaky).await;
    sqlx::query(
        "UPDATE discussion_deliveries d SET deadline_from = now() - interval '2 hours'
           FROM discussion_events e WHERE e.id = d.event_id AND e.comment_id = $1",
    )
    .bind(late.as_uuid())
    .execute(&pool)
    .await
    .unwrap();
    // A consumer this binary does not know is left alone (backout safety).
    add_delivery(&pool, healthy, "future.consumer.v9").await;

    let always_retry = Fixture::new(flaky, &[Step::Retry, Step::Retry, Step::Retry], Step::Apply);
    let worker = OutboxWorker::with_handlers(
        pool.clone(),
        settings(),
        vec![Arc::new(AdmissionShadow), always_retry.clone()],
    );
    worker.run_once().await.unwrap();
    for _ in 0..2 {
        make_due(&pool).await;
        worker.run_once().await.unwrap();
    }
    let state = |rows: Vec<Delivery>, consumer: &str| {
        let row = rows.into_iter().find(|d| d.consumer == consumer).unwrap();
        (row.state, row.outcome, row.attempts)
    };
    assert_eq!(
        state(deliveries_of(&pool, healthy).await, SHADOW),
        ("succeeded".into(), Some("applied".into()), 1),
        "poison work never blocks healthy work"
    );
    assert_eq!(
        state(deliveries_of(&pool, healthy).await, "future.consumer.v9"),
        ("ready".into(), None, 0)
    );
    assert_eq!(worker.backlog().await.unwrap().unhandled, 1);
    assert_eq!(
        state(deliveries_of(&pool, poison).await, SHADOW),
        ("dead".into(), Some("unsupported_event".into()), 1)
    );
    assert_eq!(
        state(deliveries_of(&pool, exhausted).await, flaky),
        ("dead".into(), Some("retry_exhausted".into()), 3)
    );
    assert_eq!(
        state(deliveries_of(&pool, late).await, SHADOW),
        ("dead".into(), Some("deadline_exceeded".into()), 0)
    );
    assert!(observations(&pool, poison).await.is_empty());
    let listed = outbox::dead_letters(&pool, 10).await.unwrap();
    assert_eq!(listed.len(), 3);

    // Requeue: dead letters only, audited, fresh budget, fence kept.
    let event_of = |comment: CommentId| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM discussion_events WHERE comment_id = $1")
                .bind(comment.as_uuid())
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let exhausted_event = event_of(exhausted).await;
    assert_eq!(
        outbox::requeue(&pool, exhausted_event, flaky, "Fixture đã sửa")
            .await
            .unwrap(),
        RequeueOutcome::Requeued {
            previous_attempts: 3
        }
    );
    assert_eq!(
        outbox::requeue(&pool, event_of(healthy).await, SHADOW, "không được")
            .await
            .unwrap(),
        RequeueOutcome::Refused(vink_app::domain::discussion::outbox::DeliveryState::Succeeded),
        "a finished effect never runs again"
    );
    assert_eq!(
        outbox::requeue(&pool, Uuid::now_v7(), SHADOW, "không có")
            .await
            .unwrap(),
        RequeueOutcome::NotFound
    );
    let requeued = state(deliveries_of(&pool, exhausted).await, flaky);
    assert_eq!(requeued, ("ready".into(), None, 0));
    worker.run_once().await.unwrap();
    let row = deliveries_of(&pool, exhausted)
        .await
        .into_iter()
        .find(|d| d.consumer == flaky)
        .unwrap();
    assert_eq!((row.state.as_str(), row.generation), ("succeeded", 4));
    assert_eq!(effects(&pool, flaky).await, 1);
    let audit: (String, i32, String) = sqlx::query_as(
        "SELECT previous_outcome, previous_attempts, note FROM discussion_delivery_requeues",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        audit,
        ("retry_exhausted".into(), 3, "Fixture đã sửa".into())
    );
    // The audit is append-only and events are immutable, even for the runtime credential.
    for sql in [
        "UPDATE discussion_delivery_requeues SET note = 'sửa'",
        "DELETE FROM discussion_delivery_requeues",
        "UPDATE discussion_events SET source_version = source_version + 1",
    ] {
        let refused = sqlx::query(sql).execute(&pool).await.unwrap_err();
        assert!(
            refused.to_string().contains("append-only")
                || refused.to_string().contains("immutable"),
            "{sql}: {refused}"
        );
    }

    // Retention: only events whose deliveries all finished go; open and dead ones stay, and the
    // shadow receipt outlives its event.
    let observations_before =
        count(&pool, "SELECT count(*) FROM comment_admission_observations").await;
    edit_events(
        &pool,
        "UPDATE discussion_events SET created_at = now() - interval '40 days'",
        None,
    )
    .await;
    sqlx::query(
        "UPDATE discussion_deliveries SET finished_at = now() - interval '40 days'
          WHERE finished_at IS NOT NULL",
    )
    .execute(&pool)
    .await
    .unwrap();
    let pruned = outbox::prune(&pool, 30, 1_000).await.unwrap();
    // healthy keeps its event (unhandled open delivery); poison and late are dead.
    assert_eq!(
        pruned, 1,
        "only the fully finished event of `exhausted` goes"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM discussion_events").await,
        3
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM comment_admission_observations").await,
        observations_before
    );
}

// ---------------------------------------------------------------------------------------- T8

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn old_events_never_overwrite_newer_human_decisions_or_revive_removed_comments(pool: PgPool) {
    let vink = setup(&pool).await;
    let approved = post(&vink, "Đã duyệt trước").await;
    let rejected = post(&vink, "Đã từ chối trước").await;
    let removed = post(&vink, "Người viết đã gỡ").await;
    let untouched = post(&vink, "Chưa ai xem").await;
    for (id, action) in [
        (approved, ModerationAction::Approve),
        (rejected, ModerationAction::Reject),
    ] {
        vink.moderate_comment(
            Some(PrincipalId::from_uuid(MODERATOR)),
            ModerateComment {
                comment_id: id,
                expected_version: 1,
                action,
                reason: (action == ModerationAction::Reject).then(|| "Lý do kiểm thử.".to_string()),
            },
        )
        .await
        .unwrap();
    }
    vink.remove_own_comment(other(), sample_work(), removed)
        .await
        .unwrap();
    let before: Vec<_> = futures_rows(&pool, &[approved, rejected, removed, untouched]).await;
    // Withdrawal after submission does not make the internal observation public.
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();

    let worker = OutboxWorker::production(pool.clone(), settings());
    assert_eq!(worker.run_once().await.unwrap(), 4);
    assert_eq!(
        futures_rows(&pool, &[approved, rejected, removed, untouched]).await,
        before
    );
    assert_eq!(
        observations(&pool, approved).await,
        [(
            "pending".into(),
            "trust_unknown".into(),
            2,
            "approved".into()
        )]
    );
    assert_eq!(
        observations(&pool, rejected).await,
        [(
            "pending".into(),
            "trust_unknown".into(),
            2,
            "rejected".into()
        )]
    );
    assert_eq!(
        observations(&pool, untouched).await,
        [(
            "pending".into(),
            "trust_unknown".into(),
            1,
            "pending".into()
        )]
    );
    assert!(observations(&pool, removed).await.is_empty());
    assert_eq!(
        deliveries_of(&pool, removed).await[0].outcome.as_deref(),
        Some(SkipReason::TargetRemoved.code())
    );
    // The candidate never approves: no trust authority exists.
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM comment_admission_observations WHERE outcome = 'approved'"
        )
        .await,
        0
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_moderation_in_flight_delays_the_observation_without_deadlock_or_overwrite(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = post(&vink, "Đang được kiểm duyệt").await;
    // A moderator's transaction holds the comment row FOR UPDATE (as `moderate_comment` does).
    let mut moderator = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM comments WHERE id = $1 FOR UPDATE")
        .bind(id.as_uuid())
        .execute(&mut *moderator)
        .await
        .unwrap();
    let started = Instant::now();
    let worker = OutboxWorker::production(pool.clone(), settings());
    assert_eq!(worker.run_once().await.unwrap(), 1);
    // The observation's foreign key waited at most the lock bound, then the attempt failed
    // retryably; nothing was written and the moderator was never blocked by the worker.
    assert!(started.elapsed() < Duration::from_secs(5));
    let row = &deliveries_of(&pool, id).await[0];
    assert_eq!(
        (row.state.as_str(), row.last_error.as_deref()),
        ("ready", Some("database"))
    );
    assert!(observations(&pool, id).await.is_empty());
    sqlx::query(
        "UPDATE comments SET review = 'approved', moderation_version = 2, updated_at = now()
          WHERE id = $1",
    )
    .bind(id.as_uuid())
    .execute(&mut *moderator)
    .await
    .unwrap();
    moderator.commit().await.unwrap();
    make_due(&pool).await;
    worker.run_once().await.unwrap();
    assert_eq!(
        observations(&pool, id).await,
        [(
            "pending".into(),
            "trust_unknown".into(),
            2,
            "approved".into()
        )]
    );
    assert_eq!(comment_row(&pool, id).await.0, "approved");
}

async fn futures_rows(
    pool: &PgPool,
    ids: &[CommentId],
) -> Vec<(String, Option<String>, i64, String)> {
    let mut rows = Vec::new();
    for id in ids {
        rows.push(comment_row(pool, *id).await);
    }
    rows
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_suspended_writer_is_observed_as_refused_without_touching_the_comment(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = post(&vink, "Trước khi bị đình chỉ").await;
    let before = comment_row(&pool, id).await;
    suspend_principal(
        &pool,
        PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL),
        &SuspensionReason::parse("Đình chỉ trong bộ kiểm thử outbox.").unwrap(),
    )
    .await
    .unwrap();
    OutboxWorker::production(pool.clone(), settings())
        .run_once()
        .await
        .unwrap();
    assert_eq!(
        observations(&pool, id).await,
        [("refused".into(), "suspended".into(), 1, "pending".into())]
    );
    assert_eq!(comment_row(&pool, id).await, before);
}

// ---------------------------------------------------------------------------------------- T10

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn timeouts_panics_and_shutdown_never_acknowledge_unfinished_work(pool: PgPool) {
    let vink = setup(&pool).await;
    fixture_table(&pool).await;
    let slow = "test.fixture.slow";
    let panicky = "test.fixture.panic";
    let id = post(&vink, "Cho fixture chậm").await;
    add_delivery(&pool, id, slow).await;
    add_delivery(&pool, id, panicky).await;
    let quick = OutboxSettings {
        handler_timeout: Duration::from_millis(300),
        ..settings()
    };
    let slow_handler = Fixture::new(slow, &[Step::Sleep(Duration::from_secs(3))], Step::Apply);
    let panic_handler = Fixture::new(panicky, &[Step::Panic], Step::Apply);
    let worker = OutboxWorker::with_handlers(
        pool.clone(),
        quick,
        vec![slow_handler.clone(), panic_handler.clone()],
    );
    worker.run_once().await.unwrap();
    let rows = deliveries_of(&pool, id).await;
    let row = |consumer: &str| rows.iter().find(|d| d.consumer == consumer).unwrap();
    assert_eq!(
        (row(slow).state.as_str(), row(slow).last_error.as_deref()),
        ("ready", Some("timeout"))
    );
    assert_eq!(
        effects(&pool, slow).await,
        0,
        "a timed-out effect rolled back"
    );
    // `run_once` joins without the supervisor: the panic leaves the lease to expire.
    assert_eq!(row(panicky).state, "leased");
    assert_eq!(effects(&pool, panicky).await, 0);

    // The supervised loop records a panic as a retryable failure.
    make_due(&pool).await;
    let panic_again = Fixture::new(panicky, &[Step::Panic], Step::Apply);
    let supervised = OutboxWorker::with_handlers(
        pool.clone(),
        OutboxSettings {
            poll_min: Duration::from_millis(20),
            poll_max: Duration::from_millis(50),
            ..settings()
        },
        vec![panic_again],
    );
    let (stop, stopped) = watch::channel(false);
    let running = tokio::spawn(supervised.run(stopped));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let rows = deliveries_of(&pool, id).await;
        let panicked = rows.iter().find(|d| d.consumer == panicky).unwrap();
        if panicked.last_error.as_deref() == Some("panic") && panicked.state == "ready" {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "panic never recorded: {panicked:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    stop.send(true).unwrap();
    running.await.unwrap();

    // Shutdown with a handler that outlives the drain: aborted, rolled back, still leased.
    let hang = "test.fixture.hang";
    let id2 = post(&vink, "Treo khi tắt").await;
    add_delivery(&pool, id2, hang).await;
    let hanging = Fixture::new(hang, &[], Step::Sleep(Duration::from_secs(30)));
    let draining = OutboxWorker::with_handlers(
        pool.clone(),
        OutboxSettings {
            drain: Duration::from_millis(200),
            handler_timeout: Duration::from_secs(25),
            poll_min: Duration::from_millis(20),
            ..settings()
        },
        vec![hanging.clone()],
    );
    let (stop, stopped) = watch::channel(false);
    let running = tokio::spawn(draining.run(stopped));
    let deadline = Instant::now() + Duration::from_secs(10);
    while hanging.calls.load(Ordering::SeqCst) == 0 {
        assert!(
            Instant::now() < deadline,
            "the hanging handler never started"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let asked = Instant::now();
    stop.send(true).unwrap();
    running.await.unwrap();
    assert!(
        asked.elapsed() < Duration::from_secs(3),
        "shutdown respects the drain bound"
    );
    let rows = deliveries_of(&pool, id2).await;
    let left = rows.iter().find(|d| d.consumer == hang).unwrap();
    assert_eq!((left.state.as_str(), left.generation), ("leased", 1));
    assert_eq!(
        effects(&pool, hang).await,
        0,
        "nothing acknowledged or applied"
    );
}

// ---------------------------------------------------------------------------------------- T11

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_stuck_worker_never_blocks_public_reads_or_new_comments(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = post(&vink, "Đã có trước").await;
    vink.moderate_comment(
        Some(PrincipalId::from_uuid(MODERATOR)),
        ModerateComment {
            comment_id: id,
            expected_version: 1,
            action: ModerationAction::Approve,
            reason: None,
        },
    )
    .await
    .unwrap();
    // A worker holds every delivery row and never finishes.
    let mut stuck = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM discussion_deliveries FOR UPDATE")
        .execute(&mut *stuck)
        .await
        .unwrap();
    let started = Instant::now();
    let page = vink
        .public_discussion(sample_work(), None, Some(20))
        .await
        .unwrap();
    assert_eq!(page.page().roots.len(), 1);
    post(&vink, "Mới trong lúc worker treo").await;
    // Moderation and removal paths do not touch the queue either.
    vink.remove_own_comment(other(), sample_work(), id)
        .await
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(2));
    stuck.rollback().await.unwrap();

    // Nothing from the queue reaches a public or viewer response.
    OutboxWorker::production(pool.clone(), settings())
        .run_once()
        .await
        .unwrap();
    let app = app(&pool).await;
    let work = dev_seed::SAMPLE_WORK;
    for (path, actor) in [
        (format!("/api/v1/works/{work}/discussion"), None),
        (
            format!("/api/v1/works/{work}/discussion"),
            Some(dev_seed::OTHER_PRINCIPAL),
        ),
        (
            format!("/api/v1/works/{work}/discussion/me"),
            Some(dev_seed::OTHER_PRINCIPAL),
        ),
        ("/api/v1/moderation/comments".to_string(), Some(MODERATOR)),
    ] {
        let (status, _, body) = call(&app, "GET", &path, actor, None, None).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        for leaked in [
            "trusted-cohort",
            "admission_shadow",
            "trust_unknown",
            "observ",
        ] {
            assert!(!body.contains(leaked), "{path} leaks {leaked}");
        }
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_restricted_runtime_role_is_enough_to_run_the_outbox(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = post(&vink, "Vai trò hạn chế").await;
    let role = format!("vink_outbox_rt_{}", Uuid::now_v7().simple());
    // The runtime grants of services/postgres/provision/owner.sql, nothing more.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE ROLE {role} LOGIN PASSWORD 'fixture' NOSUPERUSER NOCREATEDB NOCREATEROLE;
         GRANT USAGE ON SCHEMA public TO {role};
         GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO {role};
         GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO {role};"
    )))
    .execute(&pool)
    .await
    .unwrap();
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
    let worker = OutboxWorker::production(restricted.clone(), settings());
    assert_eq!(worker.run_once().await.unwrap(), 1);
    assert_eq!(observations(&pool, id).await.len(), 1);
    restricted.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "REVOKE ALL ON ALL TABLES IN SCHEMA public FROM {role};
         REVOKE ALL ON ALL SEQUENCES IN SCHEMA public FROM {role};
         REVOKE ALL ON SCHEMA public FROM {role};
         DROP ROLE {role};"
    )))
    .execute(&pool)
    .await
    .unwrap();
}

// ---------------------------------------------------------------------------------------- T12

#[sqlx::test(migrations = false)]
async fn upgrading_keeps_old_comments_and_new_ones_start_owing_work(pool: PgPool) {
    let previous = Migrator::with_migrations(
        MIGRATOR
            .migrations
            .iter()
            .filter(|m| m.version < OUTBOX_MIGRATION)
            .cloned()
            .collect(),
    );
    previous.run(&pool).await.unwrap();
    let vink = setup(&pool).await;
    // A comment the old binary stored: same columns, no event.
    let thread = Uuid::now_v7();
    sqlx::query("INSERT INTO discussion_threads (id, work_id) VALUES ($1, $2)")
        .bind(thread)
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&pool)
        .await
        .unwrap();
    let old = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id, body, review,
                               moderation_version, admission_policy, admission_reason)
         VALUES ($1, $2, 'root', false, $3, 'Trước nâng cấp', 'pending', 1, 'premoderation.v1',
                 'policy_premoderation')",
    )
    .bind(old)
    .bind(thread)
    .bind(dev_seed::OTHER_PRINCIPAL)
    .execute(&pool)
    .await
    .unwrap();
    // A binary ahead of its schema fails closed: the submission rolls back entirely.
    assert!(
        vink.submit_comment(other(), submit("Binary mới, schema cũ", &key()))
            .await
            .is_err()
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM comments").await, 1);
    MIGRATOR.run(&pool).await.unwrap();
    assert_eq!(schema_state(&pool).await.unwrap(), SchemaState::Current);
    // No backfill: the old comment owes nothing and is untouched; reads keep working.
    assert_eq!(
        count(&pool, "SELECT count(*) FROM discussion_events").await,
        0
    );
    let review: String = sqlx::query_scalar("SELECT review FROM comments WHERE id = $1")
        .bind(old)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(review, "pending");
    let new = post(&vink, "Sau nâng cấp").await;
    assert_eq!(deliveries_of(&pool, new).await.len(), 1);

    // An event written by a newer binary (schema 2) waits as a dead letter in this one and is
    // requeued after the roll-forward, keeping its identity and fence.
    edit_events(
        &pool,
        "UPDATE discussion_events SET schema_version = 2 WHERE comment_id = $1",
        Some(new),
    )
    .await;
    let worker = OutboxWorker::production(pool.clone(), settings());
    worker.run_once().await.unwrap();
    assert_eq!(
        deliveries_of(&pool, new).await[0].outcome.as_deref(),
        Some("unsupported_event")
    );
    edit_events(
        &pool,
        "UPDATE discussion_events SET schema_version = 1 WHERE comment_id = $1",
        Some(new),
    )
    .await;
    let event: Uuid = sqlx::query_scalar("SELECT id FROM discussion_events WHERE comment_id = $1")
        .bind(new.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
    outbox::requeue(&pool, event, SHADOW, "Đã nâng cấp binary")
        .await
        .unwrap();
    worker.run_once().await.unwrap();
    let row = &deliveries_of(&pool, new).await[0];
    assert_eq!((row.state.as_str(), row.generation), ("succeeded", 2));
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_schema_rollback_refuses_while_work_is_owed_and_keeps_comments(pool: PgPool) {
    let vink = setup(&pool).await;
    let id = post(&vink, "Trước khi rollback").await;
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations-rollback/20261114000000_discussion_outbox.down.sql"),
    )
    .unwrap();
    let rollback = |pool: PgPool, script: String| async move {
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(sqlx::AssertSqlSafe(script))
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    };
    let refused = rollback(pool.clone(), script.clone()).await.unwrap_err();
    assert!(refused.to_string().contains("still owes work"), "{refused}");
    assert_eq!(
        deliveries_of(&pool, id).await[0].state,
        "ready",
        "nothing was dropped"
    );
    // Drained: the rollback runs, comments and receipts stay, the history goes back one step.
    OutboxWorker::production(pool.clone(), settings())
        .run_once()
        .await
        .unwrap();
    let comments = count(&pool, "SELECT count(*) FROM comments").await;
    let receipts = count(&pool, "SELECT count(*) FROM discussion_command_receipts").await;
    rollback(pool.clone(), script).await.unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM comments").await,
        comments
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM discussion_command_receipts").await,
        receipts
    );
    let table: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('discussion_deliveries')::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(table, None);
    assert_eq!(
        count(&pool, "SELECT max(version) FROM _sqlx_migrations").await,
        20261113000000
    );
    // Roll-forward applies the migration again on the same data.
    MIGRATOR.run(&pool).await.unwrap();
    assert_eq!(schema_state(&pool).await.unwrap(), SchemaState::Current);
}

// ---------------------------------------------------------------------------------------- T9/T10/T11 process

fn database_url(pool: &PgPool) -> String {
    let base = std::env::var("DATABASE_URL").expect("the postgres lane sets DATABASE_URL");
    let database = pool.connect_options().get_database().unwrap().to_owned();
    let (prefix, _) = base.rsplit_once('/').unwrap();
    format!("{prefix}/{database}")
}

fn spawn_worker(database_url: &str, profile: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_vink-worker"))
        .env_clear()
        .env("DATABASE_URL", database_url)
        .env("VINK_PROFILE", profile)
        .env("VINK_PUBLIC_ORIGIN", "https://vink.example")
        .env("RUST_LOG", "info")
        .env("VINK_LOG_FORMAT", "json")
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap()
}

fn terminate(child: &Child) {
    let status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
}

async fn wait_exit(mut child: Child, within: Duration) -> (Option<i32>, String) {
    let deadline = Instant::now() + within;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut stderr = String::new();
            use std::io::Read;
            child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut stderr)
                .unwrap();
            return (status.code(), stderr);
        }
        assert!(Instant::now() < deadline, "the worker did not exit in time");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_succeeded(pool: &PgPool, n: i64) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let done = count(
            pool,
            "SELECT count(*) FROM discussion_deliveries WHERE state = 'succeeded'",
        )
        .await;
        if done >= n {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "only {done} of {n} deliveries succeeded"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_worker_process_drains_backlog_survives_outages_kills_and_restarts(pool: PgPool) {
    let vink = setup(&pool).await;
    let url = database_url(&pool);
    let bodies: Vec<String> = (0..5).map(|n| format!("Nội dung riêng số {n}")).collect();
    let mut ids = Vec::new();
    for body in &bodies {
        ids.push(post(&vink, body).await);
    }
    // Startup backlog: processed without any notification.
    let worker = spawn_worker(&url, "local");
    wait_succeeded(&pool, 5).await;
    // Outage: every connection of the worker is terminated; it reconnects by itself.
    sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity
          WHERE datname = current_database() AND application_name = 'vink-app'
            AND pid <> pg_backend_pid()",
    )
    .execute(&pool)
    .await
    .unwrap();
    ids.push(post(&vink, "Sau khi mất kết nối").await);
    wait_succeeded(&pool, 6).await;
    // Forced kill, more work, restart: the new process finishes it exactly once.
    let mut killed = worker;
    killed.kill().unwrap();
    killed.wait().unwrap();
    for n in 0..3 {
        ids.push(post(&vink, &format!("Trong lúc worker chết {n}")).await);
    }
    let restarted = spawn_worker(&url, "local");
    wait_succeeded(&pool, 9).await;
    terminate(&restarted);
    let (code, logs) = wait_exit(restarted, Duration::from_secs(15)).await;
    assert_eq!(code, Some(0), "{logs}");
    assert!(logs.contains("\"outbox\":\"ready\""), "{logs}");
    assert!(logs.contains("vink-worker stopped"), "{logs}");
    assert_eq!(
        count(&pool, "SELECT count(*) FROM comment_admission_observations").await,
        9,
        "each comment observed exactly once"
    );
    // Logs carry codes and counts: no body and no comment identifier.
    for body in &bodies {
        assert!(!logs.contains(body.as_str()));
    }
    for id in &ids {
        assert!(!logs.contains(&id.to_string()));
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_worker_process_reports_unavailable_queues_and_refuses_privileged_roles(pool: PgPool) {
    // No database answers on this port: the worker starts, says so and stops cleanly.
    // SIGTERM only after the start line: a signal that lands before the process has installed its
    // handler takes the default action (nothing is claimed yet), which is not what this checks.
    let mut worker = spawn_worker("postgres://vink@127.0.0.1:9/vink", "local");
    let stderr = worker.stderr.take().unwrap();
    let (sender, lines) = std::sync::mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        use std::io::BufRead;
        let mut all = String::new();
        for line in std::io::BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
        {
            all.push_str(&line);
            all.push('\n');
            let _ = sender.send(line);
        }
        all
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match lines.try_recv() {
            Ok(line) if line.contains("vink-worker started") => break,
            Ok(_) => continue,
            Err(_) => {
                assert!(
                    Instant::now() < deadline,
                    "the worker never logged its start"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
    terminate(&worker);
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = worker.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "the worker did not exit in time");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let (code, logs) = (status.code(), reader.join().unwrap());
    assert_eq!(code, Some(0), "{logs}");
    assert!(logs.contains("unavailable"), "{logs}");
    assert!(!logs.contains("\"outbox\":\"ready\""), "{logs}");

    // Production with the harness superuser: the outbox refuses to run.
    let refused = spawn_worker(&database_url(&pool), "production");
    let (code, logs) = wait_exit(refused, Duration::from_secs(15)).await;
    assert_eq!(code, Some(2), "{logs}");
    assert!(logs.contains("refused_privileged_role"), "{logs}");
}
