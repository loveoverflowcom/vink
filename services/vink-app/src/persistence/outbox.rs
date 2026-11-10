//! PostgreSQL adapter of the discussion outbox (DISC-04, #120). Decisions come from
//! [`crate::domain::discussion::outbox`]; this module only loads their facts, writes what they
//! return and keeps every network-free effect inside the fenced transaction.
//!
//! **Producer.** [`enqueue`] runs inside the business transaction that made the transition (today
//! only `submit_comment`), after the comment insert: the event and one delivery per consumer of
//! its type commit or roll back with the comment. A failed enqueue fails the submission.
//!
//! **Claim.** [`claim`] is one short transaction: due deliveries of the consumers this binary
//! handles, `FOR UPDATE OF d SKIP LOCKED`, oldest `available_at` first, at most `limit` rows; the
//! pure [`decide_claim`] then advances each row's generation and attempts and sets its lease (or
//! dead-letters it). It commits before any handler runs; no transaction is held during work.
//! Ordering is best effort: there is no global FIFO and no `id > last_seen` checkpoint.
//!
//! **Effect.** [`complete`] locks the delivery row `FOR UPDATE`, checks [`check_ownership`]
//! under that lock, runs the handler in a savepoint of the same transaction, records the pure
//! [`decide_completion`] and commits. A stale worker (reclaimed, or its delivery already
//! finished) writes nothing: not the effect, not the outcome. A handler result other than
//! "applied" rolls back to the savepoint, so a failed attempt leaves no partial effect. If the
//! transaction itself fails, [`record_failure`] records a retry in a new fenced transaction; if
//! that also fails, the lease expires and the attempt is spent. A commit whose response was lost
//! is safe too: the next fenced write finds the delivery no longer leased and writes nothing.
//!
//! **Lock order.** The worker locks `discussion_deliveries` rows first (claim, complete, requeue,
//! retention through `discussion_events`); no comment path ever locks a delivery. Handlers read
//! `comments`, receipts and suspensions without locks and write only their own receipt table,
//! whose foreign key takes `FOR KEY SHARE` on the observed comment row: that one row serializes
//! briefly with a moderation or removal holding it `FOR UPDATE` (bounded by `lock_timeout` and
//! the handler timeout), and no cycle exists because those paths never wait on a delivery. The
//! producer inserts new rows that no worker statement can have locked.
//!
//! **Privacy.** Events and deliveries hold identifiers, versions and codes only. Logs and errors
//! from here carry codes and counts, never a body, address, session or moderation reason.

use std::future::Future;
use std::pin::Pin;

use sqlx::postgres::PgPool;
use sqlx::{PgConnection, Postgres, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::discussion::{integrity, review_code, state};
use super::postgres::classify;
use crate::application::ports::StoreError;
use crate::domain::discussion::admission::{AdmissionDecision, InitialReview, Standing};
use crate::domain::discussion::lifecycle::ModerationVersion;
use crate::domain::discussion::outbox::{
    ClaimDecision, ClaimGeneration, Completion, DeadReason, DeliveryFacts, DeliveryState,
    EventKind, HandlerResult, OutboxPolicy, Ownership, Requeue, ShadowObservation, ShadowTarget,
    Transient, check_ownership, decide_claim, decide_completion, decide_requeue, observe_shadow,
};
use crate::domain::discussion::submission::PayloadDigest;
use crate::domain::ids::CommentId;

/// Statement bounds of the fenced transaction: a handler never waits long on a lock it does not
/// own, and a slow statement ends as a retryable failure instead of holding the lease.
const EFFECT_BOUNDS: &str = "SET LOCAL lock_timeout = '2s'";
const EFFECT_STATEMENT_BOUND: &str = "SET LOCAL statement_timeout = '5s'";
/// Recording a failure may wait for the abandoned attempt's backend to finish its statement
/// (bounded by the statement timeout above) and roll back, so it waits a little longer.
const FAILURE_BOUNDS: &str = "SET LOCAL lock_timeout = '6s'";

fn micros(at: OffsetDateTime) -> i64 {
    i64::try_from(at.unix_timestamp_nanos() / 1_000).unwrap_or(i64::MAX)
}

fn instant(micros: i64) -> Result<OffsetDateTime, StoreError> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(micros) * 1_000)
        .map_err(|_| integrity("outbox instant out of range", micros))
}

