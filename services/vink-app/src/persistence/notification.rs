//! PostgreSQL adapter of the Notifications v1 producer outbox and relay (#346; contract
//! `contracts/http/v1/notifications.md` §1). Decisions come from [`crate::domain::notification`]
//! and the shared outbox lifecycle; this module loads their facts and writes what they return.
//!
//! **Producer ([`emit`]).** Called inside the business transaction that made the transition,
//! after its own writes and after every lock of that path: it reads the activation row
//! `FOR KEY SHARE` (the last lock any producer takes), and only while active writes one
//! immutable event (canonical envelope + SHA-256) and its single relay delivery. An error
//! here fails the caller's transaction, so a committed transition always owes its notice and a
//! rolled-back or no-op one owes nothing. `vink-app notifications activate|deactivate` takes the
//! same row `FOR UPDATE`, so a transition commits wholly before or wholly after a cutover. A
//! logical event already owed (same `dedup_key`) is not written twice.
//!
//! **Relay ([`claim`], [`record`]).** One short transaction claims due deliveries
//! (`FOR UPDATE SKIP LOCKED`, generation + 1, lease) and commits before any I/O. The caller sends
//! the stored envelope outside any App transaction, then [`record`] locks the delivery row,
//! checks the claim fence under that lock and writes the pure decision. A worker that lost its
//! claim (lease expired and reclaimed, or the delivery already acknowledged) writes nothing: its
//! late ACK can neither finish nor reschedule the new owner's attempt.
//!
//! **Lock order.** Producers: their own path's locks, then `notification_activation` (KEY
//! SHARE); event and delivery rows are new. Relay: `notification_deliveries` rows only. Operator
//! commands: the activation row alone, or delivery rows alone. No path takes them in reverse.
//!
//! **Privacy.** Events hold identifiers, versions, codes and booleans. Logs and errors from here
//! carry codes and counts, never an envelope, a body, an address or a reason.

use sqlx::postgres::PgPool;
use sqlx::{PgConnection, Postgres, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::discussion::integrity;
use super::postgres::classify;
use crate::application::ports::StoreError;
use crate::domain::discussion::outbox::{
    ClaimDecision, ClaimGeneration, DeliveryFacts, DeliveryState, OutboxPolicy, Requeue,
    StaleReason, decide_claim, decide_requeue,
};
use crate::domain::ids::WorkId;
use crate::domain::notification::{
    Activation, ActivationChange, Envelope, Notice, RelayCompletion, RelayResult,
    decide_relay_completion, work_unreadable,
};
use crate::domain::publication::{NotificationStamp, PublicEpoch};

/// Acknowledged events are kept this long: the published replay horizon for a Notification
/// restore (contract §1.3 step 5). A restore older than this cannot be replayed from the App.
pub const REPLAY_HORIZON_DAYS: i32 = 30;

const LOCK_BOUND: &str = "SET LOCAL lock_timeout = '2s'";
const STATEMENT_BOUND: &str = "SET LOCAL statement_timeout = '5s'";

fn micros(at: OffsetDateTime) -> i64 {
    i64::try_from(at.unix_timestamp_nanos() / 1_000).unwrap_or(i64::MAX)
}

fn instant(micros: i64) -> Result<OffsetDateTime, StoreError> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(micros) * 1_000)
        .map_err(|_| integrity("notification instant out of range", micros))
}

fn activation_of(active: bool, epoch: i64) -> Result<Activation, StoreError> {
    Activation::restore(active, epoch)
        .ok_or_else(|| integrity("stored notification activation", epoch))
}

