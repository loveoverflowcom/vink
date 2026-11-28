//! PostgreSQL adapter of the Điểm Vink projection (#646; contract K4..K7,
//! `migrations/20261209000000_reputation_projection.sql`). Every effect comes from
//! [`crate::domain::reputation::entitlement::apply`]; this module loads as-of facts, writes the
//! returned record, current count and ledger delta, and never decides a rule itself.
//!
//! **Consumer ([`consume_batch`]).** One batch at a time (transaction advisory lock), events in id
//! order, each applied and deleted in the same transaction. For conflicting source transactions
//! id order is commit order (#645), so applying in id order replays a valid serialization.
//! Community events carry the context their transaction read; markers carry the facts as of their
//! transaction. Maturity is applied as of the threshold before a later event of the same actor,
//! and a bounded scan matures held actors without new events.
//!
//! **Backfill ([`backfill`]).** One REPEATABLE READ snapshot of the sources, taken while producers
//! are active: writes facts, anchors, entitlements and profiles, deletes the events that snapshot
//! already reflects and sets readiness with each calendar's first full period (Q33).
//!
//! **Operator ([`reconcile`], [`correct`]).** Reconcile recounts profiles from entitlements.
//! Correction (Q38) retires entitlements in a scope fixed at apply time and restates their ledger
//! history; source counts never change.
//!
//! **Privacy.** Identifiers, counters and states only. Logs carry counts.

use sqlx::postgres::PgPool;
use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::discussion::integrity;
use super::postgres::classify;
use crate::application::ports::StoreError;
use crate::domain::reputation::calendar::{
    Calendar, PeriodIndex, first_full_period, is_mature, maturity_threshold, period_of,
};
use crate::domain::reputation::credit::{CreditRecord, CreditState, CreditStep};
use crate::domain::reputation::entitlement::{
    Cause, Component, EntitlementFacts, EntitlementRecord, Periods, apply,
};
use crate::domain::reputation::formula::{ComponentCount, CurrentDelta, FormulaVersion, score_v1};

/// Seed of the advisory lock that serializes consumer batches, backfill and operator commands.
const PROJECTION_LOCK: i64 = 6_000_646;
/// Held actors matured per batch by the scan.
const MATURITY_BATCH: i64 = 100;

fn component_of(raw: &str) -> Result<Component, StoreError> {
    match raw {
        "impact" => Ok(Component::Impact),
        "contribution" => Ok(Component::Contribution),
        other => Err(integrity("stored reputation component", other)),
    }
}

async fn lock(tx: &mut Transaction<'_, Postgres>) -> Result<(), StoreError> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(PROJECTION_LOCK)
        .execute(&mut **tx)
        .await
        .map_err(classify)?;
    Ok(())
}

/// Each calendar's first full period, once the backfill committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Readiness {
    pub ready_at: OffsetDateTime,
    pub week_start: PeriodIndex,
    pub month_start: PeriodIndex,
}

async fn readiness(tx: &mut Transaction<'_, Postgres>) -> Result<Option<Readiness>, StoreError> {
    let row: (Option<OffsetDateTime>, Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT ready_at, week_start, month_start FROM reputation_readiness")
            .fetch_one(&mut **tx)
            .await
            .map_err(classify)?;
    Ok(match row {
        (Some(ready_at), Some(week), Some(month)) => Some(Readiness {
            ready_at,
            week_start: PeriodIndex::new(week),
            month_start: PeriodIndex::new(month),
        }),
        (None, None, None) => None,
        _ => return Err(integrity("partial reputation readiness", "readiness")),
    })
}

fn periods(at: OffsetDateTime, ready: Readiness) -> Periods {
    Periods {
        week: period_of(Calendar::Week, at),
        month: period_of(Calendar::Month, at),
        week_start: ready.week_start,
        month_start: ready.month_start,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    component: Component,
    actor: Uuid,
    target: Uuid,
}

#[derive(sqlx::FromRow)]
struct EntitlementRow {
    recipient_id: Uuid,
    work_id: Uuid,
    anchor_selected: bool,
    anchor_order: i64,
    effective: bool,
    terminal: bool,
    week_state: String,
    week_pending: Option<i64>,
    week_last: Option<i64>,
    month_state: String,
    month_pending: Option<i64>,
    month_last: Option<i64>,
}

fn credit(code: &str, pending: Option<i64>, last: Option<i64>) -> Result<CreditRecord, StoreError> {
    let state = CreditState::restore(code, pending.map(PeriodIndex::new))
        .ok_or_else(|| integrity("stored reputation credit state", code))?;
    Ok(CreditRecord::restore(state, last.map(PeriodIndex::new)))
}

impl EntitlementRow {
    fn record(&self) -> Result<EntitlementRecord, StoreError> {
        EntitlementRecord::restore(
            self.effective,
            self.terminal,
            credit(&self.week_state, self.week_pending, self.week_last)?,
            credit(&self.month_state, self.month_pending, self.month_last)?,
        )
        .ok_or_else(|| integrity("stored reputation entitlement", self.recipient_id))
    }
}

const ENTITLEMENT_COLUMNS: &str = "recipient_id, work_id, anchor_selected, anchor_order, \
     effective, terminal, week_state, week_pending, week_last, month_state, month_pending, \
     month_last";

async fn load(
    tx: &mut Transaction<'_, Postgres>,
    key: Key,
) -> Result<Option<EntitlementRow>, StoreError> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ENTITLEMENT_COLUMNS} FROM reputation_entitlements
          WHERE component = $1 AND actor_id = $2 AND target_id = $3 FOR UPDATE"
    )))
    .bind(key.component.as_str())
    .bind(key.actor)
    .bind(key.target)
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)
}