/// Writes `kind`'s event for `comment` at `version` and one ready delivery per consumer that the
/// type owes, in the caller's transaction.
pub(super) async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    kind: EventKind,
    comment: CommentId,
    version: ModerationVersion,
) -> Result<(), StoreError> {
    let event = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO discussion_events (id, event_type, schema_version, comment_id, source_version)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(event)
    .bind(kind.code())
    .bind(kind.current_schema())
    .bind(comment.as_uuid())
    .bind(version.get())
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    let consumers: Vec<&str> = kind.consumers().iter().map(|c| c.code()).collect();
    sqlx::query(
        "INSERT INTO discussion_deliveries (event_id, consumer)
         SELECT $1, consumer FROM unnest($2::text[]) AS consumer",
    )
    .bind(event)
    .bind(&consumers)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}

/// An event as stored. Fields are raw: the handler decides whether it can read them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxEvent {
    pub id: Uuid,
    pub event_type: String,
    pub schema_version: i16,
    pub comment_id: Uuid,
    pub source_version: i64,
}

/// A delivery this worker claimed, with the generation every later write must still match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claimed {
    pub event: OutboxEvent,
    pub consumer: String,
    pub generation: ClaimGeneration,
    pub attempts: u32,
    pub reclaimed: bool,
}

/// What one claim transaction did. Counts only.
#[derive(Debug, Default)]
pub struct ClaimBatch {
    pub claimed: Vec<Claimed>,
    pub dead_lettered: usize,
    pub reclaimed: usize,
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// One consumer's effect. `apply` runs inside the fenced transaction (in a savepoint): it may
/// read through `tx` and write only that consumer's own tables; every statement it issues
/// commits only if the delivery is still owned and the result is [`HandlerResult::Applied`]. It
/// must not call the network: an effect outside the database needs its own idempotency design.
pub trait EffectHandler: Send + Sync {
    /// The stored consumer code this handler serves.
    fn consumer(&self) -> &str;

    fn apply<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        event: &'a OutboxEvent,
    ) -> BoxFuture<'a, Result<HandlerResult, sqlx::Error>>;
}

fn delivery_facts(row: &sqlx::postgres::PgRow) -> Result<DeliveryFacts, StoreError> {
    let raw_state: String = row.try_get("state").map_err(classify)?;
    let attempts: i32 = row.try_get("attempts").map_err(classify)?;
    let generation: i64 = row.try_get("generation").map_err(classify)?;
    Ok(DeliveryFacts {
        state: DeliveryState::from_code(&raw_state)
            .ok_or_else(|| integrity("stored delivery state", &raw_state))?,
        attempts: u32::try_from(attempts).map_err(|_| integrity("stored attempts", attempts))?,
        generation: ClaimGeneration::restore(generation)
            .ok_or_else(|| integrity("stored generation", generation))?,
        available_at_micros: micros(row.try_get("available_at").map_err(classify)?),
        deadline_from_micros: micros(row.try_get("deadline_from").map_err(classify)?),
    })
}

