//! Browser-owned private author summaries, never a public cache representation.
use super::publication::{Audience, Decision, Lifecycle};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorWorkList {
    pub works: Vec<AuthorWorkCard>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorWorkCard {
    pub work_id: String,
    pub title: String,
    pub draft_version: Option<i64>,
    pub updated_at: String,
    #[serde(default)]
    pub publication: Option<PublicationSummary>,
    #[serde(default)]
    pub views: Option<WorkViewsSummary>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkViewsSummary {
    pub state: WorkViewsState,
    #[serde(default)]
    pub count: Option<i64>,
    #[serde(default)]
    pub counted_since: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkViewsState {
    Counted,
    NotStarted,
    NeverPublic,
    Unavailable,
    #[serde(other)]
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationSummary {
    pub audience: Audience,
    pub lifecycle: Lifecycle,
    pub generation: i64,
    pub intent_revision_id: Option<String>,
    pub intent_moderation: Option<Decision>,
    pub latest_revision_id: Option<String>,
    pub latest_moderation: Option<Decision>,
    pub review_withdrawn: bool,
    pub has_draft_changes: Option<bool>,
    pub current_public: Option<CurrentPublicSummary>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentPublicSummary {
    pub revision_id: String,
    pub title: String,
    pub tham: Option<i64>,
    pub comments: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn labels(summary: &crate::ui::patterns::AuthorWorkSummary) -> Vec<&'static str> {
        summary.status.iter().map(|label| label.short).collect()
    }

    #[test]
    fn author_work_fixture_and_unknowns_never_invent_private_published_or_zero() {
        let raw = include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/author.works.page.json"
        );
        let page: AuthorWorkList = serde_json::from_str(raw).unwrap();
        assert_eq!(
            page.works[0]
                .publication
                .as_ref()
                .unwrap()
                .current_public
                .as_ref()
                .unwrap()
                .tham,
            Some(0)
        );
        let facts = crate::ui::adapters::author_work_summary(&page.works[0]);
        assert_eq!(
            labels(&facts),
            ["Đã đăng", "Có bản sửa", "Bản mới chờ duyệt"]
        );
        let public = facts.public.unwrap();
        assert_eq!((public.tham, public.comments), (Some(0), Some(12)));
        assert_eq!(public.title.as_deref(), Some("Tên bản đang công khai"));
        let unknown = raw
            .replace("\"public\"", "\"future-audience\"")
            .replace("\"pending\"", "\"future-decision\"");
        let page: AuthorWorkList = serde_json::from_str(&unknown).unwrap();
        let facts = crate::ui::adapters::author_work_summary(&page.works[0]);
        assert_eq!(labels(&facts), ["Chưa rõ"]);
        assert!(facts.public.is_none());
    }
    #[test]
    fn author_work_axes_unavailable_private_and_missing_draft_are_distinct() {
        let mut page: AuthorWorkList = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/author.works.page.json"
        ))
        .unwrap();
        let w = &mut page.works[0];
        w.publication
            .as_mut()
            .unwrap()
            .current_public
            .as_mut()
            .unwrap()
            .tham = None;
        let facts = crate::ui::adapters::author_work_summary(w);
        let public = facts.public.unwrap();
        assert_eq!((public.tham, public.comments), (None, Some(12)));
        w.publication.as_mut().unwrap().audience = Audience::Private;
        w.publication.as_mut().unwrap().current_public = None;
        w.publication.as_mut().unwrap().intent_revision_id = None;
        w.publication.as_mut().unwrap().latest_moderation = Some(Decision::Rejected);
        let facts = crate::ui::adapters::author_work_summary(w);
        assert_eq!(labels(&facts), ["Chỉ mình tôi", "Bản mới không được duyệt"]);
        assert!(facts.public.is_none());
        w.draft_version = None;
        let p = w.publication.as_mut().unwrap();
        p.generation = 1;
        p.latest_revision_id = None;
        p.latest_moderation = None;
        let facts = crate::ui::adapters::author_work_summary(w);
        assert_eq!(labels(&facts), ["Chỉ mình tôi"]);
        assert!(facts.editor_href.is_none() && facts.public.is_none());
        w.publication = None;
        let facts = crate::ui::adapters::author_work_summary(w);
        assert!(facts.editor_href.is_none() && facts.public.is_none());
        assert_eq!(labels(&facts), ["Chưa rõ"]);
        assert_eq!(
            facts.views,
            crate::ui::patterns::AuthorWorkViews::Counted {
                count: 8,
                counted_since: None,
            }
        );
        w.views = None;
        let facts = crate::ui::adapters::author_work_summary(w);
        assert_eq!(
            facts.views,
            crate::ui::patterns::AuthorWorkViews::Unavailable
        );
    }

    #[test]
    fn author_work_views_fixtures_cover_counted_zero_historical_not_started_never_public_and_unavailable()
     {
        use crate::ui::patterns::AuthorWorkViews;
        let counted_page: AuthorWorkList = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/author.works.views.json"
        ))
        .unwrap();
        assert_eq!(counted_page.works.len(), 6);

        let pub_with_since = crate::ui::adapters::author_work_summary(&counted_page.works[0]);
        assert_eq!(
            pub_with_since.views,
            AuthorWorkViews::Counted {
                count: 1234,
                counted_since: Some("01/12/2026".into()),
            }
        );

        let pub_zero = crate::ui::adapters::author_work_summary(&counted_page.works[1]);
        assert_eq!(
            pub_zero.views,
            AuthorWorkViews::Counted {
                count: 0,
                counted_since: None,
            }
        );

        let private_historical = crate::ui::adapters::author_work_summary(&counted_page.works[2]);
        assert!(private_historical.public.is_none());
        assert_eq!(
            private_historical.views,
            AuthorWorkViews::Counted {
                count: 3,
                counted_since: Some("01/12/2026".into()),
            }
        );

        let withdrawn_historical = crate::ui::adapters::author_work_summary(&counted_page.works[3]);
        assert!(withdrawn_historical.public.is_none());
        assert_eq!(
            withdrawn_historical.views,
            AuthorWorkViews::Counted {
                count: 5,
                counted_since: None,
            }
        );

        let never_public = crate::ui::adapters::author_work_summary(&counted_page.works[4]);
        assert!(never_public.public.is_none());
        assert_eq!(never_public.views, AuthorWorkViews::NeverPublic);

        let unavailable = crate::ui::adapters::author_work_summary(&counted_page.works[5]);
        assert!(unavailable.public.is_some());
        assert_eq!(unavailable.views, AuthorWorkViews::Unavailable);

        let not_started_page: AuthorWorkList = serde_json::from_str(include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/author.works.views-not-started.json"
        ))
        .unwrap();
        let not_started = crate::ui::adapters::author_work_summary(&not_started_page.works[0]);
        assert_eq!(not_started.views, AuthorWorkViews::NotStarted);

        // Unknown future state, missing views, or negative count all degrade safely to Unavailable.
        let future_json = include_str!(
            "../../../../services/vink-app/contracts/http/v1/fixtures/author.works.views-not-started.json"
        )
        .replace("\"not_started\"", "\"future_state\"");
        let future_page: AuthorWorkList = serde_json::from_str(&future_json).unwrap();
        assert_eq!(
            crate::ui::adapters::author_work_summary(&future_page.works[0]).views,
            AuthorWorkViews::Unavailable
        );
        let mut negative = counted_page.works[0].clone();
        negative.views.as_mut().unwrap().count = Some(-1);
        assert_eq!(
            crate::ui::adapters::author_work_summary(&negative).views,
            AuthorWorkViews::Unavailable
        );
    }
}
