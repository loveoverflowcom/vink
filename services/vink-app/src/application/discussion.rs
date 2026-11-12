//! Discussion use cases. Vink owns comments (owner decision 03/11/2026); there is no external
//! forum behind these methods.
//!
//! Each use case authenticates the actor first, then validates raw input through the domain, then
//! hands one validated command to [`DiscussionStore`]. The store applies the pure domain decision
//! inside its transaction and reports which decision it applied; this module only maps that to a
//! result. Comment and thread IDs are generated here, never taken from the client.
//!
//! HTTP API v1 exposes every use case (D2T-02b/c). Writes need a server-resolved principal, which
//! production cannot produce until D-05. Moderation authority is an active grant created by an
//! operator and rechecked by the store at commit; it is never derived from work ownership.

use time::OffsetDateTime;

use super::Vink;
use super::ports::{DiscussionStore, StoreError};
use crate::domain::discussion::admission::{PolicySelection, Refusal};
use crate::domain::discussion::body::{BodyError, CommentBody};
use crate::domain::discussion::lifecycle::{
    CommentState, LifecycleError, ModerationVersion, Review, Transition,
};
pub use crate::domain::discussion::moderation::ModerationAction;
use crate::domain::discussion::moderation::{ModerationReason, ReasonError, decision_reason};
use crate::domain::discussion::page::{CursorScope, PageCursor, PageError, PageLimit};
use crate::domain::discussion::policy::Denied;
use crate::domain::discussion::settings::{
    CommentsMode, DiscussionSettings, SettingsError, SettingsGeneration,
};
use crate::domain::discussion::submission::{CommandKey, PayloadDigest};
use crate::domain::discussion::visibility::{CommentView, ReplyContext};
use crate::domain::ids::{CommentId, PrincipalId, ThreadId, WorkId};

/// Untrusted submission. Only [`Vink::submit_comment`] turns it into a [`NewComment`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitComment {
    pub work_id: WorkId,
    /// `None` starts a root; `Some` replies to that comment (root or reply) in the work's thread.
    pub target: Option<CommentId>,
    pub body: String,
    pub command_key: String,
}

/// A validated submission with server-chosen identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewComment {
    pub comment_id: CommentId,
    /// Used only if the work has no thread yet; an existing thread keeps its identity.
    pub thread_candidate: ThreadId,
    pub work_id: WorkId,
    pub target: Option<CommentId>,
    pub body: CommentBody,
    pub key: CommandKey,
    pub digest: PayloadDigest,
    /// Which admission policies run (#120). Always [`ADMISSION`] until D-08 changes.
    pub selection: PolicySelection,
}

/// The admission selection of this deployment: premoderation only (COM-02, D-08). The candidate
/// publish-first policy is not even shadowed while no trust authority or observation store exists
/// (`docs/architecture/discussion-admission.md`).
pub const ADMISSION: PolicySelection = PolicySelection::PREMODERATION;

/// What the store did with a submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitOutcome {
    Created {
        comment_id: CommentId,
        created_at: OffsetDateTime,
        /// The initial state the admission chose and the store wrote.
        state: CommentState,
    },
    /// The actor's standing refused the submission at commit; nothing was written.
    Refused(Refusal),
    /// An earlier commit with the same key and payload; nothing new was written.
    Replayed {
        comment_id: CommentId,
        state: CommentState,
    },
    KeyReused,
    WorkNotFound,
    HiddenReadOnly,
    CommentsClosed,
    TargetUnavailable,
}

/// A store's report of a guarded change: what it found, and the pure decision it applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guarded<T, E> {
    NotFound,
    Denied(Denied),
    Decided(Result<T, E>),
}

/// Untrusted moderation request. The moderator is the authenticated actor; whether they may
/// decide is answered by the store inside the decision's transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerateComment {
    pub comment_id: CommentId,
    pub expected_version: i64,
    pub action: ModerationAction,
    /// Required for reject/remove and shown to the comment's writer (MOD-04).
    pub reason: Option<String>,
}