/// Writes what `notices` owe in the caller's transaction, if the feature is active there.
/// Returns how many new events were written.
pub(crate) async fn emit(
    conn: &mut PgConnection,
    notices: impl IntoIterator<Item = Notice>,
) -> Result<usize, StoreError> {
    let notices: Vec<Notice> = notices.into_iter().collect();
    if notices.is_empty() {
        return Ok(0);
    }
    // Before migration 20261127000000 nothing could have been activated: an absent activation
    // table means "never activated" (the migration creates it inactive), not an error. Only
    // schema-staged tests reach this; a serving binary requires the current schema.
    let exists: bool =
        sqlx::query_scalar("SELECT to_regclass('notification_activation') IS NOT NULL")
            .fetch_one(&mut *conn)
            .await
            .map_err(classify)?;
    if !exists {
        return Ok(0);
    }
    let row = sqlx::query(
        "SELECT active, epoch, now() AS now FROM notification_activation FOR KEY SHARE",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(classify)?;
    let activation = activation_of(
        row.try_get("active").map_err(classify)?,
        row.try_get("epoch").map_err(classify)?,
    )?;
    let Some(epoch) = activation.producing() else {
        return Ok(0);
    };
    let now: OffsetDateTime = row.try_get("now").map_err(classify)?;
    let mut written = 0;
    for notice in notices {
        let dedup_key = notice.dedup_key();
        let envelope = Envelope::new(Uuid::now_v7(), epoch, now, notice)
            .ok_or_else(|| integrity("notification envelope", epoch))?;
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO notification_events
                    (id, event_type, schema_version, source_owner, dedup_key, activation_epoch,
                     envelope, payload_digest, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (dedup_key) DO NOTHING
             RETURNING id",
        )
        .bind(envelope.event_id())
        .bind(envelope.notice().event_type().code())
        .bind(crate::domain::notification::SCHEMA_VERSION)
        .bind(envelope.notice().source_owner().code())
        .bind(&dedup_key)
        .bind(epoch)
        .bind(envelope.canonical())
        .bind(envelope.digest())
        .bind(now)
        .fetch_optional(&mut *conn)
        .await
        .map_err(classify)?;
        if let Some(event) = inserted {
            sqlx::query(
                "INSERT INTO notification_deliveries (event_id, available_at, deadline_from)
                 VALUES ($1, $2, $2)",
            )
            .bind(event)
            .bind(now)
            .execute(&mut *conn)
            .await
            .map_err(classify)?;
            written += 1;
        }
    }
    Ok(written)
}

/// The work's readable epoch as stored (0 for a work that was never readable).
pub(crate) async fn public_epoch(
    conn: &mut PgConnection,
    work: WorkId,
) -> Result<PublicEpoch, StoreError> {
    let raw: Option<i64> =
        sqlx::query_scalar("SELECT public_epoch FROM work_publications WHERE work_id = $1")
            .bind(work.as_uuid())
            .fetch_optional(&mut *conn)
            .await
            .map_err(classify)?;
    let raw = raw.unwrap_or(0);
    PublicEpoch::new(raw).ok_or_else(|| integrity("stored public epoch", raw))
}

/// COM-11 stamp at commit. Locks the public pointer FOR SHARE (as submission and endorsement
/// do), so a withdrawal or republication commits wholly before this read or after the caller's
/// commit: an approval that commits after a withdrawal never counts as readable.
pub(crate) async fn locked_stamp(
    conn: &mut PgConnection,
    work: WorkId,
) -> Result<NotificationStamp, StoreError> {
    let readable: Option<Uuid> =
        sqlx::query_scalar("SELECT revision_id FROM publications WHERE work_id = $1 FOR SHARE")
            .bind(work.as_uuid())
            .fetch_optional(&mut *conn)
            .await
            .map_err(classify)?;
    Ok(NotificationStamp {
        epoch: public_epoch(conn, work).await?,
        readable: readable.is_some(),
    })
}

/// The caller deleted the work's public pointer (`rows` rows): its readable epoch ended.
pub(crate) async fn work_left_public(
    conn: &mut PgConnection,
    work: WorkId,
    rows: u64,
) -> Result<(), StoreError> {
    if rows == 0 {
        return Ok(());
    }
    let ended = public_epoch(conn, work).await?;
    emit(conn, [work_unreadable(work, ended)]).await?;
    Ok(())
}

/// A delivery this relay claimed, with the generation every later write must still match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedNotification {
    pub event_id: Uuid,
    /// The stored canonical envelope (no digest).
    pub envelope: String,
    pub payload_digest: String,
    pub generation: ClaimGeneration,
    pub attempts: u32,
    pub reclaimed: bool,
}

