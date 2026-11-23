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
    }
}
