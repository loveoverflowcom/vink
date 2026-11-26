//! PostgreSQL adapter of work views (#341, CHOICE-02 A). Each event is one transaction: the
//! work's facts under the publication share lock → the event's receipt under its advisory lock →
//! the reader's window row under FOR UPDATE → the exact pure decision (`decide_view`) → that
//! decision's writes → commit → the durable answer. The schema
//! (`migrations/20261130000000_work_views.sql`) holds the structural rules on its own: totals that
//! only grow by one, windows that only move forward, receipts that are never rewritten, a basis
//! revision of the same work present exactly when the work was public.
//!
//! Lock order, the prefix of endorsement's: publication (FOR SHARE) → the event's advisory lock
//! → the window row (FOR UPDATE) → the work's total row → its day row → the receipt. A
//! withdrawal, hide or republish (pointer DELETE/UPDATE) either commits before the decision reads
//! the pointer or waits until it commits, so "public at the decision" is "public at commit"
//! (E-34). The advisory lock gives one event exactly one winner, whatever reader or work its
//! retries carry. No principal row is locked: a view is not an authority-bearing write.
//!
//! An absent window is not a lock on a future row. A first view inserts with `ON CONFLICT DO
//! NOTHING`; if a concurrent first view of the same reader (another event, E-29) committed
//! meanwhile, the insert writes nothing and the transaction reads the winner's row FOR UPDATE and
//! decides again. Replacing an expired window is an UPDATE conditional on the instant decided
//! from.
//!
//! The decision clock is the caller's `now` (the HTTP adapter's clock), never the database's, so
//! windows, receipts, retention and day buckets all use one clock and tests can drive it.

use sqlx::{PgConnection, Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::PgStore;
use super::postgres::{classify, integrity};
use crate::application::ports::{StoreError, ViewPrune, ViewWrite, WorkViewStore};
use crate::domain::ids::{PrincipalId, RevisionId, WorkId};
use crate::domain::work_view::{
    CountWrite, EventDigest, RECEIPT_RETENTION, ReaderDigest, VIEW_WINDOW, ViewDecision, ViewFacts,
    ViewReceipt, ViewVerdict, WorkFacts, decide_view,
};

/// A first view can lose its window insert to a concurrent first view at most once: afterwards
/// the winner's row exists and every read locks it.
const CLAIM_ATTEMPTS: usize = 2;

/// Seed of the advisory-lock hash for the work-view event namespace.
pub const EVENT_LOCK_SEED: i64 = 8_341;

/// The advisory lock of one event, as the write takes it (tests hold it to order a race).
pub const EVENT_LOCK_SQL: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1, $2))";

async fn lock_event(
    tx: &mut Transaction<'_, Postgres>,
    event: EventDigest,
) -> Result<(), StoreError> {
    sqlx::query(EVENT_LOCK_SQL)
        .bind(event.hex())
        .bind(EVENT_LOCK_SEED)
        .execute(&mut **tx)
        .await
        .map_err(classify)?;
    Ok(())
}

async fn receipt(
    tx: &mut Transaction<'_, Postgres>,
    event: EventDigest,
) -> Result<Option<ViewReceipt>, StoreError> {
    let row: Option<(Uuid, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT work_id, verdict, basis_revision_id FROM work_view_receipts
          WHERE event_digest = $1",
    )
    .bind(event.as_bytes().as_slice())
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)?;
    row.map(|(work, verdict, basis)| {
        let verdict = ViewVerdict::parse(&verdict)
            .ok_or_else(|| integrity("stored work view verdict", &verdict))?;
        ViewReceipt::restore(
            WorkId::from_uuid(work),
            verdict,
            basis.map(RevisionId::from_uuid),
        )
        .ok_or_else(|| integrity("stored work view receipt", work))
    })
    .transpose()
}

async fn locked_window(
    tx: &mut Transaction<'_, Postgres>,
    work: WorkId,
    reader: ReaderDigest,
) -> Result<Option<OffsetDateTime>, StoreError> {
    sqlx::query_scalar(
        "SELECT counted_at FROM work_view_windows
          WHERE work_id = $1 AND reader_digest = $2 FOR UPDATE",
    )
    .bind(work.as_uuid())
    .bind(reader.as_bytes().as_slice())
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)
}

