//! Canonical wire types of HTTP API v1, owned by this service. The contract text and fixtures
//! live in `services/vink-app/contracts/http/v1/`; consumers keep their own representations.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::application::{PublicWork, PublicWorkSummary, SavedDraft};
use crate::domain::document::DocumentData;
use crate::domain::draft::{DraftSummary, WorkingDraft};

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
}
