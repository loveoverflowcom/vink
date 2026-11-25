//! Use cases. Application code authenticates the actor, validates input through the domain and
//! delegates I/O to the ports in [`ports`]; it imports no HTTP, SQL or Leptos types.

pub mod abuse;
pub mod account;
pub mod author_discovery;
pub mod author_works;
pub mod catalog;
pub mod discovery;
pub mod discussion;
pub mod drafts;
pub mod endorsement;
pub mod follow;
pub mod handle;
pub mod moderation;
pub mod ports;
pub mod post_review;
pub mod profile;
pub mod publication;
pub mod saved_works;
pub mod social_retirement;
pub mod taxonomy;
pub mod work_view;

use time::OffsetDateTime;

use crate::domain::document::{
    Document, DocumentData, DocumentError, DocumentFormat, Title, TitleAuthorityError, TitleError,
    TitledDocument,
};
use crate::domain::draft::{DraftChange, DraftSummary, DraftVersion, WorkingDraft};
use crate::domain::ids::{PrincipalId, RevisionId, WorkId};
use crate::domain::taxonomy::CatalogTag;
use ports::{DraftStore, SaveOutcome, StoreError};

/// Read model of the current public snapshot of a work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicWork {
    pub work_id: WorkId,
    pub revision_id: RevisionId,
    /// The published revision's format, source and title, checked against its content hash.
    pub content: TitledDocument,
    pub author: crate::application::profile::AuthorAttribution,
    pub published_at: OffsetDateTime,
    /// Public tags of the bound metadata snapshot: active only, snapshot order, current labels.
    pub tags: Vec<CatalogTag>,
}

/// Read model of the reader route: either a published work or a review_withdrawn notice (O-17@r2, #159).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReaderWork {
    Hidden {
        work_id: WorkId,
    },
    Published(PublicWork),
    ReviewWithdrawn {
        work_id: WorkId,
        author: crate::application::profile::AuthorAttribution,
    },
}

/// One work of a public list (home page or search, TAG-PR-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicWorkSummary {
    pub work_id: WorkId,
    pub title: Title,
    pub excerpt: Option<String>,
    pub author: crate::application::profile::AuthorAttribution,
    pub published_at: OffsetDateTime,
    /// The immutable first-public instant, when the publication history can establish it.
    pub first_published_at: Option<OffsetDateTime>,
    /// As in [`PublicWork::tags`].
    pub tags: Vec<CatalogTag>,
}

/// Untrusted save input. Only `save_draft` turns its raw fields into a [`DraftChange`].
/// `title` is required for `default` documents and refused for `markdown` ones (#263).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveDraft {
    pub work_id: WorkId,
    pub expected_version: i64,
    pub title: Option<String>,
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
    /// The explicit default title is invalid.
    #[error("title: {0}")]
    Title(#[from] TitleError),
    /// The title input contradicts its format or the first Markdown heading yields an invalid title.
    #[error("title: {0}")]
    TitleAuthority(TitleAuthorityError),
    #[error("document: {0}")]
    Document(#[from] DocumentError),
}

impl From<TitleAuthorityError> for InvalidInput {
    fn from(error: TitleAuthorityError) -> Self {
        match error {
            TitleAuthorityError::Title(title) => Self::Title(title),
            other => Self::TitleAuthority(other),
        }
    }
}

impl InvalidInput {
    pub fn field(&self) -> &'static str {
        match self {
            Self::ExpectedVersion => "expected_version",
            Self::Title(_) | Self::TitleAuthority(_) => "title",
            Self::Document(_) => "document",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DraftError {
    /// No authenticated Vink principal for this request.
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("invalid search: {0}")]
    Query(crate::domain::draft_query::DraftQueryError),
    /// Suspended by Vink at the write's commit (standing fence); nothing was written.
    #[error("principal suspended")]
    Suspended,
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
            StoreError::Integrity(_)
            | StoreError::UnknownPrincipal
            | StoreError::PrincipalSuspended => Self::Internal,
        }
    }
}

impl From<StoreError> for DraftError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            // The resolved principal has no Vink record: not an authenticated Vink actor.
            StoreError::UnknownPrincipal => Self::Unauthenticated,
            StoreError::PrincipalSuspended => Self::Suspended,
            StoreError::Integrity(_) => Self::Internal,
        }
    }
}

/// The Vink application facade shared by the HTTP API, the SSR host and the worker.
#[derive(Debug, Clone)]
pub struct Vink<S> {
    store: S,
}

impl<S> Vink<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &S {
        &self.store
    }
}

impl<S> Vink<S>
where
    S: DraftStore,
{
    pub async fn list_drafts(
        &self,
        actor: Option<PrincipalId>,
    ) -> Result<Vec<DraftSummary>, DraftError> {
        let owner = actor.ok_or(DraftError::Unauthenticated)?;
        Ok(self
            .search_drafts(Some(owner), drafts::SearchDrafts::default())
            .await?
            .drafts)
    }

    pub async fn create_draft(
        &self,
        actor: Option<PrincipalId>,
    ) -> Result<WorkingDraft, DraftError> {
        self.create_draft_with_format(actor, DocumentFormat::Default)
            .await
    }

    pub async fn create_draft_with_format(
        &self,
        actor: Option<PrincipalId>,
        format: DocumentFormat,
    ) -> Result<WorkingDraft, DraftError> {
        let owner = actor.ok_or(DraftError::Unauthenticated)?;
        Ok(self
            .store
            .create_draft(owner, WorkId::generate(), format)
            .await?)
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
        let document = Document::parse(command.document).map_err(InvalidInput::from)?;
        let content = TitledDocument::author(document, command.title.as_deref())
            .map_err(InvalidInput::from)?;
        let change = DraftChange::new(expected, content);
        match self
            .store
            .save_draft(owner, command.work_id, &change)
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

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "discussion_tests.rs"]
mod discussion_tests;

#[cfg(test)]
#[path = "endorsement_tests.rs"]
mod endorsement_tests;

#[cfg(test)]
#[path = "taxonomy_tests.rs"]
mod taxonomy_tests;

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod catalog_tests;

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod discovery_tests;

#[cfg(test)]
#[path = "work_view_tests.rs"]
mod work_view_tests;
