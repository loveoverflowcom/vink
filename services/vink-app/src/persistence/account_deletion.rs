//! Fence a verified Identity subject, then erase its private content under the permanent fence.
use sqlx::{PgConnection, Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{PgStore, moderation_write, postgres::classify, principal::locked_standing};
use crate::application::ports::StoreError;
use crate::application::social_retirement::{SocialRetirement, SocialRetirementError};
use crate::domain::endorsement::EndorsementDelta;
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, WorkId};
use crate::domain::notification::{
    ThamFacts, UngFacts, principal_standing_changed, tham_changed, ung_changed,
};
use crate::identity::client::AccountDeletion;

pub(super) const NATIVE_AUTHORITY: &str = "vink-identity";

// Used by native principal provisioning as well. A subject may have no PrincipalId yet.
pub(super) async fn lock_subject(conn: &mut PgConnection, subject: &str) -> Result<(), StoreError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 297))")
        .bind(subject)
        .execute(conn)
        .await
        .map_err(classify)?;
    Ok(())
}

impl PgStore {
    /// The receipt and all visibility and authority changes commit together.
    /// Return the canonical Social operation for that cleanup, if there is a binding.
    pub async fn apply_account_deletion(
        &self,
        operation: AccountDeletion,
    ) -> Result<Option<SocialRetirement>, SocialRetirementError> {
        let subject = operation.account_id.to_string();
        let mut tx = self.pool().begin().await.map_err(classify)?;
        sqlx::query("SET LOCAL lock_timeout = '2s'")
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        lock_subject(&mut tx, &subject).await?;

        let saved: Option<(Uuid, OffsetDateTime, Option<Uuid>)> = sqlx::query_as(
            "SELECT operation_id, accepted_at, principal_id FROM account_deletions WHERE account_id = $1",
        )
        .bind(operation.account_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        if let Some((id, accepted, principal)) = saved {
            if id != operation.operation_id || accepted != operation.accepted_at {
                return Err(SocialRetirementError::OperationConflict);
            }
            return social_command(&mut tx, principal.map(PrincipalId::from_uuid), id).await;
        }

        let principal: Option<Uuid> = sqlx::query_scalar(
            "SELECT principal_id FROM principal_bindings WHERE authority = $1 AND subject_id = $2",
        )
        .bind(NATIVE_AUTHORITY)
        .bind(&subject)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let principal = principal.map(PrincipalId::from_uuid);
        let works = if let Some(principal) = principal {
            let works = owned_works(&mut tx, principal).await?;
            // Match Publication's locks before acquiring Principal. If a new draft won the
            // race with the fence, roll back and let the next attempt include it in this order.
            for work in &works {
                moderation_write::lock_work(&mut tx, *work).await?;
            }
            locked_standing(&mut tx, principal).await?;
            if works != owned_works(&mut tx, principal).await? {
                return Err(StoreError::Unavailable(
                    "new work committed before account deletion".into(),
                )
                .into());
            }
            works
        } else {
            Vec::new()
        };

        sqlx::query(
            "INSERT INTO account_deletions (account_id, operation_id, accepted_at, principal_id)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(operation.account_id)
        .bind(operation.operation_id)
        .bind(operation.accepted_at)
        .bind(principal.map(|principal| principal.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(classify)?;

        let social = social_command(&mut tx, principal, operation.operation_id).await?;
        if let (Some(principal), Some(social)) = (principal, social) {
            super::social_retirement::fence(&mut tx, social).await?;
            for work in works {
                super::publication::retire_owned_work(&mut tx, principal, work).await?;
            }
            purge_principal(&mut tx, principal).await?;
        }
        tx.commit().await.map_err(classify)?;
        Ok(social)
    }

    pub async fn purge_account_deletion(
        &self,
        operation: AccountDeletion,
    ) -> Result<(), SocialRetirementError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        sqlx::query("SET LOCAL lock_timeout = '2s'")
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        let row: Option<(Option<Uuid>, bool)> = sqlx::query_as(
            "SELECT principal_id, purged FROM account_deletions
             WHERE account_id = $1 AND operation_id = $2 AND accepted_at = $3 FOR UPDATE",
        )
        .bind(operation.account_id)
        .bind(operation.operation_id)
        .bind(operation.accepted_at)
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let (principal, purged) = row.ok_or(SocialRetirementError::NotFenced)?;
        if purged {
            return Ok(());
        }
        if let Some(principal) = principal.map(PrincipalId::from_uuid) {
            for work in owned_works(&mut tx, principal).await? {
                super::publication::erase_owned_work_content(&mut tx, work).await?;
            }
        }
        sqlx::query("UPDATE account_deletions SET purged = true WHERE operation_id = $1")
            .bind(operation.operation_id)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        Ok(())
    }

    pub async fn account_deletion_applied(&self, operation: Uuid) -> Result<bool, StoreError> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM account_deletions WHERE operation_id = $1)",
        )
        .bind(operation)
        .fetch_one(self.pool())
        .await
        .map_err(classify)
    }
}

