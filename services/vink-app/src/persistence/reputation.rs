//! PostgreSQL adapter of the Điểm Vink outbox (#645; contract K3,
//! `migrations/20261208000000_reputation_outbox.sql`). Decisions come from
//! [`crate::domain::reputation::outbox`]; this module writes what they return.
//!
//! **Producer ([`emit_community`]).** Called inside the endorsement transaction after its own
//! writes and after every lock of that path: it reads the activation row `FOR KEY SHARE` (also
//! when inactive) and, only while active, writes one immutable event with the transaction clock
//! `now()` — the same instant as the endorsement row and its receipt. Markers for work, comment
//! and principal facts are written by triggers in the same transactions as those changes.
//!
//! **Lock order.** Producers: their own path's locks, then `notification_activation` (when the
//! path emits a notice), then `reputation_activation` (KEY SHARE); event rows are new. Operator
//! commands take the activation row alone. No path takes them in reverse.
//!
//! **Privacy.** Events hold identifiers, versions and booleans, never text. Logs and errors from
//! here carry codes and counts.

use sqlx::postgres::PgPool;
use sqlx::{PgConnection, Row};

use super::discussion::integrity;
use super::postgres::classify;
use crate::application::ports::StoreError;
use crate::domain::reputation::outbox::{Activation, ActivationChange, CommunityEvent};

fn activation_of(active: bool, epoch: i64) -> Result<Activation, StoreError> {
    Activation::restore(active, epoch)
        .ok_or_else(|| integrity("stored reputation activation", epoch))
}

/// Writes the community event an accepted endorsement owes, if the feature is active in this
/// transaction. Returns whether a row was written.
pub(crate) async fn emit_community(
    conn: &mut PgConnection,
    event: Option<CommunityEvent>,
) -> Result<bool, StoreError> {
    let Some(event) = event else {
        return Ok(false);
    };
    // Before migration 20261208000000 nothing could have been activated: an absent table means
    // "never activated". Only schema-staged tests reach this; a serving binary needs the schema.
    let exists: bool =
        sqlx::query_scalar("SELECT to_regclass('reputation_activation') IS NOT NULL")
            .fetch_one(&mut *conn)
            .await
            .map_err(classify)?;
    if !exists {
        return Ok(false);
    }
    let row = sqlx::query("SELECT active, epoch FROM reputation_activation FOR KEY SHARE")
        .fetch_one(&mut *conn)
        .await
        .map_err(classify)?;
    let activation = activation_of(
        row.try_get("active").map_err(classify)?,
        row.try_get("epoch").map_err(classify)?,
    )?;
    let Some(epoch) = activation.producing() else {
        return Ok(false);
    };
    let query = match event {
        CommunityEvent::Tham {
            actor,
            recipient,
            work,
            revision,
            state,
            work_public,
            hidden_revision,
        } => sqlx::query(
            "INSERT INTO reputation_events
                    (kind, activation_epoch, accepted_at, actor_id, recipient_id, work_id,
                     revision_id, selected, version, accepted_order, work_public,
                     hidden_revision_id)
             VALUES ('tham', $1, now(), $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(epoch)
        .bind(actor.as_uuid())
        .bind(recipient.as_uuid())
        .bind(work.as_uuid())
        .bind(revision.as_uuid())
        .bind(state.selected())
        .bind(state.version().get())
        .bind(state.order().get())
        .bind(work_public)
        .bind(hidden_revision.map(|revision| revision.as_uuid())),
        CommunityEvent::Ung {
            actor,
            recipient,
            work,
            comment,
            state,
            work_public,
            comment_public,
        } => sqlx::query(
            "INSERT INTO reputation_events
                    (kind, activation_epoch, accepted_at, actor_id, recipient_id, work_id,
                     comment_id, selected, version, accepted_order, work_public, comment_public)
             VALUES ('ung', $1, now(), $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(epoch)
        .bind(actor.as_uuid())
        .bind(recipient.as_uuid())
        .bind(work.as_uuid())
        .bind(comment.as_uuid())
        .bind(state.selected())
        .bind(state.version().get())
        .bind(state.order().get())
        .bind(work_public)
        .bind(comment_public),
    };
    query.execute(&mut *conn).await.map_err(classify)?;
    Ok(true)
}

pub async fn activation(pool: &PgPool) -> Result<Activation, StoreError> {
    let (active, epoch): (bool, i64) =
        sqlx::query_as("SELECT active, epoch FROM reputation_activation")
            .fetch_one(pool)
            .await
            .map_err(classify)?;
    activation_of(active, epoch)
}

/// `vink-app reputation activate|deactivate`: one audited cutover, serialized with every producer
/// through the activation row.
pub async fn set_activation(
    pool: &PgPool,
    active: bool,
    note: &str,
) -> Result<ActivationChange, StoreError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let (stored_active, epoch): (bool, i64) =
        sqlx::query_as("SELECT active, epoch FROM reputation_activation FOR UPDATE")
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?;
    let change = activation_of(stored_active, epoch)?.change(active);
    if let ActivationChange::Changed(next) = change {
        sqlx::query("UPDATE reputation_activation SET active = $1, epoch = $2, changed_at = now()")
            .bind(next.active())
            .bind(next.epoch())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        sqlx::query(
            "INSERT INTO reputation_activation_changes (active, epoch, note) VALUES ($1, $2, $3)",
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

/// Owed events per kind, for `vink-app reputation status`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Backlog {
    pub per_kind: Vec<(String, i64)>,
    pub oldest_seconds: Option<i64>,
}

pub async fn backlog(pool: &PgPool) -> Result<Backlog, StoreError> {
    let per_kind: Vec<(String, i64)> =
        sqlx::query_as("SELECT kind, count(*) FROM reputation_events GROUP BY kind ORDER BY kind")
            .fetch_all(pool)
            .await
            .map_err(classify)?;
    let oldest_seconds: Option<i64> = sqlx::query_scalar(
        "SELECT floor(extract(epoch FROM now() - min(accepted_at)))::bigint FROM reputation_events",
    )
    .fetch_one(pool)
    .await
    .map_err(classify)?;
    Ok(Backlog {
        per_kind,
        oldest_seconds,
    })
}
