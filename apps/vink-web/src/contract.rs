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