/// Facts of one entitlement from the as-of tables. `anchor_selected` is the Ưng selection.
async fn facts(
    tx: &mut Transaction<'_, Postgres>,
    key: Key,
    anchor_selected: bool,
) -> Result<EntitlementFacts, StoreError> {
    let (selected, valid, public, mature): (i64, i64, bool, bool) = match key.component {
        Component::Impact => sqlx::query_as(
            "SELECT
               (SELECT count(*) FROM reputation_anchors a
                 WHERE a.actor_id = $1 AND a.work_id = $2 AND a.selected),
               (SELECT count(*) FROM reputation_anchors a
                  LEFT JOIN reputation_work_facts w ON w.work_id = a.work_id
                 WHERE a.actor_id = $1 AND a.work_id = $2 AND a.selected
                   AND a.revision_id IS DISTINCT FROM w.hidden_revision_id),
               coalesce((SELECT work_public FROM reputation_work_facts WHERE work_id = $2), false),
               coalesce((SELECT matured_at IS NOT NULL FROM reputation_principal_facts
                          WHERE principal_id = $1), false)",
        )
        .bind(key.actor)
        .bind(key.target)
        .fetch_one(&mut **tx)
        .await
        .map_err(classify)?,
        Component::Contribution => {
            let (public, mature): (bool, bool) = sqlx::query_as(
                "SELECT
                   coalesce((SELECT w.work_public AND c.comment_public
                               FROM reputation_comment_facts c
                               JOIN reputation_work_facts w ON w.work_id = c.work_id
                              WHERE c.comment_id = $2), false),
                   coalesce((SELECT matured_at IS NOT NULL FROM reputation_principal_facts
                              WHERE principal_id = $1), false)",
            )
            .bind(key.actor)
            .bind(key.target)
            .fetch_one(&mut **tx)
            .await
            .map_err(classify)?;
            let n = i64::from(anchor_selected);
            (n, n, public, mature)
        }
    };
    Ok(EntitlementFacts {
        selected_anchors: u32::try_from(selected).unwrap_or(u32::MAX),
        valid_selected_anchors: u32::try_from(valid).unwrap_or(u32::MAX),
        target_public: public,
        actor_mature: mature,
    })
}

/// Applies one change to one entitlement and writes every effect the pure decision returned.
#[allow(clippy::too_many_arguments)]
async fn change(
    tx: &mut Transaction<'_, Postgres>,
    key: Key,
    recipient: Uuid,
    work: Uuid,
    cause: Cause,
    at: OffsetDateTime,
    ready: Readiness,
    anchor: Option<(bool, i64)>,
) -> Result<(), StoreError> {
    let row = load(tx, key).await?;
    let record = row.as_ref().map(EntitlementRow::record).transpose()?;
    let record = record.unwrap_or(EntitlementRecord::NEW);
    let (recipient, work) = row
        .as_ref()
        .map_or((recipient, work), |row| (row.recipient_id, row.work_id));
    let (anchor_selected, anchor_order) = match (anchor, &row) {
        (Some(anchor), _) => anchor,
        (None, Some(row)) => (row.anchor_selected, row.anchor_order),
        (None, None) => (false, 1),
    };
    let mut facts = facts(tx, key, anchor_selected).await?;
    if cause == Cause::Maturity {
        facts.actor_mature = true;
    }
    let applied = apply(record, cause, facts, periods(at, ready));
    let week = applied.record.week();
    let month = applied.record.month();
    sqlx::query(
        "INSERT INTO reputation_entitlements
                (component, actor_id, target_id, work_id, recipient_id, anchor_selected,
                 anchor_order, effective, terminal, week_state, week_pending, week_last,
                 month_state, month_pending, month_last)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
         ON CONFLICT (component, actor_id, target_id) DO UPDATE SET
                anchor_selected = EXCLUDED.anchor_selected, anchor_order = EXCLUDED.anchor_order,
                effective = EXCLUDED.effective, terminal = EXCLUDED.terminal,
                week_state = EXCLUDED.week_state, week_pending = EXCLUDED.week_pending,
                week_last = EXCLUDED.week_last, month_state = EXCLUDED.month_state,
                month_pending = EXCLUDED.month_pending, month_last = EXCLUDED.month_last",
    )
    .bind(key.component.as_str())
    .bind(key.actor)
    .bind(key.target)
    .bind(work)
    .bind(recipient)
    .bind(anchor_selected)
    .bind(anchor_order)
    .bind(applied.record.effective())
    .bind(applied.record.terminal())
    .bind(week.state().code())
    .bind(week.state().pending().map(PeriodIndex::get))
    .bind(week.last().map(PeriodIndex::get))
    .bind(month.state().code())
    .bind(month.state().pending().map(PeriodIndex::get))
    .bind(month.last().map(PeriodIndex::get))
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    bump_profile(tx, recipient, key.component, applied.current).await?;
    for (calendar, step) in [
        (Calendar::Week, applied.week),
        (Calendar::Month, applied.month),
    ] {
        ledger(tx, calendar, step, key, recipient).await?;
    }
    Ok(())
}

