//! Social deletion intake and restartable edge cleanup in the App database.
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{PgStore, follow, handle, postgres::classify, principal::locked_standing};
use crate::application::social_retirement::{
    SocialRetirement, SocialRetirementError, SocialRetirementProgress,
};

const EDGE_BATCH: i64 = 100;

async fn checkpoint(
    tx: &mut Transaction<'_, Postgres>,
    command: SocialRetirement,
) -> Result<Option<SocialRetirementProgress>, SocialRetirementError> {
    let saved: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT operation_id, edges_complete
           FROM social_retirements WHERE principal_id = $1 FOR UPDATE",
    )
    .bind(command.principal.as_uuid())
    .fetch_optional(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(match saved {
        None => None,
        Some((operation, _)) if operation != command.operation_id => {
            return Err(SocialRetirementError::OperationConflict);
        }
        Some((_, false)) => Some(SocialRetirementProgress::Fenced),
        Some((_, true)) => Some(SocialRetirementProgress::Complete),
    })
}

impl PgStore {
    /// Apply the fence for a verified Account operation. Take Principal before Profile so a
    /// competing Follow/rename either commits first or sees the irrevocable tombstone.
    pub async fn fence_social_retirement(
        &self,
        command: SocialRetirement,
    ) -> Result<SocialRetirementProgress, SocialRetirementError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        // A previously suspended account may also be deleted by a verified Account operation.
        locked_standing(&mut tx, command.principal).await?;
        let bound = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM principal_bindings WHERE principal_id = $1)",
        )
        .bind(command.principal.as_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        if !bound {
            return Err(SocialRetirementError::Unbound);
        }
        let created = sqlx::query(
            "INSERT INTO social_retirements (principal_id, operation_id)
              VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(command.principal.as_uuid())
        .bind(command.operation_id)
        .execute(&mut *tx)
        .await
        .map_err(classify)?
        .rows_affected()
            != 0;

        let progress = if created {
            // Absence of a profile can occur before backfill. The durable marker still redacts
            // attribution in the same commit and prevents future provisioning.
            handle::clear_current_handle(&mut tx, command.principal).await?;
            SocialRetirementProgress::Fenced
        } else {
            checkpoint(&mut tx, command)
                .await?
                .ok_or(SocialRetirementError::OperationConflict)?
        };
        tx.commit().await.map_err(classify)?;
        Ok(progress)
    }

    /// Apply one bounded batch after the fence has committed. Only the operation checkpoint
    /// and edge rows are locked, and edges are locked in their global pair order.
    pub async fn step_social_retirement(
        &self,
        command: SocialRetirement,
    ) -> Result<SocialRetirementProgress, SocialRetirementError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let progress = checkpoint(&mut tx, command)
            .await?
            .ok_or(SocialRetirementError::NotFenced)?;
        let progress = match progress {
            SocialRetirementProgress::Complete => SocialRetirementProgress::Complete,
            SocialRetirementProgress::Fenced => {
                let affected =
                    follow::retire_edges_batch(&mut tx, command.principal, EDGE_BATCH).await?;
                if affected < EDGE_BATCH as u64 {
                    sqlx::query("UPDATE social_retirements SET edges_complete = true WHERE principal_id = $1")
                        .bind(command.principal.as_uuid())
                        .execute(&mut *tx)
                        .await
                        .map_err(classify)?;
                    SocialRetirementProgress::Complete
                } else {
                    SocialRetirementProgress::Fenced
                }
            }
        };
        tx.commit().await.map_err(classify)?;
        Ok(progress)
    }
}
