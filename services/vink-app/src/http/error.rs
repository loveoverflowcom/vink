use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::contract::{ErrorBody, ErrorEnvelope};
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
    DependencyUnavailable,
    Internal,
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::VersionConflict { .. } => StatusCode::CONFLICT,
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
