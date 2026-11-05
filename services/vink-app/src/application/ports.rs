//! Storage ports implemented by the PostgreSQL adapter.

use std::future::Future;

use time::OffsetDateTime;

use super::discussion::{
    Guarded, ModerationAction, ModerationQueue, NewComment, ReplyPage, RootPage, SubmitOutcome,
};
use super::endorsement::CommentBatch;
use super::{PublicWork, PublicWorkSummary};
use crate::domain::discussion::lifecycle::{LifecycleError, ModerationVersion, Transition};
use crate::domain::discussion::moderation::ModerationReason;
use crate::domain::discussion::page::{PageCursor, PageLimit};
use crate::domain::discussion::settings::{
    CommentsMode, ModeChange, SettingsError, SettingsGeneration,
};
use crate::domain::draft::{DraftChange, DraftSummary, DraftVersion, WorkingDraft};
use crate::domain::endorsement::{
    EndorsementCommand, EndorsementCount, EndorsementFacts, EndorsementOutcome, EndorsementState,
    EndorsementTargetFacts,
};
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, WorkId};

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

    /// Receives validated content, not raw transport data. Ownership and expected version must
    /// still be checked atomically with the write; a `DraftChange` is not an authority witness.
    fn save_draft(
        &self,
        owner: PrincipalId,
        work_id: WorkId,
        change: &DraftChange,
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

/// Vink-owned discussion storage. Every write method runs one transaction that loads the facts
/// under locks, applies the named pure domain decision and writes only what that decision says;
/// it reports the decision through [`Guarded`] or [`SubmitOutcome`]. Reads return `None` when the
/// work (or root) is not readable, so callers cannot tell missing from hidden.
pub trait DiscussionStore: Send + Sync {
    /// `decide_submission` under share locks on the publication and thread rows (serialized with
    /// withdrawal and mode changes) and the target row; inserts comment + receipt atomically.
    fn submit_comment(
        &self,
        actor: PrincipalId,
        comment: &NewComment,
    ) -> impl Future<Output = Result<SubmitOutcome, StoreError>> + Send;

    /// `authorize_writer_removal` then `CommentState::remove_by_author`; drops the stored text.
    fn remove_own_comment(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        comment_id: CommentId,
    ) -> impl Future<Output = Result<Guarded<Transition, LifecycleError>, StoreError>> + Send;

    /// `authorize_mode_change` then `DiscussionSettings::change_mode` on the locked thread row;
    /// stores `ModeChange::write()` (a change or a reaffirmation), never an `AlreadyCurrent`.
    fn set_comments_mode(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        expected: SettingsGeneration,
        mode: CommentsMode,
    ) -> impl Future<Output = Result<Guarded<ModeChange, SettingsError>, StoreError>> + Send;

    /// In one transaction: the actor's active grant under a share lock (a revocation waits or
    /// wins), `authorize_moderation` (grant + recusal), then `ModerationAction::apply` on the
    /// locked comment row and an audit row naming the grant and reason. A non-moderator learns
    /// nothing about the comment (`Denied(NotModerator)` before any lookup).
    fn moderate_comment(
        &self,
        actor: PrincipalId,
        comment_id: CommentId,
        expected: ModerationVersion,
        action: ModerationAction,
        reason: Option<&ModerationReason>,
    ) -> impl Future<Output = Result<Guarded<Transition, LifecycleError>, StoreError>> + Send;

    /// Pending, present comments oldest first; `None` when `actor` holds no active grant.
    fn moderation_queue(
        &self,
        actor: PrincipalId,
        after: Option<PageCursor>,
        limit: PageLimit,
    ) -> impl Future<Output = Result<Option<ModerationQueue>, StoreError>> + Send;

    fn root_page(
        &self,
        work_id: WorkId,
        viewer: Option<PrincipalId>,
        after: Option<PageCursor>,
        limit: PageLimit,
    ) -> impl Future<Output = Result<Option<RootPage>, StoreError>> + Send;

    fn reply_page(
        &self,
        work_id: WorkId,
        root_id: CommentId,
        viewer: Option<PrincipalId>,
        after: Option<PageCursor>,
        limit: PageLimit,
    ) -> impl Future<Output = Result<Option<ReplyPage>, StoreError>> + Send;
}

/// What one endorsement transaction found and decided. `outcome` is exactly what
/// `decide_endorsement` returned for `facts` (capability, target facts, the actor's retained
/// record and the key's receipt, all read under the transaction's locks); the store wrote the
/// accepted write (state + receipt) and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndorsementWriteRecord {
    pub facts: EndorsementFacts,
    pub outcome: EndorsementOutcome,
    /// Selected rows of the command's target after the commit. Read only when the target is
    /// visible; `public_count` never shows it otherwise.
    pub count: EndorsementCount,
    /// Works only: the revision of the current publication, if any (public information).
    pub published: Option<RevisionId>,
}

/// Public facts of one requested comment of the work's thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentSummaryFacts {
    pub comment_id: CommentId,
    pub facts: EndorsementTargetFacts,
    pub count: EndorsementCount,
}

/// One read snapshot for the public summaries of a work's current revision and some of its
/// comments. `comments` holds only requested comments that exist in that work's thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorsementSummaryFacts {
    pub work: Option<EndorsementTargetFacts>,
    pub published: Option<RevisionId>,
    /// Selected rows of the current publication's revision.
    pub work_count: EndorsementCount,
    pub comments: Vec<CommentSummaryFacts>,
}

/// One requested comment and the actor's retained record of it, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnCommentFacts {
    pub comment_id: CommentId,
    pub facts: EndorsementTargetFacts,
    pub state: Option<EndorsementState>,
}

/// One read snapshot of the actor's own endorsements of a work's current revision and some of
/// its comments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnEndorsementFacts {
    pub can_endorse: bool,
    pub work: Option<EndorsementTargetFacts>,
    pub published: Option<RevisionId>,
    /// The actor's record of the current publication's revision.
    pub work_state: Option<EndorsementState>,
    pub comments: Vec<OwnCommentFacts>,
}

/// Vink-owned literary endorsement storage (LVE, #73). A write is ONE transaction that, in the
/// discussion lock order (publication FOR SHARE → thread FOR SHARE → comment FOR SHARE), then the
/// actor's command-key namespace (transaction advisory lock) and the actor's retained record (FOR
/// UPDATE), loads the facts and the key's receipt, calls `decide_endorsement`, writes the accepted
/// state with a conditional UPDATE or a conflict-checked INSERT (a concurrent first command makes
/// it read and decide again) plus the receipt, and commits before answering. Records and receipts
/// are never deleted. Reads run in one read-only snapshot and return facts; the application
/// projects them with the pure core. An actor without a principal row is
/// [`StoreError::UnknownPrincipal`].
pub trait EndorsementStore: Send + Sync {
    fn set_endorsement(
        &self,
        actor: PrincipalId,
        command: EndorsementCommand,
    ) -> impl Future<Output = Result<EndorsementWriteRecord, StoreError>> + Send;

    fn endorsement_summaries(
        &self,
        work_id: WorkId,
        comments: &CommentBatch,
    ) -> impl Future<Output = Result<EndorsementSummaryFacts, StoreError>> + Send;

    fn own_endorsements(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        comments: &CommentBatch,
    ) -> impl Future<Output = Result<OwnEndorsementFacts, StoreError>> + Send;
}
