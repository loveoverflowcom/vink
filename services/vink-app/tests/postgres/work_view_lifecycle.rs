//! Work views (#323 OQ-2, VIEW-08, VIEW-11) across account deletion (#297/#550) on real
//! PostgreSQL. The write path is `Vink::record_work_view` over `PgStore`; deletion is the
//! production `PgStore::apply_account_deletion`, which calls `erase_work` inside its fence
//! transaction. Races hold a real lock on a second connection, so both commit orders run on
//! purpose: a decision that wins its locks commits first and is then erased, and a decision that
//! waits behind the deletion finds the author fenced and writes nothing. A trigger injects a late
//! failure to prove the erase rolls back with the fence and a retry erases once.
//!
//! The author's account and a reader's account are different cases: only the author's deletion
//! erases a work's views; a reader's deletion leaves every total it contributed to (no rows are
//! keyed by person, its window expires with retention).

use sqlx::Connection;
use time::OffsetDateTime;
use vink_app::application::work_view::WorkViewResult;
use vink_app::domain::work_view::count::ViewCount;
use vink_app::domain::work_view::{EventDigest, ViewEventId, ViewRefusal, ViewSkip, ViewVerdict};
use vink_app::identity::client::AccountDeletion;
use vink_app::persistence::work_view::{EVENT_LOCK_SEED, EVENT_LOCK_SQL};

use super::work_view::{event_at, guest, new_member, view};
use super::work_view_intake::{guest_app, post_view_req};
use super::work_view_reads::author_views;
use super::*;

/// Advisory key a test trigger waits on, to pause a deletion after its erase.
const PAUSE_KEY: i64 = 323_002;

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc().replace_nanosecond(0).unwrap()
}

fn deletion() -> AccountDeletion {
    AccountDeletion {
        account_id: Uuid::new_v4(),
        operation_id: Uuid::new_v4(),
        accepted_at: now(),
    }
}

/// Binds `principal` to the deletion's verified Identity subject, as native sign-in does.
async fn bind(pool: &PgPool, principal: Uuid, operation: &AccountDeletion) {
    sqlx::query(
        "INSERT INTO principal_bindings (principal_id, authority, subject_id)
         VALUES ($1, 'vink-identity', $2)",
    )
    .bind(principal)
    .bind(operation.account_id.to_string())
    .execute(pool)
    .await
    .unwrap();
}

/// Rows of each view table for `work`: totals, days, windows, receipts.
async fn view_rows(pool: &PgPool, work: WorkId) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM work_view_totals WHERE work_id = $1),
                (SELECT count(*) FROM work_view_days WHERE work_id = $1),
                (SELECT count(*) FROM work_view_windows WHERE work_id = $1),
                (SELECT count(*) FROM work_view_receipts WHERE work_id = $1)",
    )
    .bind(work.as_uuid())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn total(pool: &PgPool, work: WorkId) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE(sum(views), 0)::bigint FROM work_view_totals WHERE work_id = $1",
    )
    .bind(work.as_uuid())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn day_sum(pool: &PgPool, work: WorkId) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE(sum(views), 0)::bigint FROM work_view_days WHERE work_id = $1",
    )
    .bind(work.as_uuid())
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn fenced(pool: &PgPool, operation: &AccountDeletion) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM account_deletions WHERE operation_id = $1)")
        .bind(operation.operation_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Holds one event's advisory lock on a second connection: the decision of that event takes the
/// publication share lock and the author fence, then waits here.
async fn hold_event(conn: &mut PgConnection, event: &str) -> i32 {
    let digest = EventDigest::of(ViewEventId::parse(event).unwrap());
    sqlx::query(EVENT_LOCK_SQL)
        .bind(digest.hex())
        .bind(EVENT_LOCK_SEED)
        .execute(&mut *conn)
        .await
        .unwrap();
    backend_pid(conn).await
}

/// A decision that holds the public pointer when the deletion starts commits its count first;
/// the deletion waits for it, then erases every row, and a replay of that event writes nothing.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_public_view_decided_first_is_erased_by_the_author_deletion(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    let work = sample_work();
    let operation = deletion();
    bind(&pool, dev_seed::SAMPLE_AUTHOR, &operation).await;
    let earlier = view(&vink, guest(), work, &event_at(now()), now())
        .await
        .unwrap();
    assert_eq!(earlier.result, WorkViewResult::Counted);

    let event = event_at(now());
    let mut conn = pool.acquire().await.unwrap();
    let mut gate = conn.begin().await.unwrap();
    let blocker = hold_event(&mut gate, &event).await;
    let reading = {
        let (vink, event) = (vink.clone(), event.clone());
        tokio::spawn(async move { view(&vink, guest(), work, &event, now()).await })
    };
    wait_blocked_by(&pool, blocker).await;
    let store = PgStore::new(pool.clone());
    let deleting = tokio::spawn(async move { store.apply_account_deletion(operation).await });
    // The deletion waits for the view's share lock on the pointer (lock_work FOR UPDATE).
    wait_blocked(&pool, 2).await;
    gate.commit().await.unwrap();

    assert_eq!(
        reading.await.unwrap().unwrap().result,
        WorkViewResult::Counted
    );
    deleting.await.unwrap().unwrap();
    assert!(fenced(&pool, &operation).await);
    assert_eq!(
        view_rows(&pool, work).await,
        (0, 0, 0, 0),
        "OQ-2: all four tables"
    );

    // The receipt went with the work: the old request is decided again and finds no work.
    let replay = view(&vink, guest(), work, &event, now()).await.unwrap();
    assert_eq!(
        replay.result,
        WorkViewResult::Ignored(ViewRefusal::WorkNotFound)
    );
    assert_eq!(view_rows(&pool, work).await, (0, 0, 0, 0));
}

