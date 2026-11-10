//! Subject → principal bindings and principal suspensions (D-05/S-03 spike).

use sqlx::postgres::PgPool;
use sqlx::{Postgres, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::PgStore;
use super::postgres::classify;
use crate::application::ports::{BoundPrincipal, PrincipalStore, Provisioned, StoreError};
use crate::domain::ids::PrincipalId;
use crate::domain::principal::{PenName, Standing, SubjectKey, SuspensionReason};

const BOUND: &str = "SELECT b.principal_id,
        EXISTS (SELECT 1 FROM principal_suspensions s
                 WHERE s.principal_id = b.principal_id AND s.lifted_at IS NULL) AS suspended
   FROM principal_bindings b WHERE b.authority = $1 AND b.subject_id = $2";

fn bound(row: &sqlx::postgres::PgRow) -> Result<BoundPrincipal, sqlx::Error> {
    Ok(BoundPrincipal {
        principal: PrincipalId::from_uuid(row.try_get("principal_id")?),
        standing: if row.try_get("suspended")? {
            Standing::Suspended
        } else {
            Standing::Active
        },
    })
}

/// The standing fence of a write transaction. Locks the actor's principal row FOR UPDATE, which
/// conflicts with the FOR KEY SHARE lock that inserting a suspension takes through
/// `principal_suspensions_principal_id_fkey`: a suspension commits wholly before this reads the
/// standing, or waits until the caller's transaction ends. The standing is read in a SEPARATE
/// statement after the lock, because in READ COMMITTED a subquery of the locking statement would
/// still use the snapshot taken before the wait. Lifting a suspension is not fenced (a stale
/// "suspended" fails closed). Take this lock before any statement of the same transaction that
/// references the principal through a foreign key: upgrading an already held FOR KEY SHARE can
/// deadlock with a concurrent transaction of the same principal. Used through [`require_active`]
/// by every protected write, and directly by draft tag saves and discussion submissions (whose
/// pure decisions take the standing as a fact).
pub(super) async fn locked_standing(
    tx: &mut Transaction<'_, Postgres>,
    actor: PrincipalId,
) -> Result<Standing, StoreError> {
    let known: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM principals WHERE id = $1 FOR UPDATE")
            .bind(actor.as_uuid())
            .fetch_optional(&mut **tx)
            .await
            .map_err(classify)?;
    if known.is_none() {
        tracing::info!("principal has no Vink record");
        return Err(StoreError::UnknownPrincipal);
    }
    let suspended: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM principal_suspensions
                         WHERE principal_id = $1 AND lifted_at IS NULL)",
    )
    .bind(actor.as_uuid())
    .fetch_one(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(if suspended {
        Standing::Suspended
    } else {
        Standing::Active
    })
}

/// The standing fence for writes whose decision does not model standing: a suspension committed
/// before this point refuses the write with [`StoreError::PrincipalSuspended`]; one committed
/// later waits for this transaction. Callers place it where their own lock order says (see each
/// adapter's module comment), always before a statement that references the actor by foreign key.
pub(super) async fn require_active(
    tx: &mut Transaction<'_, Postgres>,
    actor: PrincipalId,
) -> Result<(), StoreError> {
    match locked_standing(tx, actor).await? {
        Standing::Active => Ok(()),
        Standing::Suspended => {
            tracing::info!("write refused at commit: principal suspended");
            Err(StoreError::PrincipalSuspended)
        }
    }
}

impl PrincipalStore for PgStore {
    async fn principal_for_subject(
        &self,
        subject: &SubjectKey,
    ) -> Result<Option<BoundPrincipal>, StoreError> {
        let row = sqlx::query(BOUND)
            .bind(subject.authority())
            .bind(subject.id())
            .fetch_optional(self.pool())
            .await
            .map_err(classify)?;
        row.as_ref().map(bound).transpose().map_err(classify)
    }

