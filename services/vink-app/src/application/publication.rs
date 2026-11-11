//! Owner-local planning use case (#150, #157). No persistent adapter or route yet.
//! Storage integration must execute this plan against freshly locked facts and commit every
//! effect atomically; this call alone does not publish anything or dispatch evaluation.
use super::ports::BoundPrincipal;
use crate::domain::discussion::lifecycle::CommentState;
use crate::domain::discussion::visibility::{CommentView, OwnOverlay, ReplyContext, RootListing};
use crate::domain::ids::{CommentId, PrincipalId};
use crate::domain::principal::Standing;
use crate::domain::publication::{
    AuthorCommand, AuthorTransition, Publication, PublicationError, WorkAccessPlan,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PublicationPlanError {
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("principal suspended")]
    Suspended,
    #[error(transparent)]
    Rejected(#[from] PublicationError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TombstoneWriteError {
    #[error("work is unavailable")]
    WorkUnavailable,
    #[error("tombstones are read-only")]
    TombstoneReadOnly,
    /// COM-11: the hidden page of a work made private keeps its thread readable, not writable.
    #[error("the hidden page is read-only")]
    HiddenReadOnly,
}

impl TombstoneWriteError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkUnavailable => "work_unavailable",
            Self::TombstoneReadOnly => "tombstone_read_only",
            Self::HiddenReadOnly => "hidden_read_only",
        }
    }
}

/// What a comment's reviewer (evaluator or person) may see of the work (COM-11): the readable
/// body only while the work is public; on the hidden page or tombstone only the comment's own
/// snapshot, never the withdrawn body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentReviewContext<'a> {
    WithWork(&'a crate::domain::publication::Snapshot),
    CommentOnly,
}

pub fn plan_comment_review_context(publication: &Publication) -> CommentReviewContext<'_> {
    match publication.access_plan() {
        WorkAccessPlan::PublicBody(snapshot) => CommentReviewContext::WithWork(snapshot),
        _ => CommentReviewContext::CommentOnly,
    }
}

pub fn plan_publication(
    actor: Option<BoundPrincipal>,
    current: &Publication,
    command: AuthorCommand,
) -> Result<AuthorTransition, PublicationPlanError> {
    let actor = actor.ok_or(PublicationPlanError::Unauthenticated)?;
    if actor.standing != Standing::Active {
        return Err(PublicationPlanError::Suspended);
    }
    Ok(current.decide_author(actor.principal, command)?)
}

/// Access classification for a work: public body, author tombstone, or unavailable (#157, AC-03).
pub fn plan_work_access(publication: &Publication) -> WorkAccessPlan<'_> {
    publication.access_plan()
}

/// Composes work access with discussion comment visibility (#157, AC-04).
/// When a work is private, never public, deleted while hidden, or removed by moderation,
/// its thread is completely unavailable. On a tombstone or public work, the existing
/// discussion visibility policy applies.
pub fn plan_comment_view(
    publication: &Publication,
    author: PrincipalId,
    state: CommentState,
    viewer: Option<PrincipalId>,
) -> CommentView {
    if !publication.access_plan().thread_accessible() {
        CommentView::Unavailable
    } else {
        crate::domain::discussion::visibility::view(author, state, viewer)
    }
}

/// Composes work access with discussion root listing (#157, AC-04).
pub fn plan_root_listing(
    publication: &Publication,
    author: PrincipalId,
    state: CommentState,
    public_replies: i64,
    viewer: Option<PrincipalId>,
) -> Option<RootListing> {
    if !publication.access_plan().thread_accessible() {
        None
    } else {
        let view = plan_comment_view(publication, author, state, viewer);
        crate::domain::discussion::visibility::root_listing(view, public_replies)
    }
}

/// Composes work access with discussion reply context (#157, AC-04).
pub fn plan_reply_context(
    publication: &Publication,
    replied_to_reply: bool,
    context: Option<(CommentId, CommentState)>,
) -> ReplyContext {
    if !publication.access_plan().thread_accessible() {
        ReplyContext::Unavailable
    } else {
        crate::domain::discussion::visibility::reply_context(replied_to_reply, context)
    }
}

