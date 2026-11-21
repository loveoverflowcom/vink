//! Search the current public profile snapshot. Neither publications nor viewer facts
//! participate in eligibility, matching, pagination, or the public representation.
use super::PgStore;
use super::discussion::read_snapshot;
use super::postgres::classify;
use crate::application::author_discovery::{AuthorPage, AuthorSearch, DiscoverAuthors};
use crate::application::discovery::DiscoveryError;
use crate::application::handle::AuthorRef;

/// Each branch contributes at most one extra row. Disjoint matches prevent a Handle that
/// also matched by name from displacing another author. A missing pattern contributes nothing.
/// Exposed for the plan test, which uses the same parameters as the adapter.
pub const AUTHOR_SEARCH_SQL: &str = "
    (SELECT p.display_name AS pen_name, s.current_handle AS handle
       FROM author_profiles s JOIN principals p ON p.id = s.principal_id
      WHERE s.current_handle > $3 AND s.current_handle LIKE $2
      ORDER BY s.current_handle LIMIT $4)
    UNION ALL
    (SELECT p.display_name AS pen_name, s.current_handle AS handle
       FROM principals p JOIN author_profiles s ON s.principal_id = p.id
      WHERE p.display_name_search_key COLLATE \"C\" LIKE $1
        AND s.current_handle > $3
        AND ($2::text IS NULL OR s.current_handle NOT LIKE $2)
      ORDER BY s.current_handle LIMIT $4)
    ORDER BY handle LIMIT $4";

impl PgStore {
    pub async fn discover_authors(
        &self,
        input: DiscoverAuthors,
    ) -> Result<AuthorPage, DiscoveryError> {
        let search = AuthorSearch::parse(input)?;
        let mut tx = read_snapshot(self.pool()).await?;
        let ready = sqlx::query_scalar::<_, bool>(
            "SELECT ready FROM author_handle_rollout WHERE singleton",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
        if !ready {
            return Err(DiscoveryError::Unavailable);
        }
        let name_pattern = match search.name.as_ref() {
            Some(name) => sqlx::query_scalar::<_, String>("SELECT vink_search_key($1)")
                .bind(name.text())
                .fetch_one(&mut *tx)
                .await
                .map(|key| (!key.is_empty()).then(|| format!("{key}%")))
                .map_err(classify)?,
            None => None,
        };
        let scope = search.scope(name_pattern.as_deref());
        let after = search.after(&scope)?;
        // The concrete prefix and absent branches need a custom plan even on a warm pool.
        let rows: Vec<(String, String)> = sqlx::query_as(AUTHOR_SEARCH_SQL)
            .persistent(false)
            .bind(name_pattern.as_deref())
            .bind(search.handle_pattern.as_deref())
            .bind(after)
            .bind(search.size.fetch() as i64)
            .fetch_all(&mut *tx)
            .await
            .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        let authors = rows
            .into_iter()
            .map(|(name, handle)| AuthorRef::new(name, handle))
            .collect();
        Ok(scope.page(authors, search.size))
    }
}