#[derive(Debug, Default)]
pub struct NotificationClaims {
    pub claimed: Vec<ClaimedNotification>,
    pub dead_lettered: usize,
    pub reclaimed: usize,
}

fn delivery_facts(row: &sqlx::postgres::PgRow) -> Result<DeliveryFacts, StoreError> {
    let raw_state: String = row.try_get("state").map_err(classify)?;
    let attempts: i32 = row.try_get("attempts").map_err(classify)?;
    let generation: i64 = row.try_get("generation").map_err(classify)?;
    let state = DeliveryState::from_code(&raw_state)
        .filter(|state| *state != DeliveryState::Skipped)
        .ok_or_else(|| integrity("stored notification delivery state", &raw_state))?;
    Ok(DeliveryFacts {
        state,
        attempts: u32::try_from(attempts).map_err(|_| integrity("stored attempts", attempts))?,
        generation: ClaimGeneration::restore(generation)
            .ok_or_else(|| integrity("stored generation", generation))?,
        available_at_micros: micros(row.try_get("available_at").map_err(classify)?),
        deadline_from_micros: micros(row.try_get("deadline_from").map_err(classify)?),
    })
}

async fn bounded(tx: &mut Transaction<'_, Postgres>) -> Result<(), StoreError> {
    for bound in [LOCK_BOUND, STATEMENT_BOUND] {
        sqlx::query(bound)
            .execute(&mut **tx)
            .await
            .map_err(classify)?;
    }
    Ok(())
}

/// Claims up to `limit` due deliveries and commits the claims before any I/O.
pub async fn claim(
    pool: &PgPool,
    limit: i64,
    policy: OutboxPolicy,
) -> Result<NotificationClaims, StoreError> {
    let mut batch = NotificationClaims::default();
    if limit <= 0 {
        return Ok(batch);
    }
    let mut tx = pool.begin().await.map_err(classify)?;
    bounded(&mut tx).await?;
    let now: OffsetDateTime = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
    let rows = sqlx::query(
        "SELECT d.event_id, d.state, d.attempts, d.generation, d.available_at, d.deadline_from,
                e.envelope, e.payload_digest
           FROM notification_deliveries d
           JOIN notification_events e ON e.id = d.event_id
          WHERE d.state IN ('ready', 'leased') AND d.available_at <= $1
          ORDER BY d.available_at
          LIMIT $2
            FOR UPDATE OF d SKIP LOCKED",
    )
    .bind(now)
    .bind(limit)
    .fetch_all(&mut *tx)
    .await
    .map_err(classify)?;
    for row in &rows {
        let facts = delivery_facts(row)?;
        let event_id: Uuid = row.try_get("event_id").map_err(classify)?;
        match decide_claim(facts, policy, micros(now)) {
            ClaimDecision::NotEligible => {}
            ClaimDecision::Claim {
                generation,
                attempts,
                lease_until_micros,
                reclaimed,
            } => {
                sqlx::query(
                    "UPDATE notification_deliveries
                        SET state = 'leased', generation = $2, attempts = $3, available_at = $4,
                            expired_leases = expired_leases + $5,
                            last_error = CASE WHEN $5 = 1 THEN 'lease_expired' ELSE last_error END,
                            updated_at = $6
                      WHERE event_id = $1",
                )
                .bind(event_id)
                .bind(generation.get())
                .bind(i32::try_from(attempts).map_err(|_| integrity("attempts", attempts))?)
                .bind(instant(lease_until_micros)?)
                .bind(i32::from(reclaimed))
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
                batch.reclaimed += usize::from(reclaimed);
                batch.claimed.push(ClaimedNotification {
                    event_id,
                    envelope: row.try_get("envelope").map_err(classify)?,
                    payload_digest: row.try_get("payload_digest").map_err(classify)?,
                    generation,
                    attempts,
                    reclaimed,
                });
            }
            ClaimDecision::DeadLetter { reason, reclaimed } => {
                sqlx::query(
                    "UPDATE notification_deliveries
                        SET state = 'dead', outcome = $2, finished_at = $4, updated_at = $4,
                            expired_leases = expired_leases + $3,
                            last_error = CASE WHEN $3 = 1 THEN 'lease_expired' ELSE last_error END
                      WHERE event_id = $1",
                )
                .bind(event_id)
                .bind(reason.code())
                .bind(i32::from(reclaimed))
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
                batch.reclaimed += usize::from(reclaimed);
                batch.dead_lettered += 1;
                tracing::warn!(
                    reason = reason.code(),
                    "notification delivery dead-lettered at claim"
                );
            }
        }
    }
    tx.commit().await.map_err(classify)?;
    Ok(batch)
}