/// Composes work access with discussion own overlay (#157, AC-04).
pub fn plan_own_overlay(
    publication: &Publication,
    writer: PrincipalId,
    state: CommentState,
    viewer: Option<PrincipalId>,
) -> Option<OwnOverlay> {
    if !publication.access_plan().thread_accessible() {
        None
    } else {
        let viewer = viewer?;
        crate::domain::discussion::visibility::own_overlay(writer, state, viewer)
    }
}

/// Enforces the tombstone write barrier: tombstones accept no new comments or replies (#157, AC-04, O-03).
pub fn check_comment_submission(publication: &Publication) -> Result<(), TombstoneWriteError> {
    match publication.access_plan() {
        WorkAccessPlan::PublicBody(_) => Ok(()),
        WorkAccessPlan::AuthorTombstone { .. } => Err(TombstoneWriteError::TombstoneReadOnly),
        WorkAccessPlan::AuthorHidden { .. } => Err(TombstoneWriteError::HiddenReadOnly),
        WorkAccessPlan::Unavailable => Err(TombstoneWriteError::WorkUnavailable),
    }
}

/// Work-level gate for a writer removing their own comment (#157, AC-04, O-03 baseline).
/// A tombstone keeps own removal; an unavailable work hides its thread, so removal answers as
/// if the comment did not exist. The comment's own lifecycle still decides the transition.
pub fn check_own_comment_removal(publication: &Publication) -> Result<(), TombstoneWriteError> {
    if publication.access_plan().thread_accessible() {
        Ok(())
    } else {
        Err(TombstoneWriteError::WorkUnavailable)
    }
}

use super::Vink;
use super::ports::{
    AuthorPrivateOutcome, AuthorPublicRequestOutcome, AuthorPublicationStateFacts,
    AuthorPublicationStore, AuthorSnapshotCapture, CaptureSnapshotOutcome, RequestPublicOutcome,
    SetPrivateOutcome, StoreError,
};
use crate::domain::discussion::submission::{CommandKey, CommandKeyError};
use crate::domain::draft::DraftVersion;
use crate::domain::ids::{RevisionId, WorkId};
use crate::domain::publication::IntentGeneration;
use crate::domain::taxonomy::MetadataVersion;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthorPublicationError {
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("principal suspended")]
    Suspended,
    #[error("not found")]
    NotFound,
    #[error("audience intent generation changed; current generation is {current_generation}")]
    Stale { current_generation: i64 },
    #[error("revision was rejected")]
    RevisionRejected,
    #[error("unknown revision")]
    UnknownRevision,
    #[error("work is unavailable")]
    WorkUnavailable,
    #[error("version conflict")]
    DraftOrMetadataConflict {
        current_draft_version: Option<i64>,
        current_metadata_version: Option<i64>,
    },
    #[error("command key was already used for a different payload")]
    KeyReused,
    #[error("invalid command key: {0}")]
    InvalidCommandKey(#[from] CommandKeyError),
    #[error("storage unavailable")]
    Unavailable,
    #[error("stored data failed validation")]
    Internal,
}

impl From<StoreError> for AuthorPublicationError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            StoreError::UnknownPrincipal => Self::Unauthenticated,
            StoreError::PrincipalSuspended => Self::Suspended,
            StoreError::Integrity(_) => Self::Internal,
        }
    }
}

