//! Private work summaries: one row per owned work, independent of draft existence.
use super::{DraftError, Vink, drafts::SearchDrafts, ports::AuthorWorkStore};
use crate::domain::{
    document::Title,
    draft::DraftVersion,
    draft_query::DraftSearch,
    ids::{PrincipalId, RevisionId, WorkId},
};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorWorkSummary {
    pub work_id: WorkId,
    pub title: Title,
    pub draft_version: Option<DraftVersion>,
    pub updated_at: OffsetDateTime,
    /// None means inconsistent/unknown state, never a guessed private/public state.
    pub publication: Option<PublicationSummary>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationSummary {
    pub audience: String,
    pub lifecycle: String,
    pub generation: i64,
    pub intent_revision_id: Option<RevisionId>,
    pub intent_moderation: Option<String>,
    pub latest_revision_id: Option<RevisionId>,
    pub latest_moderation: Option<String>,
    pub review_withdrawn: bool,
    /// Content/title and tag identity comparison against the current public revision.
    /// None when there is no draft/public revision or legacy metadata is unknown.
    pub has_draft_changes: Option<bool>,
    pub current_public: Option<CurrentPublicSummary>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentPublicSummary {
    pub revision_id: RevisionId,
    pub title: String,
    /// None means metrics unavailable; zero is an observed count in the read snapshot.
    pub tham: Option<i64>,
    pub comments: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorWorkPage {
    pub works: Vec<AuthorWorkSummary>,
    pub next_cursor: Option<String>,
}
impl<S: AuthorWorkStore> Vink<S> {
    pub async fn search_author_works(
        &self,
        actor: Option<PrincipalId>,
        input: SearchDrafts,
    ) -> Result<AuthorWorkPage, DraftError> {
        let owner = actor.ok_or(DraftError::Unauthenticated)?;
        let search = DraftSearch::parse_author_works(
            owner,
            input.query.as_deref().unwrap_or_default(),
            input.sort.as_deref(),
            input.limit.as_deref(),
            input.cursor.as_deref(),
        )
        .map_err(DraftError::Query)?;
        let mut works = self.store.search_author_works(owner, &search).await?;
        let more = works.len() > search.size();
        works.truncate(search.size());
        let next_cursor = more
            .then(|| {
                works
                    .last()
                    .map(|w| search.cursor_after(w.work_id, w.title.as_str(), w.updated_at))
            })
            .flatten();
        Ok(AuthorWorkPage { works, next_cursor })
    }
}
