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

#[cfg(test)]
mod replay;
#[cfg(test)]
mod tests;
