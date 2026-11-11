use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::contract::{ErrorBody, ErrorEnvelope, TagIds};
use crate::application::catalog::{CatalogQueryInput, CatalogReadError};
use crate::application::discovery::{DiscoveryError, DiscoveryInput};
use crate::application::discussion::{DiscussionError, DiscussionInput};
use crate::application::endorsement::{EndorsementError, EndorsementInput};
use crate::application::publication::AuthorPublicationError;
use crate::application::taxonomy::{TagError, TagInput, TagRejection};
use crate::application::{DraftError, InvalidInput, ReadError};
use crate::domain::endorsement::EndorsementState;
use crate::domain::ids::{RevisionId, TagId};

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
    /// Authors and comment writers do not endorse their own content (LVE CHOICE-08 A).
    SelfEndorsement,
    /// The actor has no current endorsement capability.
    EndorsementNotPermitted,
    /// The endorsement's accepted order is not `expected_order`.
    EndorsementConflict {
        observed: EndorsementState,
    },
    /// The endorsement command key was already used for a different command.
    EndorsementKeyReused,
    /// The endorsed revision is no longer the public one.
    RevisionChanged {
        published: RevisionId,
    },
    /// Sign-in (D-05/S-03 spike): no authority resolves requests in this deployment.
    AuthorityNotConfigured,
    /// A sign-in ceiling refused the request: this network (App) or the address, account or
    /// global volume (Identity). `Retry-After` is sent when the App's window end is known.
    RateLimited {
        retry_after_seconds: Option<u32>,
    },
    /// Wrong code; the same sign-in accepts another attempt.
    CodeRejected,
    /// The sign-in challenge is expired, used, exhausted or missing: request a new code.
    ChallengeClosed,
    /// The mailbox was proven but its Identity account is disabled.
    AccountDisabled,
    /// Wrong username or password on native password sign-in.
    InvalidCredentials,
    /// Username already registered on native password sign-in.
    UsernameTaken,
    /// The principal is suspended by Vink.
    PrincipalSuspended,
    /// The draft tags moved past `expected_version`; `observed` is what is stored now.
    TagConflict {
        current_version: i64,
        observed: TagIds,
    },
    /// The tag is not in the curated catalog ("chưa có thẻ này"); no proposal exists.
    UnknownTag {
        field: &'static str,
        tag_id: TagId,
    },
    /// The tag was retired and is not already on the draft.
    TagRetired {
        field: &'static str,
        tag_id: TagId,
        successor_id: Option<TagId>,
    },
    /// A form tag in `themes` or a theme tag in `form`.
    WrongFacet {
        field: &'static str,
        tag_id: TagId,
    },
    TooManyThemes,
    PublicationConflict {
        current_generation: i64,
    },
    RevisionRejected,
    UnknownRevision,
    WorkUnavailable,
    PublicationKeyReused,
    SnapshotConflict {
        current_draft_version: Option<i64>,
        current_metadata_version: Option<i64>,
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
            | Self::ModeratorRecused
            | Self::SelfEndorsement
            | Self::EndorsementNotPermitted => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::VersionConflict { .. }
            | Self::DiscussionConflict { .. }
            | Self::PublicationConflict { .. }
            | Self::RevisionRejected
            | Self::WorkUnavailable
            | Self::SnapshotConflict { .. }
            | Self::CommentsClosed
            | Self::TargetUnavailable
            | Self::CommentRemoved
            | Self::ModerationConflict { .. }
            | Self::EndorsementConflict { .. }
            | Self::RevisionChanged { .. } => StatusCode::CONFLICT,
            Self::KeyReused | Self::EndorsementKeyReused | Self::PublicationKeyReused => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            Self::UnknownRevision => StatusCode::NOT_FOUND,
            Self::AuthorityNotConfigured => StatusCode::NOT_IMPLEMENTED,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::CodeRejected => StatusCode::UNPROCESSABLE_ENTITY,
            Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::UsernameTaken => StatusCode::CONFLICT,
            Self::ChallengeClosed => StatusCode::CONFLICT,
            Self::AccountDisabled | Self::PrincipalSuspended => StatusCode::FORBIDDEN,
            Self::TagConflict { .. } | Self::TagRetired { .. } => StatusCode::CONFLICT,
            Self::UnknownTag { .. } | Self::WrongFacet { .. } | Self::TooManyThemes => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
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
            Self::SelfEndorsement => (
                "self_endorsement",
                "authors and comment writers cannot endorse their own content".to_string(),
                None,
                None,
            ),
            Self::EndorsementNotPermitted => (
                "endorsement_not_permitted",
                "the actor may not endorse now; nothing was written".to_string(),
                None,
                None,
            ),
            Self::EndorsementConflict { observed } => (
                "version_conflict",
                "another accepted command moved this endorsement past expected_order; \
                 nothing was written"
                    .to_string(),
                Some(observed.order().get()),
                Some("expected_order"),
            ),
            Self::EndorsementKeyReused => (
                "idempotency_key_reused",
                "command_key was already used for a different endorsement command; nothing was \
                 written"
                    .to_string(),
                None,
                Some("command_key"),
            ),
            Self::RevisionChanged { .. } => (
                "revision_changed",
                "the work was republished since the page was read; nothing was written".to_string(),
                None,
                None,
            ),
            Self::AuthorityNotConfigured => (
                "authority_not_configured",
                "sign-in is not available in this deployment".to_string(),
                None,
                None,
            ),
            Self::RateLimited { .. } => (
                "rate_limited",
                "too many sign-in requests; try again later".to_string(),
                None,
                None,
            ),
            Self::CodeRejected => (
                "code_rejected",
                "the code is not correct; try again".to_string(),
                None,
                Some("code"),
            ),
            Self::InvalidCredentials => (
                "invalid_credentials",
                "invalid username or password".to_string(),
                None,
                None,
            ),
            Self::UsernameTaken => (
                "username_taken",
                "username is already taken".to_string(),
                None,
                Some("username"),
            ),
            Self::ChallengeClosed => (
                "challenge_closed",
                "this code can no longer be used; request a new one".to_string(),
                None,
                None,
            ),
            Self::AccountDisabled => (
                "account_disabled",
                "this account is disabled; no session was started".to_string(),
                None,
                None,
            ),
            Self::PrincipalSuspended => (
                "principal_suspended",
                "this Vink account is suspended".to_string(),
                None,
                None,
            ),
            Self::TagConflict {
                current_version, ..
            } => (
                "version_conflict",
                "the draft tags changed since expected_version; nothing was written".to_string(),
                Some(*current_version),
                Some("expected_version"),
            ),
            Self::UnknownTag { field, .. } => (
                "unknown_tag",
                "the tag is not in the curated catalog; nothing was written".to_string(),
                None,
                Some(*field),
            ),
            Self::TagRetired { field, .. } => (
                "tag_retired",
                "the tag was retired and cannot be added; nothing was written".to_string(),
                None,
                Some(*field),
            ),
            Self::WrongFacet { field, .. } => (
                "wrong_facet",
                "the tag belongs to the other slot; nothing was written".to_string(),
                None,
                Some(*field),
            ),
            Self::TooManyThemes => (
                "too_many_themes",
                "a work has at most three distinct themes; nothing was written".to_string(),
                None,
                Some("themes"),
            ),
            Self::PublicationConflict { current_generation } => (
                "version_conflict",
                "audience intent generation changed; nothing was written".to_string(),
                Some(*current_generation),
                Some("expected_generation"),
            ),
            Self::RevisionRejected => (
                "revision_rejected",
                "the requested revision was rejected by moderation; nothing was written".to_string(),
                None,
                Some("revision_id"),
            ),
            Self::UnknownRevision => (
                "unknown_revision",
                "the revision was not found on this work".to_string(),
                None,
                Some("revision_id"),
            ),
            Self::WorkUnavailable => (
                "work_unavailable",
                "the work is not active; nothing was written".to_string(),
                None,
                None,
            ),
            Self::PublicationKeyReused => (
                "idempotency_key_reused",
                "command_key was already used for a different command or payload; nothing was written"
                    .to_string(),
                None,
                Some("command_key"),
            ),
            // The field names the version that moved, so a client reconciles the right one.
            Self::SnapshotConflict {
                current_draft_version: None,
                current_metadata_version: Some(current),
            } => (
                "version_conflict",
                "draft metadata changed since expected_metadata_version; nothing was written"
                    .to_string(),
                Some(*current),
                Some("expected_metadata_version"),
            ),
            Self::SnapshotConflict {
                current_draft_version,
                ..
            } => (
                "version_conflict",
                "draft text changed since expected_draft_version; nothing was written".to_string(),
                *current_draft_version,
                Some("expected_draft_version"),
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
                current_revision_id: match self {
                    Self::RevisionChanged { published } => Some(published.to_string()),
                    _ => None,
                },
                endorsement: match self {
                    Self::EndorsementConflict { observed } => Some((*observed).into()),
                    _ => None,
                },
                tags: match self {
                    Self::TagConflict { observed, .. } => Some(observed.clone()),
                    _ => None,
                },
                tag_id: match self {
                    Self::UnknownTag { tag_id, .. }
                    | Self::TagRetired { tag_id, .. }
                    | Self::WrongFacet { tag_id, .. } => Some(tag_id.to_string()),
                    _ => None,
                },
                successor_id: match self {
                    Self::TagRetired {
                        successor_id: Some(id),
                        ..
                    } => Some(id.to_string()),
                    _ => None,
                },
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status(), Json(self.envelope())).into_response();
        if let Self::RateLimited {
            retry_after_seconds: Some(seconds),
        } = self
            && let Ok(value) = axum::http::HeaderValue::from_str(&seconds.to_string())
        {
            response
                .headers_mut()
                .insert(axum::http::header::RETRY_AFTER, value);
        }
        response
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
            DraftError::Suspended => Self::PrincipalSuspended,
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
            DiscussionError::Suspended => Self::PrincipalSuspended,
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

impl From<EndorsementInput> for ApiError {
    fn from(error: EndorsementInput) -> Self {
        Self::Validation {
            field: error.field(),
            message: error.to_string(),
        }
    }
}

impl From<EndorsementError> for ApiError {
    fn from(error: EndorsementError) -> Self {
        match error {
            EndorsementError::Unauthenticated => Self::Unauthenticated,
            EndorsementError::Suspended => Self::PrincipalSuspended,
            EndorsementError::NotFound => Self::NotFound,
            EndorsementError::SelfEndorsement => Self::SelfEndorsement,
            EndorsementError::RevisionChanged { published } => Self::RevisionChanged { published },
            EndorsementError::NotPermitted => Self::EndorsementNotPermitted,
            EndorsementError::Conflict { observed } => Self::EndorsementConflict { observed },
            EndorsementError::KeyReused => Self::EndorsementKeyReused,
            EndorsementError::Invalid(invalid) => invalid.into(),
            EndorsementError::Unavailable => Self::DependencyUnavailable,
            EndorsementError::Internal => Self::Internal,
        }
    }
}

impl From<TagInput> for ApiError {
    fn from(error: TagInput) -> Self {
        Self::Validation {
            field: error.field(),
            message: error.to_string(),
        }
    }
}

impl From<TagError> for ApiError {
    fn from(error: TagError) -> Self {
        match error {
            TagError::Unauthenticated => Self::Unauthenticated,
            TagError::Suspended => Self::PrincipalSuspended,
            TagError::NotFound => Self::NotFound,
            TagError::VersionConflict { current, observed } => Self::TagConflict {
                current_version: current.get(),
                observed: TagIds::from(&observed),
            },
            TagError::Invalid(invalid) => invalid.into(),
            TagError::Rejected(rejection) => {
                let field = rejection.field();
                match rejection {
                    TagRejection::UnknownTag { tag, .. } => Self::UnknownTag { field, tag_id: tag },
                    TagRejection::RetiredTag { tag, successor, .. } => Self::TagRetired {
                        field,
                        tag_id: tag,
                        successor_id: successor,
                    },
                    TagRejection::WrongFacet { tag, .. } => Self::WrongFacet { field, tag_id: tag },
                    TagRejection::TooManyThemes => Self::TooManyThemes,
                }
            }
            TagError::Unavailable => Self::DependencyUnavailable,
            TagError::Internal => Self::Internal,
        }
    }
}

impl From<CatalogQueryInput> for ApiError {
    fn from(error: CatalogQueryInput) -> Self {
        Self::Validation {
            field: error.field(),
            message: error.to_string(),
        }
    }
}

impl From<CatalogReadError> for ApiError {
    fn from(error: CatalogReadError) -> Self {
        match error {
            CatalogReadError::Invalid(invalid) => invalid.into(),
            CatalogReadError::NotFound => Self::NotFound,
            CatalogReadError::Unavailable => Self::DependencyUnavailable,
            CatalogReadError::Internal => Self::Internal,
        }
    }
}

impl From<DiscoveryInput> for ApiError {
    fn from(error: DiscoveryInput) -> Self {
        Self::Validation {
            field: error.field(),
            message: error.to_string(),
        }
    }
}

impl From<DiscoveryError> for ApiError {
    fn from(error: DiscoveryError) -> Self {
        match error {
            DiscoveryError::Invalid(invalid) => invalid.into(),
            DiscoveryError::Unavailable => Self::DependencyUnavailable,
            DiscoveryError::Internal => Self::Internal,
        }
    }
}

impl From<AuthorPublicationError> for ApiError {
    fn from(error: AuthorPublicationError) -> Self {
        match error {
            AuthorPublicationError::Unauthenticated => Self::Unauthenticated,
            AuthorPublicationError::Suspended => Self::PrincipalSuspended,
            AuthorPublicationError::NotFound => Self::NotFound,
            AuthorPublicationError::Stale { current_generation } => {
                Self::PublicationConflict { current_generation }
            }
            AuthorPublicationError::RevisionRejected => Self::RevisionRejected,
            AuthorPublicationError::UnknownRevision => Self::UnknownRevision,
            AuthorPublicationError::WorkUnavailable => Self::WorkUnavailable,
            AuthorPublicationError::DraftOrMetadataConflict {
                current_draft_version,
                current_metadata_version,
            } => Self::SnapshotConflict {
                current_draft_version,
                current_metadata_version,
            },
            AuthorPublicationError::KeyReused => Self::PublicationKeyReused,
            AuthorPublicationError::InvalidCommandKey(_) => Self::Validation {
                field: "command_key",
                message: "command key must be 8-128 characters of A-Z, a-z, 0-9, '-' or '_'".into(),
            },
            AuthorPublicationError::Unavailable => Self::DependencyUnavailable,
            AuthorPublicationError::Internal => Self::Internal,
        }
    }
}