/// A private work has no pointer to lock. The decision still holds the author fence (FOR KEY
/// SHARE of the principal), so the deletion's standing lock waits for its receipt to commit and
/// the erase removes it together with the history kept while private (OQ-1 A).
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_private_view_decided_first_leaves_no_receipt_after_the_author_deletion(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    let work = sample_work();
    let operation = deletion();
    bind(&pool, dev_seed::SAMPLE_AUTHOR, &operation).await;
    view(&vink, guest(), work, &event_at(now()), now())
        .await
        .unwrap();
    vink.set_private(author(), work, "views-oq2-private", 2)
        .await
        .unwrap();
    assert!(
        matches!(
            author_views(&vink, work).await,
            Some(ViewCount::Counted { views: 1, .. })
        ),
        "OQ-1 A: the author still reads the history while private"
    );

    let event = event_at(now());
    let mut conn = pool.acquire().await.unwrap();
    let mut gate = conn.begin().await.unwrap();
    let blocker = hold_event(&mut gate, &event).await;
    let reading = {
        let (vink, event) = (vink.clone(), event.clone());
        tokio::spawn(async move { view(&vink, guest(), work, &event, now()).await })
    };
    wait_blocked_by(&pool, blocker).await;
    let store = PgStore::new(pool.clone());
    let deleting = tokio::spawn(async move { store.apply_account_deletion(operation).await });
    // The deletion's FOR UPDATE of the principal waits for the view's FOR KEY SHARE.
    wait_blocked(&pool, 2).await;
    gate.commit().await.unwrap();

    assert_eq!(
        reading.await.unwrap().unwrap().result,
        WorkViewResult::Skipped(ViewSkip::NotPublic)
    );
    deleting.await.unwrap().unwrap();
    assert_eq!(view_rows(&pool, work).await, (0, 0, 0, 0));
}

/// A decision that arrives while the deletion holds its locks waits behind it, then finds the
/// author fenced: neither a new event nor a replay of an event counted before the deletion can
/// recreate a row after the erase.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_view_waiting_behind_the_author_deletion_writes_nothing(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    let work = sample_work();
    let operation = deletion();
    bind(&pool, dev_seed::SAMPLE_AUTHOR, &operation).await;
    let counted = event_at(now());
    assert_eq!(
        view(&vink, guest(), work, &counted, now())
            .await
            .unwrap()
            .result,
        WorkViewResult::Counted
    );
    assert_eq!(
        view(&vink, guest(), work, &counted, now())
            .await
            .unwrap()
            .result,
        WorkViewResult::Replayed(ViewVerdict::Counted)
    );

    // Pause the deletion at its last step (the principal tombstone), after erase_work and the
    // pointer clear, so a waiting decision is ordered after the erase.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE FUNCTION pause_test_deletion() RETURNS trigger LANGUAGE plpgsql AS
         $$BEGIN PERFORM pg_advisory_xact_lock({PAUSE_KEY}); RETURN NEW; END;$$;
         CREATE TRIGGER pause_test_deletion BEFORE UPDATE ON principals FOR EACH ROW
             WHEN (NEW.display_name = 'Tài khoản đã xóa')
             EXECUTE FUNCTION pause_test_deletion()"
    )))
    .execute(&pool)
    .await
    .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let mut gate = conn.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(PAUSE_KEY)
        .execute(&mut *gate)
        .await
        .unwrap();
    let blocker = backend_pid(&mut gate).await;
    let store = PgStore::new(pool.clone());
    let deleting = tokio::spawn(async move { store.apply_account_deletion(operation).await });
    wait_blocked_by(&pool, blocker).await;
    let fresh = {
        let vink = vink.clone();
        tokio::spawn(async move { view(&vink, guest(), work, &event_at(now()), now()).await })
    };
    let replay = {
        let (vink, counted) = (vink.clone(), counted.clone());
        tokio::spawn(async move { view(&vink, guest(), work, &counted, now()).await })
    };
    // Deletion behind the gate; both decisions behind the deletion's pointer lock.
    wait_blocked(&pool, 3).await;
    // Release within the deletion's 2 s lock_timeout.
    gate.commit().await.unwrap();

    deleting.await.unwrap().unwrap();
    let not_found = WorkViewResult::Ignored(ViewRefusal::WorkNotFound);
    assert_eq!(fresh.await.unwrap().unwrap().result, not_found);
    assert_eq!(replay.await.unwrap().unwrap().result, not_found);
    assert_eq!(view_rows(&pool, work).await, (0, 0, 0, 0));

    // Over HTTP the fenced work answers exactly like any other Ok (no oracle) and writes nothing.
    let http = guest_app(&pool);
    for event in [counted, event_at(now())] {
        let (status, _, cache, body) = post_view_req(&http, work.as_uuid(), &event, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
        assert_eq!(cache.as_deref(), Some("no-store"));
    }
    assert_eq!(view_rows(&pool, work).await, (0, 0, 0, 0));
}

