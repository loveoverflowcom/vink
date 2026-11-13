//! Canonical wire types of HTTP API v1, owned by this service. The contract text and fixtures
//! live in `services/vink-app/contracts/http/v1/`; consumers keep their own representations.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::application::catalog::{CatalogPage, TagResolution, TagSuggestion, facet_code};
use crate::application::discovery::{AppliedFilter, WorkPage};
use crate::application::discussion::{
    CommentItem, ModerationAction, ModerationQueue, OwnComment, OwnCommentsQuery, OwnPage,
    OwnPlacement, OwnStatus, PublicReplyPage, PublicRootPage, ReplyPage, RootEntry, RootItem,
    RootPage, SubmittedComment,
};
use crate::application::endorsement::{
    Capability, EndorsementOutcomeView, EndorsementResult, EndorsementSummaries, OwnEndorsement,
    OwnEndorsements, Summary,
};
use crate::application::taxonomy::{DraftWorkTags, SavedWorkTags};
use crate::application::{PublicWork, PublicWorkSummary, SavedDraft};
use crate::domain::discussion::lifecycle::{CommentState, Review};
use crate::domain::discussion::page::CursorScope;
use crate::domain::discussion::settings::DiscussionSettings;
use crate::domain::discussion::visibility::{CommentView, ReplyContext};
use crate::domain::document::DocumentData;
use crate::domain::draft::{DraftSummary, WorkingDraft};
use crate::domain::endorsement::{EndorsementState, EndorsementTarget};
use crate::domain::ids::{CommentId, PrincipalId, WorkId};
use crate::domain::taxonomy::{CatalogTag, MatchTier, TagFacet, WorkTags};