impl<S> Vink<S>
where
    S: AuthorPublicationStore,
{
    pub async fn load_author_publication_state(
        &self,
        actor: Option<PrincipalId>,
        work_id: WorkId,
    ) -> Result<AuthorPublicationStateFacts, AuthorPublicationError> {
        let actor = actor.ok_or(AuthorPublicationError::Unauthenticated)?;
        self.store
            .load_author_publication_state(actor, work_id)
            .await?
            .ok_or(AuthorPublicationError::NotFound)
    }

    pub async fn capture_snapshot(
        &self,
        actor: Option<PrincipalId>,
        work_id: WorkId,
        command_key: &str,
        expected_draft_version: i64,
        expected_metadata_version: i64,
    ) -> Result<AuthorSnapshotCapture, AuthorPublicationError> {
        let actor = actor.ok_or(AuthorPublicationError::Unauthenticated)?;
        let key = CommandKey::parse(command_key)?;
        let draft_ver = DraftVersion::new(expected_draft_version).ok_or(
            AuthorPublicationError::DraftOrMetadataConflict {
                current_draft_version: None,
                current_metadata_version: None,
            },
        )?;
        let meta_ver = MetadataVersion::new(expected_metadata_version).ok_or(
            AuthorPublicationError::DraftOrMetadataConflict {
                current_draft_version: None,
                current_metadata_version: None,
            },
        )?;

        match self
            .store
            .capture_snapshot(actor, work_id, key.as_str(), draft_ver, meta_ver)
            .await?
        {
            CaptureSnapshotOutcome::Saved(capture) => Ok(capture),
            CaptureSnapshotOutcome::Conflict {
                current_draft_version,
                current_metadata_version,
            } => Err(AuthorPublicationError::DraftOrMetadataConflict {
                current_draft_version: current_draft_version.map(|v| v.get()),
                current_metadata_version: current_metadata_version.map(|v| v.get()),
            }),
            CaptureSnapshotOutcome::KeyReused => Err(AuthorPublicationError::KeyReused),
            CaptureSnapshotOutcome::NotFound => Err(AuthorPublicationError::NotFound),
        }
    }

    pub async fn request_public(
        &self,
        actor: Option<PrincipalId>,
        work_id: WorkId,
        command_key: &str,
        revision_id: RevisionId,
        expected_generation: i64,
    ) -> Result<AuthorPublicRequestOutcome, AuthorPublicationError> {
        let actor = actor.ok_or(AuthorPublicationError::Unauthenticated)?;
        let key = CommandKey::parse(command_key)?;
        let generation =
            IntentGeneration::new(expected_generation).ok_or(AuthorPublicationError::Stale {
                current_generation: 1,
            })?;

        match self
            .store
            .request_public(actor, work_id, key.as_str(), revision_id, generation)
            .await?
        {
            RequestPublicOutcome::Accepted(outcome) => Ok(outcome),
            RequestPublicOutcome::Stale { current_generation } => {
                Err(AuthorPublicationError::Stale {
                    current_generation: current_generation.get(),
                })
            }
            RequestPublicOutcome::RevisionRejected => Err(AuthorPublicationError::RevisionRejected),
            RequestPublicOutcome::UnknownRevision => Err(AuthorPublicationError::UnknownRevision),
            RequestPublicOutcome::WorkUnavailable => Err(AuthorPublicationError::WorkUnavailable),
            RequestPublicOutcome::KeyReused => Err(AuthorPublicationError::KeyReused),
            RequestPublicOutcome::NotFound => Err(AuthorPublicationError::NotFound),
        }
    }

    pub async fn set_private(
        &self,
        actor: Option<PrincipalId>,
        work_id: WorkId,
        command_key: &str,
        expected_generation: i64,
    ) -> Result<AuthorPrivateOutcome, AuthorPublicationError> {
        let actor = actor.ok_or(AuthorPublicationError::Unauthenticated)?;
        let key = CommandKey::parse(command_key)?;
        let generation =
            IntentGeneration::new(expected_generation).ok_or(AuthorPublicationError::Stale {
                current_generation: 1,
            })?;

        match self
            .store
            .set_private(actor, work_id, key.as_str(), generation)
            .await?
        {
            SetPrivateOutcome::Accepted(outcome) => Ok(outcome),
            SetPrivateOutcome::Stale { current_generation } => Err(AuthorPublicationError::Stale {
                current_generation: current_generation.get(),
            }),
            SetPrivateOutcome::WorkUnavailable => Err(AuthorPublicationError::WorkUnavailable),
            SetPrivateOutcome::KeyReused => Err(AuthorPublicationError::KeyReused),
            SetPrivateOutcome::NotFound => Err(AuthorPublicationError::NotFound),
        }
    }
}

#[cfg(test)]
mod replay;
#[cfg(test)]
mod tests;