/// A pending comment as a moderator reviews it. Shows the text and its writer's display name;
/// `can_decide` is false where this moderator must recuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueItem {
    pub comment_id: CommentId,
    pub work_id: WorkId,
    /// Title of the current public snapshot; `None` when the work is not public.
    pub work_title: Option<String>,
    pub is_root: bool,
    pub writer: AuthorName,
    pub body: String,
    pub created_at: OffsetDateTime,
    pub version: ModerationVersion,
    pub can_decide: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationQueue {
    /// All pending, present comments; the backlog size, not the page size.
    pub pending_total: i64,
    /// Oldest first.
    pub items: Vec<QueueItem>,
    pub next: Option<PageCursor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetCommentsMode {
    pub work_id: WorkId,
    pub expected_generation: i64,
    pub mode: CommentsMode,
}

/// Status of a comment as its own writer sees it. Never used for other readers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnStatus {
    Pending,
    Approved,
    Rejected,
    Removed,
}

impl From<CommentState> for OwnStatus {
    fn from(state: CommentState) -> Self {
        match (state.removal(), state.review()) {
            (Some(_), _) => Self::Removed,
            (None, Review::Pending) => Self::Pending,
            (None, Review::Approved) => Self::Approved,
            (None, Review::Rejected) => Self::Rejected,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmittedComment {
    pub comment_id: CommentId,
    pub status: OwnStatus,
    /// True when an earlier request with the same key had already committed this comment.
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorName {
    pub id: PrincipalId,
    pub display_name: String,
}

/// A comment the viewer may read. `view` is never [`CommentView::Unavailable`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentItem {
    pub id: CommentId,
    pub view: CommentView,
    pub author: AuthorName,
    pub body: String,
    pub created_at: OffsetDateTime,
    pub context: ReplyContext,
    /// Display name of the replied-to writer, present only when `context` is a public reply.
    pub context_author: Option<String>,
    /// The moderator's reason, only on the viewer's own rejected comment (MOD-04).
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootEntry {
    Comment(CommentItem),
    /// Hidden root kept as a placeholder for its public replies: no text, author or reason.
    Anchor {
        id: CommentId,
        created_at: OffsetDateTime,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootItem {
    pub entry: RootEntry,
    /// Public (approved, present) replies; the viewer's own pending replies are not counted.
    pub public_replies: i64,
    /// First replies the viewer may read, at most `PREVIEW_REPLIES`.
    pub preview: Vec<CommentItem>,
    /// Continue this root's replies after the preview, when more exist.
    pub preview_next: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootPage {
    pub settings: DiscussionSettings,
    /// Capability for the UI: only the work's author may open/close comments.
    pub viewer_is_work_author: bool,
    pub roots: Vec<RootItem>,
    pub next: Option<PageCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyPage {
    pub replies: Vec<CommentItem>,
    pub next: Option<PageCursor>,
}

/// A root page checked to hold nothing but public facts (#120 DISC-02/03). The only constructor
/// is the checked [`TryFrom`], so a value of this type cannot carry own pending/rejected
/// comments, rejection reasons or viewer capabilities, whatever the store returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicRootPage(RootPage);

/// A reply page read without a viewer, checked like [`PublicRootPage`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicReplyPage(ReplyPage);

impl PublicRootPage {
    pub const fn page(&self) -> &RootPage {
        &self.0
    }
}

impl PublicReplyPage {
    pub const fn page(&self) -> &ReplyPage {
        &self.0
    }
}

/// Holds only when every listed comment is public and carries no private field.
fn public_item(item: &CommentItem) -> bool {
    item.view == CommentView::Public && item.rejection_reason.is_none()
}

/// Refused: the page carried a viewer fact. A read that produced it fails closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotPublic;

impl TryFrom<RootPage> for PublicRootPage {
    type Error = NotPublic;

    fn try_from(page: RootPage) -> Result<Self, NotPublic> {
        let public = !page.viewer_is_work_author
            && page.roots.iter().all(|root| {
                root.preview.iter().all(public_item)
                    && match &root.entry {
                        RootEntry::Comment(item) => public_item(item),
                        RootEntry::Anchor { .. } => true,
                    }
            });
        public.then_some(Self(page)).ok_or(NotPublic)
    }
}

impl TryFrom<ReplyPage> for PublicReplyPage {
    type Error = NotPublic;

    fn try_from(page: ReplyPage) -> Result<Self, NotPublic> {
        page.replies
            .iter()
            .all(public_item)
            .then_some(Self(page))
            .ok_or(NotPublic)
    }
}

fn public_only<T: TryFrom<P, Error = NotPublic>, P>(page: P) -> Result<T, DiscussionError> {
    T::try_from(page).map_err(|NotPublic| {
        tracing::error!("an anonymous discussion read carried viewer state");
        DiscussionError::Internal
    })
}

/// Most own comments one exact lookup may name.
pub const MAX_OWN_LOOKUP: usize = 20;

/// Untrusted query of the private overlay: either a page of the viewer's unpublished comments
/// (`cursor`, `limit`) or an exact lookup of comment IDs (`ids`, comma-separated), never both.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnCommentsQuery {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub ids: Option<String>,
}

/// A validated overlay read. The actor is never part of it: the store scopes every row to the
/// authenticated viewer itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnRead {
    /// The viewer's present, unpublished (pending or rejected) comments in the work's thread,
    /// oldest first, wherever they sit in the public lists.
    Unpublished {
        after: Option<PageCursor>,
        limit: PageLimit,
    },
    /// The current state of these comments, if the viewer wrote them in this work: reconciles a
    /// submit result (pending → approved, rejected or removed) without trusting the receipt.
    Exact(Vec<CommentId>),
}

impl OwnRead {
    pub fn parse(query: &OwnCommentsQuery) -> Result<Self, DiscussionInput> {
        let Some(raw) = query.ids.as_deref() else {
            let after = query
                .cursor
                .as_deref()
                .map(|raw| PageCursor::decode_in(raw, CursorScope::Own))
                .transpose()?;
            return Ok(Self::Unpublished {
                after,
                limit: PageLimit::new(query.limit)?,
            });
        };
        if query.cursor.is_some() || query.limit.is_some() {
            return Err(DiscussionInput::OwnIds);
        }
        // Bounded before splitting: 20 UUIDs with commas.
        if raw.len() > MAX_OWN_LOOKUP * 37 {
            return Err(DiscussionInput::OwnIds);
        }
        let mut ids = Vec::new();
        for part in raw.split(',') {
            let id = CommentId::parse(part).ok_or(DiscussionInput::OwnIds)?;
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.len() > MAX_OWN_LOOKUP {
            return Err(DiscussionInput::OwnIds);
        }
        Ok(Self::Exact(ids))
    }
}

/// Where an own comment sits, as far as its writer may know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnPlacement {
    Root,
    Reply {
        /// The writer replied under this root, so its identity is theirs to know; its text and
        /// writer are not part of the overlay.
        root_id: CommentId,
        /// Whether the root is listed for this viewer now (shown or as an anchor). When it is not,
        /// the reply cannot appear in the public lists and the overlay is its only place.
        root_listed: bool,
        context: ReplyContext,
        /// Name of the replied-to writer, only when that context is public.
        context_author: Option<String>,
    },
}

/// One of the viewer's own comments, as only its writer reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnComment {
    pub id: CommentId,
    pub status: OwnStatus,
    pub created_at: OffsetDateTime,
    pub placement: OwnPlacement,
    /// The writer's text while the comment is present; never after a removal.
    pub body: Option<String>,
    /// The moderator's reason, only while the comment is rejected (MOD-04).
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnPage {
    pub items: Vec<OwnComment>,
    /// Only for [`OwnRead::Unpublished`]; an `o1.` cursor no public list accepts.
    pub next: Option<PageCursor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DiscussionInput {
    #[error("command_key: {0}")]
    CommandKey(#[from] crate::domain::discussion::submission::CommandKeyError),
    #[error("body: {0}")]
    Body(#[from] BodyError),
    #[error("{0}")]
    Page(#[from] PageError),
    #[error("expected version must be >= 1")]
    ExpectedVersion,
    #[error("reason: {0}")]
    Reason(#[from] ReasonError),
    #[error(
        "ids must be 1 to {MAX_OWN_LOOKUP} comma-separated comment IDs, without cursor or limit"
    )]
    OwnIds,
}

impl DiscussionInput {
    pub fn field(&self) -> &'static str {
        match self {
            Self::CommandKey(_) => "command_key",
            Self::Body(_) => "body",
            Self::Page(PageError::Limit) => "limit",
            Self::Page(PageError::Cursor) => "cursor",
            Self::ExpectedVersion => "expected_version",
            Self::Reason(_) => "reason",
            Self::OwnIds => "ids",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiscussionError {
    #[error("unauthenticated")]
    Unauthenticated,
    /// Suspended by Vink at the write's commit (#120 standing fence).
    #[error("principal suspended")]
    Suspended,
    /// Missing, not public, or hidden from this actor; deliberately indistinguishable.
    #[error("not found")]
    NotFound,
    /// The comment is visible but belongs to someone else (the work's author included).
    #[error("only the writer can remove this comment")]
    NotCommentWriter,
    #[error("moderation authority required")]
    NotModerator,
    /// The moderator wrote the comment or owns the work.
    #[error("moderator must recuse from this comment")]
    Recused,
    #[error("comments are closed for this work")]
    CommentsClosed,
    #[error("hidden work is read-only")]
    HiddenReadOnly,
    #[error("reply target is not available")]
    TargetUnavailable,
    #[error("command key was already used for a different comment")]
    KeyReused,
    /// The comments settings generation moved on; nothing was written.
    #[error("conflict; current generation is {current}")]
    Conflict { current: i64 },
    /// The comment's moderation version moved on; nothing was written.
    #[error("moderation conflict; current version is {current}")]
    ModerationConflict { current: i64 },
    #[error("comment was removed")]
    Removed,
    #[error("invalid input: {0}")]
    Invalid(#[from] DiscussionInput),
    #[error("storage unavailable")]
    Unavailable,
    #[error("internal error")]
    Internal,
}

impl From<StoreError> for DiscussionError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            StoreError::UnknownPrincipal => Self::Unauthenticated,
            StoreError::PrincipalSuspended => Self::Suspended,
            StoreError::Integrity(_) => Self::Internal,
        }
    }
}

fn denied(denied: Denied) -> DiscussionError {
    match denied {
        // Like drafts, a work the actor does not own is indistinguishable from a missing one.
        Denied::NotWorkAuthor | Denied::Hidden => DiscussionError::NotFound,
        Denied::NotCommentWriter => DiscussionError::NotCommentWriter,
        Denied::NotModerator => DiscussionError::NotModerator,
        Denied::Recused => DiscussionError::Recused,
    }
}

fn lifecycle(error: LifecycleError) -> DiscussionError {
    match error {
        LifecycleError::Removed => DiscussionError::Removed,
        LifecycleError::Stale { current } => DiscussionError::ModerationConflict {
            current: current.get(),
        },
        LifecycleError::VersionExhausted => DiscussionError::Internal,
    }
}

fn page(
    after: Option<&str>,
    limit: Option<u32>,
) -> Result<(Option<PageCursor>, PageLimit), DiscussionInput> {
    let after = after.map(PageCursor::decode).transpose()?;
    Ok((after, PageLimit::new(limit)?))
}

impl<S> Vink<S>
where
    S: DiscussionStore,
{
    /// Root list of a public work's discussion. Readable by anyone; `viewer` only adds the
    /// viewer's own pending/rejected comments and the work-author capability.
    pub async fn discussion(
        &self,
        viewer: Option<PrincipalId>,
        work_id: WorkId,
        after: Option<&str>,
        limit: Option<u32>,
    ) -> Result<RootPage, DiscussionError> {
        let (after, limit) = page(after, limit)?;
        self.store
            .root_page(work_id, viewer, after, limit)
            .await?
            .ok_or(DiscussionError::NotFound)
    }

    /// The public projection of the root list (#120 DISC-02/03): read without any actor, so it
    /// never depends on Identity, and checked to hold only public facts.
    pub async fn public_discussion(
        &self,
        work_id: WorkId,
        after: Option<&str>,
        limit: Option<u32>,
    ) -> Result<PublicRootPage, DiscussionError> {
        public_only(self.discussion(None, work_id, after, limit).await?)
    }

    pub async fn public_replies(
        &self,
        work_id: WorkId,
        root_id: CommentId,
        after: Option<&str>,
        limit: Option<u32>,
    ) -> Result<PublicReplyPage, DiscussionError> {
        public_only(self.replies(None, work_id, root_id, after, limit).await?)
    }

    /// The viewer's private overlay of a public work (#120 DISC-02/03): their own comments read
    /// apart from the public lists, so a pending comment is recoverable wherever it sits, under
    /// a root on a later page or under a root that is no longer listed. Only the authenticated
    /// writer's own rows are ever read; the store re-checks each one against the domain.
    pub async fn own_comments(
        &self,
        viewer: Option<PrincipalId>,
        work_id: WorkId,
        query: &OwnCommentsQuery,
    ) -> Result<OwnPage, DiscussionError> {
        let viewer = viewer.ok_or(DiscussionError::Unauthenticated)?;
        let read = OwnRead::parse(query)?;
        self.store
            .own_comments(work_id, viewer, &read)
            .await?
            .ok_or(DiscussionError::NotFound)
    }

    pub async fn replies(
        &self,
        viewer: Option<PrincipalId>,
        work_id: WorkId,
        root_id: CommentId,
        after: Option<&str>,
        limit: Option<u32>,
    ) -> Result<ReplyPage, DiscussionError> {
        let (after, limit) = page(after, limit)?;
        self.store
            .reply_page(work_id, root_id, viewer, after, limit)
            .await?
            .ok_or(DiscussionError::NotFound)
    }

    /// Submits a root or a reply. The result is pending review; a retry with the same key after
    /// a lost response returns the same comment instead of creating another.
    pub async fn submit_comment(
        &self,
        actor: Option<PrincipalId>,
        command: SubmitComment,
    ) -> Result<SubmittedComment, DiscussionError> {
        let actor = actor.ok_or(DiscussionError::Unauthenticated)?;
        let key = CommandKey::parse(&command.command_key).map_err(DiscussionInput::from)?;
        let body = CommentBody::parse(&command.body).map_err(DiscussionInput::from)?;
        let digest = PayloadDigest::of_submission(command.work_id, command.target, &body);
        let new = NewComment {
            comment_id: CommentId::generate(),
            thread_candidate: ThreadId::generate(),
            work_id: command.work_id,
            target: command.target,
            body,
            key,
            digest,
            selection: ADMISSION,
        };
        match self.store.submit_comment(actor, &new).await? {
            SubmitOutcome::Created {
                comment_id, state, ..
            } => Ok(SubmittedComment {
                comment_id,
                status: state.into(),
                replayed: false,
            }),
            SubmitOutcome::Refused(Refusal::Suspended) => Err(DiscussionError::Suspended),
            // The principal's record could not be read as active: like an unresolved session.
            SubmitOutcome::Refused(Refusal::StandingUnknown) => {
                Err(DiscussionError::Unauthenticated)
            }
            SubmitOutcome::Replayed { comment_id, state } => Ok(SubmittedComment {
                comment_id,
                status: state.into(),
                replayed: true,
            }),
            SubmitOutcome::KeyReused => Err(DiscussionError::KeyReused),
            SubmitOutcome::HiddenReadOnly => Err(DiscussionError::HiddenReadOnly),
            SubmitOutcome::WorkNotFound => Err(DiscussionError::NotFound),
            SubmitOutcome::CommentsClosed => Err(DiscussionError::CommentsClosed),
            SubmitOutcome::TargetUnavailable => Err(DiscussionError::TargetUnavailable),
        }
    }

    /// The writer removes their own comment. The work's author has no such power over others.
    pub async fn remove_own_comment(
        &self,
        actor: Option<PrincipalId>,
        work_id: WorkId,
        comment_id: CommentId,
    ) -> Result<OwnStatus, DiscussionError> {
        let actor = actor.ok_or(DiscussionError::Unauthenticated)?;
        match self
            .store
            .remove_own_comment(actor, work_id, comment_id)
            .await?
        {
            Guarded::NotFound => Err(DiscussionError::NotFound),
            Guarded::Denied(reason) => Err(denied(reason)),
            Guarded::Decided(Ok(Transition::Changed(state) | Transition::Unchanged(state))) => {
                Ok(state.into())
            }
            Guarded::Decided(Err(error)) => Err(lifecycle(error)),
        }
    }

    /// The work's author opens or closes comments on that work.
    pub async fn set_comments_mode(
        &self,
        actor: Option<PrincipalId>,
        command: SetCommentsMode,
    ) -> Result<DiscussionSettings, DiscussionError> {
        let actor = actor.ok_or(DiscussionError::Unauthenticated)?;
        let expected = SettingsGeneration::new(command.expected_generation)
            .ok_or(DiscussionInput::ExpectedVersion)?;
        match self
            .store
            .set_comments_mode(actor, command.work_id, expected, command.mode)
            .await?
        {
            Guarded::NotFound => Err(DiscussionError::NotFound),
            Guarded::Denied(reason) => Err(denied(reason)),
            Guarded::Decided(Ok(decided)) => Ok(decided.settings()),
            Guarded::Decided(Err(SettingsError::Stale { current })) => {
                Err(DiscussionError::Conflict {
                    current: current.generation().get(),
                })
            }
            Guarded::Decided(Err(SettingsError::GenerationExhausted)) => {
                Err(DiscussionError::Internal)
            }
        }
    }

    /// The pending queue, oldest first. Only an actor with an active grant may read it.
    pub async fn moderation_queue(
        &self,
        actor: Option<PrincipalId>,
        after: Option<&str>,
        limit: Option<u32>,
    ) -> Result<ModerationQueue, DiscussionError> {
        let actor = actor.ok_or(DiscussionError::Unauthenticated)?;
        let (after, limit) = page(after, limit)?;
        self.store
            .moderation_queue(actor, after, limit)
            .await?
            .ok_or(DiscussionError::NotModerator)
    }

    /// A moderation decision bound to the version the moderator reviewed (MOD-02). The store
    /// rechecks the actor's grant and recusal in the same transaction as the write.
    pub async fn moderate_comment(
        &self,
        actor: Option<PrincipalId>,
        command: ModerateComment,
    ) -> Result<CommentState, DiscussionError> {
        let actor = actor.ok_or(DiscussionError::Unauthenticated)?;
        let expected = ModerationVersion::new(command.expected_version)
            .ok_or(DiscussionInput::ExpectedVersion)?;
        let reason: Option<ModerationReason> =
            decision_reason(command.action, command.reason.as_deref())
                .map_err(DiscussionInput::from)?;
        match self
            .store
            .moderate_comment(
                actor,
                command.comment_id,
                expected,
                command.action,
                reason.as_ref(),
            )
            .await?
        {
            Guarded::NotFound => Err(DiscussionError::NotFound),
            Guarded::Denied(reason) => Err(denied(reason)),
            Guarded::Decided(Ok(Transition::Changed(state) | Transition::Unchanged(state))) => {
                Ok(state)
            }
            Guarded::Decided(Err(error)) => Err(lifecycle(error)),
        }
    }
}