/// `GET /api/v1/works?q=&tag=&tag=&limit=&cursor=` (TAG-PR-04): one page of the public library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkList {
    /// Newest first, at most `limit` (default 12, at most 24).
    pub works: Vec<WorkCard>,
    /// What this answer is for, so a client can drop answers to an older search and canonicalize
    /// its URL.
    pub filters: WorkFilters,
    /// Present only when another matching work exists after this page; `null` is the end.
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkFilters {
    /// The query text exactly as received; `null` when absent or blank.
    pub q: Option<String>,
    /// One entry per distinct requested slug, in request order.
    pub tags: Vec<TagFilterView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagFilterView {
    /// The requested slug, normalized (NFC, lowercase).
    pub requested: String,
    /// `canonical`, `redirect` (filtering by `tag.slug`), `retired` (matches no work; `tag`
    /// carries the successor hint) or `not_found` (no tag has this slug; `tag` is `null`).
    pub resolution: &'static str,
    pub tag: Option<TagView>,
}

/// A tag as a public card or reader shows it: always active, current label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicTag {
    pub tag_id: String,
    pub facet: &'static str,
    pub label: String,
    pub slug: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkCard {
    pub work_id: String,
    pub title: String,
    pub author_display_name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub published_at: OffsetDateTime,
    /// Form first, then themes, in the snapshot's order.
    pub tags: Vec<PublicTag>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Publication {
    pub work_id: String,
    pub revision_id: String,
    pub title: String,
    pub author_display_name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub published_at: OffsetDateTime,
    pub document: DocumentData,
    pub tags: Vec<PublicTag>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReaderResponse {
    Hidden {
        work_id: String,
    },
    Published(Publication),
    ReviewWithdrawn {
        work_id: String,
        author_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftList {
    pub drafts: Vec<DraftCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftCard {
    pub work_id: String,
    pub title: String,
    pub version: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Draft {
    pub work_id: String,
    pub title: String,
    pub version: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub document: DocumentData,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveDraftRequest {
    pub expected_version: i64,
    pub title: String,
    pub document: DocumentData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SavedDraftResponse {
    pub work_id: String,
    pub version: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// Root page of a work's discussion, as seen by one viewer (never shared-cacheable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscussionPage {
    pub work_id: String,
    pub settings: CommentsSettings,
    pub viewer_is_work_author: bool,
    pub roots: Vec<RootThread>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CommentsSettings {
    pub comments_open: bool,
    pub generation: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RootThread {
    pub root: RootView,
    pub public_replies: i64,
    pub preview: Vec<CommentView1>,
    pub preview_next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RootView {
    Comment {
        comment: CommentView1,
    },
    /// A hidden root kept for its public replies: no text, author or reason.
    Anchor {
        comment_id: String,
        #[serde(with = "time::serde::rfc3339")]
        created_at: OffsetDateTime,
    },
}

/// One readable comment. Named `CommentView1` to keep the wire type apart from the domain's
/// `CommentView` projection it is built from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentView1 {
    pub comment_id: String,
    /// `public`, `own_pending` or `own_rejected`.
    pub visibility: &'static str,
    pub author_display_name: String,
    pub body: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    /// `None` on roots.
    pub reply_to: Option<ReplyTo>,
    /// Only the comment's writer may remove it.
    pub can_remove: bool,
    /// The moderator's reason, only on the viewer's own rejected comment.
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReplyTo {
    Root,
    Reply {
        comment_id: String,
        author_display_name: String,
    },
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplyList {
    pub replies: Vec<CommentView1>,
    pub next_cursor: Option<String>,
}

/// `?cursor=&limit=` of both discussion lists. Unknown query parameters are ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct PageQuery {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

/// Public projection of a work's root list (#120 DISC-02/03): read without resolving any actor
/// and holding only public facts, so it has no `visibility`, `can_remove`, rejection reason or
/// viewer capability at all. Same order, preview and cursors as [`DiscussionPage`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicDiscussionPage {
    pub work_id: String,
    pub settings: CommentsSettings,
    pub roots: Vec<PublicRootThread>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicRootThread {
    pub root: PublicRootView,
    pub public_replies: i64,
    pub preview: Vec<PublicComment>,
    pub preview_next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PublicRootView {
    Comment {
        comment: PublicComment,
    },
    Anchor {
        comment_id: String,
        #[serde(with = "time::serde::rfc3339")]
        created_at: OffsetDateTime,
    },
}

/// A public comment: approved and present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicComment {
    pub comment_id: String,
    pub author_display_name: String,
    pub body: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    /// `None` on roots.
    pub reply_to: Option<ReplyTo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicReplyList {
    pub replies: Vec<PublicComment>,
    pub next_cursor: Option<String>,
}

/// `?cursor=&limit=` or `?ids=` of the private overlay. Unknown parameters are ignored, like the
/// other discussion lists; a repeated parameter is refused by the query parser.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct OwnCommentsParams {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub ids: Option<String>,
}

impl From<OwnCommentsParams> for OwnCommentsQuery {
    fn from(params: OwnCommentsParams) -> Self {
        Self {
            cursor: params.cursor,
            limit: params.limit,
            ids: params.ids,
        }
    }
}

/// The viewer's private overlay (#120 DISC-02/03): authenticated, `no-store`, only the viewer's
/// own comments. Never part of a public representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnCommentList {
    pub work_id: String,
    pub comments: Vec<OwnCommentView>,
    /// `o1.` cursor of the unpublished list; `null` at its end and on exact lookups.
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnCommentView {
    pub comment_id: String,
    /// `pending`, `approved`, `rejected` or `removed`, read from the current row at this request.
    pub status: &'static str,
    /// The writer's text; `null` once removed.
    pub body: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub placement: OwnPlacementView,
    /// Present only while `rejected`.
    pub rejection_reason: Option<String>,
    /// The writer may remove a comment while it is present.
    pub can_remove: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OwnPlacementView {
    Root,
    Reply {
        root_id: String,
        /// Whether the root is listed for this viewer now; when `false` the reply cannot appear
        /// in the public lists and this overlay is its only place.
        root_listed: bool,
        reply_to: ReplyTo,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitCommentRequest {
    pub target_id: Option<String>,
    pub body: String,
    pub command_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubmittedCommentResponse {
    pub comment_id: String,
    pub status: &'static str,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemovedCommentResponse {
    pub comment_id: String,
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommentsSettingsRequest {
    pub expected_generation: i64,
    pub comments_open: bool,
}

/// Pending comments for a moderator, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModerationQueueResponse {
    pub pending_total: i64,
    pub items: Vec<QueueEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueueEntry {
    pub comment_id: String,
    pub work_id: String,
    pub work_title: Option<String>,
    /// `root` or `reply`.
    pub kind: &'static str,
    pub author_display_name: String,
    pub body: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    /// Send back as `expected_version` with the decision.
    pub version: i64,
    /// False where this moderator must recuse (own comment or own work).
    pub can_decide: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationActionDto {
    Approve,
    Reject,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModerationDecisionRequest {
    pub expected_version: i64,
    pub action: ModerationActionDto,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModerationDecisionResponse {
    pub comment_id: String,
    /// `pending`, `approved` or `rejected`.
    pub review: &'static str,
    pub removed: bool,
    pub version: i64,
}

/// `PUT /works/{work_id}/endorsement/me`: the desired state of the actor's endorsement of one
/// revision of the work, under an actor-scoped command key and the order the client last saw.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkEndorsementRequest {
    /// UUID chosen by the client; resend the same key and body to retry.
    pub command_key: String,
    pub expected_order: i64,
    pub endorsed: bool,
    /// The revision the page rendered; part of the target.
    pub revision_id: String,
}

/// `PUT /works/{work_id}/discussion/comments/{comment_id}/endorsement/me`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommentEndorsementRequest {
    pub command_key: String,
    pub expected_order: i64,
    pub endorsed: bool,
}

/// `?comment_ids=` of both endorsement batch reads. Unknown query parameters are rejected.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndorsementQuery {
    pub comment_ids: Option<String>,
}

/// One endorsement state of the actor. Account-scoped: only in `no-store` responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EndorsementStateView {
    pub selected: bool,
    /// Mutation version: advances only when `selected` changes.
    pub version: i64,
    /// Accepted order: advances on every accepted command. Send back as `expected_order`.
    pub order: i64,
}

/// The actor's own endorsement of one target, as read back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnEndorsementState {
    /// `unavailable`, `not_selected` or `selected`.
    pub state: &'static str,
    /// `null` when unavailable (no stored state is exposed).
    pub version: Option<i64>,
    pub order: Option<i64>,
    /// Advisory: selecting at `order` would be eligible now.
    pub can_endorse: bool,
    /// `own_content` or `not_permitted` when selecting is not possible; otherwise `null`.
    pub blocked_reason: Option<&'static str>,
}

/// A public aggregate. `unavailable` is never shown as zero; `count` is `null` then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EndorsementSummaryView {
    /// `available` or `unavailable`.
    pub status: &'static str,
    pub count: Option<i64>,
}

/// Result of an endorsement write, sent only after the commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EndorsementWriteResponse {
    pub work_id: String,
    /// The work target's revision; `null` for a comment.
    pub revision_id: Option<String>,
    /// `null` for a work target.
    pub comment_id: Option<String>,
    /// `accepted` (a new command advanced the order) or `replayed` (this key and body had
    /// already committed; nothing was written).
    pub outcome: &'static str,
    /// The current state: the one to show.
    pub endorsement: EndorsementStateView,
    /// `replayed` only: the state the earlier command committed (history, not current).
    pub committed: Option<EndorsementStateView>,
    pub summary: EndorsementSummaryView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkEndorsementSummary {
    pub status: &'static str,
    pub count: Option<i64>,
    /// The current publication's revision: the one `count` is about.
    pub revision_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentEndorsementSummary {
    pub comment_id: String,
    pub status: &'static str,
    pub count: Option<i64>,
}

/// `GET /works/{work_id}/endorsements`: public summaries, one snapshot, no viewer state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EndorsementSummaryList {
    pub work_id: String,
    pub work: WorkEndorsementSummary,
    pub comments: Vec<CommentEndorsementSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentOwnEndorsement {
    pub comment_id: String,
    #[serde(flatten)]
    pub own: OwnEndorsementState,
}

/// `GET /works/{work_id}/endorsements/me`: the actor's own states, one snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnEndorsementList {
    pub work_id: String,
    /// The current publication's revision: the work target `work` describes.
    pub revision_id: String,
    pub work: OwnEndorsementState,
    pub comments: Vec<CommentOwnEndorsement>,
}

/// One catalog tag as clients see it. `status` is `active` (selectable) or `retired` (kept where
/// it already is, never added); `successor` is the curator's hint on a retired tag, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagView {
    pub tag_id: String,
    pub facet: &'static str,
    pub label: String,
    pub slug: String,
    pub description: Option<String>,
    pub status: &'static str,
    pub successor: Option<TagRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagRef {
    pub tag_id: String,
    pub label: String,
    pub slug: String,
}

/// `GET /api/v1/tags?facet=`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagListQuery {
    pub facet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagList {
    pub tags: Vec<TagView>,
    pub truncated: bool,
}

/// `GET /api/v1/tags/suggestions?q=&facet=&limit=`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagSuggestionQuery {
    pub q: Option<String>,
    pub facet: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagSuggestionList {
    /// The `q` this answer is for, so a client can drop answers to an older input.
    pub query: String,
    pub suggestions: Vec<TagSuggestionView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagSuggestionView {
    pub tag: TagView,
    /// `exact`, `folded` or `prefix`.
    #[serde(rename = "match")]
    pub matched: &'static str,
    /// The alias spelling that matched, when it was not the tag's own label.
    pub via_alias: Option<String>,
}

/// `GET /api/v1/tags/slugs/{slug}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagResolutionView {
    /// `canonical`, `redirect` (use `tag.slug`) or `retired`.
    pub resolution: &'static str,
    pub tag: TagView,
}

/// `GET /api/v1/drafts/{work_id}/tags`: the author's private draft tags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftTags {
    pub work_id: String,
    pub version: i64,
    pub form: Option<TagView>,
    pub themes: Vec<TagView>,
}

/// `PUT /api/v1/drafts/{work_id}/tags`: the whole desired state. Every field is required, so a
/// client that does not know about a field cannot clear it by omission; `null`/`[]` clear.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveDraftTagsRequest {
    pub expected_version: i64,
    #[serde(deserialize_with = "present")]
    pub form: Option<String>,
    pub themes: Vec<String>,
}

/// A nullable field that must still be present: `deserialize_with` turns off serde's implicit
/// "missing `Option` is `None`".
fn present<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
}

/// Tag identities of a draft, form then themes in the author's order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagIds {
    pub form: Option<String>,
    pub themes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SavedDraftTagsResponse {
    pub work_id: String,
    pub version: i64,
    /// `false` for an accepted save that changed nothing (the version still advanced).
    pub changed: bool,
    pub form: Option<String>,
    pub themes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureSnapshotRequest {
    pub command_key: String,
    pub expected_draft_version: i64,
    pub expected_metadata_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapturedSnapshotResponse {
    pub outcome: &'static str,
    pub revision_id: String,
    pub content_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_snapshot_id: Option<String>,
    pub draft_version: i64,
    pub metadata_version: i64,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestPublicRequest {
    pub command_key: String,
    pub revision_id: String,
    pub expected_generation: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequestPublicResponse {
    pub outcome: &'static str,
    pub revision_id: String,
    pub generation: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket_id: Option<String>,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetPrivateRequest {
    pub command_key: String,
    pub expected_generation: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetPrivateResponse {
    pub outcome: &'static str,
    pub generation: i64,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthorPublicationState {
    pub work_id: String,
    pub audience: &'static str,
    pub lifecycle: &'static str,
    pub generation: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<PublishIntentView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer: Option<PublicPointerView>,
    pub revisions: Vec<AuthorRevisionView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_ticket: Option<OpenTicketView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublishIntentView {
    pub revision_id: String,
    pub generation: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicPointerView {
    pub revision_id: String,
    pub intent_generation: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthorRevisionView {
    pub can_appeal: bool,
    pub decision_reason: Option<String>,
    pub revision_id: String,
    pub title: String,
    pub content_hash: String,
    pub moderation: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_version: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_snapshot_id: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenTicketView {
    pub evaluation_id: String,
    pub revision_id: String,
    pub policy_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaseSummaryDto {
    pub title: Option<String>,
    pub id: String,
    pub subject_kind: String,
    pub work_id: String,
    pub revision_id: Option<String>,
    pub decision_version: i64,
    pub origin: String,
    pub version: i64,
    pub state: String,
    pub claimed_by: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoteRecordDto {
    pub id: String,
    pub author_id: String,
    pub note: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaseDetailDto {
    pub viewer_is_admin: bool,
    pub id: String,
    pub subject_kind: String,
    pub work_id: String,
    pub revision_id: Option<String>,
    /// SHA-256 of the exact submitted snapshot the case decides on.
    pub content_digest: Option<String>,
    pub title: Option<String>,
    /// The submitted snapshot itself (immutable revision), for the reviewer only.
    pub document: Option<DocumentData>,
    pub author_id: String,
    pub author_name: String,
    pub decision_version: i64,
    pub origin: String,
    pub version: i64,
    pub state: String,
    pub claimed_by: Option<String>,
    pub notes: Vec<NoteRecordDto>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CaseExpectedVersionReq {
    pub expected_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AddNoteReq {
    pub expected_version: i64,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TakeoverReq {
    pub expected_version: i64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConcludeReq {
    pub expected_version: i64,
    pub verdict: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaseStatusDto {
    pub status: &'static str,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConcludeOutcomeDto {
    pub status: &'static str,
    pub verdict: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreateGrantReq {
    pub grantee_kind: String,
    pub grantee_id: String,
    pub capability: String,
    pub scope_kind: String,
    pub scope_work_id: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateGrantRes {
    pub grant_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GrantFactsDto {
    pub id: String,
    pub grantee_kind: String,
    pub grantee_id: String,
    pub capability: String,
    pub scope_kind: String,
    pub scope_work_id: Option<String>,
    pub expires_at: Option<String>,
    pub revoked: bool,
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaseOpenedDto {
    pub case_id: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreateGroupReq {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateGroupRes {
    pub group_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GroupMemberReq {
    pub principal_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutomationStateDto {
    pub mode: &'static str,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SetAutomationModeReq {
    pub expected_version: i64,
    pub mode: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<&'static str>,
    /// `revision_changed` only: the revision now published.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_revision_id: Option<String>,
    /// Endorsement `version_conflict` only: the actor's current state, to reconcile from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endorsement: Option<EndorsementStateView>,
    /// Draft tags `version_conflict` only: the stored tags, to reconcile from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<TagIds>,
    /// `unknown_tag`, `tag_retired`, `wrong_facet`: the refused tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_id: Option<String>,
    /// `tag_retired` with a curator's hint: the suggested replacement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub successor_id: Option<String>,
}

impl From<&CatalogTag> for PublicTag {
    fn from(tag: &CatalogTag) -> Self {
        Self {
            tag_id: tag.id().to_string(),
            facet: facet_code(tag.facet()),
            label: tag.label().as_str().to_owned(),
            slug: tag.slug().as_str().to_owned(),
        }
    }
}

impl From<&PublicWorkSummary> for WorkCard {
    fn from(work: &PublicWorkSummary) -> Self {
        Self {
            work_id: work.work_id.to_string(),
            title: work.title.as_str().to_string(),
            author_display_name: work.author_display_name.clone(),
            published_at: work.published_at,
            tags: work.tags.iter().map(PublicTag::from).collect(),
        }
    }
}

impl From<&AppliedFilter> for TagFilterView {
    fn from(filter: &AppliedFilter) -> Self {
        let (resolution, tag) = match &filter.resolution {
            None => ("not_found", None),
            Some(resolved) => {
                let view = TagResolutionView::from(resolved);
                (view.resolution, Some(view.tag))
            }
        };
        Self {
            requested: filter.requested.as_str().to_owned(),
            resolution,
            tag,
        }
    }
}

impl From<&WorkPage> for WorkList {
    fn from(page: &WorkPage) -> Self {
        Self {
            works: page.works.iter().map(WorkCard::from).collect(),
            filters: WorkFilters {
                q: page.query.clone(),
                tags: page.filters.iter().map(TagFilterView::from).collect(),
            },
            next_cursor: page.next.map(|cursor| cursor.encode()),
        }
    }
}

impl From<&PublicWork> for Publication {
    fn from(work: &PublicWork) -> Self {
        Self {
            work_id: work.work_id.to_string(),
            revision_id: work.revision_id.to_string(),
            title: work.title.as_str().to_string(),
            author_display_name: work.author_display_name.clone(),
            published_at: work.published_at,
            document: work.document.to_data(),
            tags: work.tags.iter().map(PublicTag::from).collect(),
        }
    }
}

impl From<crate::application::ReaderWork> for ReaderResponse {
    fn from(work: crate::application::ReaderWork) -> Self {
        match work {
            crate::application::ReaderWork::Published(public) => {
                Self::Published(Publication::from(&public))
            }
            crate::application::ReaderWork::Hidden { work_id } => Self::Hidden {
                work_id: work_id.to_string(),
            },
            crate::application::ReaderWork::ReviewWithdrawn {
                work_id,
                author_display_name,
            } => Self::ReviewWithdrawn {
                work_id: work_id.to_string(),
                author_name: author_display_name,
            },
        }
    }
}

impl From<&crate::application::ReaderWork> for ReaderResponse {
    fn from(work: &crate::application::ReaderWork) -> Self {
        match work {
            crate::application::ReaderWork::Published(public) => {
                Self::Published(Publication::from(public))
            }
            crate::application::ReaderWork::Hidden { work_id } => Self::Hidden {
                work_id: work_id.to_string(),
            },
            crate::application::ReaderWork::ReviewWithdrawn {
                work_id,
                author_display_name,
            } => Self::ReviewWithdrawn {
                work_id: work_id.to_string(),
                author_name: author_display_name.clone(),
            },
        }
    }
}

impl From<&DraftSummary> for DraftCard {
    fn from(draft: &DraftSummary) -> Self {
        Self {
            work_id: draft.work_id.to_string(),
            title: draft.title.as_str().to_string(),
            version: draft.version.get(),
            updated_at: draft.updated_at,
        }
    }
}

impl From<&WorkingDraft> for Draft {
    fn from(draft: &WorkingDraft) -> Self {
        Self {
            work_id: draft.work_id.to_string(),
            title: draft.title.as_str().to_string(),
            version: draft.version.get(),
            updated_at: draft.updated_at,
            document: draft.document.to_data(),
        }
    }
}

impl From<&SavedDraft> for SavedDraftResponse {
    fn from(saved: &SavedDraft) -> Self {
        Self {
            work_id: saved.work_id.to_string(),
            version: saved.version.get(),
            updated_at: saved.updated_at,
        }
    }
}

pub const fn own_status(status: OwnStatus) -> &'static str {
    match status {
        OwnStatus::Pending => "pending",
        OwnStatus::Approved => "approved",
        OwnStatus::Rejected => "rejected",
        OwnStatus::Removed => "removed",
    }
}

impl From<DiscussionSettings> for CommentsSettings {
    fn from(settings: DiscussionSettings) -> Self {
        Self {
            comments_open: settings.accepts_new_comments(),
            generation: settings.generation().get(),
        }
    }
}

impl CommentView1 {
    fn new(item: &CommentItem, viewer: Option<PrincipalId>, is_root: bool) -> Self {
        let reply_to = (!is_root).then(|| reply_to(item.context, item.context_author.as_ref()));
        Self {
            comment_id: item.id.to_string(),
            visibility: match item.view {
                CommentView::Public => "public",
                CommentView::OwnPending => "own_pending",
                CommentView::OwnRejected => "own_rejected",
                // The store never lists unavailable comments; keep the mapping total.
                CommentView::Unavailable => "unavailable",
            },
            author_display_name: item.author.display_name.clone(),
            body: item.body.clone(),
            created_at: item.created_at,
            reply_to,
            can_remove: viewer == Some(item.author.id),
            rejection_reason: item.rejection_reason.clone(),
        }
    }
}

fn replies(items: &[CommentItem], viewer: Option<PrincipalId>) -> Vec<CommentView1> {
    items
        .iter()
        .map(|item| CommentView1::new(item, viewer, false))
        .collect()
}

impl RootThread {
    fn new(root: &RootItem, viewer: Option<PrincipalId>) -> Self {
        Self {
            root: match &root.entry {
                RootEntry::Comment(item) => RootView::Comment {
                    comment: CommentView1::new(item, viewer, true),
                },
                RootEntry::Anchor { id, created_at } => RootView::Anchor {
                    comment_id: id.to_string(),
                    created_at: *created_at,
                },
            },
            public_replies: root.public_replies,
            preview: replies(&root.preview, viewer),
            preview_next_cursor: root.preview_next.map(|cursor| cursor.encode()),
        }
    }
}

impl DiscussionPage {
    pub fn new(work_id: WorkId, page: &RootPage, viewer: Option<PrincipalId>) -> Self {
        Self {
            work_id: work_id.to_string(),
            settings: page.settings.into(),
            viewer_is_work_author: page.viewer_is_work_author,
            roots: page
                .roots
                .iter()
                .map(|root| RootThread::new(root, viewer))
                .collect(),
            next_cursor: page.next.map(|cursor| cursor.encode()),
        }
    }
}

fn reply_to(context: ReplyContext, context_author: Option<&String>) -> ReplyTo {
    match (context, context_author) {
        (ReplyContext::Root, _) => ReplyTo::Root,
        (ReplyContext::Reply(id), Some(name)) => ReplyTo::Reply {
            comment_id: id.to_string(),
            author_display_name: name.clone(),
        },
        // A context without a public writer name is reported as unavailable, never guessed.
        (ReplyContext::Reply(_), None) | (ReplyContext::Unavailable, _) => ReplyTo::Unavailable,
    }
}

impl PublicComment {
    /// Built only from a [`PublicRootPage`] or [`PublicReplyPage`], whose items are public.
    fn new(item: &CommentItem, is_root: bool) -> Self {
        Self {
            comment_id: item.id.to_string(),
            author_display_name: item.author.display_name.clone(),
            body: item.body.clone(),
            created_at: item.created_at,
            reply_to: (!is_root).then(|| reply_to(item.context, item.context_author.as_ref())),
        }
    }
}

impl PublicDiscussionPage {
    pub fn new(work_id: WorkId, public: &PublicRootPage) -> Self {
        let page = public.page();
        Self {
            work_id: work_id.to_string(),
            settings: page.settings.into(),
            roots: page
                .roots
                .iter()
                .map(|root| PublicRootThread {
                    root: match &root.entry {
                        RootEntry::Comment(item) => PublicRootView::Comment {
                            comment: PublicComment::new(item, true),
                        },
                        RootEntry::Anchor { id, created_at } => PublicRootView::Anchor {
                            comment_id: id.to_string(),
                            created_at: *created_at,
                        },
                    },
                    public_replies: root.public_replies,
                    preview: root
                        .preview
                        .iter()
                        .map(|item| PublicComment::new(item, false))
                        .collect(),
                    preview_next_cursor: root.preview_next.map(|cursor| cursor.encode()),
                })
                .collect(),
            next_cursor: page.next.map(|cursor| cursor.encode()),
        }
    }
}

impl From<&PublicReplyPage> for PublicReplyList {
    fn from(public: &PublicReplyPage) -> Self {
        let page = public.page();
        Self {
            replies: page
                .replies
                .iter()
                .map(|item| PublicComment::new(item, false))
                .collect(),
            next_cursor: page.next.map(|cursor| cursor.encode()),
        }
    }
}

impl From<&OwnComment> for OwnCommentView {
    fn from(own: &OwnComment) -> Self {
        Self {
            comment_id: own.id.to_string(),
            status: own_status(own.status),
            body: own.body.clone(),
            created_at: own.created_at,
            placement: match &own.placement {
                OwnPlacement::Root => OwnPlacementView::Root,
                OwnPlacement::Reply {
                    root_id,
                    root_listed,
                    context,
                    context_author,
                } => OwnPlacementView::Reply {
                    root_id: root_id.to_string(),
                    root_listed: *root_listed,
                    reply_to: reply_to(*context, context_author.as_ref()),
                },
            },
            rejection_reason: own.rejection_reason.clone(),
            can_remove: own.status != OwnStatus::Removed,
        }
    }
}

impl OwnCommentList {
    pub fn new(work_id: WorkId, page: &OwnPage) -> Self {
        Self {
            work_id: work_id.to_string(),
            comments: page.items.iter().map(OwnCommentView::from).collect(),
            next_cursor: page.next.map(|cursor| cursor.encode_in(CursorScope::Own)),
        }
    }
}

impl ReplyList {
    pub fn new(page: &ReplyPage, viewer: Option<PrincipalId>) -> Self {
        Self {
            replies: replies(&page.replies, viewer),
            next_cursor: page.next.map(|cursor| cursor.encode()),
        }
    }
}

impl From<&SubmittedComment> for SubmittedCommentResponse {
    fn from(submitted: &SubmittedComment) -> Self {
        Self {
            comment_id: submitted.comment_id.to_string(),
            status: own_status(submitted.status),
            replayed: submitted.replayed,
        }
    }
}

impl RemovedCommentResponse {
    pub fn new(comment_id: CommentId, status: OwnStatus) -> Self {
        Self {
            comment_id: comment_id.to_string(),
            status: own_status(status),
        }
    }
}

impl From<ModerationActionDto> for ModerationAction {
    fn from(action: ModerationActionDto) -> Self {
        match action {
            ModerationActionDto::Approve => Self::Approve,
            ModerationActionDto::Reject => Self::Reject,
            ModerationActionDto::Remove => Self::Remove,
        }
    }
}

impl From<&ModerationQueue> for ModerationQueueResponse {
    fn from(queue: &ModerationQueue) -> Self {
        Self {
            pending_total: queue.pending_total,
            items: queue
                .items
                .iter()
                .map(|item| QueueEntry {
                    comment_id: item.comment_id.to_string(),
                    work_id: item.work_id.to_string(),
                    work_title: item.work_title.clone(),
                    kind: if item.is_root { "root" } else { "reply" },
                    author_display_name: item.writer.display_name.clone(),
                    body: item.body.clone(),
                    created_at: item.created_at,
                    version: item.version.get(),
                    can_decide: item.can_decide,
                })
                .collect(),
            next_cursor: queue.next.map(|cursor| cursor.encode()),
        }
    }
}

impl ModerationDecisionResponse {
    pub fn new(comment_id: CommentId, state: CommentState) -> Self {
        Self {
            comment_id: comment_id.to_string(),
            review: match state.review() {
                Review::Pending => "pending",
                Review::Approved => "approved",
                Review::Rejected => "rejected",
            },
            removed: !state.is_present(),
            version: state.version().get(),
        }
    }
}

impl From<EndorsementState> for EndorsementStateView {
    fn from(state: EndorsementState) -> Self {
        Self {
            selected: state.selected(),
            version: state.version().get(),
            order: state.order().get(),
        }
    }
}

impl From<OwnEndorsement> for OwnEndorsementState {
    fn from(own: OwnEndorsement) -> Self {
        let (can_endorse, blocked_reason) = match own.capability {
            Capability::Allowed => (true, None),
            Capability::OwnContent => (false, Some("own_content")),
            Capability::NotPermitted => (false, Some("not_permitted")),
            Capability::Unavailable => (false, None),
        };
        Self {
            state: match own.state {
                None => "unavailable",
                Some(state) if state.selected() => "selected",
                Some(_) => "not_selected",
            },
            version: own.state.map(|state| state.version().get()),
            order: own.state.map(|state| state.order().get()),
            can_endorse,
            blocked_reason,
        }
    }
}

fn summary(summary: Summary) -> EndorsementSummaryView {
    match summary {
        Some(count) => EndorsementSummaryView {
            status: "available",
            count: Some(count.get()),
        },
        None => EndorsementSummaryView {
            status: "unavailable",
            count: None,
        },
    }
}

impl From<&EndorsementOutcomeView> for EndorsementWriteResponse {
    fn from(view: &EndorsementOutcomeView) -> Self {
        let (revision_id, comment_id) = match view.target {
            EndorsementTarget::Work { revision, .. } => (Some(revision.to_string()), None),
            EndorsementTarget::Comment { comment, .. } => (None, Some(comment.to_string())),
        };
        let (outcome, endorsement, committed) = match view.result {
            EndorsementResult::Accepted(state) => ("accepted", state.into(), None),
            EndorsementResult::Replayed {
                committed,
                observed,
            } => ("replayed", observed.into(), Some(committed.into())),
        };
        Self {
            work_id: view.target.work().to_string(),
            revision_id,
            comment_id,
            outcome,
            endorsement,
            committed,
            summary: summary(view.summary),
        }
    }
}

impl From<&EndorsementSummaries> for EndorsementSummaryList {
    fn from(summaries: &EndorsementSummaries) -> Self {
        Self {
            work_id: summaries.work_id.to_string(),
            work: WorkEndorsementSummary {
                status: "available",
                count: Some(summaries.work.get()),
                revision_id: summaries.revision.to_string(),
            },
            comments: summaries
                .comments
                .iter()
                .map(|entry| {
                    let view = summary(entry.summary);
                    CommentEndorsementSummary {
                        comment_id: entry.comment_id.to_string(),
                        status: view.status,
                        count: view.count,
                    }
                })
                .collect(),
        }
    }
}

impl From<&OwnEndorsements> for OwnEndorsementList {
    fn from(own: &OwnEndorsements) -> Self {
        Self {
            work_id: own.work_id.to_string(),
            revision_id: own.revision.to_string(),
            work: own.work.into(),
            comments: own
                .comments
                .iter()
                .map(|entry| CommentOwnEndorsement {
                    comment_id: entry.comment_id.to_string(),
                    own: entry.own.into(),
                })
                .collect(),
        }
    }
}

impl TagRef {
    fn new(tag: &CatalogTag) -> Self {
        Self {
            tag_id: tag.id().to_string(),
            label: tag.label().as_str().to_owned(),
            slug: tag.slug().as_str().to_owned(),
        }
    }
}

impl TagView {
    /// `successor` is the row of the hint `tag` names; the caller looked it up.
    pub fn new(tag: &CatalogTag, successor: Option<&CatalogTag>) -> Self {
        Self {
            tag_id: tag.id().to_string(),
            facet: facet_code(tag.facet()),
            label: tag.label().as_str().to_owned(),
            slug: tag.slug().as_str().to_owned(),
            description: tag.description().map(|d| d.as_str().to_owned()),
            status: if tag.status().is_active() {
                "active"
            } else {
                "retired"
            },
            successor: successor.map(TagRef::new),
        }
    }

    fn active(tag: &CatalogTag) -> Self {
        Self::new(tag, None)
    }
}

impl From<&CatalogPage> for TagList {
    fn from(page: &CatalogPage) -> Self {
        Self {
            tags: page.tags.iter().map(TagView::active).collect(),
            truncated: page.truncated,
        }
    }
}

pub const fn match_tier(tier: MatchTier) -> &'static str {
    match tier {
        MatchTier::Exact => "exact",
        MatchTier::Folded => "folded",
        MatchTier::Prefix => "prefix",
    }
}

impl TagSuggestionList {
    pub fn new(query: String, suggestions: &[TagSuggestion]) -> Self {
        Self {
            query,
            suggestions: suggestions
                .iter()
                .map(|found| TagSuggestionView {
                    tag: TagView::active(&found.tag),
                    matched: match_tier(found.tier),
                    via_alias: found.via_alias.as_ref().map(|l| l.as_str().to_owned()),
                })
                .collect(),
        }
    }
}

impl From<&TagResolution> for TagResolutionView {
    fn from(resolution: &TagResolution) -> Self {
        match resolution {
            TagResolution::Canonical(tag) => Self {
                resolution: "canonical",
                tag: TagView::active(tag),
            },
            TagResolution::Redirect(tag) => Self {
                resolution: "redirect",
                tag: TagView::active(tag),
            },
            TagResolution::Retired { tag, successor } => Self {
                resolution: "retired",
                tag: TagView::new(tag, successor.as_ref()),
            },
        }
    }
}

impl From<&DraftWorkTags> for DraftTags {
    fn from(draft: &DraftWorkTags) -> Self {
        let view = |tag: &CatalogTag| TagView::new(tag, draft.successor_of(tag));
        let (form, themes): (Vec<&CatalogTag>, Vec<&CatalogTag>) = draft
            .shown
            .iter()
            .partition(|tag| tag.facet() == TagFacet::Form);
        Self {
            work_id: draft.work_id.to_string(),
            version: draft.version.get(),
            form: form.first().map(|tag| view(tag)),
            themes: themes.into_iter().map(view).collect(),
        }
    }
}

impl From<&WorkTags> for TagIds {
    fn from(tags: &WorkTags) -> Self {
        Self {
            form: tags.form().map(|id| id.to_string()),
            themes: tags.themes().iter().map(ToString::to_string).collect(),
        }
    }
}

impl From<&SavedWorkTags> for SavedDraftTagsResponse {
    fn from(saved: &SavedWorkTags) -> Self {
        let ids = TagIds::from(&saved.tags);
        Self {
            work_id: saved.work_id.to_string(),
            version: saved.version.get(),
            changed: saved.changed,
            form: ids.form,
            themes: ids.themes,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use serde_json::Value;
    use time::macros::datetime;

    use super::*;
    use crate::application::{DraftError, InvalidInput, ReadError};
    use crate::domain::document::{BlockData, Document, DocumentError, SCHEMA_VERSION, Title};
    use crate::domain::draft::DraftVersion;
    use crate::domain::ids::{RevisionId, WorkId};
    use crate::http::ApiError;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/contracts/http/v1/fixtures");
    const AT: OffsetDateTime = datetime!(2026-10-30 08:00:00 UTC);

    fn fixture(name: &str) -> Value {
        let path = format!("{FIXTURES}/{name}");
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        serde_json::from_str(&raw).unwrap()
    }

    fn assert_matches<T: Serialize>(name: &str, value: &T) {
        assert_eq!(
            serde_json::to_value(value).unwrap(),
            fixture(name),
            "{name}"
        );
    }

    fn parse<T: DeserializeOwned>(name: &str) -> T {
        serde_json::from_value(fixture(name)).unwrap()
    }

    fn work() -> WorkId {
        WorkId::parse("33333333-3333-4333-8333-333333333333").unwrap()
    }

    fn title() -> Title {
        Title::parse("[Mẫu] Mưa trên phố cũ").unwrap()
    }

    fn document() -> Document {
        Document::parse(DocumentData {
            schema_version: SCHEMA_VERSION,
            blocks: vec![
                BlockData {
                    lines: vec![
                        "Phố cũ mưa giăng sợi mỏng,".into(),
                        "mái ngói rêu xanh lặng thầm.".into(),
                    ],
                },
                BlockData {
                    lines: vec!["    đã theo mưa về rất lâu.".into()],
                },
            ],
        })
        .unwrap()
    }

    #[test]
    fn public_responses_match_fixtures() {
        // The home page: the first page of the unfiltered search, an untagged publication.
        let page = WorkPage {
            query: None,
            filters: vec![],
            works: vec![PublicWorkSummary {
                work_id: work(),
                title: title(),
                author_display_name: "Tác giả mẫu (dữ liệu phát triển)".into(),
                published_at: AT,
                tags: vec![],
            }],
            next: None,
        };
        assert_matches("works.list.json", &WorkList::from(&page));
        let hidden = crate::application::ReaderWork::Hidden {
            work_id: WorkId::parse("0192f400-0000-7000-8000-000000000010").unwrap(),
        };
        assert_matches("reader.hidden.json", &ReaderResponse::from(hidden));
    }

    #[test]
    fn draft_messages_match_fixtures() {
        let draft = WorkingDraft {
            work_id: work(),
            version: DraftVersion::new(3).unwrap(),
            title: title(),
            document: document(),
            updated_at: AT,
        };
        assert_matches("draft.get.json", &Draft::from(&draft));
        let card = DraftSummary {
            work_id: WorkId::parse("01a18d55-f7bc-7158-96c1-a0f27ecc82b1").unwrap(),
            version: DraftVersion::FIRST,
            title: Title::parse("").unwrap(),
            updated_at: AT,
        };
        assert_matches(
            "drafts.list.json",
            &DraftList {
                drafts: vec![DraftCard::from(&card)],
            },
        );
        let saved = SavedDraft {
            work_id: work(),
            version: DraftVersion::new(4).unwrap(),
            updated_at: AT,
        };
        assert_matches(
            "draft.save.response.json",
            &SavedDraftResponse::from(&saved),
        );
        let request: SaveDraftRequest = parse("draft.save.request.json");
        assert_eq!(request.expected_version, 3);
        assert_eq!(Document::parse(request.document).unwrap(), document());
    }

    #[test]
    fn request_rejects_unknown_fields() {
        let mut raw = fixture("draft.save.request.json");
        raw["owner_id"] = Value::from("22222222-2222-4222-8222-222222222222");
        assert!(serde_json::from_value::<SaveDraftRequest>(raw).is_err());
    }

    #[test]
    fn error_envelopes_match_fixtures() {
        let cases = [
            (
                "error.validation_failed.json",
                ApiError::from(DraftError::Invalid(InvalidInput::Document(
                    DocumentError::BlankLine { block: 0, line: 1 },
                ))),
                422,
            ),
            (
                "error.unauthenticated.json",
                DraftError::Unauthenticated.into(),
                401,
            ),
            ("error.forbidden.json", ApiError::Forbidden, 403),
            ("error.not_found.json", ReadError::NotFound.into(), 404),
            (
                "error.version_conflict.json",
                DraftError::VersionConflict {
                    current: DraftVersion::new(5).unwrap(),
                }
                .into(),
                409,
            ),
            (
                "error.dependency_unavailable.json",
                ReadError::Unavailable.into(),
                503,
            ),
        ];
        for (name, error, status) in cases {
            assert_eq!(error.status().as_u16(), status, "{name}");
            assert_matches(name, &error.envelope());
        }
    }

    mod discussion {
        use super::*;
        use crate::application::discussion::{AuthorName, DiscussionError, DiscussionInput};
        use crate::domain::discussion::body::BodyError;
        use crate::domain::discussion::page::PageCursor;
        use crate::domain::discussion::settings::{CommentsMode, SettingsGeneration};
        use time::macros::datetime;
        use uuid::Uuid;

        const AT: OffsetDateTime = datetime!(2026-11-03 08:00:00 UTC);
        const REASON: &str = "Bình luận nêu danh tính đời thực của người khác. Hãy viết lại mà không kèm tên hoặc địa chỉ.";

        fn cid(n: u64) -> CommentId {
            CommentId::parse(&format!("0192f3a1-0000-7000-8000-{n:012}")).unwrap()
        }

        fn person(n: u128, name: &str) -> AuthorName {
            AuthorName {
                id: PrincipalId::from_uuid(Uuid::from_u128(n)),
                display_name: name.into(),
            }
        }

        fn viewer() -> AuthorName {
            person(99, "Bạn đọc")
        }

        fn item(
            n: u64,
            view: CommentView,
            author: AuthorName,
            body: &str,
            context: ReplyContext,
            context_author: Option<&str>,
        ) -> CommentItem {
            CommentItem {
                id: cid(n),
                view,
                author,
                body: body.into(),
                created_at: AT,
                context,
                context_author: context_author.map(Into::into),
                rejection_reason: None,
            }
        }

        #[test]
        fn discussion_page_and_replies_match_fixtures() {
            let lan = person(1, "Lan Anh");
            let minh = person(2, "Minh");
            let page = RootPage {
                settings: DiscussionSettings::INITIAL,
                viewer_is_work_author: false,
                roots: vec![
                    RootItem {
                        entry: RootEntry::Comment(item(
                            1,
                            CommentView::Public,
                            lan.clone(),
                            "Khổ cuối đọc mà nghe mưa thật 🌧️",
                            ReplyContext::Root,
                            None,
                        )),
                        public_replies: 4,
                        preview: vec![
                            item(
                                2,
                                CommentView::Public,
                                minh.clone(),
                                "Mình thích nhịp lục bát ở đây.",
                                ReplyContext::Root,
                                None,
                            ),
                            item(
                                3,
                                CommentView::Public,
                                person(3, "Hà"),
                                "Đồng ý,\nnhất là câu “mái ngói rêu xanh”.",
                                ReplyContext::Reply(cid(2)),
                                Some("Minh"),
                            ),
                            item(
                                4,
                                CommentView::OwnPending,
                                viewer(),
                                "Cảm ơn hai bạn 👩\u{200d}💻",
                                ReplyContext::Unavailable,
                                None,
                            ),
                        ],
                        preview_next: Some(PageCursor::after(AT, cid(4))),
                    },
                    RootItem {
                        entry: RootEntry::Anchor {
                            id: cid(6),
                            created_at: AT,
                        },
                        public_replies: 1,
                        preview: vec![item(
                            7,
                            CommentView::Public,
                            minh,
                            "Vẫn muốn giữ đoạn này.",
                            ReplyContext::Root,
                            None,
                        )],
                        preview_next: None,
                    },
                ],
                next: Some(PageCursor::after(AT, cid(6))),
            };
            assert_matches(
                "discussion.get.json",
                &DiscussionPage::new(work(), &page, Some(viewer().id)),
            );
            let rejected = CommentItem {
                rejection_reason: Some(REASON.into()),
                ..item(
                    9,
                    CommentView::OwnRejected,
                    viewer(),
                    "Tác giả thật là anh X ở phố Y.",
                    ReplyContext::Root,
                    None,
                )
            };
            let replies = ReplyPage {
                replies: vec![
                    item(
                        5,
                        CommentView::Public,
                        lan,
                        "Đọc lại vẫn thấy hay.",
                        ReplyContext::Root,
                        None,
                    ),
                    rejected,
                ],
                next: None,
            };
            assert_matches(
                "discussion.replies.json",
                &ReplyList::new(&replies, Some(viewer().id)),
            );
        }

        /// The public projection (#120 DISC-02/03) is the composed page without any viewer
        /// field: the same public comments, anchors, counts and cursors, nothing else.
        #[test]
        fn public_projection_matches_fixtures_and_refuses_viewer_state() {
            use crate::application::discussion::{NotPublic, PublicReplyPage, PublicRootPage};
            let lan = person(1, "Lan Anh");
            let minh = person(2, "Minh");
            let page = RootPage {
                settings: DiscussionSettings::INITIAL,
                viewer_is_work_author: false,
                roots: vec![
                    RootItem {
                        entry: RootEntry::Comment(item(
                            1,
                            CommentView::Public,
                            lan.clone(),
                            "Khổ cuối đọc mà nghe mưa thật 🌧️",
                            ReplyContext::Root,
                            None,
                        )),
                        public_replies: 4,
                        preview: vec![
                            item(
                                2,
                                CommentView::Public,
                                minh.clone(),
                                "Mình thích nhịp lục bát ở đây.",
                                ReplyContext::Root,
                                None,
                            ),
                            item(
                                3,
                                CommentView::Public,
                                person(3, "Hà"),
                                "Đồng ý,\nnhất là câu “mái ngói rêu xanh”.",
                                ReplyContext::Reply(cid(2)),
                                Some("Minh"),
                            ),
                            item(
                                5,
                                CommentView::Public,
                                lan.clone(),
                                "Đọc lại vẫn thấy hay.",
                                ReplyContext::Unavailable,
                                None,
                            ),
                        ],
                        preview_next: Some(PageCursor::after(AT, cid(5))),
                    },
                    RootItem {
                        entry: RootEntry::Anchor {
                            id: cid(6),
                            created_at: AT,
                        },
                        public_replies: 1,
                        preview: vec![item(
                            7,
                            CommentView::Public,
                            minh.clone(),
                            "Vẫn muốn giữ đoạn này.",
                            ReplyContext::Root,
                            None,
                        )],
                        preview_next: None,
                    },
                ],
                next: Some(PageCursor::after(AT, cid(6))),
            };
            let public = PublicRootPage::try_from(page.clone()).unwrap();
            assert_matches(
                "discussion.public.get.json",
                &PublicDiscussionPage::new(work(), &public),
            );
            let replies = ReplyPage {
                replies: vec![item(
                    8,
                    CommentView::Public,
                    minh,
                    "Câu này gợi nhớ phố cũ.",
                    ReplyContext::Reply(cid(5)),
                    Some("Lan Anh"),
                )],
                next: Some(PageCursor::after(AT, cid(8))),
            };
            assert_matches(
                "discussion.public.replies.json",
                &PublicReplyList::from(&PublicReplyPage::try_from(replies.clone()).unwrap()),
            );
            let json = serde_json::to_string(&PublicDiscussionPage::new(work(), &public)).unwrap();
            for private in [
                "visibility",
                "can_remove",
                "rejection_reason",
                "viewer_is_work_author",
            ] {
                assert!(!json.contains(private), "{private}");
            }
            // Negative controls: one viewer fact anywhere refuses the whole projection.
            let mut own_root = page.clone();
            if let RootEntry::Comment(root) = &mut own_root.roots[0].entry {
                root.view = CommentView::OwnPending;
            }
            let mut own_preview = page.clone();
            own_preview.roots[1].preview[0].view = CommentView::OwnRejected;
            let mut reasoned = page.clone();
            reasoned.roots[0].preview[0].rejection_reason = Some(REASON.into());
            let author_view = RootPage {
                viewer_is_work_author: true,
                ..page
            };
            for bad in [own_root, own_preview, reasoned, author_view] {
                assert_eq!(PublicRootPage::try_from(bad), Err(NotPublic));
            }
            let mut own_reply = replies;
            own_reply.replies[0].view = CommentView::OwnPending;
            assert_eq!(PublicReplyPage::try_from(own_reply), Err(NotPublic));
        }

        #[test]
        fn own_overlay_matches_its_fixture() {
            use crate::application::discussion::{OwnComment, OwnPage, OwnPlacement};
            let own =
                |n: u64, status, placement, body: Option<&str>, reason: Option<&str>| OwnComment {
                    id: cid(n),
                    status,
                    created_at: AT,
                    placement,
                    body: body.map(Into::into),
                    rejection_reason: reason.map(Into::into),
                };
            let page = OwnPage {
                items: vec![
                    own(
                        10,
                        OwnStatus::Pending,
                        OwnPlacement::Root,
                        Some("Một góc nhìn khác về khổ hai."),
                        None,
                    ),
                    // Under a root that is no longer listed: the overlay is its only place.
                    own(
                        11,
                        OwnStatus::Pending,
                        OwnPlacement::Reply {
                            root_id: cid(6),
                            root_listed: false,
                            context: ReplyContext::Root,
                            context_author: None,
                        },
                        Some("Mình vẫn nhớ đoạn này."),
                        None,
                    ),
                    own(
                        12,
                        OwnStatus::Rejected,
                        OwnPlacement::Reply {
                            root_id: cid(1),
                            root_listed: true,
                            context: ReplyContext::Reply(cid(2)),
                            context_author: Some("Minh".into()),
                        },
                        Some("Tác giả thật là anh X ở phố Y."),
                        Some(REASON),
                    ),
                ],
                next: Some(PageCursor::after(AT, cid(12))),
            };
            assert_matches(
                "discussion.me.get.json",
                &OwnCommentList::new(work(), &page),
            );
            let exact = OwnPage {
                items: vec![
                    own(
                        4,
                        OwnStatus::Approved,
                        OwnPlacement::Root,
                        Some("Cảm ơn hai bạn"),
                        None,
                    ),
                    own(9, OwnStatus::Removed, OwnPlacement::Root, None, None),
                ],
                next: None,
            };
            assert_matches(
                "discussion.me.ids.json",
                &OwnCommentList::new(work(), &exact),
            );
        }

        #[test]
        fn the_work_author_never_gets_can_remove_on_others_comments() {
            let item = item(
                1,
                CommentView::Public,
                person(1, "Lan Anh"),
                "x",
                ReplyContext::Root,
                None,
            );
            let work_author = Some(PrincipalId::from_uuid(Uuid::from_u128(42)));
            assert!(!CommentView1::new(&item, work_author, true).can_remove);
            assert!(!CommentView1::new(&item, None, true).can_remove);
            assert!(CommentView1::new(&item, Some(item.author.id), true).can_remove);
            // A public context without its writer's name is shown as unavailable, never guessed.
            let nameless = CommentItem {
                context: ReplyContext::Reply(cid(2)),
                ..item
            };
            assert_eq!(
                CommentView1::new(&nameless, None, false).reply_to,
                Some(ReplyTo::Unavailable)
            );
        }

        #[test]
        fn command_messages_match_fixtures() {
            let submitted = SubmittedComment {
                comment_id: cid(8),
                status: OwnStatus::Pending,
                replayed: false,
            };
            assert_matches(
                "discussion.comment.submit.response.json",
                &SubmittedCommentResponse::from(&submitted),
            );
            assert_matches(
                "discussion.comment.remove.response.json",
                &RemovedCommentResponse::new(cid(8), OwnStatus::Removed),
            );
            let closed = DiscussionSettings::restore(
                CommentsMode::Closed,
                SettingsGeneration::new(2).unwrap(),
            );
            assert_matches(
                "discussion.settings.response.json",
                &CommentsSettings::from(closed),
            );
            let request: SubmitCommentRequest = parse("discussion.comment.submit.request.json");
            assert_eq!(request.target_id, Some(cid(2).to_string()));
            let settings: CommentsSettingsRequest = parse("discussion.settings.request.json");
            assert_eq!(
                (settings.expected_generation, settings.comments_open),
                (1, false)
            );
            // A client cannot name an actor, a root or a role.
            for (name, field) in [
                ("discussion.comment.submit.request.json", "author_id"),
                ("discussion.comment.submit.request.json", "root_id"),
                ("discussion.settings.request.json", "owner_id"),
            ] {
                let mut raw = fixture(name);
                raw[field] = Value::from("22222222-2222-4222-8222-222222222222");
                let rejected = if name.contains("settings") {
                    serde_json::from_value::<CommentsSettingsRequest>(raw).is_err()
                } else {
                    serde_json::from_value::<SubmitCommentRequest>(raw).is_err()
                };
                assert!(rejected, "{name} accepted {field}");
            }
        }

        #[test]
        fn discussion_error_envelopes_match_fixtures() {
            let cases = [
                (
                    "error.comments_closed.json",
                    DiscussionError::CommentsClosed,
                    409,
                ),
                (
                    "error.target_unavailable.json",
                    DiscussionError::TargetUnavailable,
                    409,
                ),
                (
                    "error.idempotency_key_reused.json",
                    DiscussionError::KeyReused,
                    422,
                ),
                (
                    "error.not_comment_writer.json",
                    DiscussionError::NotCommentWriter,
                    403,
                ),
                (
                    "error.discussion_conflict.json",
                    DiscussionError::Conflict { current: 3 },
                    409,
                ),
                ("error.not_found.json", DiscussionError::NotFound, 404),
                (
                    "error.unauthenticated.json",
                    DiscussionError::Unauthenticated,
                    401,
                ),
                (
                    "error.dependency_unavailable.json",
                    DiscussionError::Unavailable,
                    503,
                ),
            ];
            for (name, error, status) in cases {
                let error = ApiError::from(error);
                assert_eq!(error.status().as_u16(), status, "{name}");
                assert_matches(name, &error.envelope());
            }
            let invalid = ApiError::from(DiscussionError::Invalid(DiscussionInput::Body(
                BodyError::Empty,
            )));
            assert_eq!(invalid.status().as_u16(), 422);
            assert_eq!(invalid.envelope().error.field, Some("body"));
        }

        #[test]
        fn moderation_messages_and_errors_match_fixtures() {
            use crate::application::discussion::{ModerationQueue, QueueItem};
            use crate::domain::discussion::lifecycle::{ModerationVersion, Review};

            let title = Some("[Mẫu] Mưa trên phố cũ".to_string());
            let queue = ModerationQueue {
                pending_total: 7,
                items: vec![
                    QueueItem {
                        comment_id: cid(10),
                        work_id: work(),
                        work_title: title.clone(),
                        is_root: true,
                        writer: person(2, "Minh"),
                        body: "Đoạn hai hơi dài, nhưng khổ cuối rất đẹp.".into(),
                        created_at: AT,
                        version: ModerationVersion::FIRST,
                        can_decide: true,
                    },
                    QueueItem {
                        comment_id: cid(11),
                        work_id: work(),
                        work_title: title,
                        is_root: false,
                        writer: person(7, "Lan"),
                        body: "Mình cũng thấy vậy.".into(),
                        created_at: AT,
                        version: ModerationVersion::FIRST,
                        can_decide: true,
                    },
                ],
                next: Some(PageCursor::after(AT, cid(11))),
            };
            assert_matches(
                "moderation.queue.json",
                &ModerationQueueResponse::from(&queue),
            );
            let request: ModerationDecisionRequest = parse("moderation.decision.request.json");
            assert_eq!(request.action, ModerationActionDto::Reject);
            assert_eq!(request.reason.as_deref(), Some(REASON));
            let rejected =
                CommentState::restore(Review::Rejected, None, ModerationVersion::new(2).unwrap());
            assert_matches(
                "moderation.decision.response.json",
                &ModerationDecisionResponse::new(cid(10), rejected),
            );
            // The moderator is the authenticated actor, never a body field.
            let mut raw = fixture("moderation.decision.request.json");
            raw["moderator_id"] = Value::from("22222222-2222-4222-8222-222222222222");
            assert!(serde_json::from_value::<ModerationDecisionRequest>(raw).is_err());
            let mut raw = fixture("moderation.decision.request.json");
            raw["action"] = Value::from("restore");
            assert!(serde_json::from_value::<ModerationDecisionRequest>(raw).is_err());

            for (name, error, status) in [
                (
                    "error.not_moderator.json",
                    DiscussionError::NotModerator,
                    403,
                ),
                (
                    "error.moderator_recused.json",
                    DiscussionError::Recused,
                    403,
                ),
                ("error.comment_removed.json", DiscussionError::Removed, 409),
                (
                    "error.moderation_conflict.json",
                    DiscussionError::ModerationConflict { current: 2 },
                    409,
                ),
            ] {
                let error = ApiError::from(error);
                assert_eq!(error.status().as_u16(), status, "{name}");
                assert_matches(name, &error.envelope());
            }
        }
    }

    /// LVE (#73): the provider serializes every endorsement message and error exactly as its
    /// fixture; requests decode and reject unknown fields (an actor, count or role cannot ride in).
    mod endorsement {
        use super::*;
        use crate::application::endorsement::{
            Capability, CommentSummary, EndorsementError, EndorsementInput, EndorsementOutcomeView,
            EndorsementResult, EndorsementSummaries, OwnCommentEndorsement, OwnEndorsement,
            OwnEndorsements,
        };
        use crate::domain::endorsement::{EndorsementCount, EndorsementState, EndorsementTarget};

        fn cid(n: u64) -> CommentId {
            CommentId::parse(&format!("0192f3a1-0000-7000-8000-{n:012}")).unwrap()
        }

        fn rid(raw: &str) -> RevisionId {
            RevisionId::parse(raw).unwrap()
        }

        fn state(selected: bool, version: i64, order: i64) -> EndorsementState {
            EndorsementState::restore(selected, version, order).unwrap()
        }

        fn count(raw: i64) -> Option<EndorsementCount> {
            EndorsementCount::new(raw)
        }

        const PUBLISHED: &str = "44444444-4444-4444-8444-444444444444";

        #[test]
        fn write_messages_match_fixtures() {
            let request: WorkEndorsementRequest = parse("endorsement.work.put.request.json");
            assert_eq!(
                request,
                WorkEndorsementRequest {
                    command_key: "0192f3a1-0000-7000-8000-00000000c0de".into(),
                    expected_order: 1,
                    endorsed: true,
                    revision_id: PUBLISHED.into(),
                }
            );
            let request: CommentEndorsementRequest = parse("endorsement.comment.put.request.json");
            assert_eq!((request.expected_order, request.endorsed), (3, false));

            assert_matches(
                "endorsement.work.put.response.json",
                &EndorsementWriteResponse::from(&EndorsementOutcomeView {
                    target: EndorsementTarget::Work {
                        work: work(),
                        revision: rid(PUBLISHED),
                    },
                    result: EndorsementResult::Accepted(state(true, 2, 2)),
                    summary: count(12),
                }),
            );
            assert_matches(
                "endorsement.comment.put.response.json",
                &EndorsementWriteResponse::from(&EndorsementOutcomeView {
                    target: EndorsementTarget::Comment {
                        work: work(),
                        comment: cid(2),
                    },
                    result: EndorsementResult::Replayed {
                        committed: state(false, 3, 4),
                        observed: state(true, 4, 5),
                    },
                    summary: count(7),
                }),
            );
        }

        #[test]
        fn requests_reject_client_supplied_authority() {
            for (name, field) in [
                ("endorsement.work.put.request.json", "actor_id"),
                ("endorsement.work.put.request.json", "count"),
                ("endorsement.comment.put.request.json", "role"),
                ("endorsement.comment.put.request.json", "work_id"),
            ] {
                let mut raw = fixture(name);
                raw[field] = Value::from("22222222-2222-4222-8222-222222222222");
                let rejected = if name.contains("work") {
                    serde_json::from_value::<WorkEndorsementRequest>(raw).is_err()
                } else {
                    serde_json::from_value::<CommentEndorsementRequest>(raw).is_err()
                };
                assert!(rejected, "{name} must reject {field}");
            }
            let mut raw = fixture("endorsement.work.put.request.json");
            raw.as_object_mut().unwrap().remove("revision_id");
            assert!(
                serde_json::from_value::<WorkEndorsementRequest>(raw).is_err(),
                "the revision is part of the target"
            );
            assert!(
                serde_json::from_value::<EndorsementQuery>(
                    serde_json::json!({"comment_ids": "x", "viewer": "me"})
                )
                .is_err()
            );
        }

        #[test]
        fn batch_reads_match_fixtures() {
            assert_matches(
                "endorsements.get.json",
                &EndorsementSummaryList::from(&EndorsementSummaries {
                    work_id: work(),
                    revision: rid(PUBLISHED),
                    work: EndorsementCount::new(12).unwrap(),
                    comments: vec![
                        CommentSummary {
                            comment_id: cid(1),
                            summary: count(3),
                        },
                        CommentSummary {
                            comment_id: cid(2),
                            summary: count(0),
                        },
                        CommentSummary {
                            comment_id: cid(9),
                            summary: None,
                        },
                    ],
                }),
            );
            assert_matches(
                "endorsements.me.get.json",
                &OwnEndorsementList::from(&OwnEndorsements {
                    work_id: work(),
                    revision: rid(PUBLISHED),
                    work: OwnEndorsement {
                        state: Some(state(true, 2, 3)),
                        capability: Capability::Allowed,
                    },
                    comments: vec![
                        OwnCommentEndorsement {
                            comment_id: cid(1),
                            own: OwnEndorsement {
                                state: Some(EndorsementState::INITIAL),
                                capability: Capability::Allowed,
                            },
                        },
                        OwnCommentEndorsement {
                            comment_id: cid(5),
                            own: OwnEndorsement {
                                state: Some(EndorsementState::INITIAL),
                                capability: Capability::OwnContent,
                            },
                        },
                        OwnCommentEndorsement {
                            comment_id: cid(9),
                            own: OwnEndorsement::UNAVAILABLE,
                        },
                    ],
                }),
            );
        }

        #[test]
        fn endorsement_error_envelopes_match_fixtures() {
            for (name, error, status) in [
                (
                    "error.self_endorsement.json",
                    EndorsementError::SelfEndorsement,
                    403,
                ),
                (
                    "error.endorsement_not_permitted.json",
                    EndorsementError::NotPermitted,
                    403,
                ),
                (
                    "error.endorsement_conflict.json",
                    EndorsementError::Conflict {
                        observed: state(false, 3, 4),
                    },
                    409,
                ),
                (
                    "error.revision_changed.json",
                    EndorsementError::RevisionChanged {
                        published: rid("0192f3a1-0000-7000-8000-000000000045"),
                    },
                    409,
                ),
                (
                    "error.endorsement_key_reused.json",
                    EndorsementError::KeyReused,
                    422,
                ),
                ("error.not_found.json", EndorsementError::NotFound, 404),
                (
                    "error.unauthenticated.json",
                    EndorsementError::Unauthenticated,
                    401,
                ),
                (
                    "error.dependency_unavailable.json",
                    EndorsementError::Unavailable,
                    503,
                ),
            ] {
                let error = ApiError::from(error);
                assert_eq!(error.status().as_u16(), status, "{name}");
                assert_matches(name, &error.envelope());
            }
            for (input, field) in [
                (EndorsementInput::ExpectedOrder, "expected_order"),
                (EndorsementInput::CommandKey, "command_key"),
                (EndorsementInput::Revision, "revision_id"),
                (EndorsementInput::TooManyComments, "comment_ids"),
            ] {
                let error = ApiError::from(EndorsementError::Invalid(input));
                assert_eq!(error.status().as_u16(), 422);
                assert_eq!(error.envelope().error.field, Some(field));
                assert_eq!(error.envelope().error.code, "validation_failed");
            }
            assert_eq!(
                ApiError::from(EndorsementError::Internal).status().as_u16(),
                500
            );
        }
    }

    mod taxonomy {
        use super::*;
        use crate::application::catalog::{CatalogPage, TagResolution, TagSuggestion};
        use crate::application::taxonomy::{
            DraftWorkTags, SavedWorkTags, TagError, TagInput, TagRejection,
        };
        use crate::domain::ids::TagId;
        use crate::domain::taxonomy::{
            CatalogTag, MatchTier, MetadataVersion, SelectionError, TagDescription, TagFacet,
            TagLabel, TagStatus, WorkTags,
        };

        fn tid(n: u64) -> TagId {
            TagId::parse(&format!("a0000000-0000-4000-8000-{n:012}")).unwrap()
        }

        fn tag(n: u64, facet: TagFacet, label: &str, description: Option<&str>) -> CatalogTag {
            status(n, facet, label, description, TagStatus::Active)
        }

        fn status(
            n: u64,
            facet: TagFacet,
            label: &str,
            description: Option<&str>,
            status: TagStatus,
        ) -> CatalogTag {
            let label = TagLabel::parse(label).unwrap();
            CatalogTag::restore(
                tid(n),
                facet,
                label.clone(),
                label.slug(),
                description.map(|d| TagDescription::parse(d).unwrap()),
                status,
            )
            .unwrap()
        }

        fn tan_van() -> CatalogTag {
            tag(
                1,
                TagFacet::Form,
                "Tản văn",
                Some("Văn xuôi ngắn viết từ trải nghiệm và cảm xúc riêng."),
            )
        }

        fn ky_uc() -> CatalogTag {
            tag(
                11,
                TagFacet::Theme,
                "Ký ức",
                Some("Những điều đã qua còn ở lại trong người viết."),
            )
        }

        fn do_thi() -> CatalogTag {
            status(
                13,
                TagFacet::Theme,
                "Đô thị",
                None,
                TagStatus::Retired {
                    successor: Some(tid(14)),
                },
            )
        }

        fn mua() -> CatalogTag {
            tag(14, TagFacet::Theme, "Mùa", None)
        }

        #[test]
        fn discovery_reads_match_fixtures() {
            use crate::application::discovery::AppliedFilter;
            use crate::domain::discovery::{DiscoveryCursor, PagePosition, SearchScope};
            use crate::domain::taxonomy::TagSlug;

            let author = "Lê Thu".to_string();
            let card =
                |n: u64, title: &str, minutes: i64, tags: Vec<CatalogTag>| PublicWorkSummary {
                    work_id: WorkId::parse(&format!("33333333-3333-4333-8333-{n:012}")).unwrap(),
                    title: Title::parse(title).unwrap(),
                    author_display_name: author.clone(),
                    published_at: AT - time::Duration::minutes(minutes),
                    tags,
                };
            let works = vec![
                card(1, "Mưa tháng Bảy", 0, vec![tan_van(), ky_uc()]),
                card(2, "Mùa mưa ở phố cũ", 90, vec![ky_uc()]),
            ];
            let last = &works[1];
            let next = DiscoveryCursor::new(
                PagePosition {
                    published_at: last.published_at,
                    work_id: last.work_id,
                },
                SearchScope::of(Some("mua"), &[ky_uc().id()]),
            );
            let page = WorkPage {
                query: Some("mưa".into()),
                filters: vec![AppliedFilter {
                    requested: TagSlug::parse("hồi-ức").unwrap(),
                    resolution: Some(TagResolution::Redirect(ky_uc())),
                }],
                works,
                next: Some(next),
            };
            assert_matches("works.search.json", &WorkList::from(&page));

            let unmatched = WorkPage {
                query: None,
                filters: vec![
                    AppliedFilter {
                        requested: TagSlug::parse("đô-thị").unwrap(),
                        resolution: Some(TagResolution::Retired {
                            tag: do_thi(),
                            successor: Some(mua()),
                        }),
                    },
                    AppliedFilter {
                        requested: TagSlug::parse("không-có").unwrap(),
                        resolution: None,
                    },
                ],
                works: vec![],
                next: None,
            };
            assert_matches("works.search.unmatched.json", &WorkList::from(&unmatched));

            let public = PublicWork {
                work_id: work(),
                revision_id: RevisionId::parse("44444444-4444-4444-8444-444444444444").unwrap(),
                title: title(),
                author_display_name: "Tác giả mẫu (dữ liệu phát triển)".into(),
                published_at: AT,
                document: document(),
                tags: vec![tan_van(), ky_uc()],
            };
            assert_matches("publication.get.json", &Publication::from(&public));
        }

        #[test]
        fn catalog_reads_match_fixtures() {
            let page = CatalogPage {
                tags: vec![tan_van(), tag(2, TagFacet::Form, "Thơ", None), ky_uc()],
                truncated: false,
            };
            assert_matches("tags.list.json", &TagList::from(&page));
            let suggestions = [
                TagSuggestion {
                    tag: tag(15, TagFacet::Theme, "Hội họa", None),
                    tier: MatchTier::Prefix,
                    via_alias: None,
                },
                TagSuggestion {
                    tag: ky_uc(),
                    tier: MatchTier::Prefix,
                    via_alias: Some(TagLabel::parse("Hồi ức").unwrap()),
                },
            ];
            assert_matches(
                "tags.suggestions.json",
                &TagSuggestionList::new("hồi".into(), &suggestions),
            );
            assert_matches(
                "tag.resolve.redirect.json",
                &TagResolutionView::from(&TagResolution::Redirect(ky_uc())),
            );
            assert_matches(
                "tag.resolve.retired.json",
                &TagResolutionView::from(&TagResolution::Retired {
                    tag: do_thi(),
                    successor: Some(mua()),
                }),
            );
        }

        #[test]
        fn draft_tag_messages_match_fixtures() {
            let tags = WorkTags::restore(Some(tid(1)), vec![tid(11), tid(13)]).unwrap();
            let draft = DraftWorkTags {
                work_id: work(),
                version: MetadataVersion::new(4).unwrap(),
                tags,
                shown: vec![tan_van(), ky_uc(), do_thi()],
                successors: vec![mua()],
            };
            assert_matches("draft.tags.get.json", &DraftTags::from(&draft));
            let request: SaveDraftTagsRequest = parse("draft.tags.put.request.json");
            assert_eq!(request.expected_version, 4);
            assert_eq!(request.form, Some(tid(1).to_string()));
            assert_eq!(request.themes, [tid(11).to_string(), tid(14).to_string()]);
            let saved = SavedWorkTags {
                work_id: work(),
                version: MetadataVersion::new(5).unwrap(),
                tags: WorkTags::restore(Some(tid(1)), vec![tid(11), tid(14)]).unwrap(),
                changed: true,
            };
            assert_matches(
                "draft.tags.put.response.json",
                &SavedDraftTagsResponse::from(&saved),
            );
        }

        /// The request is the whole desired state: an omitted field is never a clear, and no
        /// client field can name an actor, owner or grant.
        #[test]
        fn tag_request_requires_every_field_and_refuses_authority_fields() {
            let valid = fixture("draft.tags.put.request.json");
            for field in ["expected_version", "form", "themes"] {
                let mut raw = valid.clone();
                raw.as_object_mut().unwrap().remove(field);
                assert!(
                    serde_json::from_value::<SaveDraftTagsRequest>(raw).is_err(),
                    "missing {field} must be refused"
                );
            }
            let mut cleared = valid.clone();
            cleared["form"] = Value::Null;
            cleared["themes"] = Value::Array(Vec::new());
            let cleared: SaveDraftTagsRequest = serde_json::from_value(cleared).unwrap();
            assert_eq!((cleared.form, cleared.themes.len()), (None, 0));
            for forged in ["actor_id", "owner_id", "can_curate", "version"] {
                let mut raw = valid.clone();
                raw[forged] = Value::from("22222222-2222-4222-8222-222222222222");
                assert!(
                    serde_json::from_value::<SaveDraftTagsRequest>(raw).is_err(),
                    "{forged}"
                );
            }
        }

        #[test]
        fn tag_error_envelopes_match_fixtures() {
            let cases = [
                (
                    "error.tag_version_conflict.json",
                    ApiError::from(TagError::VersionConflict {
                        current: MetadataVersion::new(5).unwrap(),
                        observed: WorkTags::restore(Some(tid(1)), vec![tid(11), tid(12)]).unwrap(),
                    }),
                    409,
                ),
                (
                    "error.unknown_tag.json",
                    TagError::Rejected(TagRejection::UnknownTag {
                        tag: tid(99),
                        slot: TagFacet::Theme,
                    })
                    .into(),
                    422,
                ),
                (
                    "error.tag_retired.json",
                    TagError::Rejected(TagRejection::RetiredTag {
                        tag: tid(13),
                        slot: TagFacet::Theme,
                        successor: Some(tid(14)),
                    })
                    .into(),
                    409,
                ),
                (
                    "error.wrong_facet.json",
                    TagError::Rejected(TagRejection::WrongFacet {
                        tag: tid(11),
                        slot: TagFacet::Form,
                    })
                    .into(),
                    422,
                ),
                (
                    "error.too_many_themes.json",
                    TagError::Rejected(TagRejection::TooManyThemes).into(),
                    422,
                ),
            ];
            for (name, error, status) in cases {
                assert_eq!(error.status().as_u16(), status, "{name}");
                assert_matches(name, &error.envelope());
            }
            // Existing codes are reused where the meaning is the same.
            for (error, status, code) in [
                (TagError::Unauthenticated, 401, "unauthenticated"),
                (TagError::Suspended, 403, "principal_suspended"),
                (TagError::NotFound, 404, "not_found"),
                (TagError::Unavailable, 503, "dependency_unavailable"),
                (TagError::Internal, 500, "internal"),
            ] {
                let error = ApiError::from(error);
                assert_eq!(error.status().as_u16(), status, "{code}");
                assert_eq!(error.envelope().error.code, code);
            }
            let retired = ApiError::from(TagError::Rejected(TagRejection::RetiredTag {
                tag: tid(13),
                slot: TagFacet::Form,
                successor: None,
            }));
            let body = serde_json::to_value(retired.envelope()).unwrap();
            assert_eq!(body["error"]["field"], "form");
            assert!(body["error"].get("successor_id").is_none(), "{body}");
            for (input, field) in [
                (TagInput::ExpectedVersion, "expected_version"),
                (TagInput::Selection(SelectionError::Form), "form"),
                (
                    TagInput::Selection(SelectionError::Theme { index: 1 }),
                    "themes",
                ),
                (
                    TagInput::Selection(SelectionError::TooManyThemeRefs),
                    "themes",
                ),
            ] {
                let error = ApiError::from(TagError::Invalid(input));
                assert_eq!(error.status().as_u16(), 422);
                assert_eq!(error.envelope().error.code, "validation_failed");
                assert_eq!(error.envelope().error.field, Some(field));
            }
        }
    }
    /// The author's publication commands (PUB-AUTHOR-WEB-01): the Web desk decodes these exact
    /// fixtures, so a provider change to the state, the commands or their refusals fails both sides.
    mod author_publication {
        use super::*;

        const WORK: &str = "33333333-3333-4333-8333-333333333333";
        const R1: &str = "0192f3a1-0000-7000-8000-0000000000b1";
        const R2: &str = "0192f3a1-0000-7000-8000-0000000000b2";

        fn at(minutes: i64) -> OffsetDateTime {
            datetime!(2026-10-30 08:00:00 UTC) + time::Duration::minutes(minutes)
        }

        fn revision(
            id: &str,
            title: &str,
            hash: &str,
            moderation: &'static str,
            decision_version: Option<i64>,
            snapshot: &str,
            minutes: i64,
        ) -> AuthorRevisionView {
            AuthorRevisionView {
                can_appeal: false,
                decision_reason: None,
                revision_id: id.to_string(),
                title: title.to_string(),
                content_hash: hash.repeat(32).chars().take(64).collect(),
                moderation,
                decision_version,
                metadata_snapshot_id: Some(snapshot.to_string()),
                created_at: at(minutes),
            }
        }

        #[test]
        fn author_state_messages_match_fixtures() {
            let public_r1_pending_r2 = AuthorPublicationState {
                work_id: WORK.into(),
                audience: "public",
                lifecycle: "active",
                generation: 4,
                intent: Some(PublishIntentView {
                    revision_id: R2.into(),
                    generation: 4,
                }),
                pointer: Some(PublicPointerView {
                    revision_id: R1.into(),
                    intent_generation: 2,
                }),
                revisions: vec![
                    revision(
                        R1,
                        "[Mẫu] Mưa trên phố cũ",
                        "b1",
                        "approved",
                        Some(2),
                        "0192f3a1-0000-7000-8000-0000000000e1",
                        0,
                    ),
                    revision(
                        R2,
                        "[Mẫu] Mưa trên phố cũ — bản hai",
                        "b2",
                        "pending",
                        Some(1),
                        "0192f3a1-0000-7000-8000-0000000000e2",
                        30,
                    ),
                ],
                open_ticket: Some(OpenTicketView {
                    evaluation_id: "0192f3a1-0000-7000-8000-0000000000c2".into(),
                    revision_id: R2.into(),
                    policy_version: 1,
                }),
            };
            assert_matches(
                "publication.state.public_r1_pending_r2.json",
                &public_r1_pending_r2,
            );
            let mut rejected = revision(
                R1,
                "[Mẫu] Mưa trên phố cũ",
                "b1",
                "rejected",
                Some(2),
                "0192f3a1-0000-7000-8000-0000000000e1",
                0,
            );
            rejected.can_appeal = true;
            rejected.decision_reason = Some("Nội dung chưa phù hợp chính sách cộng đồng.".into());
            assert_matches(
                "publication.state.public_rejected.json",
                &AuthorPublicationState {
                    work_id: WORK.into(),
                    audience: "public",
                    lifecycle: "active",
                    generation: 3,
                    intent: Some(PublishIntentView {
                        revision_id: R1.into(),
                        generation: 3,
                    }),
                    pointer: None,
                    revisions: vec![rejected],
                    open_ticket: None,
                },
            );
            assert_matches(
                "publication.state.private.json",
                &AuthorPublicationState {
                    work_id: WORK.into(),
                    audience: "private",
                    lifecycle: "active",
                    generation: 1,
                    intent: None,
                    pointer: None,
                    revisions: vec![],
                    open_ticket: None,
                },
            );
        }

        #[test]
        fn command_messages_match_fixtures() {
            let capture: CaptureSnapshotRequest = parse("snapshot.capture.request.json");
            assert_eq!(
                (
                    capture.expected_draft_version,
                    capture.expected_metadata_version
                ),
                (7, 5)
            );
            let hash: String = "b2".repeat(32);
            assert_matches(
                "snapshot.capture.response.json",
                &CapturedSnapshotResponse {
                    outcome: "saved",
                    revision_id: R2.into(),
                    content_hash: hash,
                    metadata_snapshot_id: Some("0192f3a1-0000-7000-8000-0000000000e2".into()),
                    draft_version: 7,
                    metadata_version: 5,
                    replayed: false,
                },
            );
            let request: RequestPublicRequest = parse("publication.request_public.request.json");
            assert_eq!(
                (request.revision_id.as_str(), request.expected_generation),
                (R2, 3)
            );
            assert_matches(
                "publication.request_public.response.json",
                &RequestPublicResponse {
                    outcome: "submitted",
                    revision_id: R2.into(),
                    generation: 4,
                    ticket_id: Some("0192f3a1-0000-7000-8000-0000000000c2".into()),
                    replayed: false,
                },
            );
            // A replay does not carry the ticket: the client reads the state instead.
            assert_matches(
                "publication.request_public.response.replayed.json",
                &RequestPublicResponse {
                    outcome: "submitted",
                    revision_id: R2.into(),
                    generation: 4,
                    ticket_id: None,
                    replayed: true,
                },
            );
            let private: SetPrivateRequest = parse("publication.set_private.request.json");
            assert_eq!(private.expected_generation, 4);
            assert_matches(
                "publication.set_private.response.json",
                &SetPrivateResponse {
                    outcome: "made_private",
                    generation: 5,
                    replayed: false,
                },
            );
        }

        /// A client reconciles the right version because the refusal names the one that moved.
        #[test]
        fn publication_refusals_match_fixtures() {
            let cases = [
                (
                    "error.snapshot_draft_conflict.json",
                    ApiError::SnapshotConflict {
                        current_draft_version: Some(8),
                        current_metadata_version: None,
                    },
                    409,
                ),
                (
                    "error.snapshot_metadata_conflict.json",
                    ApiError::SnapshotConflict {
                        current_draft_version: None,
                        current_metadata_version: Some(6),
                    },
                    409,
                ),
                (
                    "error.generation_conflict.json",
                    ApiError::PublicationConflict {
                        current_generation: 5,
                    },
                    409,
                ),
                (
                    "error.revision_rejected.json",
                    ApiError::RevisionRejected,
                    409,
                ),
                (
                    "error.unknown_revision.json",
                    ApiError::UnknownRevision,
                    404,
                ),
                (
                    "error.work_unavailable.json",
                    ApiError::WorkUnavailable,
                    409,
                ),
                (
                    "error.publication_key_reused.json",
                    ApiError::PublicationKeyReused,
                    422,
                ),
            ];
            for (name, error, status) in cases {
                assert_eq!(error.status().as_u16(), status, "{name}");
                assert_matches(name, &error.envelope());
            }
        }

        /// Unknown request fields never select an actor, an owner or a moderation outcome.
        #[test]
        fn command_requests_refuse_authority_fields() {
            for (name, forged) in [
                ("snapshot.capture.request.json", "actor_id"),
                ("publication.request_public.request.json", "moderation"),
                ("publication.set_private.request.json", "owner_id"),
            ] {
                let mut raw = fixture(name);
                raw[forged] = Value::from("approved");
                let refused = match name {
                    "snapshot.capture.request.json" => {
                        serde_json::from_value::<CaptureSnapshotRequest>(raw).is_err()
                    }
                    "publication.request_public.request.json" => {
                        serde_json::from_value::<RequestPublicRequest>(raw).is_err()
                    }
                    _ => serde_json::from_value::<SetPrivateRequest>(raw).is_err(),
                };
                assert!(refused, "{name} accepted {forged}");
            }
        }
    }
}
