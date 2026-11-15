//! Owner-only draft search. The session supplies authority; a cursor only names a position.
use super::ports::DraftStore;
use super::{DraftError, Vink};
use crate::domain::draft_query::{DraftPage, DraftSearch};
use crate::domain::ids::PrincipalId;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchDrafts {
    pub query: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<String>,
    pub cursor: Option<String>,
}

impl<S: DraftStore> Vink<S> {
    pub async fn search_drafts(
        &self,
        actor: Option<PrincipalId>,
        input: SearchDrafts,
    ) -> Result<DraftPage, DraftError> {
        let owner = actor.ok_or(DraftError::Unauthenticated)?;
        let search = DraftSearch::parse(
            owner,
            input.query.as_deref().unwrap_or_default(),
            input.sort.as_deref(),
            input.limit.as_deref(),
            input.cursor.as_deref(),
        )
        .map_err(DraftError::Query)?;
        let rows = self.store.search_drafts(owner, &search).await?;
        Ok(search.page(rows))
    }
}
