//! Provider-owned, bounded private author-work representation.
use crate::application::author_works as app;
use crate::domain::work_view::count::ViewCount;
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
    pub views: WorkViews,
}
/// Work views of one work (#342). Always present, with every key, so a consumer never mistakes
/// a missing number for zero: `count` is a number only in state `counted` (possibly 0), and
/// `counted_since` is set only there, when the total starts later than the work's public life
/// ("Tính từ"). Unknown future states must be treated as `unavailable`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkViews {
    /// `counted` | `not_started` | `never_public` | `unavailable`.
    pub state: &'static str,
    pub count: Option<u64>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub counted_since: Option<OffsetDateTime>,
}
impl From<Option<ViewCount>> for WorkViews {
    fn from(views: Option<ViewCount>) -> Self {
        let (state, count, counted_since) = match views {
            Some(ViewCount::Counted { views, since }) => ("counted", Some(views), since),
            Some(ViewCount::NotStarted) => ("not_started", None, None),
            Some(ViewCount::NeverPublic) => ("never_public", None, None),
            None => ("unavailable", None, None),
        };
        Self {
            state,
            count,
            counted_since,
        }
    }
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
    pub has_review_context: bool,
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
                        has_review_context: p.has_review_context,
                        has_draft_changes: p.has_draft_changes,
                        current_public: p.current_public.map(|p| CurrentPublicSummary {
                            revision_id: p.revision_id.to_string(),
                            title: p.title,
                            tham: p.tham,
                            comments: p.comments,
                        }),
                    }),
                    views: w.views.into(),
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
                    has_review_context: true,
                    has_draft_changes: Some(true),
                    current_public: Some(app::CurrentPublicSummary {
                        revision_id: revision("44444444-4444-4444-8444-444444444444"),
                        title: "Tên bản đang công khai".into(),
                        tham: Some(0),
                        comments: Some(12),
                    }),
                }),
                views: Some(ViewCount::Counted {
                    views: 8,
                    since: None,
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

    fn uuid(prefix: &str, n: u8) -> uuid::Uuid {
        format!("{prefix}{n:02}").parse().unwrap()
    }

    /// One row of the work-view examples: `public` = current public title with its counters,
    /// `private` = no pointer (hidden, or withdrawn when `withdrawn`), `draft` = never public.
    fn card(
        n: u8,
        title: &str,
        shape: &str,
        views: Option<ViewCount>,
        updated: &str,
    ) -> app::AuthorWorkSummary {
        let revision = RevisionId::from_uuid(uuid("66666666-6666-4666-8666-6666666666", n));
        let public = shape == "public";
        let withdrawn = shape == "withdrawn";
        let counters = match n {
            1 => (4, 2),
            6 => (1, 3),
            _ => (0, 0),
        };
        let publication = if shape == "draft" {
            app::PublicationSummary {
                audience: "private".into(),
                lifecycle: "active".into(),
                generation: 1,
                intent_revision_id: None,
                intent_moderation: None,
                latest_revision_id: None,
                latest_moderation: None,
                review_withdrawn: false,
                has_review_context: false,
                has_draft_changes: None,
                current_public: None,
            }
        } else {
            app::PublicationSummary {
                audience: if public { "public" } else { "private" }.into(),
                lifecycle: "active".into(),
                generation: if public { 2 } else { 3 },
                intent_revision_id: public.then_some(revision),
                intent_moderation: public.then(|| "approved".into()),
                latest_revision_id: Some(revision),
                latest_moderation: Some(if withdrawn { "rejected" } else { "approved" }.into()),
                review_withdrawn: withdrawn,
                has_review_context: true,
                has_draft_changes: Some(false),
                current_public: public.then(|| app::CurrentPublicSummary {
                    revision_id: revision,
                    title: title.into(),
                    tham: Some(counters.0),
                    comments: Some(counters.1),
                }),
            }
        };
        app::AuthorWorkSummary {
            work_id: WorkId::from_uuid(uuid("55555555-5555-4555-8555-5555555555", n)),
            title: Title::parse(title).unwrap(),
            draft_version: DraftVersion::new(if shape == "draft" { 7 } else { 2 }),
            updated_at: time::OffsetDateTime::parse(
                updated,
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap(),
            publication: Some(publication),
            views,
        }
    }

    /// AC-04 of #342: every state a consumer must tell apart, as the provider serializes it.
    /// `author.works.views.json` is a page after counting started (L-12 with "Tính từ", L-02
    /// zero, L-07 private and L-08 withdrawn keeping their totals, L-01 never public, L-05
    /// unavailable while Thấm/Nhận xét still read); `author.works.views-not-started.json` is a
    /// page before any event was decided (VIEW-09).
    #[test]
    fn work_view_states_match_provider_fixtures() {
        let start = time::macros::datetime!(2026-12-01 01:00 UTC);
        let counted = |views, since| Some(ViewCount::Counted { views, since });
        let after = "2026-12-02T03:00:00Z";
        let started = app::AuthorWorkPage {
            next_cursor: None,
            works: vec![
                card(
                    1,
                    "Mưa trên phố cũ",
                    "public",
                    counted(1234, Some(start)),
                    after,
                ),
                card(2, "Gió mùa", "public", counted(0, None), after),
                card(3, "Thư gửi mẹ", "private", counted(3, Some(start)), after),
                card(4, "Bến đò", "withdrawn", counted(5, None), after),
                card(
                    5,
                    "Bản nháp chưa đăng",
                    "draft",
                    Some(ViewCount::NeverPublic),
                    after,
                ),
                card(6, "Đêm trắng", "public", None, after),
            ],
        };
        let before = "2026-11-30T03:00:00Z";
        let not_started = app::AuthorWorkPage {
            next_cursor: None,
            works: vec![
                card(
                    1,
                    "Mưa trên phố cũ",
                    "public",
                    Some(ViewCount::NotStarted),
                    before,
                ),
                card(
                    5,
                    "Bản nháp chưa đăng",
                    "draft",
                    Some(ViewCount::NeverPublic),
                    before,
                ),
            ],
        };
        for (page, fixture) in [
            (
                started,
                include_str!("../../contracts/http/v1/fixtures/author.works.views.json"),
            ),
            (
                not_started,
                include_str!(
                    "../../contracts/http/v1/fixtures/author.works.views-not-started.json"
                ),
            ),
        ] {
            let actual = serde_json::to_value(AuthorWorkList::from(page)).unwrap();
            let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();
            assert_eq!(actual, expected);
            for work in actual["works"].as_array().unwrap() {
                let views = work["views"].as_object().unwrap();
                assert_eq!(views.len(), 3, "every key is always present");
                // A number exists only for a measured state; nothing else ever reads as 0.
                assert_eq!(views["count"].is_number(), views["state"] == "counted");
                assert!(views["counted_since"].is_null() || views["state"] == "counted");
            }
        }
    }
}