/// Records what the receiver answered for `claimed` (or how the attempt failed), fenced by the
/// claim generation under the delivery's row lock.
pub async fn record(
    pool: &PgPool,
    claimed: &ClaimedNotification,
    result: RelayResult,
    policy: OutboxPolicy,
    jitter_micros: i64,
) -> Result<RelayCompletion, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    bounded(&mut tx).await?;
    let row = sqlx::query(
        "SELECT state, attempts, generation, available_at, deadline_from, now() AS now
           FROM notification_deliveries WHERE event_id = $1 FOR UPDATE",
    )
    .bind(claimed.event_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(classify)?;
    let Some(row) = row else {
        return Ok(RelayCompletion::Stale(StaleReason::NotLeased));
    };
    let facts = delivery_facts(&row)?;
    let now: OffsetDateTime = row.try_get("now").map_err(classify)?;
    let completion = decide_relay_completion(
        facts,
        claimed.generation,
        result,
        policy,
        micros(now),
        jitter_micros,
    );
    let updated = match completion {
        RelayCompletion::Stale(_) => {
            tx.rollback().await.map_err(classify)?;
            return Ok(completion);
        }
        RelayCompletion::Retry {
            available_at_micros,
            error,
        } => sqlx::query(
            "UPDATE notification_deliveries
                SET state = 'ready', available_at = $3, last_error = $4, updated_at = $5
              WHERE event_id = $1 AND generation = $2 AND state = 'leased'",
        )
        .bind(claimed.event_id)
        .bind(claimed.generation.get())
        .bind(instant(available_at_micros)?)
        .bind(error)
        .bind(now),
        terminal => sqlx::query(
            "UPDATE notification_deliveries
                SET state = $3, outcome = $4, finished_at = $5, updated_at = $5
              WHERE event_id = $1 AND generation = $2 AND state = 'leased'",
        )
        .bind(claimed.event_id)
        .bind(claimed.generation.get())
        .bind(terminal.state().unwrap_or("leased"))
        .bind(terminal.outcome())
        .bind(now),
    }
    .execute(&mut *tx)
    .await
    .map_err(classify)?;
    if updated.rows_affected() != 1 {
        return Err(integrity(
            "fenced notification delivery update",
            updated.rows_affected(),
        ));
    }
    tx.commit().await.map_err(classify)?;
    Ok(completion)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequeueOutcome {
    Requeued { previous_attempts: i32 },
    Refused(DeliveryState),
    NotFound,
}

/// Operator requeue of a dead letter: ready now with a fresh budget and deadline; generation
/// and event kept; audited in the same transaction.
pub async fn requeue(
    pool: &PgPool,
    event_id: Uuid,
    note: &str,
) -> Result<RequeueOutcome, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let row: Option<(String, Option<String>, i32)> = sqlx::query_as(
        "SELECT state, outcome, attempts FROM notification_deliveries
          WHERE event_id = $1 FOR UPDATE",
    )
    .bind(event_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(classify)?;
    let Some((raw_state, outcome, attempts)) = row else {
        return Ok(RequeueOutcome::NotFound);
    };
    let current = DeliveryState::from_code(&raw_state)
        .ok_or_else(|| integrity("stored notification delivery state", &raw_state))?;
    match decide_requeue(current) {
        Requeue::Refused(state) => Ok(RequeueOutcome::Refused(state)),
        Requeue::Requeued => {
            reopen(
                &mut tx,
                event_id,
                "requeue",
                outcome.as_deref(),
                attempts,
                note,
            )
            .await?;
            tx.commit().await.map_err(classify)?;
            Ok(RequeueOutcome::Requeued {
                previous_attempts: attempts,
            })
        }
    }
}

async fn reopen(
    tx: &mut Transaction<'_, Postgres>,
    event_id: Uuid,
    kind: &str,
    outcome: Option<&str>,
    attempts: i32,
    note: &str,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE notification_deliveries
            SET state = 'ready', attempts = 0, available_at = now(), deadline_from = now(),
                last_error = outcome, outcome = NULL, finished_at = NULL, updated_at = now()
          WHERE event_id = $1",
    )
    .bind(event_id)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    sqlx::query(
        "INSERT INTO notification_delivery_requeues
                (event_id, kind, previous_outcome, previous_attempts, note)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(event_id)
    .bind(kind)
    .bind(outcome.unwrap_or("unknown"))
    .bind(attempts)
    .bind(note)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}

/// Operator replay after a Notification restore (contract §1.3 step 5): every acknowledged
/// event created at or after `since` is owed again, re-sent with its stored envelope and digest
/// (the receiver's receipt makes an effect it still holds `already_applied`). Bounded by event
/// retention, which is the published replay horizon. Audited per event.
pub async fn replay(
    pool: &PgPool,
    since: OffsetDateTime,
    note: &str,
    limit: i64,
) -> Result<u64, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let rows: Vec<(Uuid, Option<String>, i32)> = sqlx::query_as(
        "SELECT d.event_id, d.outcome, d.attempts
           FROM notification_deliveries d JOIN notification_events e ON e.id = d.event_id
          WHERE d.state = 'succeeded' AND e.created_at >= $1
          ORDER BY e.created_at, e.id
          LIMIT $2
            FOR UPDATE OF d",
    )
    .bind(since)
    .bind(limit)
    .fetch_all(&mut *tx)
    .await
    .map_err(classify)?;
    for (event_id, outcome, attempts) in &rows {
        reopen(
            &mut tx,
            *event_id,
            "replay",
            outcome.as_deref(),
            *attempts,
            note,
        )
        .await?;
    }
    tx.commit().await.map_err(classify)?;
    Ok(rows.len() as u64)
}