    /// One transaction: insert the principal, then the binding. If another request bound the
    /// subject first, `ON CONFLICT DO NOTHING` waits for it and inserts nothing; the transaction
    /// rolls back (no orphan principal) and the winner's binding is read.
    async fn provision_principal(
        &self,
        subject: &SubjectKey,
        candidate: PrincipalId,
        pen_name: &PenName,
    ) -> Result<Provisioned, StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, $2)")
            .bind(candidate.as_uuid())
            .bind(pen_name.as_str())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO principal_bindings (authority, subject_id, principal_id)
             VALUES ($1, $2, $3)
             ON CONFLICT ON CONSTRAINT principal_bindings_pkey DO NOTHING
             RETURNING principal_id",
        )
        .bind(subject.authority())
        .bind(subject.id())
        .bind(candidate.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        if inserted.is_some() {
            tx.commit().await.map_err(classify)?;
            return Ok(Provisioned {
                bound: BoundPrincipal {
                    principal: candidate,
                    standing: Standing::Active,
                },
                created: true,
            });
        }
        tx.rollback().await.map_err(classify)?;
        let existing = self
            .principal_for_subject(subject)
            .await?
            .ok_or_else(|| StoreError::Integrity("binding vanished after conflict".into()))?;
        Ok(Provisioned {
            bound: existing,
            created: false,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspensionOutcome {
    Suspended(Uuid),
    AlreadySuspended(Uuid),
}

/// Operator command: suspends `principal`. Every later request of that principal resolves
/// without authority, whatever Identity answers. Idempotent per active suspension.
pub async fn suspend_principal(
    pool: &PgPool,
    principal: PrincipalId,
    reason: &SuspensionReason,
) -> Result<SuspensionOutcome, StoreError> {
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO principal_suspensions (id, principal_id, reason) VALUES ($1, $2, $3)
         ON CONFLICT (principal_id) WHERE lifted_at IS NULL DO NOTHING
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(principal.as_uuid())
    .bind(reason.as_str())
    .fetch_optional(pool)
    .await
    .map_err(classify)?;
    if let Some(id) = inserted {
        return Ok(SuspensionOutcome::Suspended(id));
    }
    let active: Uuid = sqlx::query_scalar(
        "SELECT id FROM principal_suspensions WHERE principal_id = $1 AND lifted_at IS NULL",
    )
    .bind(principal.as_uuid())
    .fetch_one(pool)
    .await
    .map_err(classify)?;
    Ok(SuspensionOutcome::AlreadySuspended(active))
}

/// Operator command: lifts the active suspension. `None`: nothing was active. Lifting restores
/// the principal's authority on its next request; it does not revive revoked sessions.
pub async fn reinstate_principal(
    pool: &PgPool,
    principal: PrincipalId,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "UPDATE principal_suspensions SET lifted_at = now()
          WHERE principal_id = $1 AND lifted_at IS NULL
         RETURNING id",
    )
    .bind(principal.as_uuid())
    .fetch_optional(pool)
    .await
    .map_err(classify)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspensionRecord {
    pub principal: PrincipalId,
    pub display_name: String,
    pub suspended_at: OffsetDateTime,
    pub reason: String,
}

pub async fn active_suspensions(pool: &PgPool) -> Result<Vec<SuspensionRecord>, StoreError> {
    let rows = sqlx::query(
        "SELECT s.principal_id, p.display_name, s.suspended_at, s.reason
           FROM principal_suspensions s JOIN principals p ON p.id = s.principal_id
          WHERE s.lifted_at IS NULL ORDER BY s.suspended_at, s.id",
    )
    .fetch_all(pool)
    .await
    .map_err(classify)?;
    rows.iter()
        .map(|row| {
            Ok(SuspensionRecord {
                principal: PrincipalId::from_uuid(row.try_get("principal_id")?),
                display_name: row.try_get("display_name")?,
                suspended_at: row.try_get("suspended_at")?,
                reason: row.try_get("reason")?,
            })
        })
        .collect::<Result<_, sqlx::Error>>()
        .map_err(classify)
}
