//! Provider-owned, bounded private author-work representation.
use crate::application::author_works as app;
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthorWorkList {
    pub works: Vec<AuthorWorkCard>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthorWorkCard {
    pub work_id: String,
    pub title: String,
    pub draft_version: Option<i64>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub publication: Option<PublicationSummary>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicationSummary {
    pub audience: String,
    pub lifecycle: String,
    pub generation: i64,
    pub intent_revision_id: Option<String>,
    pub intent_moderation: Option<String>,
    pub latest_revision_id: Option<String>,
    pub latest_moderation: Option<String>,
    pub review_withdrawn: bool,
    pub has_draft_changes: Option<bool>,
    pub current_public: Option<CurrentPublicSummary>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CurrentPublicSummary {
    pub revision_id: String,
    pub title: String,
    pub tham: Option<i64>,
    pub comments: Option<i64>,
}
impl From<app::AuthorWorkPage> for AuthorWorkList {
    fn from(page: app::AuthorWorkPage) -> Self {
        Self {
            next_cursor: page.next_cursor,
            works: page
                .works
                .into_iter()
                .map(|w| AuthorWorkCard {
                    work_id: w.work_id.to_string(),
                    title: w.title.as_str().into(),
                    draft_version: w.draft_version.map(|v| v.get()),
                    updated_at: w.updated_at,
                    publication: w.publication.map(|p| PublicationSummary {
                        audience: p.audience,
                        lifecycle: p.lifecycle,
                        generation: p.generation,
                        intent_revision_id: p.intent_revision_id.map(|id| id.to_string()),
                        intent_moderation: p.intent_moderation,
                        latest_revision_id: p.latest_revision_id.map(|id| id.to_string()),
                        latest_moderation: p.latest_moderation,
                        review_withdrawn: p.review_withdrawn,
                        has_draft_changes: p.has_draft_changes,
                        current_public: p.current_public.map(|p| CurrentPublicSummary {
                            revision_id: p.revision_id.to_string(),
                            title: p.title,
                            tham: p.tham,
                            comments: p.comments,
                        }),
                    }),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        document::Title,
        draft::DraftVersion,
        ids::{RevisionId, WorkId},
    };
    #[test]
    fn author_work_contract_matches_provider_fixture_and_old_draft_contract_stays_separate() {
        let revision = |s: &str| RevisionId::from_uuid(s.parse().unwrap());
        let page = app::AuthorWorkPage {
            next_cursor: None,
            works: vec![app::AuthorWorkSummary {
                work_id: WorkId::from_uuid("33333333-3333-4333-8333-333333333333".parse().unwrap()),
                title: Title::parse("Bản nháp riêng ỹ ỷ").unwrap(),
                draft_version: DraftVersion::new(3),
                updated_at: time::OffsetDateTime::from_unix_timestamp(1794744000).unwrap(),
                publication: Some(app::PublicationSummary {
                    audience: "public".into(),
                    lifecycle: "active".into(),
                    generation: 3,
                    intent_revision_id: Some(revision("44444444-4444-4444-8444-444444444445")),
                    intent_moderation: Some("pending".into()),
                    latest_revision_id: Some(revision("44444444-4444-4444-8444-444444444445")),
                    latest_moderation: Some("pending".into()),
                    review_withdrawn: false,
                    has_draft_changes: Some(true),
                    current_public: Some(app::CurrentPublicSummary {
                        revision_id: revision("44444444-4444-4444-8444-444444444444"),
                        title: "Tên bản đang công khai".into(),
                        tham: Some(0),
                        comments: Some(12),
                    }),
                }),
            }],
        };
        let actual = serde_json::to_value(AuthorWorkList::from(page)).unwrap();
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../contracts/http/v1/fixtures/author.works.page.json"
        ))
        .unwrap();
        assert_eq!(actual, expected);
        assert!(actual["works"][0].get("document").is_none());
    }
}