async fn ledger(
    tx: &mut Transaction<'_, Postgres>,
    calendar: Calendar,
    step: CreditStep,
    key: Key,
    recipient: Uuid,
) -> Result<(), StoreError> {
    let Some(period) = step.period else {
        return Ok(());
    };
    if step.delta == 0 {
        return Ok(());
    }
    let delta = i16::try_from(step.delta).map_err(|_| integrity("credit delta", step.delta))?;
    sqlx::query(
        "INSERT INTO reputation_ledger
                (calendar, period, component, actor_id, target_id, recipient_id, net)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (calendar, period, component, actor_id, target_id)
         DO UPDATE SET net = reputation_ledger.net + EXCLUDED.net",
    )
    .bind(calendar.as_str())
    .bind(period.get())
    .bind(key.component.as_str())
    .bind(key.actor)
    .bind(key.target)
    .bind(recipient)
    .bind(delta)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}

async fn bump_profile(
    tx: &mut Transaction<'_, Postgres>,
    recipient: Uuid,
    component: Component,
    delta: CurrentDelta,
) -> Result<(), StoreError> {
    if delta == CurrentDelta::None {
        return Ok(());
    }
    let stored: Option<(i64, i64)> = sqlx::query_as(
        "SELECT impact, contribution FROM reputation_profiles WHERE principal_id = $1 FOR UPDATE",
    )
    .bind(recipient)
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)?;
    let (impact, contribution) = stored.unwrap_or((0, 0));
    let count = |raw: i64| {
        ComponentCount::from_stored(raw).map_err(|_| integrity("stored reputation count", raw))
    };
    let (mut impact, mut contribution) = (count(impact)?, count(contribution)?);
    let bump = |value: ComponentCount| {
        value
            .apply(delta)
            .map_err(|_| integrity("reputation count out of bounds", recipient))
    };
    match component {
        Component::Impact => impact = bump(impact)?,
        Component::Contribution => contribution = bump(contribution)?,
    }
    write_profile(tx, recipient, impact, contribution).await
}

async fn write_profile(
    tx: &mut Transaction<'_, Postgres>,
    recipient: Uuid,
    impact: ComponentCount,
    contribution: ComponentCount,
) -> Result<(), StoreError> {
    let score = score_v1(impact, contribution);
    sqlx::query(
        "INSERT INTO reputation_profiles
                (principal_id, impact, contribution, score_micros, formula_version, updated_at)
         VALUES ($1, $2, $3, $4, $5, now())
         ON CONFLICT (principal_id) DO UPDATE SET
                impact = EXCLUDED.impact, contribution = EXCLUDED.contribution,
                score_micros = EXCLUDED.score_micros, formula_version = EXCLUDED.formula_version,
                updated_at = now()",
    )
    .bind(recipient)
    .bind(i64::try_from(impact.get()).unwrap_or(i64::MAX))
    .bind(i64::try_from(contribution.get()).unwrap_or(i64::MAX))
    .bind(score.get())
    .bind(FormulaVersion::CURRENT.get())
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}

/// Applies maturity to `actor` as of its threshold: held entitlements become current-only.
async fn mature(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    created_at: OffsetDateTime,
    ready: Readiness,
) -> Result<(), StoreError> {
    let threshold = maturity_threshold(created_at);
    sqlx::query(
        "INSERT INTO reputation_principal_facts (principal_id, matured_at) VALUES ($1, $2)
         ON CONFLICT (principal_id) DO UPDATE SET matured_at = EXCLUDED.matured_at
          WHERE reputation_principal_facts.matured_at IS NULL",
    )
    .bind(actor)
    .bind(threshold)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    let held: Vec<(String, Uuid, Uuid, Uuid)> = sqlx::query_as(
        "SELECT component, target_id, recipient_id, work_id FROM reputation_entitlements
          WHERE actor_id = $1 AND NOT terminal AND NOT effective
          ORDER BY component, target_id",
    )
    .bind(actor)
    .fetch_all(&mut **tx)
    .await
    .map_err(classify)?;
    for (component, target, recipient, work) in held {
        let key = Key {
            component: component_of(&component)?,
            actor,
            target,
        };
        // As of the threshold, never at processing time (Q42).
        let at = threshold.max(ready.ready_at);
        change(tx, key, recipient, work, Cause::Maturity, at, ready, None).await?;
    }
    Ok(())
}