/// Claims up to `limit` due deliveries of `consumers` and commits the claims.
pub async fn claim(
    pool: &PgPool,
    consumers: &[&str],
    limit: i64,
    policy: OutboxPolicy,
) -> Result<ClaimBatch, StoreError> {
    let mut batch = ClaimBatch::default();
    if limit <= 0 || consumers.is_empty() {
        return Ok(batch);
    }
    let mut tx = pool.begin().await.map_err(classify)?;
    sqlx::query(EFFECT_BOUNDS)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    sqlx::query(EFFECT_STATEMENT_BOUND)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    let now: OffsetDateTime = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
    let rows = sqlx::query(
        "SELECT d.event_id, d.consumer, d.state, d.attempts, d.generation, d.available_at,
                d.deadline_from, e.event_type, e.schema_version, e.comment_id, e.source_version
           FROM discussion_deliveries d
           JOIN discussion_events e ON e.id = d.event_id
          WHERE d.state IN ('ready', 'leased') AND d.available_at <= $1
            AND d.consumer = ANY($2)
          ORDER BY d.available_at
          LIMIT $3
            FOR UPDATE OF d SKIP LOCKED",
    )
    .bind(now)
    .bind(consumers)
    .bind(limit)
    .fetch_all(&mut *tx)
    .await
    .map_err(classify)?;
    for row in &rows {
        let facts = delivery_facts(row)?;
        let event_id: Uuid = row.try_get("event_id").map_err(classify)?;
        let consumer: String = row.try_get("consumer").map_err(classify)?;
        match decide_claim(facts, policy, micros(now)) {
            ClaimDecision::NotEligible => {}
            ClaimDecision::Claim {
                generation,
                attempts,
                lease_until_micros,
                reclaimed,
            } => {
                sqlx::query(
                    "UPDATE discussion_deliveries
                        SET state = 'leased', generation = $3, attempts = $4, available_at = $5,
                            expired_leases = expired_leases + $6,
                            last_error = CASE WHEN $6 = 1 THEN 'lease_expired' ELSE last_error END,
                            updated_at = $7
                      WHERE event_id = $1 AND consumer = $2",
                )
                .bind(event_id)
                .bind(&consumer)
                .bind(generation.get())
                .bind(i32::try_from(attempts).map_err(|_| integrity("attempts", attempts))?)
                .bind(instant(lease_until_micros)?)
                .bind(i32::from(reclaimed))
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
                batch.reclaimed += usize::from(reclaimed);
                batch.claimed.push(Claimed {
                    event: OutboxEvent {
                        id: event_id,
                        event_type: row.try_get("event_type").map_err(classify)?,
                        schema_version: row.try_get("schema_version").map_err(classify)?,
                        comment_id: row.try_get("comment_id").map_err(classify)?,
                        source_version: row.try_get("source_version").map_err(classify)?,
                    },
                    consumer,
                    generation,
                    attempts,
                    reclaimed,
                });
            }
            ClaimDecision::DeadLetter { reason, reclaimed } => {
                // Retry exhaustion cannot discharge a content-review obligation. The safe
                // effect/receipt (or explicit poison debt) commits with this dead transition.
                finalize_dead(
                    &mut tx,
                    &OutboxEvent {
                        id: event_id,
                        event_type: row.try_get("event_type").map_err(classify)?,
                        schema_version: row.try_get("schema_version").map_err(classify)?,
                        comment_id: row.try_get("comment_id").map_err(classify)?,
                        source_version: row.try_get("source_version").map_err(classify)?,
                    },
                    &consumer,
                )
                .await?;
                sqlx::query(
                    "UPDATE discussion_deliveries
                        SET state = 'dead', outcome = $3, finished_at = $5, updated_at = $5,
                            expired_leases = expired_leases + $4,
                            last_error = CASE WHEN $4 = 1 THEN 'lease_expired' ELSE last_error END
                      WHERE event_id = $1 AND consumer = $2",
                )
                .bind(event_id)
                .bind(&consumer)
                .bind(reason.code())
                .bind(i32::from(reclaimed))
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
                batch.reclaimed += usize::from(reclaimed);
                batch.dead_lettered += 1;
                tracing::warn!(
                    consumer = consumer.as_str(),
                    reason = reason.code(),
                    "outbox delivery dead-lettered at claim"
                );
            }
        }
    }
    tx.commit().await.map_err(classify)?;
    Ok(batch)
}

/// Locks the delivery and returns its facts and the transaction's clock.
async fn lock_delivery(
    tx: &mut Transaction<'_, Postgres>,
    claimed: &Claimed,
    lock_bound: &'static str,
) -> Result<Option<(DeliveryFacts, OffsetDateTime)>, StoreError> {
    sqlx::query(lock_bound)
        .execute(&mut **tx)
        .await
        .map_err(classify)?;
    sqlx::query(EFFECT_STATEMENT_BOUND)
        .execute(&mut **tx)
        .await
        .map_err(classify)?;
    let row = sqlx::query(
        "SELECT state, attempts, generation, available_at, deadline_from, now() AS now
           FROM discussion_deliveries
          WHERE event_id = $1 AND consumer = $2
            FOR UPDATE",
    )
    .bind(claimed.event.id)
    .bind(&claimed.consumer)
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)?;
    row.map(|row| Ok((delivery_facts(&row)?, row.try_get("now").map_err(classify)?)))
        .transpose()
}