/// A failure after the erase rolls the whole fence back: views, totals and the right to count
/// stay. The retry erases exactly once, and a replayed deletion leaves the empty state alone.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_failed_author_deletion_keeps_views_and_its_retry_erases_them(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = sample_work();
    let operation = deletion();
    bind(&pool, dev_seed::SAMPLE_AUTHOR, &operation).await;
    view(&vink, guest(), work, &event_at(now()), now())
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE FUNCTION reject_test_deletion() RETURNS trigger LANGUAGE plpgsql AS
         $$BEGIN RAISE EXCEPTION 'injected deletion failure'; END;$$;
         CREATE TRIGGER reject_test_deletion BEFORE UPDATE ON principals FOR EACH ROW
             WHEN (NEW.display_name = 'Tài khoản đã xóa')
             EXECUTE FUNCTION reject_test_deletion()",
    )
    .execute(&pool)
    .await
    .unwrap();
    let store = PgStore::new(pool.clone());
    assert!(store.apply_account_deletion(operation).await.is_err());
    assert!(!fenced(&pool, &operation).await);
    assert_eq!(
        view_rows(&pool, work).await,
        (1, 1, 1, 1),
        "rolled back with the fence"
    );
    // Still an active author with a public work: a new reader counts.
    assert_eq!(
        view(&vink, guest(), work, &event_at(now()), now())
            .await
            .unwrap()
            .result,
        WorkViewResult::Counted
    );
    assert_eq!(
        (total(&pool, work).await, day_sum(&pool, work).await),
        (2, 2)
    );

    sqlx::raw_sql(
        "DROP TRIGGER reject_test_deletion ON principals; DROP FUNCTION reject_test_deletion()",
    )
    .execute(&pool)
    .await
    .unwrap();
    store.apply_account_deletion(operation).await.unwrap();
    assert_eq!(view_rows(&pool, work).await, (0, 0, 0, 0));
    store.apply_account_deletion(operation).await.unwrap();
    assert_eq!(view_rows(&pool, work).await, (0, 0, 0, 0));
    assert_eq!(
        view(&vink, guest(), work, &event_at(now()), now())
            .await
            .unwrap()
            .result,
        WorkViewResult::Ignored(ViewRefusal::WorkNotFound)
    );
}

/// Deleting a reader's account is not deleting a work: the totals and days it contributed to
/// stay, the author keeps reading the same number, and its window leaves only by retention.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_reader_account_deletion_keeps_the_counts_it_contributed(pool: PgPool) {
    let vink = seeded(&pool).await;
    let work = sample_work();
    let reader = new_member(&pool, "Người đọc sẽ xóa").await;
    let operation = deletion();
    bind(&pool, reader, &operation).await;
    let at = now();
    assert_eq!(
        view(
            &vink,
            vink_app::application::work_view::Viewer::Member(PrincipalId::from_uuid(reader)),
            work,
            &event_at(at),
            at,
        )
        .await
        .unwrap()
        .result,
        WorkViewResult::Counted
    );
    let before = author_views(&vink, work).await;

    PgStore::new(pool.clone())
        .apply_account_deletion(operation)
        .await
        .unwrap();
    assert!(fenced(&pool, &operation).await);
    assert_eq!(view_rows(&pool, work).await, (1, 1, 1, 1));
    assert_eq!(
        (total(&pool, work).await, day_sum(&pool, work).await),
        (1, 1)
    );
    assert_eq!(author_views(&vink, work).await, before);

    let pruned = vink
        .prune_work_views(at + time::Duration::days(2), 100)
        .await
        .unwrap();
    assert_eq!((pruned.windows, pruned.receipts), (1, 1));
    assert_eq!(view_rows(&pool, work).await, (1, 1, 0, 0));
    assert_eq!(
        author_views(&vink, work).await,
        before,
        "totals are never pruned"
    );
}
