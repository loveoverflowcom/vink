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
        assert!(
            facts.status.contains("Đã đăng · Có bản sửa") && facts.status.contains("chờ duyệt")
        );
        assert_eq!(facts.metrics.as_deref(), Some("Thấm 0 · Nhận xét 12"));
        assert_eq!(
            facts.public_context.as_deref(),
            Some("Bản công khai: Tên bản đang công khai")
        );
        let unknown = raw
            .replace("\"public\"", "\"future-audience\"")
            .replace("\"pending\"", "\"future-decision\"");
        let page: AuthorWorkList = serde_json::from_str(&unknown).unwrap();
        let facts = crate::ui::adapters::author_work_summary(&page.works[0]);
        assert!(facts.status.contains("Chưa xác định"));
        assert!(facts.metrics.is_none());
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
        assert_eq!(facts.metrics.as_deref(), Some("Thấm — · Nhận xét 12"));
        assert!(facts.public_context.unwrap().contains("Chưa tải được"));
        w.publication.as_mut().unwrap().audience = Audience::Private;
        w.publication.as_mut().unwrap().current_public = None;
        w.publication.as_mut().unwrap().intent_revision_id = None;
        w.publication.as_mut().unwrap().latest_moderation = Some(Decision::Rejected);
        let facts = crate::ui::adapters::author_work_summary(w);
        assert!(facts.status.contains("Chỉ mình tôi") && facts.status.contains("không được duyệt"));
        assert!(facts.metrics.is_none());
        w.draft_version = None;
        w.publication = None;
        let facts = crate::ui::adapters::author_work_summary(w);
        assert!(facts.editor_href.is_none() && facts.metrics.is_none());
        assert!(
            facts.status.contains("Chưa xác định") && facts.status.contains("Không có bản nháp")
        );
    }
}