/// Matures `actor` first when `at` is past its threshold and maturity was not applied yet.
async fn ensure_mature(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    at: OffsetDateTime,
    ready: Readiness,
) -> Result<(), StoreError> {
    let row: Option<(OffsetDateTime, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT p.created_at, f.matured_at FROM principals p
           LEFT JOIN reputation_principal_facts f ON f.principal_id = p.id
          WHERE p.id = $1",
    )
    .bind(actor)
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)?;
    if let Some((created_at, None)) = row
        && is_mature(created_at, at)
    {
        mature(tx, actor, created_at, ready).await?;
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct EventRow {
    id: i64,
    kind: String,
    accepted_at: OffsetDateTime,
    actor_id: Option<Uuid>,
    recipient_id: Option<Uuid>,
    work_id: Option<Uuid>,
    revision_id: Option<Uuid>,
    comment_id: Option<Uuid>,
    principal_id: Option<Uuid>,
    selected: Option<bool>,
    accepted_order: Option<i64>,
    work_public: Option<bool>,
    hidden_revision_id: Option<Uuid>,
    comment_public: Option<bool>,
    principal_suspended: Option<bool>,
    principal_deleted: Option<bool>,
}

fn required<T>(value: Option<T>, what: &'static str, event: i64) -> Result<T, StoreError> {
    value.ok_or_else(|| integrity(what, event))
}

async fn set_work_facts(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    public: bool,
    hidden: Option<Option<Uuid>>,
) -> Result<(), StoreError> {
    match hidden {
        Some(hidden) => sqlx::query(
            "INSERT INTO reputation_work_facts (work_id, work_public, hidden_revision_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (work_id) DO UPDATE SET work_public = EXCLUDED.work_public,
                    hidden_revision_id = EXCLUDED.hidden_revision_id",
        )
        .bind(work)
        .bind(public)
        .bind(hidden),
        // An Ưng event knows the work's visibility, not its hidden revision.
        None => sqlx::query(
            "INSERT INTO reputation_work_facts (work_id, work_public) VALUES ($1, $2)
             ON CONFLICT (work_id) DO UPDATE SET work_public = EXCLUDED.work_public",
        )
        .bind(work)
        .bind(public),
    }
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}

async fn entitlements_where(
    tx: &mut Transaction<'_, Postgres>,
    condition: &'static str,
    subject: Uuid,
) -> Result<Vec<(Key, Uuid, Uuid)>, StoreError> {
    let rows: Vec<(String, Uuid, Uuid, Uuid, Uuid)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT component, actor_id, target_id, recipient_id, work_id
           FROM reputation_entitlements WHERE NOT terminal AND {condition}
          ORDER BY component, actor_id, target_id"
    )))
    .bind(subject)
    .fetch_all(&mut **tx)
    .await
    .map_err(classify)?;
    rows.into_iter()
        .map(|(component, actor, target, recipient, work)| {
            Ok((
                Key {
                    component: component_of(&component)?,
                    actor,
                    target,
                },
                recipient,
                work,
            ))
        })
        .collect()
}

async fn apply_event(
    tx: &mut Transaction<'_, Postgres>,
    event: &EventRow,
    ready: Readiness,
) -> Result<(), StoreError> {
    let id = event.id;
    let at = event.accepted_at;
    match event.kind.as_str() {
        "tham" | "ung" => {
            let actor = required(event.actor_id, "event actor", id)?;
            let recipient = required(event.recipient_id, "event recipient", id)?;
            let work = required(event.work_id, "event work", id)?;
            let selected = required(event.selected, "event selection", id)?;
            let order = required(event.accepted_order, "event order", id)?;
            let public = required(event.work_public, "event work context", id)?;
            ensure_mature(tx, actor, at, ready).await?;
            if event.kind == "tham" {
                set_work_facts(tx, work, public, Some(event.hidden_revision_id)).await?;
                let revision = required(event.revision_id, "event revision", id)?;
                let stored: Option<i64> = sqlx::query_scalar(
                    "SELECT accepted_order FROM reputation_anchors
                      WHERE actor_id = $1 AND work_id = $2 AND revision_id = $3 FOR UPDATE",
                )
                .bind(actor)
                .bind(work)
                .bind(revision)
                .fetch_optional(&mut **tx)
                .await
                .map_err(classify)?;
                if stored.is_some_and(|stored| stored >= order) {
                    return Ok(()); // Already reflected (backfill or replay).
                }
                sqlx::query(
                    "INSERT INTO reputation_anchors
                            (actor_id, work_id, revision_id, selected, accepted_order)
                     VALUES ($1, $2, $3, $4, $5)
                     ON CONFLICT (actor_id, work_id, revision_id) DO UPDATE SET
                            selected = EXCLUDED.selected, accepted_order = EXCLUDED.accepted_order",
                )
                .bind(actor)
                .bind(work)
                .bind(revision)
                .bind(selected)
                .bind(order)
                .execute(&mut **tx)
                .await
                .map_err(classify)?;
                let key = Key {
                    component: Component::Impact,
                    actor,
                    target: work,
                };
                change(tx, key, recipient, work, Cause::Community, at, ready, None).await
            } else {
                set_work_facts(tx, work, public, None).await?;
                let comment = required(event.comment_id, "event comment", id)?;
                let comment_public = required(event.comment_public, "event comment context", id)?;
                sqlx::query(
                    "INSERT INTO reputation_comment_facts (comment_id, work_id, comment_public)
                     VALUES ($1, $2, $3)
                     ON CONFLICT (comment_id) DO UPDATE SET comment_public = EXCLUDED.comment_public",
                )
                .bind(comment)
                .bind(work)
                .bind(comment_public)
                .execute(&mut **tx)
                .await
                .map_err(classify)?;
                let key = Key {
                    component: Component::Contribution,
                    actor,
                    target: comment,
                };
                if load(tx, key)
                    .await?
                    .is_some_and(|row| row.anchor_order >= order)
                {
                    return Ok(());
                }
                change(
                    tx,
                    key,
                    recipient,
                    work,
                    Cause::Community,
                    at,
                    ready,
                    Some((selected, order)),
                )
                .await
            }
        }
        "work" => {
            let work = required(event.work_id, "marker work", id)?;
            let public = required(event.work_public, "marker work facts", id)?;
            set_work_facts(tx, work, public, Some(event.hidden_revision_id)).await?;
            for (key, recipient, work) in entitlements_where(tx, "work_id = $1", work).await? {
                change(tx, key, recipient, work, Cause::Neutral, at, ready, None).await?;
            }
            Ok(())
        }
        "comment" => {
            let work = required(event.work_id, "marker work", id)?;
            let comment = required(event.comment_id, "marker comment", id)?;
            let public = required(event.comment_public, "marker comment facts", id)?;
            sqlx::query(
                "INSERT INTO reputation_comment_facts (comment_id, work_id, comment_public)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (comment_id) DO UPDATE SET comment_public = EXCLUDED.comment_public",
            )
            .bind(comment)
            .bind(work)
            .bind(public)
            .execute(&mut **tx)
            .await
            .map_err(classify)?;
            for (key, recipient, work) in
                entitlements_where(tx, "component = 'contribution' AND target_id = $1", comment)
                    .await?
            {
                change(tx, key, recipient, work, Cause::Neutral, at, ready, None).await?;
            }
            Ok(())
        }
        "principal" => {
            let principal = required(event.principal_id, "marker principal", id)?;
            let suspended = required(event.principal_suspended, "marker standing", id)?;
            let deleted = required(event.principal_deleted, "marker deletion", id)?;
            let was_deleted: bool = sqlx::query_scalar(
                "SELECT coalesce((SELECT deleted FROM reputation_principal_facts
                                   WHERE principal_id = $1), false)",
            )
            .bind(principal)
            .fetch_one(&mut **tx)
            .await
            .map_err(classify)?;
            sqlx::query(
                "INSERT INTO reputation_principal_facts (principal_id, suspended, deleted)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (principal_id) DO UPDATE SET suspended = EXCLUDED.suspended,
                        deleted = reputation_principal_facts.deleted OR EXCLUDED.deleted",
            )
            .bind(principal)
            .bind(suspended)
            .bind(deleted)
            .execute(&mut **tx)
            .await
            .map_err(classify)?;
            if deleted && !was_deleted {
                retire_deleted(tx, principal, at, ready).await?;
            }
            Ok(())
        }
        other => Err(integrity("unknown reputation event kind", other)),
    }
}

