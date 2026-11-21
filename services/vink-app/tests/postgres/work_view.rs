//! Work views (#341) on real PostgreSQL. Every event goes through `Vink::record_work_view` and
//! `PgStore`, the production transaction that locks the facts, calls `decide_view` and commits;
//! every assertion reads the durable rows back. Publication changes are the real author and
//! moderation use cases (snapshot, request-public, evaluator approval, set-private, restore).
//! Races hold a real lock on a second connection and release it, so both commit orders are
//! exercised on purpose. Failures are injected by a trigger. Time is the decision clock the test
//! passes, so windows, receipts and retention run on a virtual timeline.

use std::collections::HashMap;
use std::time::Instant;

use serde::Deserialize;
use sqlx::Connection;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::{NoContext, Timestamp};
use vink_app::application::work_view::{
    RecordWorkView, Viewer, WorkViewError, WorkViewRecord, WorkViewResult,
};
use vink_app::domain::ids::RevisionId;
use vink_app::domain::work_view::{
    EventDigest, GuestToken, ViewDigestKey, ViewEventId, ViewRefusal, ViewSkip, ViewVerdict,
};
use vink_app::evaluation_worker::EvaluationWorker;
use vink_app::persistence::work_view::{EVENT_LOCK_SEED, EVENT_LOCK_SQL, erase_work};

use super::*;

const KEY: ViewDigestKey = ViewDigestKey::from_bytes([0x5a; 32]);

fn at(raw: &str) -> OffsetDateTime {
    OffsetDateTime::parse(raw, &Rfc3339).unwrap()
}

fn t0() -> OffsetDateTime {
    at("2026-12-01T01:00:00Z")
}

fn event_at(time: OffsetDateTime) -> String {
    let nanos = time.unix_timestamp_nanos();
    let seconds = u64::try_from(nanos.div_euclid(1_000_000_000)).unwrap();
    let subsec = u32::try_from(nanos.rem_euclid(1_000_000_000)).unwrap();
    Uuid::new_v7(Timestamp::from_unix(NoContext, seconds, subsec)).to_string()
}

fn guest() -> Viewer {
    Viewer::Guest(GuestToken::generate().expose())
}

fn member(id: Uuid) -> Viewer {
    Viewer::Member(PrincipalId::from_uuid(id))
}

async fn view(
    vink: &Vink<PgStore>,
    viewer: Viewer,
    work: WorkId,
    event: &str,
    now: OffsetDateTime,
) -> Result<WorkViewRecord, WorkViewError> {
    vink.record_work_view(
        &KEY,
        viewer,
        RecordWorkView {
            work_id: work,
            event_id: event.into(),
        },
        now,
    )
    .await
}

