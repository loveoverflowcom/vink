use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::contract::{ErrorBody, ErrorEnvelope};
use crate::application::discussion::{DiscussionError, DiscussionInput};
use crate::application::{DraftError, InvalidInput, ReadError};

/// Outcome classes of HTTP API v1. Messages are for developers; clients branch on `code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    Validation {
        field: &'static str,
        message: String,
    },
    Unauthenticated,
    Forbidden,
    NotFound,
    VersionConflict {
        current_version: i64,
    },
    /// The comments settings moved past `expected_generation`.
    DiscussionConflict {
        current_version: i64,
    },
    CommentsClosed,
    TargetUnavailable,
    KeyReused,
    NotCommentWriter,
    NotModerator,
    ModeratorRecused,
    CommentRemoved,
    /// The comment's moderation version moved past `expected_version`.
    ModerationConflict {
        current_version: i64,
    },
    DependencyUnavailable,
    Internal,
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::Forbidden
            | Self::NotCommentWriter
            | Self::NotModerator
            | Self::ModeratorRecused => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::VersionConflict { .. }
            | Self::DiscussionConflict { .. }
            | Self::CommentsClosed
            | Self::TargetUnavailable
            | Self::CommentRemoved
            | Self::ModerationConflict { .. } => StatusCode::CONFLICT,
            Self::KeyReused => StatusCode::UNPROCESSABLE_ENTITY,
            Self::DependencyUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn envelope(&self) -> ErrorEnvelope {
        let (code, message, current_version, field) = match self {
            Self::Validation { field, message } => {
                ("validation_failed", message.clone(), None, Some(*field))
            }
            Self::Unauthenticated => (
                "unauthenticated",
                "no authenticated Vink principal".to_string(),
                None,
                None,
            ),
            Self::Forbidden => (
                "forbidden",
                "request origin is not allowed for this action".to_string(),
                None,
                None,
            ),
            Self::NotFound => ("not_found", "resource not found".to_string(), None, None),
            Self::VersionConflict { current_version } => (
                "version_conflict",
                "the draft changed since expected_version; nothing was written".to_string(),
                Some(*current_version),
                None,
            ),
            Self::DiscussionConflict { current_version } => (
                "version_conflict",
                "the comments settings changed since the expected generation; nothing was written"
                    .to_string(),
                Some(*current_version),
                None,
            ),
            Self::CommentsClosed => (
                "comments_closed",
                "the work's author closed comments; nothing was written".to_string(),
                None,
                None,
            ),
            Self::TargetUnavailable => (
                "target_unavailable",
                "the reply target is missing, hidden or not public; nothing was written"
                    .to_string(),
                None,
                None,
            ),
            Self::KeyReused => (
                "idempotency_key_reused",
                "command_key was already used for a different comment; nothing was written"
                    .to_string(),
                None,
                Some("command_key"),
            ),
            Self::NotCommentWriter => (
                "not_comment_writer",
                "only the comment's writer can remove it".to_string(),
                None,
                None,
            ),
            Self::NotModerator => (
                "not_moderator",
                "an active moderator grant is required".to_string(),
                None,
                None,
            ),
            Self::ModeratorRecused => (
                "moderator_recused",
                "a moderator cannot decide on their own comment or on comments of their own work"
                    .to_string(),
                None,
                None,
            ),
            Self::CommentRemoved => (
                "comment_removed",
                "the comment was removed; no decision applies".to_string(),
                None,
                None,
            ),
            Self::ModerationConflict { current_version } => (
                "version_conflict",
                "the comment changed since expected_version; nothing was written".to_string(),
                Some(*current_version),
                None,
            ),
            Self::DependencyUnavailable => (
                "dependency_unavailable",
                "a required dependency is unavailable; retry later".to_string(),
                None,
                None,
            ),
            Self::Internal => ("internal", "internal error".to_string(), None, None),
        };
        ErrorEnvelope {
            error: ErrorBody {
                code,
                message,
                current_version,
                field,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status(), Json(self.envelope())).into_response()
    }
}

impl From<InvalidInput> for ApiError {
    fn from(error: InvalidInput) -> Self {
        Self::Validation {
            field: error.field(),
            message: error.to_string(),
        }
    }
}

impl From<DraftError> for ApiError {
    fn from(error: DraftError) -> Self {
        match error {
            DraftError::Unauthenticated => Self::Unauthenticated,
            DraftError::NotFound => Self::NotFound,
            DraftError::VersionConflict { current } => Self::VersionConflict {
                current_version: current.get(),
            },
            DraftError::Invalid(invalid) => invalid.into(),
            DraftError::Unavailable => Self::DependencyUnavailable,
            DraftError::Internal => Self::Internal,
        }
    }
}

impl From<ReadError> for ApiError {
    fn from(error: ReadError) -> Self {
        match error {
            ReadError::NotFound => Self::NotFound,
            ReadError::Unavailable => Self::DependencyUnavailable,
            ReadError::Internal => Self::Internal,
        }
    }
}

impl From<DiscussionInput> for ApiError {
    fn from(error: DiscussionInput) -> Self {
        Self::Validation {
            field: error.field(),
            message: error.to_string(),
        }
    }
}

impl From<DiscussionError> for ApiError {
    fn from(error: DiscussionError) -> Self {
        match error {
            DiscussionError::Unauthenticated => Self::Unauthenticated,
            DiscussionError::NotFound => Self::NotFound,
            DiscussionError::NotCommentWriter => Self::NotCommentWriter,
            DiscussionError::CommentsClosed => Self::CommentsClosed,
            DiscussionError::TargetUnavailable => Self::TargetUnavailable,
            DiscussionError::KeyReused => Self::KeyReused,
            DiscussionError::Conflict { current } => Self::DiscussionConflict {
                current_version: current,
            },
            DiscussionError::Invalid(invalid) => invalid.into(),
            DiscussionError::Unavailable => Self::DependencyUnavailable,
            DiscussionError::NotModerator => Self::NotModerator,
            DiscussionError::Recused => Self::ModeratorRecused,
            DiscussionError::Removed => Self::CommentRemoved,
            DiscussionError::ModerationConflict { current } => Self::ModerationConflict {
                current_version: current,
            },
            DiscussionError::Internal => Self::Internal,
        }
    }
}
