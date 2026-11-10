use sqlx::postgres::PgPool;
use sqlx::types::Json;
use time::OffsetDateTime;
use uuid::Uuid;

use super::principal::require_active;
use crate::application::ports::{DraftStore, SaveOutcome, StoreError};
use crate::domain::document::{Document, DocumentData, Title};
use crate::domain::draft::{DraftChange, DraftSummary, DraftVersion, WorkingDraft};
use crate::domain::ids::{PrincipalId, WorkId};

#[derive(Debug, Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// Connection-level failures are retryable unavailability; anything else is an integrity error.
/// sqlx error text carries no bound parameters, so manuscript text never reaches the log.
pub(super) fn classify(error: sqlx::Error) -> StoreError {
    let classified = classify_inner(error);
    match &classified {
        StoreError::Unavailable(detail) => tracing::warn!(%detail, "postgres unavailable"),
        StoreError::Integrity(detail) => tracing::error!(%detail, "postgres integrity error"),
        StoreError::UnknownPrincipal => tracing::info!("principal has no Vink record"),
        StoreError::PrincipalSuspended => {}
    }
    classified
}

fn classify_inner(error: sqlx::Error) -> StoreError {
    // A resolved principal without a Vink record, caught by the FK of the row it would own.
    if let sqlx::Error::Database(db) = &error
        && matches!(
            db.constraint(),
            Some(
                "works_owner_id_fkey"
                    | "comments_author_id_fkey"
                    | "discussion_command_receipts_actor_id_fkey"
                    | "comment_moderation_decisions_moderator_id_fkey"
                    | "discussion_moderator_grants_principal_id_fkey"
                    | "work_endorsements_actor_id_fkey"
                    | "comment_endorsements_actor_id_fkey"
                    | "endorsement_receipts_actor_id_fkey"
                    | "principal_suspensions_principal_id_fkey"
            )
        )
    {
        return StoreError::UnknownPrincipal;
    }
    if vink_store_postgres::is_unavailable(&error) {
        StoreError::Unavailable(error.to_string())
    } else {
        StoreError::Integrity(error.to_string())
    }
}

fn integrity(what: &str, detail: impl std::fmt::Display) -> StoreError {
    StoreError::Integrity(format!("{what}: {detail}"))
}

pub(super) fn title(raw: &str) -> Result<Title, StoreError> {
    Title::parse(raw).map_err(|e| integrity("stored title", e))
}

pub(super) fn document(Json(data): Json<DocumentData>) -> Result<Document, StoreError> {
    Document::parse(data).map_err(|e| integrity("stored document", e))
}

fn version(raw: i64) -> Result<DraftVersion, StoreError> {
    DraftVersion::new(raw).ok_or_else(|| integrity("stored draft version", raw))
}

#[derive(sqlx::FromRow)]
struct DraftRow {
    work_id: Uuid,
    version: i64,
    title: String,
    document: Json<DocumentData>,
    updated_at: OffsetDateTime,
}

impl TryFrom<DraftRow> for WorkingDraft {
    type Error = StoreError;

    fn try_from(row: DraftRow) -> Result<Self, StoreError> {
        Ok(Self {
            work_id: WorkId::from_uuid(row.work_id),
            version: version(row.version)?,
            title: title(&row.title)?,
            document: document(row.document)?,
            updated_at: row.updated_at,
        })
    }
}

impl DraftStore for PgStore {
    async fn list_drafts(&self, owner: PrincipalId) -> Result<Vec<DraftSummary>, StoreError> {
        let rows: Vec<(Uuid, i64, String, OffsetDateTime)> = sqlx::query_as(
            "SELECT d.work_id, d.version, d.title, d.updated_at
               FROM working_drafts d
               JOIN works w ON w.id = d.work_id
              WHERE w.owner_id = $1
              ORDER BY d.updated_at DESC, d.work_id
              LIMIT 100",
        )
        .bind(owner.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(classify)?;
        rows.into_iter()
            .map(|(work_id, raw_version, raw_title, updated_at)| {
                Ok(DraftSummary {
                    work_id: WorkId::from_uuid(work_id),
                    version: version(raw_version)?,
                    title: title(&raw_title)?,
                    updated_at,
                })
            })
            .collect()
    }

    async fn create_draft(
        &self,
        owner: PrincipalId,
        work_id: WorkId,
    ) -> Result<WorkingDraft, StoreError> {
        let empty = Document::empty();
        let mut tx = self.pool.begin().await.map_err(classify)?;
        // Standing fence first: nothing else is locked and `works.owner_id` references the actor.
        require_active(&mut tx, owner).await?;
        sqlx::query("INSERT INTO works (id, owner_id) VALUES ($1, $2)")
            .bind(work_id.as_uuid())
            .bind(owner.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        let row: DraftRow = sqlx::query_as(
            "INSERT INTO working_drafts (work_id, version, title, document)
             VALUES ($1, $2, '', $3)
             RETURNING work_id, version, title, document, updated_at",
        )
        .bind(work_id.as_uuid())
        .bind(DraftVersion::FIRST.get())
        .bind(Json(empty.to_data()))
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        row.try_into()
    }

    async fn load_draft(
        &self,
        owner: PrincipalId,
        work_id: WorkId,
    ) -> Result<Option<WorkingDraft>, StoreError> {
        let row: Option<DraftRow> = sqlx::query_as(
            "SELECT d.work_id, d.version, d.title, d.document, d.updated_at
               FROM working_drafts d
               JOIN works w ON w.id = d.work_id
              WHERE d.work_id = $1 AND w.owner_id = $2",
        )
        .bind(work_id.as_uuid())
        .bind(owner.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(classify)?;
        row.map(WorkingDraft::try_from).transpose()
    }

    async fn save_draft(
        &self,
        owner: PrincipalId,
        work_id: WorkId,
        change: &DraftChange,
    ) -> Result<SaveOutcome, StoreError> {
        // Standing fence, then owner and version in the UPDATE predicate, so the check and the
        // write are one atomic statement; a concurrent save that commits first makes this one
        // match no row.
        let mut tx = self.pool.begin().await.map_err(classify)?;
        require_active(&mut tx, owner).await?;
        let saved: Option<(i64, OffsetDateTime)> = sqlx::query_as(
            "UPDATE working_drafts d
                SET title = $4, document = $5, version = d.version + 1, updated_at = now()
               FROM works w
              WHERE w.id = d.work_id AND d.work_id = $1 AND w.owner_id = $2 AND d.version = $3
          RETURNING d.version, d.updated_at",
        )
        .bind(work_id.as_uuid())
        .bind(owner.as_uuid())
        .bind(change.expected_version().get())
        .bind(change.title().as_str())
        .bind(Json(change.document().to_data()))
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        if let Some((raw_version, updated_at)) = saved {
            tx.commit().await.map_err(classify)?;
            return Ok(SaveOutcome::Saved {
                version: version(raw_version)?,
                updated_at,
            });
        }
        // Nothing was written. Classify only for the owner, so other owners' drafts stay hidden.
        let current: Option<i64> = sqlx::query_scalar(
            "SELECT d.version FROM working_drafts d JOIN works w ON w.id = d.work_id
              WHERE d.work_id = $1 AND w.owner_id = $2",
        )
        .bind(work_id.as_uuid())
        .bind(owner.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        Ok(match current {
            Some(raw) => SaveOutcome::Conflict {
                current: version(raw)?,
            },
            None => SaveOutcome::NotFound,
        })
    }
}