async fn scalar(pool: &PgPool, sql: &'static str, work: WorkId) -> i64 {
    sqlx::query_scalar(sql)
        .bind(work.as_uuid())
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn total(pool: &PgPool, work: WorkId) -> i64 {
    scalar(
        pool,
        "SELECT coalesce((SELECT views FROM work_view_totals WHERE work_id = $1), 0)",
        work,
    )
    .await
}

async fn day_sum(pool: &PgPool, work: WorkId) -> i64 {
    scalar(
        pool,
        "SELECT coalesce(sum(views), 0)::bigint FROM work_view_days WHERE work_id = $1",
        work,
    )
    .await
}

async fn rows(pool: &PgPool, table: &'static str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn pointer(pool: &PgPool, work: WorkId) -> Option<Uuid> {
    sqlx::query_scalar("SELECT revision_id FROM publications WHERE work_id = $1")
        .bind(work.as_uuid())
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn new_member(pool: &PgPool, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    id
}

/// The real author/moderation path to make `revision` public in place of the current one.
async fn publish_new_revision(
    vink: &Vink<PgStore>,
    pool: &PgPool,
    generation: i64,
) -> (RevisionId, i64) {
    let work = sample_work();
    let snapshot = vink
        .capture_snapshot(author(), work, "views-snap-r2", 1, 1)
        .await
        .unwrap();
    let requested = vink
        .request_public(
            author(),
            work,
            "views-pub-r2",
            snapshot.revision_id,
            generation,
        )
        .await
        .unwrap();
    sqlx::query(
        "UPDATE work_evaluation_jobs SET scenario = 'approve' WHERE work_id = $1 AND state = 'open'",
    )
    .bind(work.as_uuid())
    .execute(pool)
    .await
    .unwrap();
    assert!(
        EvaluationWorker::run_once(pool, "views-worker")
            .await
            .unwrap()
    );
    assert_eq!(
        pointer(pool, work).await,
        Some(snapshot.revision_id.as_uuid())
    );
    (snapshot.revision_id, requested.generation.get())
}

#[derive(Deserialize)]
struct Scenario {
    readers: HashMap<String, String>,
    steps: Vec<Step>,
    #[serde(rename = "final")]
    end: Final,
}

#[derive(Deserialize)]
struct Step {
    at: String,
    #[serde(rename = "do")]
    action: String,
    #[serde(default)]
    reader: Option<String>,
    #[serde(default)]
    event: Option<String>,
    #[serde(default)]
    expect: Option<String>,
    #[serde(default)]
    basis: Option<String>,
    #[serde(default)]
    total: Option<i64>,
    #[serde(default)]
    pruned: Option<Pruned>,
}

#[derive(Deserialize)]
struct Pruned {
    windows: u64,
    receipts: u64,
}

#[derive(Deserialize)]
struct Final {
    total: i64,
    days: HashMap<String, i64>,
    windows: i64,
    receipts: i64,
    counting_started_at: String,
}

fn spoken(result: WorkViewResult) -> String {
    match result {
        WorkViewResult::Counted => "counted".into(),
        WorkViewResult::Skipped(skip) => ViewVerdict::Skipped(skip).as_str().into(),
        WorkViewResult::Replayed(verdict) => format!("replayed:{}", verdict.as_str()),
        WorkViewResult::Ignored(ViewRefusal::OutsideHorizon) => "refused:outside_horizon".into(),
        WorkViewResult::Ignored(ViewRefusal::EventReused) => "refused:event_reused".into(),
        WorkViewResult::Ignored(ViewRefusal::WorkNotFound) => "refused:work_not_found".into(),
    }
}

/// The shared timeline oracle (`tests/fixtures/work-views/scenario.json`), the same file the
/// in-memory unit test replays, through real publication changes and durable rows.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn shared_timeline_oracle_on_postgres(pool: PgPool) {
    let scenario: Scenario =
        serde_json::from_str(include_str!("../fixtures/work-views/scenario.json")).unwrap();
    let vink = seeded(&pool).await;
    let work = sample_work();
    let mut viewers = HashMap::new();
    for (name, kind) in &scenario.readers {
        let viewer = match (name.as_str(), kind.as_str()) {
            ("T", _) => member(dev_seed::SAMPLE_AUTHOR),
            ("A", _) => member(dev_seed::OTHER_PRINCIPAL),
            (_, "member") => member(new_member(&pool, &format!("Bạn đọc {name} (kiểm thử)")).await),
            (_, "guest") => guest(),
            other => panic!("{other:?}"),
        };
        viewers.insert(name.clone(), viewer);
    }
    let mut revisions = HashMap::from([(
        "R1".to_string(),
        RevisionId::from_uuid(dev_seed::SAMPLE_REVISION),
    )]);
    let mut events: HashMap<String, String> = HashMap::new();
    let mut generation = 2;
    for (n, step) in scenario.steps.iter().enumerate() {
        let now = at(&step.at);
        let label = format!("step {} ({} at {})", n + 1, step.action, step.at);
        match step.action.as_str() {
            "view" => {
                let event = events
                    .entry(step.event.clone().unwrap())
                    .or_insert_with(|| event_at(now))
                    .clone();
                let viewer = viewers[step.reader.as_deref().unwrap()].clone();
                let record = view(&vink, viewer, work, &event, now).await.unwrap();
                assert_eq!(
                    &spoken(record.result),
                    step.expect.as_ref().unwrap(),
                    "{label}"
                );
                assert_eq!(
                    record.basis,
                    step.basis.as_deref().map(|b| revisions[b]),
                    "{label}: basis"
                );
                assert_eq!(Some(total(&pool, work).await), step.total, "{label}: total");
                assert_eq!(
                    day_sum(&pool, work).await,
                    total(&pool, work).await,
                    "{label}"
                );
            }
            "replace_revision" => {
                let (r2, next) = publish_new_revision(&vink, &pool, generation).await;
                revisions.insert("R2".into(), r2);
                generation = next;
            }
            "set_private" => {
                let done = vink
                    .set_private(author(), work, "views-hide-1", generation)
                    .await
                    .unwrap();
                generation = done.generation.get();
                assert_eq!(pointer(&pool, work).await, None, "{label}");
            }
            "republish" => {
                let r2 = revisions["R2"];
                let done = vink
                    .request_public(author(), work, "views-restore", r2, generation)
                    .await
                    .unwrap();
                generation = done.generation.get();
                assert_eq!(pointer(&pool, work).await, Some(r2.as_uuid()), "{label}");
            }
            "prune" => {
                let pruned = vink.prune_work_views(now, 1_000).await.unwrap();
                let expected = step.pruned.as_ref().unwrap();
                assert_eq!(
                    (pruned.windows, pruned.receipts),
                    (expected.windows, expected.receipts),
                    "{label}"
                );
                assert!(pruned.drained);
            }
            other => panic!("unknown action {other}"),
        }
    }
    assert_eq!(total(&pool, work).await, scenario.end.total);
    let days: Vec<(time::Date, i64)> =
        sqlx::query_as("SELECT day, views FROM work_view_days WHERE work_id = $1")
            .bind(work.as_uuid())
            .fetch_all(&pool)
            .await
            .unwrap();
    let days: HashMap<String, i64> = days.into_iter().map(|(d, v)| (d.to_string(), v)).collect();
    assert_eq!(days, scenario.end.days);
    assert_eq!(rows(&pool, "work_view_windows").await, scenario.end.windows);
    assert_eq!(
        rows(&pool, "work_view_receipts").await,
        scenario.end.receipts
    );
    let started: OffsetDateTime = sqlx::query_scalar("SELECT started_at FROM work_view_counting")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(started, at(&scenario.end.counting_started_at));
    // Only the three kinds of data VIEW-11 allows: the reader appears only as a keyed digest.
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT table_name || '.' || column_name FROM information_schema.columns
          WHERE table_name LIKE 'work_view%' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        columns,
        [
            "work_view_counting.singleton",
            "work_view_counting.started_at",
            "work_view_days.day",
            "work_view_days.views",
            "work_view_days.work_id",
            "work_view_receipts.basis_revision_id",
            "work_view_receipts.decided_at",
            "work_view_receipts.event_digest",
            "work_view_receipts.verdict",
            "work_view_receipts.work_id",
            "work_view_totals.updated_at",
            "work_view_totals.views",
            "work_view_totals.work_id",
            "work_view_windows.counted_at",
            "work_view_windows.reader_digest",
            "work_view_windows.work_id",
        ],
        "no reader identity, address or history column"
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn one_event_sent_concurrently_counts_once(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    let event = event_at(t0());
    let viewer = guest();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let (vink, event, viewer) = (vink.clone(), event.clone(), viewer.clone());
        tasks.spawn(async move { view(&vink, viewer, sample_work(), &event, t0()).await });
    }
    let results: Vec<WorkViewResult> = tasks
        .join_all()
        .await
        .into_iter()
        .map(|r| r.unwrap().result)
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == WorkViewResult::Counted)
            .count(),
        1
    );
    assert!(results.iter().all(|r| matches!(
        r,
        WorkViewResult::Counted | WorkViewResult::Replayed(ViewVerdict::Counted)
    )));
    assert_eq!(total(&pool, sample_work()).await, 1);
    assert_eq!(rows(&pool, "work_view_receipts").await, 1);
    assert_eq!(rows(&pool, "work_view_windows").await, 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn two_tabs_of_one_reader_count_once(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    let viewer = guest();
    let mut tasks = tokio::task::JoinSet::new();
    for n in 0..8 {
        let (vink, viewer) = (vink.clone(), viewer.clone());
        let now = t0() + time::Duration::milliseconds(n);
        tasks.spawn(async move { view(&vink, viewer, sample_work(), &event_at(now), now).await });
    }
    let results: Vec<WorkViewResult> = tasks
        .join_all()
        .await
        .into_iter()
        .map(|r| r.unwrap().result)
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == WorkViewResult::Counted)
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == WorkViewResult::Skipped(ViewSkip::WithinWindow))
            .count(),
        7
    );
    assert_eq!(total(&pool, sample_work()).await, 1);
    assert_eq!(rows(&pool, "work_view_windows").await, 1);
    assert_eq!(rows(&pool, "work_view_receipts").await, 8);
}