// Earlier Social adapter callers already had an independent operation ID. Keep that receipt
// and complete its batch instead of creating another Social transition for the same principal.
async fn social_command(
    conn: &mut PgConnection,
    principal: Option<PrincipalId>,
    fallback: Uuid,
) -> Result<Option<SocialRetirement>, SocialRetirementError> {
    let Some(principal) = principal else {
        return Ok(None);
    };
    let prior: Option<Uuid> =
        sqlx::query_scalar("SELECT operation_id FROM social_retirements WHERE principal_id = $1")
            .bind(principal.as_uuid())
            .fetch_optional(conn)
            .await
            .map_err(classify)?;
    SocialRetirement::new(principal, prior.unwrap_or(fallback))
        .map(Some)
        .ok_or(SocialRetirementError::OperationConflict)
}

async fn owned_works(
    conn: &mut PgConnection,
    principal: PrincipalId,
) -> Result<Vec<WorkId>, StoreError> {
    Ok(
        sqlx::query_scalar("SELECT id FROM works WHERE owner_id = $1 ORDER BY id")
            .bind(principal.as_uuid())
            .fetch_all(conn)
            .await
            .map_err(classify)?
            .into_iter()
            .map(WorkId::from_uuid)
            .collect(),
    )
}

async fn purge_principal(
    tx: &mut Transaction<'_, Postgres>,
    principal: PrincipalId,
) -> Result<(), StoreError> {
    let id = principal.as_uuid();
    // Preserve valid public comments and existing moderation evidence. A pending comment becomes
    // terminal, drops its text, and advances the version any in-flight decision would need.
    sqlx::query(
        "UPDATE comments SET removal = 'author', body = NULL,
                moderation_version = moderation_version + 1, updated_at = clock_timestamp()
          WHERE author_id = $1 AND review = 'pending' AND removal IS NULL",
    )
    .bind(id)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;

    // Unselect with the same version/order change as a user removal. Keep replay receipts and
    // use the normal Notifications retractions; counts are aggregates of the selected rows.
    let works: Vec<(Uuid, Uuid, Uuid, i64, i64)> = sqlx::query_as(
        "UPDATE work_endorsements e SET selected = false, version = version + 1,
                accepted_order = accepted_order + 1, updated_at = clock_timestamp()
           FROM works w WHERE e.actor_id = $1 AND e.selected AND w.id = e.work_id
         RETURNING e.work_id, e.revision_id, w.owner_id, e.version, e.accepted_order",
    )
    .bind(id)
    .fetch_all(&mut **tx)
    .await
    .map_err(classify)?;
    for (work, revision, owner, version_after, accepted_order) in works {
        let work = WorkId::from_uuid(work);
        let stamp = super::notification::locked_stamp(tx, work).await?;
        let notice = tham_changed(ThamFacts {
            work,
            revision: RevisionId::from_uuid(revision),
            owner: PrincipalId::from_uuid(owner),
            actor: principal,
            delta: EndorsementDelta::Remove,
            version_after,
            accepted_order,
            stamp,
        });
        super::notification::emit(tx, notice).await?;
    }

    let comments: Vec<(Uuid, Uuid, Uuid, i64, i64)> = sqlx::query_as(
        "UPDATE comment_endorsements e SET selected = false, version = version + 1,
                accepted_order = accepted_order + 1, updated_at = clock_timestamp()
           FROM comments c JOIN discussion_threads t ON t.id = c.thread_id
          WHERE e.actor_id = $1 AND e.selected AND c.id = e.comment_id
        RETURNING t.work_id, e.comment_id, c.author_id, e.version, e.accepted_order",
    )
    .bind(id)
    .fetch_all(&mut **tx)
    .await
    .map_err(classify)?;
    for (work, comment, writer, version_after, accepted_order) in comments {
        let work = WorkId::from_uuid(work);
        let stamp = super::notification::locked_stamp(tx, work).await?;
        let notice = ung_changed(UngFacts {
            work,
            comment: CommentId::from_uuid(comment),
            writer: PrincipalId::from_uuid(writer),
            actor: principal,
            delta: EndorsementDelta::Remove,
            version_after,
            accepted_order,
            stamp,
        });
        super::notification::emit(tx, notice).await?;
    }

    sqlx::query(
        "UPDATE author_profiles SET bio = NULL, policies = DEFAULT WHERE principal_id = $1",
    )
    .bind(id)
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    sqlx::query("UPDATE principals SET display_name = 'Tài khoản đã xóa' WHERE id = $1")
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(classify)?;
    let order: i64 = sqlx::query_scalar(
        "SELECT count(*) + count(lifted_at) + 1 FROM principal_suspensions WHERE principal_id = $1",
    )
    .bind(id)
    .fetch_one(&mut **tx)
    .await
    .map_err(classify)?;
    super::notification::emit(tx, [principal_standing_changed(principal, order)]).await?;
    Ok(())
}
