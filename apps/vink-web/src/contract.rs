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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkList {
    pub works: Vec<WorkCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkCard {
    pub work_id: String,
    pub title: String,
    pub author_display_name: String,
    pub published_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Publication {
    pub work_id: String,
    pub revision_id: String,
    pub title: String,
    pub author_display_name: String,
    pub published_at: String,
    pub document: DocumentDto,
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

impl ApiError {
    /// Classifies a non-success response from its status and optional error envelope.
    pub fn from_response(status: u16, envelope: Option<ErrorEnvelope>) -> Self {
        let body = envelope.map(|e| e.error);
        match (status, body.as_ref().map(|b| b.code.as_str())) {
            (401, _) => Self::Unauthenticated,
            (403, _) => Self::Forbidden,
            (404, _) => Self::NotFound,
            (409, Some("version_conflict")) => match body.and_then(|b| b.current_version) {
                Some(current_version) => Self::VersionConflict { current_version },
                None => Self::Unexpected { status },
            },
            (409, Some("comments_closed")) => Self::CommentsClosed,
            (409, Some("target_unavailable")) => Self::TargetUnavailable,
            (422, Some("idempotency_key_reused")) => Self::KeyReused,
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
    fn decodes_public_fixtures() {
        let list: WorkList = round_trip("works.list.json");
        assert!(!list.works.is_empty());
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
