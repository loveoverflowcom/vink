//! Storage ports implemented by the PostgreSQL adapter.

use std::future::Future;

use time::OffsetDateTime;

use super::post_review::{RequestReview, ReviewAuthority, ReviewResult, ReviewWrite};

pub trait PostReviewStore: Send + Sync {
    fn request_review(
        &self,
        authority: ReviewAuthority,
        enabled: bool,
        request: &RequestReview,
    ) -> impl Future<Output = Result<ReviewWrite, StoreError>> + Send;
    fn record_review_result(
        &self,
        authority: ReviewAuthority,
        result: &ReviewResult,
    ) -> impl Future<Output = Result<ReviewWrite, StoreError>> + Send;
}

use super::catalog::{CatalogOperation, CuratorAuthority};
use super::discussion::{
    Guarded, ModerationAction, ModerationQueue, NewComment, OwnPage, OwnRead, ReplyPage, RootPage,
    SubmitOutcome,
};
use super::endorsement::CommentBatch;
use crate::domain::discovery::{PagePosition, PageSize, TagFilter, WorkQuery};
use crate::domain::discussion::lifecycle::{LifecycleError, ModerationVersion, Transition};
use crate::domain::discussion::moderation::ModerationReason;
use crate::domain::discussion::page::{PageCursor, PageLimit};
use crate::domain::discussion::settings::{
    CommentsMode, ModeChange, SettingsError, SettingsGeneration,
};
use crate::domain::document::{Document, Title};
use crate::domain::draft::{DraftChange, DraftSummary, DraftVersion, WorkingDraft};
use crate::domain::endorsement::{
    EndorsementCommand, EndorsementCount, EndorsementFacts, EndorsementOutcome, EndorsementState,
    EndorsementTargetFacts,
};
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, TagId, WorkId};
use crate::domain::moderation::{EvaluationTicket, WorkModeration};
use crate::domain::principal::{PenName, Standing, SubjectKey};
use crate::domain::publication::{AuthorOutcome, IntentGeneration, Publication};
use crate::domain::revision::ContentHash;
use crate::domain::taxonomy::{
    CatalogCommand, CatalogOutcome, CatalogTag, CatalogVersion, MetadataSnapshot, MetadataVersion,
    SlugHolder, SlugKind, TagAlias, TagChange, TagLabel, TagOutcome, TagSlug, WorkTags,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// The database could not be reached or refused service; retrying later may succeed.
    #[error("storage unavailable: {0}")]
    Unavailable(String),
    /// The principal has no Vink record.
    #[error("unknown principal")]
    UnknownPrincipal,
    /// The actor's principal is suspended at the write's commit (the standing fence). Nothing
    /// was written.
    #[error("principal suspended")]
    PrincipalSuspended,
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

/// One public work as a discovery row shows it, read through the publication pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicCardFacts {
    pub work_id: WorkId,
    pub title: Title,
    pub author_display_name: String,
    pub published_at: OffsetDateTime,
    /// The metadata snapshot bound in the same publication row; `None`: published without one.
    pub snapshot: Option<MetadataSnapshot>,
}

/// A search of the public library (TAG-PR-04, #98), already validated by the domain.
#[derive(Debug, Clone, Copy)]
pub struct DiscoveryRequest<'a> {
    pub query: Option<&'a WorkQuery>,
    pub filter: &'a TagFilter,
    /// Start after this position (a cursor's), newest first.
    pub after: Option<PagePosition>,
    /// The adapter reads at most `size.fetch()` works: one more than a page.
    pub size: PageSize,
}

/// Everything one search read, in ONE snapshot: the slug facts of each filter slug (in the
/// filter's order), the matching works newest first, and the catalog rows of every tag on their
/// snapshots. When `filter_tags` says the filter can match nothing, no work was searched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryFacts {
    /// The folded query used for matching. Absent when there is no query or the filters exclude
    /// every work without searching.
    pub query_key: Option<String>,
    pub filters: Vec<SlugFacts>,
    pub works: Vec<PublicCardFacts>,
    pub catalog: Vec<CatalogTag>,
}

/// The current public snapshot of a work, read in ONE snapshot through the publication pointer:
/// the pointed revision, the author's current display name, the bound metadata snapshot and the
/// catalog rows of its tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationFacts {
    pub work_id: WorkId,
    pub revision_id: RevisionId,
    pub title: Title,
    pub author_display_name: String,
    pub published_at: OffsetDateTime,
    pub document: Document,
    pub snapshot: Option<MetadataSnapshot>,
    pub catalog: Vec<CatalogTag>,
}

/// Public reads. Every read goes through the publication pointer: no working draft, private
/// draft tag, unpublished or withdrawn work is joined, matched or counted.
pub trait PublicationStore: Send + Sync {
    /// A bounded page of the public library matching `request` (TAG-PR-04, #98).
    fn discover_works(
        &self,
        request: &DiscoveryRequest<'_>,
    ) -> impl Future<Output = Result<DiscoveryFacts, StoreError>> + Send;