/// Claims the reader's window for a counted view. `false` when a concurrent first view committed
/// the row first (nothing written).
async fn claim_window(
    tx: &mut Transaction<'_, Postgres>,
    work: WorkId,
    reader: ReaderDigest,
    write: &CountWrite,
) -> Result<bool, StoreError> {
    let affected = match write.replaces {
        None => sqlx::query(
            "INSERT INTO work_view_windows (work_id, reader_digest, counted_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (work_id, reader_digest) DO NOTHING",
        )
        .bind(work.as_uuid())
        .bind(reader.as_bytes().as_slice())
        .bind(write.counted_at),
        Some(previous) => sqlx::query(
            "UPDATE work_view_windows SET counted_at = $3
              WHERE work_id = $1 AND reader_digest = $2 AND counted_at = $4",
        )
        .bind(work.as_uuid())
        .bind(reader.as_bytes().as_slice())
        .bind(write.counted_at)
        .bind(previous),
    }
    .execute(&mut **tx)
    .await
    .map_err(classify)?
    .rows_affected();
    if write.replaces.is_some() && affected != 1 {
        // The row is locked FOR UPDATE: a mismatch is a broken invariant, not a race.
        return Err(integrity("locked work view window changed", work));
    }
    Ok(affected == 1)
}

async fn count(
    tx: &mut Transaction<'_, Postgres>,
    work: WorkId,
    write: &CountWrite,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO work_view_totals (work_id, views, updated_at) VALUES ($1, 1, $2)
         ON CONFLICT (work_id)
         DO UPDATE SET views = work_view_totals.views + 1, updated_at = EXCLUDED.updated_at",
    )
    .bind(work.as_uuid())
    .bind(write.counted_at)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    sqlx::query(
        "INSERT INTO work_view_days (work_id, day, views) VALUES ($1, $2, 1)
         ON CONFLICT (work_id, day) DO UPDATE SET views = work_view_days.views + 1",
    )
    .bind(work.as_uuid())
    .bind(write.day)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}