/// DEL-05 (Q20): the actor's entitlements retire without a debit; Q21: the recipient's
/// projection and ledger are purged and nothing of theirs is counted again.
async fn retire_deleted(
    tx: &mut Transaction<'_, Postgres>,
    principal: Uuid,
    at: OffsetDateTime,
    ready: Readiness,
) -> Result<(), StoreError> {
    for (key, recipient, work) in entitlements_where(tx, "actor_id = $1", principal).await? {
        change(tx, key, recipient, work, Cause::Terminal, at, ready, None).await?;
    }
    sqlx::query("DELETE FROM reputation_ledger WHERE recipient_id = $1")
        .bind(principal)
        .execute(&mut **tx)
        .await
        .map_err(classify)?;
    sqlx::query(
        "UPDATE reputation_entitlements SET terminal = true, effective = false
          WHERE recipient_id = $1",
    )
    .bind(principal)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    sqlx::query("DELETE FROM reputation_profiles WHERE principal_id = $1")
        .bind(principal)
        .execute(&mut **tx)
        .await
        .map_err(classify)?;
    Ok(())
}

/// What one consumer pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BatchReport {
    pub ready: bool,
    pub events: usize,
    pub matured: usize,
}

/// One consumer pass: matures due actors, applies up to `limit` events in id order and deletes
/// them, all in one transaction.
pub async fn consume_batch(pool: &PgPool, limit: i64) -> Result<BatchReport, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    sqlx::query("SET LOCAL lock_timeout = '5s'")
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    lock(&mut tx).await?;
    let Some(ready) = readiness(&mut tx).await? else {
        tx.commit().await.map_err(classify)?;
        return Ok(BatchReport::default());
    };
    let events: Vec<EventRow> = sqlx::query_as(
        "SELECT id, kind, accepted_at, actor_id, recipient_id, work_id, revision_id, comment_id,
                principal_id, selected, accepted_order, work_public, hidden_revision_id,
                comment_public, principal_suspended, principal_deleted
           FROM reputation_events ORDER BY id LIMIT $1 FOR UPDATE",
    )
    .bind(limit)
    .fetch_all(&mut *tx)
    .await
    .map_err(classify)?;
    let horizon: OffsetDateTime = match events.first() {
        Some(event) => event.accepted_at,
        None => sqlx::query_scalar("SELECT now()")
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?,
    };
    let due: Vec<(Uuid, OffsetDateTime)> = sqlx::query_as(
        "SELECT DISTINCT e.actor_id, p.created_at FROM reputation_entitlements e
           JOIN principals p ON p.id = e.actor_id
           LEFT JOIN reputation_principal_facts f ON f.principal_id = e.actor_id
          WHERE NOT e.terminal AND NOT e.effective AND f.matured_at IS NULL
            AND p.created_at + interval '168 hours' <= $1
          ORDER BY e.actor_id LIMIT $2",
    )
    .bind(horizon)
    .bind(MATURITY_BATCH)
    .fetch_all(&mut *tx)
    .await
    .map_err(classify)?;
    for (actor, created_at) in &due {
        mature(&mut tx, *actor, *created_at, ready).await?;
    }
    let ids: Vec<i64> = events.iter().map(|event| event.id).collect();
    for event in &events {
        apply_event(&mut tx, event, ready).await?;
    }
    if !ids.is_empty() {
        sqlx::query("DELETE FROM reputation_events WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
    }
    tx.commit().await.map_err(classify)?;
    Ok(BatchReport {
        ready: true,
        events: ids.len(),
        matured: due.len(),
    })
}