    /// Current public snapshot of a work, read through the publication pointer only.
    fn publication(
        &self,
        work_id: WorkId,
    ) -> impl Future<Output = Result<Option<PublicationFacts>, StoreError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorSnapshotCapture {
    pub revision_id: RevisionId,
    pub content_hash: ContentHash,
    pub metadata_snapshot_id: Option<uuid::Uuid>,
    pub draft_version: DraftVersion,
    pub metadata_version: MetadataVersion,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureSnapshotOutcome {
    Saved(AuthorSnapshotCapture),
    Conflict {
        current_draft_version: Option<DraftVersion>,
        current_metadata_version: Option<MetadataVersion>,
    },
    KeyReused,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorPublicRequestOutcome {
    pub outcome: AuthorOutcome,
    pub generation: IntentGeneration,
    pub ticket: Option<EvaluationTicket>,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestPublicOutcome {
    Accepted(AuthorPublicRequestOutcome),
    Stale {
        current_generation: IntentGeneration,
    },
    RevisionRejected,
    UnknownRevision,
    WorkUnavailable,
    KeyReused,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorPrivateOutcome {
    pub outcome: AuthorOutcome,
    pub generation: IntentGeneration,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetPrivateOutcome {
    Accepted(AuthorPrivateOutcome),
    Stale {
        current_generation: IntentGeneration,
    },
    WorkUnavailable,
    KeyReused,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorPublicationStateFacts {
    pub publication: Publication,
    pub moderation: WorkModeration,
    pub revision_metadata: Vec<(RevisionId, String, Option<uuid::Uuid>, OffsetDateTime)>,
}

/// Author publication commands and owner state persistence (PUB-RUNTIME-01, #158).
pub trait AuthorPublicationStore: Send + Sync {
    fn load_author_publication_state(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
    ) -> impl Future<Output = Result<Option<AuthorPublicationStateFacts>, StoreError>> + Send;

    fn capture_snapshot(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        command_key: &str,
        expected_draft_version: DraftVersion,
        expected_metadata_version: MetadataVersion,
    ) -> impl Future<Output = Result<CaptureSnapshotOutcome, StoreError>> + Send;

    fn request_public(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        command_key: &str,
        revision_id: RevisionId,
        expected_generation: IntentGeneration,
    ) -> impl Future<Output = Result<RequestPublicOutcome, StoreError>> + Send;

    fn set_private(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        command_key: &str,
        expected_generation: IntentGeneration,
    ) -> impl Future<Output = Result<SetPrivateOutcome, StoreError>> + Send;
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

    /// The private overlay of `viewer` in a public work's discussion (#120 DISC-02/03): only rows
    /// `viewer` wrote, each re-checked with `visibility::own_overlay`. `None`: the work is missing
    /// or not public.
    fn own_comments(
        &self,
        work_id: WorkId,
        viewer: PrincipalId,
        read: &OwnRead,
    ) -> impl Future<Output = Result<Option<OwnPage>, StoreError>> + Send;
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

/// The owner's draft tags as stored, with the catalog rows of those tags (retired ones too) and
/// of the successor hints of retired ones, each row once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredWorkTags {
    pub version: MetadataVersion,
    pub tags: WorkTags,
    pub catalog: Vec<CatalogTag>,
}

/// Draft tag storage. The contract is TAG-PR-01 (#95); the PostgreSQL adapter is TAG-PR-02 (#96,
/// `persistence::taxonomy`). A save is ONE transaction that, in this lock order, reads the actor's
/// principal row (FOR UPDATE, so a suspension commits before or after the whole save) and
/// standing, the work row (owner, FOR SHARE), the catalog rows of the selected tags (FOR SHARE,
/// so a retirement serializes with it) and the draft metadata row (FOR UPDATE), calls
/// [`crate::domain::taxonomy::decide_tag_change`], writes exactly an accepted
/// [`crate::domain::taxonomy::TagWrite`] and commits before answering. It never writes draft
/// text, a revision, a metadata snapshot or the publication binding. An actor without a
/// principal row is [`StoreError::UnknownPrincipal`].
pub trait WorkTagStore: Send + Sync {
    /// `None` when the work is missing or owned by someone else (indistinguishable).
    fn work_tags(
        &self,
        owner: PrincipalId,
        work_id: WorkId,
    ) -> impl Future<Output = Result<Option<StoredWorkTags>, StoreError>> + Send;

    fn save_work_tags(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        change: &TagChange,
    ) -> impl Future<Output = Result<TagOutcome, StoreError>> + Send;
}

/// The current publication of a work and the metadata snapshot bound beside its revision, read
/// from ONE publication row in one snapshot, with the catalog rows of the snapshot's tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicTagFacts {
    /// The revision bound in the same publication row as `snapshot`. A consumer that renders
    /// text from another read pairs the two by this identity.
    pub revision_id: RevisionId,
    /// `None`: published without metadata (every publication before TAG-PR-02).
    pub snapshot: Option<MetadataSnapshot>,
    pub catalog: Vec<CatalogTag>,
}

/// Public tag reads (TAG-PR-02, #96). Reads go through the current publication's binding only;
/// draft metadata is never joined. A missing or withdrawn work is `None`.
pub trait PublicTagStore: Send + Sync {
    fn public_work_tags(
        &self,
        work_id: WorkId,
    ) -> impl Future<Output = Result<Option<PublicTagFacts>, StoreError>> + Send;
}

/// The active part of the curated catalog, read in ONE snapshot: every active tag and every alias
/// of an active tag. Retired tags, former names and audit rows are not part of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveCatalog {
    pub tags: Vec<CatalogTag>,
    pub aliases: Vec<TagAlias>,
}

/// What one slug names, read in ONE snapshot: its row in the slug table, the tag it names and,
/// for a retired tag with a successor hint, the successor's row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlugFacts {
    pub holder: Option<SlugHolder>,
    pub tag: Option<CatalogTag>,
    pub successor: Option<CatalogTag>,
}

/// Public catalog reads (TAG-PR-03, #97). They never resolve an actor, read a draft or an audit
/// row, so an Identity outage cannot hide the catalog.
pub trait CatalogReadStore: Send + Sync {
    fn active_catalog(&self) -> impl Future<Output = Result<ActiveCatalog, StoreError>> + Send;

    fn slug_facts(
        &self,
        slug: &TagSlug,
    ) -> impl Future<Output = Result<SlugFacts, StoreError>> + Send;
}

/// One name of a tag in the operator listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogName {
    pub slug: TagSlug,
    pub kind: SlugKind,
    /// The curated spelling of an alias; `None` for canonical and former names.
    pub alias_label: Option<TagLabel>,
}

/// Every tag, retired ones included, with all its names: the operator's view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub tag: CatalogTag,
    pub names: Vec<CatalogName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogListing {
    pub version: CatalogVersion,
    pub entries: Vec<CatalogEntry>,
}

/// One audit row: the catalog version an accepted change produced and what it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogAuditRecord {
    pub version: CatalogVersion,
    pub operation: CatalogOperation,
    pub tag: TagId,
    pub slug: Option<TagSlug>,
    pub successor: Option<TagId>,
    pub changed: bool,
    pub authority: &'static str,
    pub database_role: String,
    pub recorded_at: OffsetDateTime,
}

/// Curated catalog governance (TAG-PR-03, #97 CHOICE-02 A). A change is ONE transaction: the
/// facts under the #96 lock order, [`crate::domain::taxonomy::decide_catalog_change`] with
/// `can_curate` decided from `authority` in that transaction, exactly the accepted write plus
/// its audit row, then commit. A refusal or conflict writes nothing, audit included.
pub trait CatalogStore: Send + Sync {
    fn change_catalog(
        &self,
        authority: CuratorAuthority,
        command: &CatalogCommand,
    ) -> impl Future<Output = Result<CatalogOutcome, StoreError>> + Send;

    fn catalog_listing(&self) -> impl Future<Output = Result<CatalogListing, StoreError>> + Send;

    /// Newest first, at most `limit` rows.
    fn catalog_history(
        &self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<CatalogAuditRecord>, StoreError>> + Send;
}

/// The principal an authenticated subject acts as, and whether it may act now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundPrincipal {
    pub principal: PrincipalId,
    pub standing: Standing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provisioned {
    pub bound: BoundPrincipal,
    /// `false` when the subject was already bound (a retry or a concurrent first request).
    pub created: bool,
}

/// Subject → principal bindings (D-05/S-03 spike). Implementations create the principal and its
/// binding atomically and never move a binding to another principal.
pub trait PrincipalStore: Send + Sync {
    fn principal_display_name(
        &self,
        principal: PrincipalId,
    ) -> impl Future<Output = Result<String, StoreError>> + Send;

    fn principal_for_subject(
        &self,
        subject: &SubjectKey,
    ) -> impl Future<Output = Result<Option<BoundPrincipal>, StoreError>> + Send;

    /// Binds `subject` to a new principal `candidate` named `pen_name`, or returns the principal
    /// it is already bound to. Exactly one principal per subject under concurrency.
    fn provision_principal(
        &self,
        subject: &SubjectKey,
        candidate: PrincipalId,
        pen_name: &PenName,
    ) -> impl Future<Output = Result<Provisioned, StoreError>> + Send;
}

/// One counted request of a fixed window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowCount {
    /// Requests of this network in this window, including the one just counted.
    pub hits: i64,
    /// Seconds until the window ends.
    pub seconds_left: i64,
}

/// Sign-in rate-limit counters (D-05). One atomic increment per request, shared by every App
/// process through PostgreSQL.
pub trait RateLimitStore: Send + Sync {
    fn count_request(
        &self,
        bucket: &str,
        network_key: &[u8; 32],
        window_seconds: i64,
    ) -> impl Future<Output = Result<WindowCount, StoreError>> + Send;

    /// Deletes windows that ended more than a day ago. Returns the rows deleted.
    fn prune_rate_limits(&self) -> impl Future<Output = Result<u64, StoreError>> + Send;
}