/// CHOICE-02 A on a hot work: every distinct reader counts and the total row serializes them.
/// The elapsed time is printed as local evidence of contention, not asserted.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn distinct_readers_on_a_hot_work_all_count(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    const READERS: i64 = 32;
    let started = Instant::now();
    let mut tasks = tokio::task::JoinSet::new();
    for n in 0..READERS {
        let vink = vink.clone();
        let now = t0() + time::Duration::milliseconds(n);
        tasks.spawn(async move { view(&vink, guest(), sample_work(), &event_at(now), now).await });
    }
    let results = tasks.join_all().await;
    let elapsed = started.elapsed();
    assert!(
        results
            .iter()
            .all(|r| r.as_ref().unwrap().result == WorkViewResult::Counted)
    );
    assert_eq!(total(&pool, sample_work()).await, READERS);
    assert_eq!(day_sum(&pool, sample_work()).await, READERS);
    eprintln!("work views: {READERS} concurrent counted views of one work in {elapsed:?}");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn lost_answer_and_restart_replay_without_effect(pool: PgPool) {
    let vink = seeded(&pool).await;
    let event = event_at(t0());
    let token = GuestToken::generate().expose();
    let first = view(
        &vink,
        Viewer::Guest(token.clone()),
        sample_work(),
        &event,
        t0(),
    )
    .await
    .unwrap();
    assert_eq!(first.result, WorkViewResult::Counted);

    // The answer (and the guest cookie with it) was lost, and the process restarted: the retry
    // reaches a new pool, possibly with a freshly issued guest token.
    let options = pool.connect_options();
    pool.close().await;
    let reopened = PgPoolOptions::new()
        .connect_with((*options).clone())
        .await
        .unwrap();
    let vink = Vink::new(PgStore::new(reopened.clone()));
    for viewer in [Viewer::Guest(token), guest()] {
        let retry = view(
            &vink,
            viewer,
            sample_work(),
            &event,
            t0() + time::Duration::seconds(3),
        )
        .await
        .unwrap();
        assert_eq!(retry.result, WorkViewResult::Replayed(ViewVerdict::Counted));
        assert_eq!(retry.basis, first.basis);
    }
    assert_eq!(total(&reopened, sample_work()).await, 1);
    assert_eq!(rows(&reopened, "work_view_receipts").await, 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_failure_before_commit_leaves_no_trace_and_the_retry_counts_once(pool: PgPool) {
    let vink = seeded(&pool).await;
    // The receipt is the last write of a counted view: failing it proves the window, the total,
    // the day and the counting start roll back with it.
    sqlx::raw_sql(
        "CREATE FUNCTION test_fail_view_receipt() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'injected work view failure'; END $$;
         CREATE TRIGGER test_fail_view_receipt BEFORE INSERT ON work_view_receipts
             FOR EACH ROW EXECUTE FUNCTION test_fail_view_receipt();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let event = event_at(t0());
    let viewer = guest();
    let failed = view(&vink, viewer.clone(), sample_work(), &event, t0()).await;
    assert_eq!(failed.unwrap_err(), WorkViewError::Internal);
    for table in [
        "work_view_totals",
        "work_view_days",
        "work_view_windows",
        "work_view_receipts",
        "work_view_counting",
    ] {
        assert_eq!(rows(&pool, table).await, 0, "{table}");
    }
    sqlx::raw_sql("DROP TRIGGER test_fail_view_receipt ON work_view_receipts")
        .execute(&pool)
        .await
        .unwrap();
    let later = t0() + time::Duration::seconds(2);
    let retry = view(&vink, viewer.clone(), sample_work(), &event, later)
        .await
        .unwrap();
    assert_eq!(retry.result, WorkViewResult::Counted);
    let again = view(&vink, viewer, sample_work(), &event, later)
        .await
        .unwrap();
    assert_eq!(again.result, WorkViewResult::Replayed(ViewVerdict::Counted));
    assert_eq!(total(&pool, sample_work()).await, 1);
}

/// A withdrawal that commits first wins: the waiting event sees no pointer (E-34).
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_withdrawal_committed_before_the_decision_skips_the_view(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    // Hold the author's principal row: set_private locks the pointer FOR UPDATE, then waits here.
    let mut conn = pool.acquire().await.unwrap();
    let mut gate = conn.begin().await.unwrap();
    sqlx::query("SELECT id FROM principals WHERE id = $1 FOR UPDATE")
        .bind(dev_seed::SAMPLE_AUTHOR)
        .execute(&mut *gate)
        .await
        .unwrap();
    let blocker = backend_pid(&mut gate).await;
    let hide = {
        let vink = vink.clone();
        tokio::spawn(async move {
            vink.set_private(author(), sample_work(), "views-race-hide", 2)
                .await
        })
    };
    wait_blocked_by(&pool, blocker).await;
    let event = event_at(t0());
    let reading = {
        let (vink, event) = (vink.clone(), event.clone());
        tokio::spawn(async move { view(&vink, guest(), sample_work(), &event, t0()).await })
    };
    // The view waits for the pointer lock set_private holds.
    wait_blocked(&pool, 2).await;
    gate.commit().await.unwrap();
    assert_eq!(hide.await.unwrap().unwrap().generation.get(), 3);
    let record = reading.await.unwrap().unwrap();
    assert_eq!(record.result, WorkViewResult::Skipped(ViewSkip::NotPublic));
    assert_eq!(record.basis, None);
    assert_eq!(total(&pool, sample_work()).await, 0);
}

/// A view that holds the pointer share lock first wins: it counts against the public work and
/// the withdrawal waits for its commit.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_view_decided_before_the_withdrawal_commits_counts(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    let event = event_at(t0());
    let digest = EventDigest::of(ViewEventId::parse(&event).unwrap());
    // Hold the event's advisory lock: the view takes the pointer share lock, then waits here.
    let mut conn = pool.acquire().await.unwrap();
    let mut gate = conn.begin().await.unwrap();
    sqlx::query(EVENT_LOCK_SQL)
        .bind(digest.hex())
        .bind(EVENT_LOCK_SEED)
        .execute(&mut *gate)
        .await
        .unwrap();
    let blocker = backend_pid(&mut gate).await;
    let reading = {
        let (vink, event) = (vink.clone(), event.clone());
        tokio::spawn(async move { view(&vink, guest(), sample_work(), &event, t0()).await })
    };
    wait_blocked_by(&pool, blocker).await;
    let hide = {
        let vink = vink.clone();
        tokio::spawn(async move {
            vink.set_private(author(), sample_work(), "views-race-late", 2)
                .await
        })
    };
    // set_private waits for the view's share lock on the pointer.
    wait_blocked(&pool, 2).await;
    gate.commit().await.unwrap();
    let record = reading.await.unwrap().unwrap();
    assert_eq!(record.result, WorkViewResult::Counted);
    assert_eq!(
        record.basis,
        Some(RevisionId::from_uuid(dev_seed::SAMPLE_REVISION))
    );
    assert_eq!(hide.await.unwrap().unwrap().generation.get(), 3);
    assert_eq!(total(&pool, sample_work()).await, 1);
    let after = view(&vink, guest(), sample_work(), &event_at(t0()), t0())
        .await
        .unwrap();
    assert_eq!(after.result, WorkViewResult::Skipped(ViewSkip::NotPublic));
    assert_eq!(
        total(&pool, sample_work()).await,
        1,
        "the old total stays (VIEW-08)"
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn retention_skips_a_window_a_decision_holds(pool: PgPool) {
    let vink = seeded(&pool).await;
    let viewer = guest();
    view(&vink, viewer.clone(), sample_work(), &event_at(t0()), t0())
        .await
        .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let mut held = conn.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM work_view_windows FOR UPDATE")
        .execute(&mut *held)
        .await
        .unwrap();
    let much_later = t0() + time::Duration::days(3);
    let pruned = vink.prune_work_views(much_later, 100).await.unwrap();
    assert_eq!(
        (pruned.windows, pruned.receipts),
        (0, 1),
        "the held window is skipped, not awaited"
    );
    held.rollback().await.unwrap();
    let pruned = vink.prune_work_views(much_later, 100).await.unwrap();
    assert_eq!((pruned.windows, pruned.receipts), (1, 0));
    assert_eq!(
        total(&pool, sample_work()).await,
        1,
        "totals are never pruned"
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn erasing_a_work_removes_only_its_view_rows(pool: PgPool) {
    let vink = seeded(&pool).await;
    let draft = vink.create_draft(author()).await.unwrap().work_id;
    view(&vink, guest(), sample_work(), &event_at(t0()), t0())
        .await
        .unwrap();
    view(&vink, guest(), sample_work(), &event_at(t0()), t0())
        .await
        .unwrap();
    let private = view(&vink, guest(), draft, &event_at(t0()), t0())
        .await
        .unwrap();
    assert_eq!(private.result, WorkViewResult::Skipped(ViewSkip::NotPublic));

    let mut tx = pool.begin().await.unwrap();
    let erased = erase_work(&mut tx, sample_work()).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        (erased.totals, erased.days, erased.windows, erased.receipts),
        (1, 1, 2, 2)
    );
    assert_eq!(rows(&pool, "work_view_totals").await, 0);
    assert_eq!(rows(&pool, "work_view_windows").await, 0);
    assert_eq!(
        rows(&pool, "work_view_receipts").await,
        1,
        "the other work keeps its receipt"
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn nothing_is_inferred_from_earlier_activity(pool: PgPool) {
    use vink_app::application::endorsement::SetWorkEndorsement;
    let vink = seeded(&pool).await;
    vink.set_work_endorsement(
        other(),
        SetWorkEndorsement {
            work_id: sample_work(),
            revision: dev_seed::SAMPLE_REVISION.to_string(),
            command_key: Uuid::now_v7().to_string(),
            expected_order: 1,
            endorsed: true,
        },
    )
    .await
    .unwrap();
    // VIEW-09: no backfill. A work published (and endorsed) before counting has no total and
    // counting has not started until the first event is decided.
    for table in [
        "work_view_totals",
        "work_view_days",
        "work_view_windows",
        "work_view_receipts",
        "work_view_counting",
    ] {
        assert_eq!(rows(&pool, table).await, 0, "{table}");
    }
    let first = view(&vink, guest(), sample_work(), &event_at(t0()), t0())
        .await
        .unwrap();
    assert_eq!(first.result, WorkViewResult::Counted);
    assert_eq!(
        total(&pool, sample_work()).await,
        1,
        "counted from the first event only"
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn unknown_works_and_invalid_events_write_nothing(pool: PgPool) {
    let vink = seeded(&pool).await;
    let missing = WorkId::from_uuid(Uuid::now_v7());
    let ignored = view(&vink, guest(), missing, &event_at(t0()), t0())
        .await
        .unwrap();
    assert_eq!(
        ignored.result,
        WorkViewResult::Ignored(ViewRefusal::WorkNotFound)
    );
    let stale = view(
        &vink,
        guest(),
        sample_work(),
        &event_at(t0() - time::Duration::hours(12)),
        t0(),
    )
    .await
    .unwrap();
    assert_eq!(
        stale.result,
        WorkViewResult::Ignored(ViewRefusal::OutsideHorizon)
    );
    let reused = event_at(t0());
    view(&vink, guest(), sample_work(), &reused, t0())
        .await
        .unwrap();
    let other_work = vink.create_draft(author()).await.unwrap().work_id;
    let clash = view(&vink, guest(), other_work, &reused, t0())
        .await
        .unwrap();
    assert_eq!(
        clash.result,
        WorkViewResult::Ignored(ViewRefusal::EventReused)
    );
    assert_eq!(rows(&pool, "work_view_receipts").await, 1);
    assert_eq!(total(&pool, sample_work()).await, 1);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_schema_refuses_rows_no_decision_produces(pool: PgPool) {
    let vink = seeded(&pool).await;
    let draft = vink.create_draft(author()).await.unwrap().work_id;
    let author_view = view(
        &vink,
        member(dev_seed::SAMPLE_AUTHOR),
        sample_work(),
        &event_at(t0()),
        t0(),
    )
    .await
    .unwrap();
    assert_eq!(
        author_view.result,
        WorkViewResult::Skipped(ViewSkip::OwnWork)
    );
    view(&vink, guest(), sample_work(), &event_at(t0()), t0())
        .await
        .unwrap();
    for (name, sql) in [
        ("total shrinks", "UPDATE work_view_totals SET views = views - 1"),
        ("total jumps", "UPDATE work_view_totals SET views = views + 2"),
        ("day jumps", "UPDATE work_view_days SET views = views + 5"),
        ("window moves back", "UPDATE work_view_windows SET counted_at = counted_at - interval '1 hour'"),
        ("receipt rewritten", "UPDATE work_view_receipts SET verdict = 'counted'"),
        ("counting start rewritten", "UPDATE work_view_counting SET started_at = now()"),
        (
            "not public with a basis",
            "INSERT INTO work_view_receipts (event_digest, work_id, verdict, basis_revision_id, decided_at)
             VALUES (decode(repeat('ab', 32), 'hex'), '33333333-3333-4333-8333-333333333333',
                     'not_public', '44444444-4444-4444-8444-444444444444', now())",
        ),
        (
            "counted without a basis",
            "INSERT INTO work_view_receipts (event_digest, work_id, verdict, basis_revision_id, decided_at)
             VALUES (decode(repeat('cd', 32), 'hex'), '33333333-3333-4333-8333-333333333333',
                     'counted', NULL, now())",
        ),
        (
            "short reader digest",
            "INSERT INTO work_view_windows (work_id, reader_digest, counted_at)
             VALUES ('33333333-3333-4333-8333-333333333333', decode(repeat('ab', 31), 'hex'), now())",
        ),
    ] {
        assert!(sqlx::raw_sql(sql).execute(&pool).await.is_err(), "{name}");
    }
    let zero = sqlx::query(
        "INSERT INTO work_view_totals (work_id, views, updated_at) VALUES ($1, 0, now())",
    )
    .bind(draft.as_uuid())
    .execute(&pool)
    .await;
    assert!(
        zero.is_err(),
        "a total row exists only after a counted view"
    );
    assert_eq!(total(&pool, sample_work()).await, 1);
}

fn rollback_script() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations-rollback/20261130000000_work_views.down.sql"),
    )
    .unwrap()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_rollback_script_refuses_once_a_view_counted(pool: PgPool) {
    let vink = seeded(&pool).await;
    view(&vink, guest(), sample_work(), &event_at(t0()), t0())
        .await
        .unwrap();
    let refused = sqlx::raw_sql(sqlx::AssertSqlSafe(rollback_script()))
        .execute(&pool)
        .await;
    assert!(refused.is_err(), "a counted view blocks the rollback");
    assert_eq!(total(&pool, sample_work()).await, 1, "nothing was dropped");
    assert_eq!(schema_state(&pool).await.unwrap(), SchemaState::Current);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_rollback_script_drops_an_uncounted_schema_and_migrating_restores_it(pool: PgPool) {
    let vink = seeded(&pool).await;
    // Only skipped events (no total): the author's own view.
    let own = view(
        &vink,
        member(dev_seed::SAMPLE_AUTHOR),
        sample_work(),
        &event_at(t0()),
        t0(),
    )
    .await
    .unwrap();
    assert_eq!(own.result, WorkViewResult::Skipped(ViewSkip::OwnWork));
    sqlx::raw_sql(sqlx::AssertSqlSafe(rollback_script()))
        .execute(&pool)
        .await
        .unwrap();
    let left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables WHERE table_name LIKE 'work_view%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(left, 0);
    vink_app::persistence::migrate(&pool).await.unwrap();
    assert_eq!(schema_state(&pool).await.unwrap(), SchemaState::Current);
    let counted = view(&vink, guest(), sample_work(), &event_at(t0()), t0())
        .await
        .unwrap();
    assert_eq!(counted.result, WorkViewResult::Counted);
}
