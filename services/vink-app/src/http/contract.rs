//! Canonical wire types of HTTP API v1, owned by this service. The contract text and fixtures
//! live in `services/vink-app/contracts/http/v1/`; consumers keep their own representations.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::application::discussion::{
    CommentItem, OwnStatus, ReplyPage, RootEntry, RootItem, RootPage, SubmittedComment,
};
use crate::application::{PublicWork, PublicWorkSummary, SavedDraft};
use crate::domain::discussion::settings::DiscussionSettings;
use crate::domain::discussion::visibility::{CommentView, ReplyContext};
use crate::domain::document::DocumentData;
use crate::domain::draft::{DraftSummary, WorkingDraft};
use crate::domain::ids::{CommentId, PrincipalId, WorkId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkList {
    pub works: Vec<WorkCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkCard {
    pub work_id: String,
    pub title: String,
    pub author_display_name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub published_at: OffsetDateTime,
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
}

impl From<&PublicWorkSummary> for WorkCard {
    fn from(work: &PublicWorkSummary) -> Self {
        Self {
            work_id: work.work_id.to_string(),
            title: work.title.as_str().to_string(),
            author_display_name: work.author_display_name.clone(),
            published_at: work.published_at,
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
        let reply_to = (!is_root).then(|| match (item.context, &item.context_author) {
            (ReplyContext::Root, _) => ReplyTo::Root,
            (ReplyContext::Reply(id), Some(name)) => ReplyTo::Reply {
                comment_id: id.to_string(),
                author_display_name: name.clone(),
            },
            // A context without a public writer name is reported as unavailable, never guessed.
            (ReplyContext::Reply(_), None) | (ReplyContext::Unavailable, _) => ReplyTo::Unavailable,
        });
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
        let summary = PublicWorkSummary {
            work_id: work(),
            title: title(),
            author_display_name: "Tác giả mẫu (dữ liệu phát triển)".into(),
            published_at: AT,
        };
        assert_matches(
            "works.list.json",
            &WorkList {
                works: vec![WorkCard::from(&summary)],
            },
        );
        let public = PublicWork {
            work_id: work(),
            revision_id: RevisionId::parse("44444444-4444-4444-8444-444444444444").unwrap(),
            title: title(),
            author_display_name: summary.author_display_name.clone(),
            published_at: AT,
            document: document(),
        };
        assert_matches("publication.get.json", &Publication::from(&public));
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
            let replies = ReplyPage {
                replies: vec![item(
                    5,
                    CommentView::Public,
                    lan,
                    "Đọc lại vẫn thấy hay.",
                    ReplyContext::Root,
                    None,
                )],
                next: None,
            };
            assert_matches(
                "discussion.replies.json",
                &ReplyList::new(&replies, Some(viewer().id)),
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
            for unreachable in [DiscussionError::NotModerator, DiscussionError::Removed] {
                assert_eq!(ApiError::from(unreachable), ApiError::Internal);
            }
        }
    }
}