/// Queue health: counts and ages only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NotificationBacklog {
    pub eligible: i64,
    pub scheduled: i64,
    pub leased: i64,
    pub dead: i64,
    pub oldest_eligible_seconds: Option<i64>,
}

pub async fn backlog(pool: &PgPool) -> Result<NotificationBacklog, StoreError> {
    let row = sqlx::query(
        "SELECT
             count(*) FILTER (WHERE state IN ('ready', 'leased') AND available_at <= now())
                 AS eligible,
             count(*) FILTER (WHERE state = 'ready' AND available_at > now()) AS scheduled,
             count(*) FILTER (WHERE state = 'leased' AND available_at > now()) AS leased,
             count(*) FILTER (WHERE state = 'dead') AS dead,
             EXTRACT(EPOCH FROM now() - min(available_at) FILTER (
                 WHERE state IN ('ready', 'leased') AND available_at <= now()))::bigint
                 AS oldest_eligible
           FROM notification_deliveries
          WHERE state IN ('ready', 'leased', 'dead')",
    )
    .fetch_one(pool)
    .await
    .map_err(classify)?;
    let count = |name: &str| row.try_get::<i64, _>(name).map_err(classify);
    Ok(NotificationBacklog {
        eligible: count("eligible")?,
        scheduled: count("scheduled")?,
        leased: count("leased")?,
        dead: count("dead")?,
        oldest_eligible_seconds: row.try_get("oldest_eligible").map_err(classify)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationDeadLetter {
    pub event_id: Uuid,
    pub event_type: String,
    pub outcome: String,
    pub attempts: i32,
    pub finished_at: OffsetDateTime,
}

pub async fn dead_letters(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<NotificationDeadLetter>, StoreError> {
    let rows = sqlx::query(
        "SELECT d.event_id, e.event_type, d.outcome, d.attempts, d.finished_at
           FROM notification_deliveries d JOIN notification_events e ON e.id = d.event_id
          WHERE d.state = 'dead'
          ORDER BY d.finished_at, d.event_id
          LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(classify)?;
    rows.iter()
        .map(|row| {
            Ok(NotificationDeadLetter {
                event_id: row.try_get("event_id")?,
                event_type: row.try_get("event_type")?,
                outcome: row.try_get("outcome")?,
                attempts: row.try_get("attempts")?,
                finished_at: row.try_get("finished_at")?,
            })
        })
        .collect::<Result<_, sqlx::Error>>()
        .map_err(classify)
}

/// Deletes up to `limit` acknowledged events whose acknowledgement is older than
/// `retain_days` (the replay horizon). Ready, leased and dead deliveries keep their event.
pub async fn prune(pool: &PgPool, retain_days: i32, limit: i64) -> Result<u64, StoreError> {
    let deleted = sqlx::query(
        "WITH doomed AS (
             SELECT e.id FROM notification_events e
               JOIN notification_deliveries d ON d.event_id = e.id
              WHERE d.state = 'succeeded'
                AND d.finished_at < now() - make_interval(days => $1)
                AND e.created_at < now() - make_interval(days => $1)
              ORDER BY e.created_at
              LIMIT $2
                FOR UPDATE OF e, d SKIP LOCKED)
         DELETE FROM notification_events e USING doomed WHERE e.id = doomed.id",
    )
    .bind(retain_days)
    .bind(limit)
    .execute(pool)
    .await
    .map_err(classify)?;
    Ok(deleted.rows_affected())
}

pub async fn activation(pool: &PgPool) -> Result<Activation, StoreError> {
    let (active, epoch): (bool, i64) =
        sqlx::query_as("SELECT active, epoch FROM notification_activation")
            .fetch_one(pool)
            .await
            .map_err(classify)?;
    activation_of(active, epoch)
}

/// `vink-app notifications activate|deactivate`: one audited cutover, serialized with every
/// producer through the activation row.
pub async fn set_activation(
    pool: &PgPool,
    active: bool,
    note: &str,
) -> Result<ActivationChange, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let (stored_active, epoch): (bool, i64) =
        sqlx::query_as("SELECT active, epoch FROM notification_activation FOR UPDATE")
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?;
    let change = activation_of(stored_active, epoch)?.change(active);
    if let ActivationChange::Changed(next) = change {
        sqlx::query(
            "UPDATE notification_activation SET active = $1, epoch = $2, changed_at = now()",
        )
        .bind(next.active())
        .bind(next.epoch())
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
        sqlx::query(
            "INSERT INTO notification_activation_changes (active, epoch, note) VALUES ($1, $2, $3)",
        )
        .bind(next.active())
        .bind(next.epoch())
        .bind(note)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
    }
    Ok(change)
}