/// Writes a non-stale completion. The `WHERE` repeats the fence; anything but one row is a
/// defect, because the caller holds the row lock and checked ownership under it.
async fn record(
    tx: &mut Transaction<'_, Postgres>,
    claimed: &Claimed,
    completion: Completion,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    let updated = match completion {
        Completion::Stale(_) => return Ok(()),
        Completion::Retry {
            available_at_micros,
            error,
        } => sqlx::query(
            "UPDATE discussion_deliveries
                SET state = 'ready', available_at = CASE WHEN consumer = 'operator_post_review.v1'
                    THEN LEAST($4, (SELECT deadline_at FROM comment_post_reviews WHERE id = $1))
                    ELSE $4 END, last_error = $5, updated_at = $6
              WHERE event_id = $1 AND consumer = $2 AND generation = $3 AND state = 'leased'",
        )
        .bind(claimed.event.id)
        .bind(&claimed.consumer)
        .bind(claimed.generation.get())
        .bind(instant(available_at_micros)?)
        .bind(error.code())
        .bind(now),
        terminal => sqlx::query(
            "UPDATE discussion_deliveries
                SET state = $4, outcome = $5, finished_at = $6, updated_at = $6
              WHERE event_id = $1 AND consumer = $2 AND generation = $3 AND state = 'leased'",
        )
        .bind(claimed.event.id)
        .bind(&claimed.consumer)
        .bind(claimed.generation.get())
        .bind(terminal.state().map_or("leased", DeliveryState::code))
        .bind(terminal.outcome())
        .bind(now),
    }
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    if updated.rows_affected() == 1 {
        Ok(())
    } else {
        Err(integrity("fenced delivery update", updated.rows_affected()))
    }
}

/// Runs `handler` for `claimed` in the fenced transaction and records its outcome.
pub async fn complete(
    pool: &PgPool,
    claimed: &Claimed,
    handler: &dyn EffectHandler,
    policy: OutboxPolicy,
    jitter_micros: i64,
) -> Result<Completion, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let Some((facts, now)) = lock_delivery(&mut tx, claimed, EFFECT_BOUNDS).await? else {
        return Ok(Completion::Stale(
            crate::domain::discussion::outbox::StaleReason::NotLeased,
        ));
    };
    if let Ownership::Stale(reason) = check_ownership(facts, claimed.generation) {
        tx.rollback().await.map_err(classify)?;
        return Ok(Completion::Stale(reason));
    }
    sqlx::query("SAVEPOINT outbox_effect")
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    let result = match handler.apply(&mut tx, &claimed.event).await {
        Ok(result) => result,
        Err(error) => {
            // The transaction may be aborted: roll it back now (releasing the row lock and
            // discarding any partial effect) and record the failure in a new fenced one.
            drop(classify(error));
            let _ = tx.rollback().await;
            return record_failure(pool, claimed, Transient::Database, policy, jitter_micros).await;
        }
    };
    if !matches!(result, HandlerResult::Applied) {
        sqlx::query("ROLLBACK TO SAVEPOINT outbox_effect")
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
    }
    let completion = decide_completion(
        facts,
        claimed.generation,
        result,
        policy,
        micros(now),
        jitter_micros,
    );
    if matches!(completion, Completion::DeadLetter(_)) {
        finalize_dead(&mut tx, &claimed.event, &claimed.consumer).await?;
    }
    record(&mut tx, claimed, completion, now).await?;
    tx.commit().await.map_err(classify)?;
    Ok(completion)
}

