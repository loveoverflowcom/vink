//! The web app's own representations of the Vink HTTP API v1.
//!
//! The canonical contract and fixtures belong to the provider:
//! `services/vink-app/contracts/http/v1/`. Tests in this module decode those fixtures so a
//! provider change that breaks this consumer fails here. Identifiers and timestamps stay
//! strings: the browser displays and echoes them, it does not interpret them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentDto {
    pub schema_version: u16,
    pub blocks: Vec<BlockDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockDto {
    pub lines: Vec<String>,
}

/// `GET /api/v1/works` (TAG-PR-04): one page of the public library. Only the default request
/// (the home page) is consumed today; search, filter and paging UI belong to #100.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkList {
    pub works: Vec<WorkCard>,
    pub filters: WorkFilters,
    /// `None` is the end of the list.
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkFilters {
    pub q: Option<String>,
    pub tags: Vec<TagFilter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagFilter {
    pub requested: String,
    /// `canonical`, `redirect`, `retired` or `not_found`.
    pub resolution: String,
    pub tag: Option<TagDto>,
}

/// A catalog tag with its description and status (filters, tag resolution).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagDto {
    pub tag_id: String,
    pub facet: String,
    pub label: String,
    pub slug: String,
    pub description: Option<String>,
    pub status: String,
    pub successor: Option<TagRefDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagRefDto {
    pub tag_id: String,
    pub label: String,
    pub slug: String,
}

/// A public tag on a card or a publication: active, current label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicTagDto {
    pub tag_id: String,
    pub facet: String,
    pub label: String,
    pub slug: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCard {
    pub work_id: String,
    pub title: String,
    pub author_display_name: String,
    pub published_at: String,
    pub tags: Vec<PublicTagDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Publication {
    pub work_id: String,
    pub revision_id: String,
    pub title: String,
    pub author_display_name: String,
    pub published_at: String,
    pub document: DocumentDto,
    pub tags: Vec<PublicTagDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReaderResponse {
    Published(Publication),
    ReviewWithdrawn {
        work_id: String,
        author_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftList {
    pub drafts: Vec<DraftCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftCard {
    pub work_id: String,
    pub title: String,
    pub version: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub work_id: String,
    pub title: String,
    pub version: i64,
    pub updated_at: String,
    pub document: DocumentDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveDraftRequest {
    pub expected_version: i64,
    pub title: String,
    pub document: DocumentDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedDraft {
    pub work_id: String,
    pub version: i64,
    pub updated_at: String,
}

/// `GET /api/v1/tags`: active tags, forms first; `truncated` when more than 200 exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagList {
    pub tags: Vec<TagDto>,
    pub truncated: bool,
}

/// `GET /api/v1/tags/suggestions`: `query` echoes `q`, so an answer to older input is dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagSuggestions {
    pub query: String,
    pub suggestions: Vec<TagSuggestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagSuggestion {
    pub tag: TagDto,
    /// `exact`, `folded` or `prefix`; kept as text so a new value is not a crash.
    #[serde(rename = "match")]
    pub match_kind: String,
    pub via_alias: Option<String>,
}

/// `GET /api/v1/drafts/{id}/tags`: the actor's draft tags with their own `version`. A retired
/// tag already on the draft stays here with `status: "retired"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftTags {
    pub work_id: String,
    pub version: i64,
    pub form: Option<TagDto>,
    pub themes: Vec<TagDto>,
}

/// `PUT /api/v1/drafts/{id}/tags`: the whole desired state; every field is required, so `form:
/// null` and `themes: []` are sent explicitly and clear, never an omission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveDraftTagsRequest {
    pub expected_version: i64,
    pub form: Option<String>,
    pub themes: Vec<String>,
}

/// Answer of an accepted tag save: identities only, as committed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedDraftTags {
    pub work_id: String,
    pub version: i64,
    pub changed: bool,
    pub form: Option<String>,
    pub themes: Vec<String>,
}

/// The stored draft tags (IDs) carried by a tag `version_conflict`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredTags {
    pub form: Option<String>,
    pub themes: Vec<String>,
}

/// Actor-free wire DTOs. Only these enter the SSR resource/hydration payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicDiscussionPage {
    pub work_id: String,
    pub settings: CommentsSettings,
    pub roots: Vec<PublicRootThread>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicRootThread {
    pub root: PublicRootView,
    pub public_replies: i64,
    pub preview: Vec<PublicComment>,
    pub preview_next_cursor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PublicRootView {
    Comment {
        comment: PublicComment,
    },
    Anchor {
        comment_id: String,
        created_at: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicComment {
    pub comment_id: String,
    pub author_display_name: String,
    pub body: String,
    pub created_at: String,
    pub reply_to: Option<ReplyTo>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicReplyList {
    pub replies: Vec<PublicComment>,
    pub next_cursor: Option<String>,
}
impl From<PublicComment> for CommentDto {
    fn from(c: PublicComment) -> Self {
        Self {
            comment_id: c.comment_id,
            visibility: "public".into(),
            author_display_name: c.author_display_name,
            body: c.body,
            created_at: c.created_at,
            reply_to: c.reply_to,
            can_remove: false,
            rejection_reason: None,
        }
    }
}
impl From<PublicReplyList> for ReplyList {
    fn from(p: PublicReplyList) -> Self {
        Self {
            replies: p.replies.into_iter().map(Into::into).collect(),
            next_cursor: p.next_cursor,
        }
    }
}
impl From<PublicDiscussionPage> for DiscussionPage {
    fn from(p: PublicDiscussionPage) -> Self {
        Self {
            work_id: p.work_id,
            settings: p.settings,
            viewer_is_work_author: false,
            next_cursor: p.next_cursor,
            roots: p
                .roots
                .into_iter()
                .map(|r| RootThread {
                    root: match r.root {
                        PublicRootView::Comment { comment } => RootView::Comment {
                            comment: comment.into(),
                        },
                        PublicRootView::Anchor {
                            comment_id,
                            created_at,
                        } => RootView::Anchor {
                            comment_id,
                            created_at,
                        },
                    },
                    public_replies: r.public_replies,
                    preview: r.preview.into_iter().map(Into::into).collect(),
                    preview_next_cursor: r.preview_next_cursor,
                })
                .collect(),
        }
    }
}

/// A bounded composed read is used ONLY for this capability. Unknown fields (roots) are
/// discarded at decoding; no viewer projection is retained or used for public rendering.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiscussionCapability {
    pub work_id: String,
    pub viewer_is_work_author: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OwnCommentList {
    pub work_id: String,
    pub comments: Vec<OwnComment>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OwnComment {
    pub comment_id: String,
    pub status: CommentStatus,
    pub body: Option<String>,
    pub created_at: String,
    pub placement: OwnPlacement,
    pub rejection_reason: Option<String>,
    pub can_remove: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentStatus {
    Pending,
    Approved,
    Rejected,
    Removed,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OwnPlacement {
    Root,
    Reply {
        root_id: String,
        root_listed: bool,
        reply_to: ReplyTo,
    },
}

/// Root page of a work's discussion (two tiers: roots + one flat reply list per root).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscussionPage {
    pub work_id: String,
    pub settings: CommentsSettings,
    pub viewer_is_work_author: bool,
    pub roots: Vec<RootThread>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentsSettings {
    pub comments_open: bool,
    pub generation: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootThread {
    pub root: RootView,
    pub public_replies: i64,
    pub preview: Vec<CommentDto>,
    pub preview_next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RootView {
    Comment {
        comment: CommentDto,
    },
    /// Hidden root kept for its public replies: render a neutral placeholder, no author.
    Anchor {
        comment_id: String,
        created_at: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentDto {
    pub comment_id: String,
    /// `public`, `own_pending` or `own_rejected`; kept as text so a new value is not a crash.
    pub visibility: String,
    pub author_display_name: String,
    pub body: String,
    pub created_at: String,
    pub reply_to: Option<ReplyTo>,
    pub can_remove: bool,
    /// Moderator's reason, only on the viewer's own rejected comment.
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReplyTo {
    Root,
    Reply {
        comment_id: String,
        author_display_name: String,
    },
    /// Shown as “a comment that is no longer visible”, never as deleted by its writer.
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyList {
    pub replies: Vec<CommentDto>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitCommentRequest {
    pub target_id: Option<String>,
    pub body: String,
    /// Generated once per composed comment and reused on every retry of it.
    pub command_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmittedComment {
    pub comment_id: String,
    pub status: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedComment {
    pub comment_id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentsSettingsRequest {
    pub expected_generation: i64,
    pub comments_open: bool,
}

/// Pending comments for a moderator, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationQueue {
    pub pending_total: i64,
    pub items: Vec<QueueEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueEntry {
    pub comment_id: String,
    pub work_id: String,
    pub work_title: Option<String>,
    pub kind: String,
    pub author_display_name: String,
    pub body: String,
    pub created_at: String,
    pub version: i64,
    pub can_decide: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationDecisionRequest {
    pub expected_version: i64,
    /// `approve`, `reject` or `remove`.
    pub action: String,
    /// Required for `reject` and `remove`; shown to the comment's writer.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationDecision {
    pub comment_id: String,
    pub review: String,
    pub removed: bool,
    pub version: i64,
}

/// One endorsement state of the actor (LVE #73): `order` advances on every accepted command and
/// is what a client sends back as `expected_order`; `version` only when `selected` changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndorsementToken {
    pub selected: bool,
    pub version: i64,
    pub order: i64,
}

/// A public aggregate: `unavailable` carries `count: null` and is never shown as zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndorsementSummary {
    /// `available` or `unavailable`; kept as text so a new value is not a crash.
    pub status: String,
    pub count: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkEndorsementSummary {
    pub status: String,
    pub count: Option<i64>,
    /// The current publication's revision: the one `count` is about.
    pub revision_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentEndorsementSummary {
    pub comment_id: String,
    pub status: String,
    pub count: Option<i64>,
}

/// `GET /works/{id}/endorsements`: public summaries, one snapshot, no viewer state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndorsementSummaries {
    pub work_id: String,
    pub work: WorkEndorsementSummary,
    pub comments: Vec<CommentEndorsementSummary>,
}

/// The actor's own state of one target as read back; `version`/`order` are `null` when the
/// target is unavailable. `can_endorse`/`blocked_reason` are advisory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnEndorsement {
    /// `unavailable`, `not_selected` or `selected`.
    pub state: String,
    pub version: Option<i64>,
    pub order: Option<i64>,
    pub can_endorse: bool,
    /// `own_content`, `not_permitted` or `null`.
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentOwnEndorsement {
    pub comment_id: String,
    #[serde(flatten)]
    pub own: OwnEndorsement,
}

/// `GET /works/{id}/endorsements/me`: the actor's own states, one snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnEndorsements {
    pub work_id: String,
    /// The current publication's revision: the work target `work` describes.
    pub revision_id: String,
    pub work: OwnEndorsement,
    pub comments: Vec<CommentOwnEndorsement>,
}

/// `PUT /works/{id}/endorsement/me`. Resend the same key and body to reconcile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkEndorsementRequest {
    pub command_key: String,
    pub expected_order: i64,
    pub endorsed: bool,
    /// The revision the page rendered; part of the target.
    pub revision_id: String,
}

/// `PUT /works/{id}/discussion/comments/{comment_id}/endorsement/me`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentEndorsementRequest {
    pub command_key: String,
    pub expected_order: i64,
    pub endorsed: bool,
}

/// Result of an endorsement write, sent only after the commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndorsementWrite {
    pub work_id: String,
    pub revision_id: Option<String>,
    pub comment_id: Option<String>,
    /// `accepted` or `replayed`.
    pub outcome: String,
    /// The current state: the one to show.
    pub endorsement: EndorsementToken,
    /// `replayed` only: what the earlier command committed (history, never shown as current).
    pub committed: Option<EndorsementToken>,
    pub summary: EndorsementSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// `revision_changed` only: the revision now published.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_revision_id: Option<String>,
    /// Endorsement `version_conflict` only: the actor's current state, to reconcile from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endorsement: Option<EndorsementToken>,
    /// Draft-tag `version_conflict` only: the stored tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<StoredTags>,
    /// Draft-tag refusals: the refused tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_id: Option<String>,
    /// `tag_retired` only, when the curator named a successor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub successor_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentAccount {
    pub principal_id: String,
    pub display_name: String,
}

/// `GET /auth/session` and `POST /auth/sign-in/complete`: the server's decision about the
/// presented session. Never a credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionState {
    pub state: SessionStateKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStateKind {
    Anonymous,
    NeedsPrincipal,
    Suspended,
    Active,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartSignInRequest {
    pub email: String,
}

/// `202` of `POST /auth/sign-in/challenges`. The challenge handle stays in an HttpOnly cookie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeSent {
    /// RFC 3339.
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompleteSignInRequest {
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionPrincipalRequest {
    pub pen_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedPrincipal {
    pub principal_id: String,
    pub created: bool,
}

/// A success body. `204 No Content` (and any empty body) decodes as JSON `null`, so `()` is a
/// lawful answer and a type that needs fields is still refused.
pub fn decode_success<T: serde::de::DeserializeOwned>(text: &str) -> Option<T> {
    if text.trim().is_empty() {
        serde_json::from_value(serde_json::Value::Null).ok()
    } else {
        serde_json::from_str(text).ok()
    }
}

/// Outcome classes of the API, as this client needs to distinguish them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiError {
    Unauthenticated,
    Forbidden,
    NotFound,
    VersionConflict {
        current_version: i64,
    },
    /// The work's author closed comments.
    CommentsClosed,
    /// The reply target is missing or no longer public.
    TargetUnavailable,
    /// The same command key was sent with another payload: a client defect, not user input.
    KeyReused,
    /// A moderation decision on a removed comment.
    CommentRemoved,
    /// `403 self_endorsement`: the work's author or the comment's writer.
    SelfEndorsement,
    /// `403 endorsement_not_permitted`: no current endorsement capability.
    EndorsementNotPermitted,
    /// `409 version_conflict` of an endorsement: another accepted command moved the order;
    /// `observed` is the actor's current state. Nothing was written.
    EndorsementConflict {
        observed: EndorsementToken,
    },
    /// `409 revision_changed`: the page's revision is no longer the public one.
    RevisionChanged {
        current_revision_id: String,
    },
    /// `409 version_conflict` of draft tags: `stored` is what the server holds. If it equals the
    /// desired state, that state holds (a lost acknowledgement); otherwise show it, never resend.
    TagConflict {
        current_version: i64,
        stored: StoredTags,
    },
    /// A draft-tag refusal (`unknown_tag`, `tag_retired`, `wrong_facet`, `too_many_themes`).
    /// Nothing was written.
    TagRefused(TagRefusal),
    Invalid {
        field: Option<String>,
    },
    Unavailable,
    /// The request may or may not have reached the server.
    Network,
    Unexpected {
        status: u16,
    },
}

/// Why the server refused a draft-tag save; `field` is `form` or `themes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagRefusal {
    pub code: TagRefusalCode,
    pub field: Option<String>,
    pub tag_id: Option<String>,
    pub successor_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TagRefusalCode {
    /// `422 unknown_tag`: not in the curated catalog (no proposal route exists).
    UnknownTag,
    /// `409 tag_retired`: retired after it was chosen and not already on the draft.
    Retired,
    /// `422 wrong_facet`: a theme as `form` or a form among `themes`.
    WrongFacet,
    /// `422 too_many_themes`: more than three distinct themes.
    TooManyThemes,
}

impl TagRefusal {
    fn from_body(code: TagRefusalCode, body: Option<ErrorBody>) -> Self {
        let (field, tag_id, successor_id) =
            body.map_or((None, None, None), |b| (b.field, b.tag_id, b.successor_id));
        Self {
            code,
            field,
            tag_id,
            successor_id,
        }
    }
}

impl ApiError {
    /// Classifies a non-success response from its status and optional error envelope.
    pub fn from_response(status: u16, envelope: Option<ErrorEnvelope>) -> Self {
        let body = envelope.map(|e| e.error);
        match (status, body.as_ref().map(|b| b.code.as_str())) {
            (401, _) => Self::Unauthenticated,
            (403, Some("self_endorsement")) => Self::SelfEndorsement,
            (403, Some("endorsement_not_permitted")) => Self::EndorsementNotPermitted,
            (403, _) => Self::Forbidden,
            (404, _) => Self::NotFound,
            (409, Some("version_conflict")) => match body {
                Some(ErrorBody {
                    endorsement: Some(observed),
                    ..
                }) => Self::EndorsementConflict { observed },
                Some(ErrorBody {
                    current_version: Some(current_version),
                    tags: Some(stored),
                    ..
                }) => Self::TagConflict {
                    current_version,
                    stored,
                },
                Some(ErrorBody {
                    current_version: Some(current_version),
                    ..
                }) => Self::VersionConflict { current_version },
                _ => Self::Unexpected { status },
            },
            (409, Some("tag_retired")) => {
                Self::TagRefused(TagRefusal::from_body(TagRefusalCode::Retired, body))
            }
            (409, Some("revision_changed")) => match body.and_then(|b| b.current_revision_id) {
                Some(current_revision_id) => Self::RevisionChanged {
                    current_revision_id,
                },
                None => Self::Unexpected { status },
            },
            (409, Some("comments_closed")) => Self::CommentsClosed,
            (409, Some("target_unavailable")) => Self::TargetUnavailable,
            (409, Some("comment_removed")) => Self::CommentRemoved,
            (422, Some("idempotency_key_reused")) => Self::KeyReused,
            (422, Some("unknown_tag")) => {
                Self::TagRefused(TagRefusal::from_body(TagRefusalCode::UnknownTag, body))
            }
            (422, Some("wrong_facet")) => {
                Self::TagRefused(TagRefusal::from_body(TagRefusalCode::WrongFacet, body))
            }
            (422, Some("too_many_themes")) => {
                Self::TagRefused(TagRefusal::from_body(TagRefusalCode::TooManyThemes, body))
            }
            (422, _) => Self::Invalid {
                field: body.and_then(|b| b.field),
            },
            (503, _) => Self::Unavailable,
            _ => Self::Unexpected { status },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;

    const FIXTURES: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../services/vink-app/contracts/http/v1/fixtures"
    );

    /// Decodes a provider fixture and checks that this representation keeps every field.
    fn round_trip<T: DeserializeOwned + Serialize>(name: &str) -> T {
        let path = format!("{FIXTURES}/{name}");
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let value: T = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(&value).unwrap(), json, "{name}");
        value
    }

    #[test]
    fn decodes_the_current_account_without_credentials() {
        let account: CurrentAccount = round_trip("auth.session.account.json");
        assert_eq!(account.display_name, "An Nhiên");
    }

    #[test]
    fn decodes_auth_fixtures_and_an_empty_success_body() {
        let needs: SessionState = round_trip("auth.session.needs_principal.json");
        assert_eq!(needs.state, SessionStateKind::NeedsPrincipal);
        assert!(needs.principal_id.is_none());
        let active: SessionState = round_trip("auth.session.active.json");
        assert_eq!(active.state, SessionStateKind::Active);
        assert!(active.principal_id.is_some());
        let _: StartSignInRequest = round_trip("auth.challenge.request.json");
        let sent: ChallengeSent = round_trip("auth.challenge.response.json");
        assert_eq!(sent.expires_at, "2026-11-07T10:10:00Z");
        let _: CompleteSignInRequest = round_trip("auth.complete.request.json");
        let _: ProvisionPrincipalRequest = round_trip("auth.principal.request.json");
        let created: ProvisionedPrincipal = round_trip("auth.principal.response.json");
        assert!(created.created);
        // Logout answers `204` without a body: a unit success, never a parse failure.
        assert_eq!(decode_success::<()>(""), Some(()));
        assert_eq!(decode_success::<SessionState>(""), None);
        assert_eq!(decode_success::<()>("{\"state\":\"active\"}"), None);
    }

    #[test]
    fn decodes_public_fixtures() {
        let list: WorkList = round_trip("works.list.json");
        assert!(!list.works.is_empty());
        assert!(list.next_cursor.is_none() && list.filters.tags.is_empty());
        let search: WorkList = round_trip("works.search.json");
        assert!(search.next_cursor.is_some());
        assert_eq!(search.filters.tags[0].resolution, "redirect");
        assert!(search.works.iter().all(|w| !w.tags.is_empty()));
        let unmatched: WorkList = round_trip("works.search.unmatched.json");
        assert!(unmatched.works.is_empty());
        let resolutions: Vec<&str> = unmatched
            .filters
            .tags
            .iter()
            .map(|t| t.resolution.as_str())
            .collect();
        assert_eq!(resolutions, ["retired", "not_found"]);
        let publication: Publication = round_trip("publication.get.json");
        assert!(
            publication
                .document
                .blocks
                .iter()
                .any(|b| b.lines.len() > 1)
        );
        let request: SaveDraftRequest = round_trip("draft.save.request.json");
        assert!(request.expected_version >= 1);
        let _: DraftList = round_trip("drafts.list.json");
        let _: Draft = round_trip("draft.get.json");
        let _: SavedDraft = round_trip("draft.save.response.json");
    }

    #[test]
    fn decodes_discussion_fixtures() {
        let page: DiscussionPage = round_trip("discussion.get.json");
        assert!(
            page.roots
                .iter()
                .any(|r| matches!(r.root, RootView::Anchor { .. }))
        );
        let contexts: Vec<_> = page
            .roots
            .iter()
            .flat_map(|r| r.preview.iter().filter_map(|c| c.reply_to.clone()))
            .collect();
        assert!(contexts.contains(&ReplyTo::Root));
        assert!(contexts.contains(&ReplyTo::Unavailable));
        assert!(contexts.iter().any(|c| matches!(c, ReplyTo::Reply { .. })));
        let _: ReplyList = round_trip("discussion.replies.json");
        let request: SubmitCommentRequest = round_trip("discussion.comment.submit.request.json");
        assert!(request.target_id.is_some());
        let _: SubmittedComment = round_trip("discussion.comment.submit.response.json");
        let _: RemovedComment = round_trip("discussion.comment.remove.response.json");
        let _: CommentsSettingsRequest = round_trip("discussion.settings.request.json");
        let settings: CommentsSettings = round_trip("discussion.settings.response.json");
        assert!(!settings.comments_open);
        let replies: ReplyList = round_trip("discussion.replies.json");
        assert!(
            replies
                .replies
                .iter()
                .any(|reply| reply.rejection_reason.is_some())
        );
        let queue: ModerationQueue = round_trip("moderation.queue.json");
        assert!(queue.items.iter().any(|item| !item.can_decide));
        let decision: ModerationDecisionRequest = round_trip("moderation.decision.request.json");
        assert_eq!(decision.action, "reject");
        let _: ModerationDecision = round_trip("moderation.decision.response.json");
    }

    #[test]
    fn classifies_discussion_error_fixtures() {
        let cases = [
            ("error.comments_closed.json", 409, ApiError::CommentsClosed),
            (
                "error.target_unavailable.json",
                409,
                ApiError::TargetUnavailable,
            ),
            (
                "error.idempotency_key_reused.json",
                422,
                ApiError::KeyReused,
            ),
            ("error.not_comment_writer.json", 403, ApiError::Forbidden),
            ("error.not_moderator.json", 403, ApiError::Forbidden),
            ("error.moderator_recused.json", 403, ApiError::Forbidden),
            ("error.comment_removed.json", 409, ApiError::CommentRemoved),
            (
                "error.moderation_conflict.json",
                409,
                ApiError::VersionConflict { current_version: 2 },
            ),
            (
                "error.discussion_conflict.json",
                409,
                ApiError::VersionConflict { current_version: 3 },
            ),
        ];
        for (name, status, expected) in cases {
            let envelope = round_trip::<ErrorEnvelope>(name);
            assert_eq!(
                ApiError::from_response(status, Some(envelope)),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn decodes_endorsement_fixtures() {
        let summaries: EndorsementSummaries = round_trip("endorsements.get.json");
        assert!(
            summaries
                .comments
                .iter()
                .any(|c| c.status == "unavailable" && c.count.is_none())
        );
        let own: OwnEndorsements = round_trip("endorsements.me.get.json");
        assert_eq!(own.work.order, Some(3));
        assert!(
            own.comments
                .iter()
                .any(|c| c.own.blocked_reason.as_deref() == Some("own_content"))
        );
        assert!(
            own.comments
                .iter()
                .any(|c| c.own.state == "unavailable" && c.own.order.is_none())
        );
        let work: WorkEndorsementRequest = round_trip("endorsement.work.put.request.json");
        assert_eq!(work.expected_order, 1);
        let _: CommentEndorsementRequest = round_trip("endorsement.comment.put.request.json");
        let accepted: EndorsementWrite = round_trip("endorsement.work.put.response.json");
        assert!(accepted.committed.is_none() && accepted.revision_id.is_some());
        let replayed: EndorsementWrite = round_trip("endorsement.comment.put.response.json");
        assert_ne!(replayed.committed, Some(replayed.endorsement));
    }

    #[test]
    fn classifies_endorsement_error_fixtures() {
        let cases = [
            (
                "error.endorsement_conflict.json",
                409,
                ApiError::EndorsementConflict {
                    observed: EndorsementToken {
                        selected: false,
                        version: 3,
                        order: 4,
                    },
                },
            ),
            (
                "error.revision_changed.json",
                409,
                ApiError::RevisionChanged {
                    current_revision_id: "0192f3a1-0000-7000-8000-000000000045".into(),
                },
            ),
            (
                "error.self_endorsement.json",
                403,
                ApiError::SelfEndorsement,
            ),
            (
                "error.endorsement_not_permitted.json",
                403,
                ApiError::EndorsementNotPermitted,
            ),
            (
                "error.endorsement_key_reused.json",
                422,
                ApiError::KeyReused,
            ),
            ("error.unauthenticated.json", 401, ApiError::Unauthenticated),
            ("error.forbidden.json", 403, ApiError::Forbidden),
        ];
        for (name, status, expected) in cases {
            let envelope = round_trip::<ErrorEnvelope>(name);
            assert_eq!(
                ApiError::from_response(status, Some(envelope)),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn decodes_tag_fixtures() {
        let list: TagList = round_trip("tags.list.json");
        assert!(!list.truncated && list.tags.iter().any(|t| t.facet == "form"));
        let suggestions: TagSuggestions = round_trip("tags.suggestions.json");
        assert_eq!(suggestions.query, "hồi");
        assert!(
            suggestions
                .suggestions
                .iter()
                .any(|s| s.via_alias.as_deref() == Some("Hồi ức") && s.match_kind == "prefix")
        );
        let draft: DraftTags = round_trip("draft.tags.get.json");
        assert_eq!(draft.version, 4);
        let retired = draft
            .themes
            .iter()
            .find(|t| t.status == "retired")
            .expect("the fixture keeps a retired theme on the draft");
        assert_eq!(
            retired.successor.as_ref().map(|s| s.label.as_str()),
            Some("Mùa")
        );
        let request: SaveDraftTagsRequest = round_trip("draft.tags.put.request.json");
        assert_eq!(request.expected_version, draft.version);
        let saved: SavedDraftTags = round_trip("draft.tags.put.response.json");
        assert_eq!(saved.version, 5);
        assert_eq!(saved.themes, request.themes);
        // `form: null` and `themes: []` are sent, never omitted: omission is a `422`.
        let clear = SaveDraftTagsRequest {
            expected_version: 5,
            form: None,
            themes: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(&clear).unwrap(),
            serde_json::json!({"expected_version": 5, "form": null, "themes": []})
        );
    }

    #[test]
    fn classifies_tag_error_fixtures() {
        let conflict = ApiError::from_response(
            409,
            Some(round_trip::<ErrorEnvelope>(
                "error.tag_version_conflict.json",
            )),
        );
        assert_eq!(
            conflict,
            ApiError::TagConflict {
                current_version: 5,
                stored: StoredTags {
                    form: Some("a0000000-0000-4000-8000-000000000001".into()),
                    themes: vec![
                        "a0000000-0000-4000-8000-000000000011".into(),
                        "a0000000-0000-4000-8000-000000000012".into(),
                    ],
                },
            }
        );
        let refusal = |name: &str, status: u16| match ApiError::from_response(
            status,
            Some(round_trip::<ErrorEnvelope>(name)),
        ) {
            ApiError::TagRefused(refusal) => refusal,
            other => panic!("{name}: {other:?}"),
        };
        let retired = refusal("error.tag_retired.json", 409);
        assert_eq!(retired.code, TagRefusalCode::Retired);
        assert_eq!(retired.field.as_deref(), Some("themes"));
        assert_eq!(
            retired.successor_id.as_deref(),
            Some("a0000000-0000-4000-8000-000000000014")
        );
        let unknown = refusal("error.unknown_tag.json", 422);
        assert_eq!(unknown.code, TagRefusalCode::UnknownTag);
        assert!(unknown.tag_id.is_some() && unknown.successor_id.is_none());
        let facet = refusal("error.wrong_facet.json", 422);
        assert_eq!(
            (facet.code, facet.field.as_deref()),
            (TagRefusalCode::WrongFacet, Some("form"))
        );
        let many = refusal("error.too_many_themes.json", 422);
        assert_eq!(
            (many.code, many.tag_id.as_deref()),
            (TagRefusalCode::TooManyThemes, None)
        );
        assert_eq!(
            ApiError::from_response(
                403,
                Some(round_trip::<ErrorEnvelope>(
                    "error.principal_suspended.json"
                ))
            ),
            ApiError::Forbidden
        );
        // A text-draft conflict carries no `tags` and stays a plain version conflict.
        assert_eq!(
            ApiError::from_response(
                409,
                Some(round_trip::<ErrorEnvelope>("error.version_conflict.json"))
            ),
            ApiError::VersionConflict { current_version: 5 }
        );
    }

    #[test]
    fn classifies_every_error_fixture() {
        let cases = [
            ("error.validation_failed.json", 422),
            ("error.unauthenticated.json", 401),
            ("error.forbidden.json", 403),
            ("error.not_found.json", 404),
            ("error.version_conflict.json", 409),
            ("error.dependency_unavailable.json", 503),
        ];
        let classified: Vec<ApiError> = cases
            .iter()
            .map(|(name, status)| {
                ApiError::from_response(*status, Some(round_trip::<ErrorEnvelope>(name)))
            })
            .collect();
        assert_eq!(
            classified,
            vec![
                ApiError::Invalid {
                    field: Some("document".into())
                },
                ApiError::Unauthenticated,
                ApiError::Forbidden,
                ApiError::NotFound,
                ApiError::VersionConflict { current_version: 5 },
                ApiError::Unavailable,
            ]
        );
    }
}

#[cfg(test)]
mod separated_discussion_tests {
    use super::*;
    #[test]
    fn provider_public_fixtures_decode_without_viewer_fields_and_refuse_private_fields() {
        let page: PublicDiscussionPage = serde_json::from_str(include_str!(
            "../../../services/vink-app/contracts/http/v1/fixtures/discussion.public.get.json"
        ))
        .unwrap();
        let replies: PublicReplyList = serde_json::from_str(include_str!(
            "../../../services/vink-app/contracts/http/v1/fixtures/discussion.public.replies.json"
        ))
        .unwrap();
        assert!(!replies.replies.is_empty());
        let mut wire = serde_json::to_value(&page).unwrap();
        let text = wire.to_string();
        for private in [
            "visibility",
            "can_remove",
            "rejection_reason",
            "viewer_is_work_author",
            "own_pending",
            "own_rejected",
        ] {
            assert!(!text.contains(private));
        }
        wire["viewer_is_work_author"] = true.into();
        assert!(serde_json::from_value::<PublicDiscussionPage>(wire).is_err());
        let mut wire = serde_json::to_value(page).unwrap();
        wire["roots"][0]["root"]["comment"]["can_remove"] = true.into();
        assert!(serde_json::from_value::<PublicDiscussionPage>(wire).is_err());
    }
    #[test]
    fn the_composed_capability_seam_discards_its_comment_projection() {
        let c: DiscussionCapability = serde_json::from_str(include_str!(
            "../../../services/vink-app/contracts/http/v1/fixtures/discussion.get.json"
        ))
        .unwrap();
        assert!(!c.work_id.is_empty());
    }
}
