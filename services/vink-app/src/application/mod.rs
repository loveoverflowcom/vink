//! Use cases. Application code authenticates the actor, validates input through the domain and
//! delegates I/O to the ports in [`ports`]; it imports no HTTP, SQL or Leptos types.

pub mod ports;

use time::OffsetDateTime;

use crate::domain::document::{Document, DocumentData, DocumentError, Title, TitleError};
use crate::domain::draft::{DraftSummary, DraftVersion, WorkingDraft};
use crate::domain::ids::{PrincipalId, RevisionId, WorkId};
use ports::{DraftStore, PublicationStore, SaveOutcome, StoreError};

/// Number of public works the home page lists. The home page is finite by design (ED-01).
pub const HOME_LIMIT: i64 = 12;

/// Read model of the current public snapshot of a work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicWork {
    pub work_id: WorkId,
    pub revision_id: RevisionId,
    pub title: Title,
    pub author_display_name: String,
    pub published_at: OffsetDateTime,
    pub document: Document,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicWorkSummary {
    pub work_id: WorkId,
    pub title: Title,
    pub author_display_name: String,
    pub published_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveDraft {
    pub work_id: WorkId,
    pub expected_version: i64,
    pub title: String,
    pub document: DocumentData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedDraft {
    pub work_id: WorkId,
    pub version: DraftVersion,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    #[error("not found")]
    NotFound,
    #[error("storage unavailable")]
    Unavailable,
    #[error("stored data failed validation")]
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidInput {
    #[error("expected_version must be >= 1")]
    ExpectedVersion,
    #[error("title: {0}")]
    Title(#[from] TitleError),
    #[error("document: {0}")]
    Document(#[from] DocumentError),
}

impl InvalidInput {
    pub fn field(&self) -> &'static str {
        match self {
            Self::ExpectedVersion => "expected_version",
            Self::Title(_) => "title",
            Self::Document(_) => "document",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DraftError {
    /// No authenticated Vink principal for this request.
    #[error("unauthenticated")]
    Unauthenticated,
    /// Missing, or owned by someone else; the two are deliberately indistinguishable.
    #[error("not found")]
    NotFound,
    #[error("version conflict; current version is {}", current.get())]
    VersionConflict { current: DraftVersion },
    #[error("invalid input: {0}")]
    Invalid(#[from] InvalidInput),
    #[error("storage unavailable")]
    Unavailable,
    #[error("stored data failed validation")]
    Internal,
}

impl From<StoreError> for ReadError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            StoreError::Integrity(_) | StoreError::UnknownPrincipal => Self::Internal,
        }
    }
}

impl From<StoreError> for DraftError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            // The resolved principal has no Vink record: not an authenticated Vink actor.
            StoreError::UnknownPrincipal => Self::Unauthenticated,
            StoreError::Integrity(_) => Self::Internal,
        }
    }
}

/// The Vink application facade shared by the HTTP API, the SSR host and the worker.
#[derive(Debug, Clone)]
pub struct Vink<S> {
    store: S,
}

impl<S> Vink<S>
where
    S: DraftStore + PublicationStore,
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub async fn public_works(&self) -> Result<Vec<PublicWorkSummary>, ReadError> {
        Ok(self.store.list_publications(HOME_LIMIT).await?)
    }

    pub async fn publication(&self, work_id: WorkId) -> Result<PublicWork, ReadError> {
        self.store
            .publication(work_id)
            .await?
            .ok_or(ReadError::NotFound)
    }

    pub async fn list_drafts(
        &self,
        actor: Option<PrincipalId>,
    ) -> Result<Vec<DraftSummary>, DraftError> {
        let owner = actor.ok_or(DraftError::Unauthenticated)?;
        Ok(self.store.list_drafts(owner).await?)
    }

    pub async fn create_draft(
        &self,
        actor: Option<PrincipalId>,
    ) -> Result<WorkingDraft, DraftError> {
        let owner = actor.ok_or(DraftError::Unauthenticated)?;
        Ok(self.store.create_draft(owner, WorkId::generate()).await?)
    }

    pub async fn load_draft(
        &self,
        actor: Option<PrincipalId>,
        work_id: WorkId,
    ) -> Result<WorkingDraft, DraftError> {
        let owner = actor.ok_or(DraftError::Unauthenticated)?;
        self.store
            .load_draft(owner, work_id)
            .await?
            .ok_or(DraftError::NotFound)
    }

    /// Conditional write: succeeds only if the stored version still equals `expected_version`
    /// and the actor owns the work. A stale save never overwrites newer text.
    pub async fn save_draft(
        &self,
        actor: Option<PrincipalId>,
        command: SaveDraft,
    ) -> Result<SavedDraft, DraftError> {
        let owner = actor.ok_or(DraftError::Unauthenticated)?;
        let expected =
            DraftVersion::new(command.expected_version).ok_or(InvalidInput::ExpectedVersion)?;
        let title = Title::parse(&command.title).map_err(InvalidInput::from)?;
        let document = Document::parse(command.document).map_err(InvalidInput::from)?;
        match self
            .store
            .save_draft(owner, command.work_id, expected, &title, &document)
            .await?
        {
            SaveOutcome::Saved {
                version,
                updated_at,
            } => Ok(SavedDraft {
                work_id: command.work_id,
                version,
                updated_at,
            }),
            SaveOutcome::Conflict { current } => Err(DraftError::VersionConflict { current }),
            SaveOutcome::NotFound => Err(DraftError::NotFound),
        }
    }
}