/// Why a backfill did not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackfillRefusal {
    /// Producers are off: the snapshot would drift from later changes.
    NotActive,
    AlreadyReady,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfillReport {
    pub ready_at: OffsetDateTime,
    pub week_start: PeriodIndex,
    pub month_start: PeriodIndex,
    pub entitlements: usize,
    pub effective: usize,
    pub profiles: usize,
    pub reflected_events: u64,
    pub applied: bool,
}

#[derive(sqlx::FromRow)]
struct BackfillRow {
    component: String,
    actor_id: Uuid,
    target_id: Uuid,
    work_id: Uuid,
    recipient_id: Uuid,
    selected: i64,
    valid: i64,
    anchor_order: i64,
    public: bool,
    mature: bool,
    actor_deleted: bool,
    recipient_deleted: bool,
}

/// Q18/Q32/Q33: the projection from one consistent snapshot of current facts. `apply = false`
/// computes the same report and rolls back.
pub async fn backfill(
    pool: &PgPool,
    apply_it: bool,
) -> Result<Result<BackfillReport, BackfillRefusal>, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    lock(&mut tx).await?;
    let active: bool = sqlx::query_scalar("SELECT active FROM reputation_activation")
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
    if !active {
        return Ok(Err(BackfillRefusal::NotActive));
    }
    if readiness(&mut tx).await?.is_some() {
        return Ok(Err(BackfillRefusal::AlreadyReady));
    }
    let ready_at: OffsetDateTime = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
    let ready = Readiness {
        ready_at,
        week_start: first_full_period(Calendar::Week, ready_at),
        month_start: first_full_period(Calendar::Month, ready_at),
    };
    for statement in [
        "INSERT INTO reputation_work_facts (work_id, work_public, hidden_revision_id)
         SELECT w.id, EXISTS (SELECT 1 FROM publications p WHERE p.work_id = w.id),
                wp.hidden_revision_id
           FROM works w LEFT JOIN work_publications wp ON wp.work_id = w.id",
        "INSERT INTO reputation_comment_facts (comment_id, work_id, comment_public)
         SELECT c.id, t.work_id, c.review = 'approved' AND c.removal IS NULL
           FROM comments c JOIN discussion_threads t ON t.id = c.thread_id
          WHERE EXISTS (SELECT 1 FROM comment_endorsements e WHERE e.comment_id = c.id)",
        "INSERT INTO reputation_anchors (actor_id, work_id, revision_id, selected, accepted_order)
         SELECT actor_id, work_id, revision_id, selected, accepted_order FROM work_endorsements",
    ] {
        sqlx::query(statement)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
    }
    sqlx::query(
        "INSERT INTO reputation_principal_facts (principal_id, matured_at, suspended, deleted)
         SELECT p.id,
                CASE WHEN p.created_at + interval '168 hours' <= $1
                     THEN p.created_at + interval '168 hours' END,
                EXISTS (SELECT 1 FROM principal_suspensions s
                         WHERE s.principal_id = p.id AND s.lifted_at IS NULL),
                EXISTS (SELECT 1 FROM account_deletions d WHERE d.principal_id = p.id)
           FROM principals p",
    )
    .bind(ready_at)
    .execute(&mut *tx)
    .await
    .map_err(classify)?;
    let rows: Vec<BackfillRow> = sqlx::query_as(
        "SELECT 'impact' AS component, e.actor_id, e.work_id AS target_id, e.work_id,
                w.owner_id AS recipient_id,
                count(*) FILTER (WHERE e.selected) AS selected,
                count(*) FILTER (WHERE e.selected
                                   AND e.revision_id IS DISTINCT FROM f.hidden_revision_id) AS valid,
                1::bigint AS anchor_order,
                bool_or(f.work_public) AS public,
                bool_or(pa.matured_at IS NOT NULL) AS mature,
                bool_or(pa.deleted) AS actor_deleted,
                bool_or(pr.deleted) AS recipient_deleted
           FROM work_endorsements e
           JOIN works w ON w.id = e.work_id
           JOIN reputation_work_facts f ON f.work_id = e.work_id
           JOIN reputation_principal_facts pa ON pa.principal_id = e.actor_id
           JOIN reputation_principal_facts pr ON pr.principal_id = w.owner_id
          GROUP BY e.actor_id, e.work_id, w.owner_id
         UNION ALL
         SELECT 'contribution', e.actor_id, e.comment_id, t.work_id, c.author_id,
                CASE WHEN e.selected THEN 1 ELSE 0 END,
                CASE WHEN e.selected THEN 1 ELSE 0 END,
                e.accepted_order,
                coalesce(f.work_public, false) AND cf.comment_public,
                pa.matured_at IS NOT NULL, pa.deleted, pr.deleted
           FROM comment_endorsements e
           JOIN comments c ON c.id = e.comment_id
           JOIN discussion_threads t ON t.id = c.thread_id
           JOIN reputation_comment_facts cf ON cf.comment_id = e.comment_id
           LEFT JOIN reputation_work_facts f ON f.work_id = t.work_id
           JOIN reputation_principal_facts pa ON pa.principal_id = e.actor_id
           JOIN reputation_principal_facts pr ON pr.principal_id = c.author_id",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(classify)?;
    let mut counts: std::collections::BTreeMap<Uuid, (u64, u64)> = Default::default();
    let mut entitlements = 0;
    let mut effective_total = 0;
    for row in rows {
        if row.recipient_deleted {
            continue;
        }
        let component = component_of(&row.component)?;
        let facts = EntitlementFacts {
            selected_anchors: u32::try_from(row.selected).unwrap_or(u32::MAX),
            valid_selected_anchors: u32::try_from(row.valid).unwrap_or(u32::MAX),
            target_public: row.public,
            actor_mature: row.mature,
        };
        let terminal = row.actor_deleted;
        let effective = !terminal && facts.effective();
        let anchor_selected = component == Component::Contribution && row.selected > 0;
        sqlx::query(
            "INSERT INTO reputation_entitlements
                    (component, actor_id, target_id, work_id, recipient_id, anchor_selected,
                     anchor_order, effective, terminal, week_state, month_state)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'unstarted', 'unstarted')",
        )
        .bind(component.as_str())
        .bind(row.actor_id)
        .bind(row.target_id)
        .bind(row.work_id)
        .bind(row.recipient_id)
        .bind(anchor_selected)
        .bind(row.anchor_order.max(1))
        .bind(effective)
        .bind(terminal)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
        entitlements += 1;
        if effective {
            effective_total += 1;
            let entry = counts.entry(row.recipient_id).or_default();
            match component {
                Component::Impact => entry.0 += 1,
                Component::Contribution => entry.1 += 1,
            }
        }
    }
    for (recipient, (impact, contribution)) in &counts {
        let impact = ComponentCount::new(*impact)
            .map_err(|_| integrity("backfill impact out of bounds", recipient))?;
        let contribution = ComponentCount::new(*contribution)
            .map_err(|_| integrity("backfill contribution out of bounds", recipient))?;
        write_profile(&mut tx, *recipient, impact, contribution).await?;
    }
    let reflected = sqlx::query("DELETE FROM reputation_events")
        .execute(&mut *tx)
        .await
        .map_err(classify)?
        .rows_affected();
    sqlx::query("UPDATE reputation_readiness SET ready_at = $1, week_start = $2, month_start = $3")
        .bind(ready.ready_at)
        .bind(ready.week_start.get())
        .bind(ready.month_start.get())
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    let report = BackfillReport {
        ready_at,
        week_start: ready.week_start,
        month_start: ready.month_start,
        entitlements,
        effective: effective_total,
        profiles: counts.len(),
        reflected_events: reflected,
        applied: apply_it,
    };
    if apply_it {
        tx.commit().await.map_err(classify)?;
    } else {
        tx.rollback().await.map_err(classify)?;
    }
    Ok(Ok(report))
}