async fn keep(
    tx: &mut Transaction<'_, Postgres>,
    event: EventDigest,
    receipt: ViewReceipt,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO work_view_receipts
                (event_digest, work_id, verdict, basis_revision_id, decided_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(event.as_bytes().as_slice())
    .bind(receipt.work().as_uuid())
    .bind(receipt.verdict().as_str())
    .bind(receipt.basis().map(|basis| basis.as_uuid()))
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    // The first decided event marks when counting started; later ones change nothing.
    sqlx::query(
        "INSERT INTO work_view_counting (singleton, started_at) VALUES (true, $1)
         ON CONFLICT (singleton) DO NOTHING",
    )
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}

impl WorkViewStore for PgStore {
    async fn record_view(&self, write: ViewWrite) -> Result<ViewDecision, StoreError> {
        let work_id = write.event.work;
        let mut tx = self.pool().begin().await.map_err(classify)?;
        // A withdraw/hide/republish (pointer DELETE/UPDATE) waits for this decision, or committed
        // before it and is what this read sees.
        let published: Option<Uuid> =
            sqlx::query_scalar("SELECT revision_id FROM publications WHERE work_id = $1 FOR SHARE")
                .bind(work_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
        // The owner never changes; the loaded row is the fact, never the request.
        // Even when the pointer is absent, the principal fence orders this decision against
        // account deletion. Read the fence in a separate statement after the lock wait.
        let owner: Option<Uuid> = sqlx::query_scalar(
            "SELECT w.owner_id FROM works w JOIN principals p ON p.id = w.owner_id
              WHERE w.id = $1 FOR KEY SHARE OF p",
        )
        .bind(work_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        if let Some(owner) = owner {
            let deleted: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM account_deletions WHERE principal_id = $1)",
            )
            .bind(owner)
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?;
            if deleted {
                return Ok(decide_view(
                    &write.event,
                    ViewFacts {
                        work: None,
                        receipt: None,
                        window: None,
                        now: write.now,
                    },
                ));
            }
        }
        let work = owner.map(|owner| WorkFacts {
            owner: PrincipalId::from_uuid(owner),
            published: published.map(RevisionId::from_uuid),
        });
        lock_event(&mut tx, write.event_digest).await?;
        let receipt = receipt(&mut tx, write.event_digest).await?;

        for _ in 0..CLAIM_ATTEMPTS {
            let window = if work.is_some() && receipt.is_none() {
                locked_window(&mut tx, work_id, write.reader).await?
            } else {
                None
            };
            let decision = decide_view(
                &write.event,
                ViewFacts {
                    work,
                    receipt,
                    window,
                    now: write.now,
                },
            );
            match &decision {
                ViewDecision::Count(count_write) => {
                    if !claim_window(&mut tx, work_id, write.reader, count_write).await? {
                        // A concurrent first view of this reader committed: decide again on it.
                        continue;
                    }
                    count(&mut tx, work_id, count_write).await?;
                    keep(&mut tx, write.event_digest, count_write.receipt, write.now).await?;
                }
                ViewDecision::Skip(skipped) => {
                    keep(&mut tx, write.event_digest, *skipped, write.now).await?;
                }
                ViewDecision::Replay(_) | ViewDecision::Refuse(_) => {}
            }
            tx.commit().await.map_err(classify)?;
            return Ok(decision);
        }
        Err(integrity("work view window claim did not settle", work_id))
    }

    async fn prune_views(&self, now: OffsetDateTime, limit: i64) -> Result<ViewPrune, StoreError> {
        prune(self.pool(), now, limit).await
    }
}

/// One retention round at `now`: windows whose view was counted more than [`VIEW_WINDOW`] ago
/// and receipts decided more than [`RECEIPT_RETENTION`] ago, at most `limit` of each, skipping
/// rows a decision holds (it is replacing or reading them).
pub async fn prune(
    pool: &sqlx::PgPool,
    now: OffsetDateTime,
    limit: i64,
) -> Result<ViewPrune, StoreError> {
    let windows = sqlx::query(
        "WITH doomed AS (
             SELECT work_id, reader_digest FROM work_view_windows
              WHERE counted_at < $1
              ORDER BY counted_at
              LIMIT $2
                FOR UPDATE SKIP LOCKED)
         DELETE FROM work_view_windows w USING doomed
          WHERE w.work_id = doomed.work_id AND w.reader_digest = doomed.reader_digest",
    )
    .bind(now - VIEW_WINDOW)
    .bind(limit)
    .execute(pool)
    .await
    .map_err(classify)?
    .rows_affected();
    let receipts = sqlx::query(
        "WITH doomed AS (
             SELECT event_digest FROM work_view_receipts
              WHERE decided_at < $1
              ORDER BY decided_at
              LIMIT $2
                FOR UPDATE SKIP LOCKED)
         DELETE FROM work_view_receipts r USING doomed
          WHERE r.event_digest = doomed.event_digest",
    )
    .bind(now - RECEIPT_RETENTION)
    .bind(limit)
    .execute(pool)
    .await
    .map_err(classify)?
    .rows_affected();
    Ok(ViewPrune { windows, receipts })
}

/// Rows of one work that [`erase_work`] removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ErasedViews {
    pub totals: u64,
    pub days: u64,
    pub windows: u64,
    pub receipts: u64,
}

/// Erases every view row of `work` inside the caller's transaction (OQ-2: the first runtime
/// route that deletes a work or an account must call this, or delete the `works` row, which
/// cascades). It locks the work's public pointer first, like a view decision, so no decision of
/// this work commits between the erase and the caller's commit; the caller must also make the
/// work non-public in that transaction or later events are decided again.
pub async fn erase_work(conn: &mut PgConnection, work: WorkId) -> Result<ErasedViews, StoreError> {
    sqlx::query("SELECT 1 FROM publications WHERE work_id = $1 FOR UPDATE")
        .bind(work.as_uuid())
        .fetch_optional(&mut *conn)
        .await
        .map_err(classify)?;
    let mut erased = ErasedViews::default();
    for (table, slot) in [
        ("work_view_totals", &mut erased.totals),
        ("work_view_days", &mut erased.days),
        ("work_view_windows", &mut erased.windows),
        ("work_view_receipts", &mut erased.receipts),
    ] {
        *slot = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {table} WHERE work_id = $1"
        )))
        .bind(work.as_uuid())
        .execute(&mut *conn)
        .await
        .map_err(classify)?
        .rows_affected();
    }
    Ok(erased)
}