/// Records a failed attempt (database error, timeout, panic) in its own fenced transaction.
pub async fn record_failure(
    pool: &PgPool,
    claimed: &Claimed,
    failure: Transient,
    policy: OutboxPolicy,
    jitter_micros: i64,
) -> Result<Completion, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let Some((facts, now)) = lock_delivery(&mut tx, claimed, FAILURE_BOUNDS).await? else {
        return Ok(Completion::Stale(
            crate::domain::discussion::outbox::StaleReason::NotLeased,
        ));
    };
    let completion = decide_completion(
        facts,
        claimed.generation,
        HandlerResult::Retry(failure),
        policy,
        micros(now),
        jitter_micros,
    );
    if matches!(completion, Completion::DeadLetter(_)) {
        finalize_dead(&mut tx, &claimed.event, &claimed.consumer).await?;
    }
    record(&mut tx, claimed, completion, now).await?;
    tx.commit().await.map_err(classify)?;
    Ok(completion)
}

async fn finalize_dead(
    tx: &mut PgConnection,
    event: &OutboxEvent,
    consumer: &str,
) -> Result<(), StoreError> {
    if consumer == crate::domain::discussion::post_review::CONSUMER {
        let result = super::post_review::apply(tx, event, true)
            .await
            .map_err(classify)?;
        if !matches!(
            result,
            HandlerResult::Applied | HandlerResult::AlreadyApplied | HandlerResult::Fail(_)
        ) {
            return Err(StoreError::Integrity(
                "post-review finalization left unresolved without quarantine".into(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequeueOutcome {
    Requeued { previous_attempts: i32 },
    Refused(DeliveryState),
    NotFound,
}

/// Operator requeue of a dead letter: back to ready now, fresh attempts and deadline, generation
/// and event kept, audited in the same transaction. Never touches another consumer's delivery.
pub async fn requeue(
    pool: &PgPool,
    event_id: Uuid,
    consumer: &str,
    note: &str,
) -> Result<RequeueOutcome, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let row: Option<(String, Option<String>, i32)> = sqlx::query_as(
        "SELECT state, outcome, attempts FROM discussion_deliveries
          WHERE event_id = $1 AND consumer = $2 FOR UPDATE",
    )
    .bind(event_id)
    .bind(consumer)
    .fetch_optional(&mut *tx)
    .await
    .map_err(classify)?;
    let Some((raw_state, outcome, attempts)) = row else {
        return Ok(RequeueOutcome::NotFound);
    };
    let current = DeliveryState::from_code(&raw_state)
        .ok_or_else(|| integrity("stored delivery state", &raw_state))?;
    match decide_requeue(current) {
        Requeue::Refused(state) => Ok(RequeueOutcome::Refused(state)),
        Requeue::Requeued => {
            sqlx::query(
                "UPDATE discussion_deliveries
                    SET state = 'ready', attempts = 0, available_at = now(),
                        deadline_from = CASE WHEN consumer = 'operator_post_review.v1' THEN deadline_from ELSE now() END,
                        last_error = outcome, outcome = NULL, finished_at = NULL, updated_at = now()
                  WHERE event_id = $1 AND consumer = $2",
            )
            .bind(event_id)
            .bind(consumer)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
            sqlx::query(
                "INSERT INTO discussion_delivery_requeues
                        (event_id, consumer, previous_outcome, previous_attempts, note)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(event_id)
            .bind(consumer)
            .bind(outcome.as_deref().unwrap_or("unknown"))
            .bind(attempts)
            .bind(note)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
            tx.commit().await.map_err(classify)?;
            Ok(RequeueOutcome::Requeued {
                previous_attempts: attempts,
            })
        }
    }
}

/// Queue health: counts and ages only, no identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Backlog {
    /// Ready and due, or leased with an expired lease, for a handled consumer.
    pub eligible: i64,
    /// Ready but waiting for a scheduled retry.
    pub scheduled: i64,
    /// Leased with a live lease.
    pub leased: i64,
    pub dead: i64,
    /// Open deliveries of consumers this binary does not handle (left for their owner).
    pub unhandled: i64,
    pub oldest_eligible_seconds: Option<i64>,
    pub oldest_open_seconds: Option<i64>,
}

pub async fn backlog(pool: &PgPool, consumers: &[&str]) -> Result<Backlog, StoreError> {
    let row = sqlx::query(
        "SELECT
             count(*) FILTER (WHERE state IN ('ready', 'leased') AND available_at <= now()
                              AND consumer = ANY($1)) AS eligible,
             count(*) FILTER (WHERE state = 'ready' AND available_at > now()) AS scheduled,
             count(*) FILTER (WHERE state = 'leased' AND available_at > now()) AS leased,
             count(*) FILTER (WHERE state = 'dead') AS dead,
             count(*) FILTER (WHERE state IN ('ready', 'leased') AND consumer <> ALL($1))
                 AS unhandled,
             EXTRACT(EPOCH FROM now() - min(available_at) FILTER (
                 WHERE state IN ('ready', 'leased') AND available_at <= now()
                   AND consumer = ANY($1)))::bigint AS oldest_eligible,
             EXTRACT(EPOCH FROM now() - min(created_at) FILTER (
                 WHERE state IN ('ready', 'leased')))::bigint AS oldest_open
           FROM discussion_deliveries
          WHERE state IN ('ready', 'leased', 'dead')",
    )
    .bind(consumers)
    .fetch_one(pool)
    .await
    .map_err(classify)?;
    let count = |name: &str| row.try_get::<i64, _>(name).map_err(classify);
    Ok(Backlog {
        eligible: count("eligible")?,
        scheduled: count("scheduled")?,
        leased: count("leased")?,
        dead: count("dead")?,
        unhandled: count("unhandled")?,
        oldest_eligible_seconds: row.try_get("oldest_eligible").map_err(classify)?,
        oldest_open_seconds: row.try_get("oldest_open").map_err(classify)?,
    })
}

/// One dead letter for the operator listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadLetter {
    pub event_id: Uuid,
    pub consumer: String,
    pub event_type: String,
    pub schema_version: i16,
    pub outcome: String,
    pub attempts: i32,
    pub finished_at: OffsetDateTime,
}

pub async fn dead_letters(pool: &PgPool, limit: i64) -> Result<Vec<DeadLetter>, StoreError> {
    let rows = sqlx::query(
        "SELECT d.event_id, d.consumer, e.event_type, e.schema_version, d.outcome, d.attempts,
                d.finished_at
           FROM discussion_deliveries d JOIN discussion_events e ON e.id = d.event_id
          WHERE d.state = 'dead'
          ORDER BY d.finished_at, d.event_id, d.consumer
          LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(classify)?;
    rows.iter()
        .map(|row| {
            Ok(DeadLetter {
                event_id: row.try_get("event_id")?,
                consumer: row.try_get("consumer")?,
                event_type: row.try_get("event_type")?,
                schema_version: row.try_get("schema_version")?,
                outcome: row.try_get("outcome")?,
                attempts: row.try_get("attempts")?,
                finished_at: row.try_get("finished_at")?,
            })
        })
        .collect::<Result<_, sqlx::Error>>()
        .map_err(classify)
}

/// Deletes up to `limit` events (and their deliveries) whose every delivery succeeded or was
/// skipped more than `retain_days` ago. Open and dead deliveries keep their event; consumer
/// receipts (such as shadow observations) are separate tables and are never pruned here.
pub async fn prune(pool: &PgPool, retain_days: i32, limit: i64) -> Result<u64, StoreError> {
    let deleted = sqlx::query(
        "WITH doomed AS (
             SELECT e.id FROM discussion_events e
              WHERE e.created_at < now() - make_interval(days => $1)
                AND NOT EXISTS (SELECT 1 FROM comment_post_reviews t WHERE t.id = e.id
                    AND NOT EXISTS (SELECT 1 FROM comment_post_review_receipts r WHERE r.ticket_id = t.id))
                AND NOT EXISTS (SELECT 1 FROM comment_post_review_quarantines q WHERE q.event_id = e.id)
                AND NOT EXISTS (
                    SELECT 1 FROM discussion_deliveries d
                     WHERE d.event_id = e.id
                       AND (d.state NOT IN ('succeeded', 'skipped')
                            OR d.finished_at >= now() - make_interval(days => $1)))
              ORDER BY e.created_at
              LIMIT $2
                FOR UPDATE SKIP LOCKED)
         DELETE FROM discussion_events e USING doomed WHERE e.id = doomed.id",
    )
    .bind(retain_days)
    .bind(limit)
    .execute(pool)
    .await
    .map_err(classify)?;
    Ok(deleted.rows_affected())
}

/// `admission_shadow.v1`: records what the publish-first candidate would decide for a submitted
/// comment, as observed now. Reads the comment, its submission receipt's digest and its writer's
/// standing; writes only `comment_admission_observations` (its receipt); never `comments`.
pub struct AdmissionShadow;

impl EffectHandler for AdmissionShadow {
    fn consumer(&self) -> &str {
        crate::domain::discussion::outbox::Consumer::AdmissionShadow.code()
    }

    fn apply<'a>(
        &'a self,
        tx: &'a mut PgConnection,
        event: &'a OutboxEvent,
    ) -> BoxFuture<'a, Result<HandlerResult, sqlx::Error>> {
        Box::pin(async move {
            if EventKind::supported(&event.event_type, event.schema_version)
                != Some(EventKind::CommentSubmitted)
            {
                return Ok(HandlerResult::Fail(DeadReason::UnsupportedEvent));
            }
            let row = sqlx::query(
                "SELECT c.review, c.removal, c.moderation_version,
                        (SELECT r.payload_digest FROM discussion_command_receipts r
                          WHERE r.actor_id = c.author_id AND r.operation = 'submit_comment'
                            AND r.comment_id = c.id) AS payload_digest,
                        EXISTS (SELECT 1 FROM principal_suspensions s
                                 WHERE s.principal_id = c.author_id AND s.lifted_at IS NULL)
                            AS suspended,
                        now() AS now
                   FROM comments c WHERE c.id = $1",
            )
            .bind(event.comment_id)
            .fetch_optional(&mut *tx)
            .await?;
            let (target, now) = match &row {
                None => {
                    let now: OffsetDateTime = sqlx::query_scalar("SELECT now()")
                        .fetch_one(&mut *tx)
                        .await?;
                    (None, now)
                }
                Some(row) => {
                    let raw_review: String = row.try_get("review")?;
                    let raw_removal: Option<String> = row.try_get("removal")?;
                    let version: i64 = row.try_get("moderation_version")?;
                    let digest: Option<Vec<u8>> = row.try_get("payload_digest")?;
                    let suspended: bool = row.try_get("suspended")?;
                    let (Ok(current), Some(payload)) = (
                        state(&raw_review, raw_removal.as_deref(), version),
                        digest.as_deref().and_then(PayloadDigest::from_bytes),
                    ) else {
                        return Ok(HandlerResult::Fail(DeadReason::InvalidPayload));
                    };
                    let standing = if suspended {
                        Standing::Suspended
                    } else {
                        Standing::Active
                    };
                    (
                        Some(ShadowTarget {
                            state: current,
                            standing,
                            payload,
                        }),
                        row.try_get("now")?,
                    )
                }
            };
            match observe_shadow(target, micros(now)) {
                ShadowObservation::Skip(reason) => Ok(HandlerResult::Skipped(reason)),
                ShadowObservation::Record {
                    candidate,
                    decision,
                    observed,
                } => {
                    let (outcome, reason) = match decision {
                        AdmissionDecision::Admit(admission) => (
                            match admission.review() {
                                InitialReview::Pending => "pending",
                                InitialReview::Approved => "approved",
                            },
                            admission.reason().code(),
                        ),
                        AdmissionDecision::Refuse(refusal) => ("refused", refusal.code()),
                    };
                    let inserted = sqlx::query(
                        "INSERT INTO comment_admission_observations
                                (comment_id, candidate_policy, event_id, outcome, reason,
                                 observed_version, observed_review)
                         VALUES ($1, $2, $3, $4, $5, $6, $7)
                         ON CONFLICT (comment_id, candidate_policy) DO NOTHING",
                    )
                    .bind(event.comment_id)
                    .bind(candidate.code())
                    .bind(event.id)
                    .bind(outcome)
                    .bind(reason)
                    .bind(observed.version().get())
                    .bind(review_code(observed.review()))
                    .execute(&mut *tx)
                    .await?;
                    Ok(if inserted.rows_affected() == 1 {
                        HandlerResult::Applied
                    } else {
                        HandlerResult::AlreadyApplied
                    })
                }
            }
        })
    }
}