/// One recipient whose stored projection differs from its entitlements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    pub recipient: Uuid,
    pub stored: (i64, i64, i64),
    pub expected: (i64, i64, i64),
}

/// Expected (impact, contribution) and the stored (impact, contribution, score) of a recipient.
type RecountRow = (Uuid, i64, i64, Option<i64>, Option<i64>, Option<i64>);

/// Recounts every profile from the effective entitlements; `fix` rewrites the drifted rows.
pub async fn reconcile(pool: &PgPool, fix: bool) -> Result<Vec<Drift>, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    lock(&mut tx).await?;
    let rows: Vec<RecountRow> = sqlx::query_as(
        "WITH expected AS (
             SELECT recipient_id AS principal_id,
                    count(*) FILTER (WHERE component = 'impact' AND effective) AS impact,
                    count(*) FILTER (WHERE component = 'contribution' AND effective) AS contribution
               FROM reputation_entitlements WHERE NOT terminal GROUP BY recipient_id
         )
         SELECT coalesce(x.principal_id, p.principal_id), coalesce(x.impact, 0),
                coalesce(x.contribution, 0), p.impact, p.contribution, p.score_micros
           FROM expected x FULL JOIN reputation_profiles p ON p.principal_id = x.principal_id",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(classify)?;
    let mut drift = Vec::new();
    for (recipient, impact, contribution, stored_i, stored_c, stored_score) in rows {
        let count = |raw: i64| {
            ComponentCount::from_stored(raw).map_err(|_| integrity("recount bounds", raw))
        };
        let (i, c) = (count(impact)?, count(contribution)?);
        let score = score_v1(i, c).get();
        let stored = (
            stored_i.unwrap_or(0),
            stored_c.unwrap_or(0),
            stored_score.unwrap_or(score_v1(ComponentCount::ZERO, ComponentCount::ZERO).get()),
        );
        let expected = (impact, contribution, score);
        if stored != expected {
            if fix {
                write_profile(&mut tx, recipient, i, c).await?;
            }
            drift.push(Drift {
                recipient,
                stored,
                expected,
            });
        }
    }
    tx.commit().await.map_err(classify)?;
    Ok(drift)
}

