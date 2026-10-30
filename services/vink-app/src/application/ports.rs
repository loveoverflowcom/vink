//! Storage ports implemented by the PostgreSQL adapter.

use std::future::Future;

use time::OffsetDateTime;

use super::{PublicWork, PublicWorkSummary};
use crate::domain::document::{Document, Title};
use crate::domain::draft::{DraftSummary, DraftVersion, WorkingDraft};
use crate::domain::ids::{PrincipalId, WorkId};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// The database could not be reached or refused service; retrying later may succeed.
    #[error("storage unavailable: {0}")]
    Unavailable(String),
    /// The principal has no Vink record.
    #[error("unknown principal")]
    UnknownPrincipal,
    /// Stored data or a statement violated an invariant.
    #[error("storage integrity error: {0}")]
    Integrity(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOutcome {
    Saved {
        version: DraftVersion,
        updated_at: OffsetDateTime,
    },
    /// The owner's draft exists at a different version; nothing was written.
    Conflict { current: DraftVersion },
    /// No draft of this work is owned by the actor; nothing was written.
    NotFound,
}

pub trait DraftStore: Send + Sync {
    fn list_drafts(
        &self,
        owner: PrincipalId,
    ) -> impl Future<Output = Result<Vec<DraftSummary>, StoreError>> + Send;

    /// Creates a work owned by `owner` with an empty working draft at version 1.
    fn create_draft(
        &self,
        owner: PrincipalId,
        work_id: WorkId,
    ) -> impl Future<Output = Result<WorkingDraft, StoreError>> + Send;

    fn load_draft(
        &self,
        owner: PrincipalId,
        work_id: WorkId,
    ) -> impl Future<Output = Result<Option<WorkingDraft>, StoreError>> + Send;

    fn save_draft(
        &self,
        owner: PrincipalId,
        work_id: WorkId,
        expected: DraftVersion,
        title: &Title,
        document: &Document,
    ) -> impl Future<Output = Result<SaveOutcome, StoreError>> + Send;
}

pub trait PublicationStore: Send + Sync {
    fn list_publications(
        &self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<PublicWorkSummary>, StoreError>> + Send;

    /// Current public snapshot of a work, read through the publication pointer only.
    fn publication(
        &self,
        work_id: WorkId,
    ) -> impl Future<Output = Result<Option<PublicWork>, StoreError>> + Send;
}