/// Q38 correction scope, fixed at apply time (never a ban on future interactions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionScope {
    Entitlement {
        component: Component,
        actor: Uuid,
        target: Uuid,
    },
    Pair {
        actor: Uuid,
        recipient: Uuid,
    },
}

impl CorrectionScope {
    /// `impact:<actor>:<work>`, `contribution:<actor>:<comment>` or `pair:<actor>:<recipient>`.
    pub fn parse(raw: &str) -> Option<Self> {
        let mut parts = raw.split(':');
        let (kind, a, b) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() {
            return None;
        }
        let (a, b) = (Uuid::parse_str(a).ok()?, Uuid::parse_str(b).ok()?);
        match kind {
            "impact" => Some(Self::Entitlement {
                component: Component::Impact,
                actor: a,
                target: b,
            }),
            "contribution" => Some(Self::Entitlement {
                component: Component::Contribution,
                actor: a,
                target: b,
            }),
            "pair" => Some(Self::Pair {
                actor: a,
                recipient: b,
            }),
            _ => None,
        }
    }

    pub fn code(self) -> String {
        match self {
            Self::Entitlement {
                component,
                actor,
                target,
            } => format!("{}:{actor}:{target}", component.as_str()),
            Self::Pair { actor, recipient } => format!("pair:{actor}:{recipient}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionOutcome {
    NotReady,
    Planned {
        entitlements: usize,
        ledger_rows: i64,
    },
    Applied {
        entitlements: usize,
        ledger_rows: i64,
    },
    AlreadyApplied,
    KeyReused,
}

/// `vink-app reputation correct dry-run|apply`: retires the entitlements in scope (current
/// rating) and deletes their ledger rows (history), audited and idempotent per key.
pub async fn correct(
    pool: &PgPool,
    key: &str,
    scope: CorrectionScope,
    note: &str,
    apply_it: bool,
) -> Result<CorrectionOutcome, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    lock(&mut tx).await?;
    let Some(ready) = readiness(&mut tx).await? else {
        return Ok(CorrectionOutcome::NotReady);
    };
    let code = scope.code();
    let existing: Option<String> =
        sqlx::query_scalar("SELECT scope FROM reputation_corrections WHERE idempotency_key = $1")
            .bind(key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
    if let Some(existing) = existing {
        return Ok(if existing == code {
            CorrectionOutcome::AlreadyApplied
        } else {
            CorrectionOutcome::KeyReused
        });
    }
    let targets: Vec<(String, Uuid, Uuid, Uuid, Uuid)> = match scope {
        CorrectionScope::Entitlement {
            component,
            actor,
            target,
        } => sqlx::query_as(
            "SELECT component, actor_id, target_id, recipient_id, work_id
               FROM reputation_entitlements
              WHERE component = $1 AND actor_id = $2 AND target_id = $3 AND NOT terminal",
        )
        .bind(component.as_str())
        .bind(actor)
        .bind(target),
        CorrectionScope::Pair { actor, recipient } => sqlx::query_as(
            "SELECT component, actor_id, target_id, recipient_id, work_id
               FROM reputation_entitlements
              WHERE actor_id = $1 AND recipient_id = $2 AND NOT terminal
              ORDER BY component, target_id",
        )
        .bind(actor)
        .bind(recipient),
    }
    .fetch_all(&mut *tx)
    .await
    .map_err(classify)?;
    let mut ledger_rows = 0i64;
    let mut keys = Vec::with_capacity(targets.len());
    for (component, actor, target, recipient, work) in &targets {
        let rows: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM reputation_ledger
              WHERE component = $1 AND actor_id = $2 AND target_id = $3",
        )
        .bind(component)
        .bind(actor)
        .bind(target)
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        ledger_rows += rows;
        keys.push((
            Key {
                component: component_of(component)?,
                actor: *actor,
                target: *target,
            },
            *recipient,
            *work,
        ));
    }
    if !apply_it {
        tx.rollback().await.map_err(classify)?;
        return Ok(CorrectionOutcome::Planned {
            entitlements: keys.len(),
            ledger_rows,
        });
    }
    let now: OffsetDateTime = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
    for (key, recipient, work) in &keys {
        change(
            &mut tx,
            *key,
            *recipient,
            *work,
            Cause::Terminal,
            now,
            ready,
            None,
        )
        .await?;
        sqlx::query(
            "DELETE FROM reputation_ledger WHERE component = $1 AND actor_id = $2 AND target_id = $3",
        )
        .bind(key.component.as_str())
        .bind(key.actor)
        .bind(key.target)
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    }
    sqlx::query(
        "INSERT INTO reputation_corrections (idempotency_key, scope, note, entitlements, ledger_rows)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(key)
    .bind(&code)
    .bind(note)
    .bind(i32::try_from(keys.len()).unwrap_or(i32::MAX))
    .bind(i32::try_from(ledger_rows).unwrap_or(i32::MAX))
    .execute(&mut *tx)
    .await
    .map_err(classify)?;
    tx.commit().await.map_err(classify)?;
    Ok(CorrectionOutcome::Applied {
        entitlements: keys.len(),
        ledger_rows,
    })
}

/// Readiness for reads and status, outside any batch.
pub async fn current_readiness(pool: &PgPool) -> Result<Option<Readiness>, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let ready = readiness(&mut tx).await?;
    tx.commit().await.map_err(classify)?;
    Ok(ready)
}
