//! Public discovery use cases (TAG-PR-04, #98): search the whole public library by text and
//! tags, one bounded page at a time, and read one publication with its public tags. Choices,
//! contract, plans and evidence: docs/development/discovery-api.md.
//!
//! No actor is resolved and nothing here reads Identity, a working draft, private draft tags or
//! an audit row. Raw input is validated through the domain first (invalid input never reaches
//! the store); the store reads ONE snapshot (filter slugs, matching works, their snapshot tags);
//! this module projects it with the pure rules: `resolve_slug` (through the catalog helper shared
//! with tag URLs), `filter_tags`, `public_tags`, `SearchScope` and `cut_page`.

use super::Vink;
use super::catalog::{TagResolution, resolution};
use super::ports::{
    DiscoveryRequest, PublicCardFacts, PublicationFacts, PublicationStore, SlugFacts, StoreError,
};
use super::{PublicWork, PublicWorkSummary, ReadError};
use crate::domain::discovery::{
    CursorError, DiscoveryCursor, FilterError, PagePosition, PageSize, PageSizeError, SearchScope,
    TagFilter, WorkQuery, WorkQueryError, cut_page, filter_tags,
};
use crate::domain::ids::WorkId;
use crate::domain::taxonomy::{CatalogTag, MetadataSnapshot, SlugResolution, TagSlug, public_tags};

/// Untrusted search input, as received (`?q=&tag=&tag=&limit=&cursor=`). A blank value means
/// the parameter is absent, as an empty field of a native GET form submits it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoverWorks {
    pub query: Option<String>,
    pub tags: Vec<String>,
    pub limit: Option<String>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DiscoveryInput {
    #[error("{0}")]
    Query(WorkQueryError),
    #[error("{0}")]
    Tag(FilterError),
    #[error("{0}")]
    Limit(PageSizeError),
    #[error("{0}")]
    Cursor(CursorError),
    /// A well-formed cursor of another search (other query, other tags, or a tag renamed into
    /// another identity is impossible): the client restarts from the first page.
    #[error("cursor belongs to another search; start from the first page")]
    CursorScope,
}

impl DiscoveryInput {
    pub const fn field(&self) -> &'static str {
        match self {
            Self::Query(_) => "q",
            Self::Tag(_) => "tag",
            Self::Limit(_) => "limit",
            Self::Cursor(_) | Self::CursorScope => "cursor",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiscoveryError {
    #[error("invalid input: {0}")]
    Invalid(#[from] DiscoveryInput),
    #[error("storage unavailable")]
    Unavailable,
    #[error("stored data failed validation")]
    Internal,
}

impl From<StoreError> for DiscoveryError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            StoreError::Integrity(_) | StoreError::UnknownPrincipal => Self::Internal,
        }
    }
}

/// What one requested filter slug named in the search's snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedFilter {
    /// The normalized slug as requested (it may be an alias or a former name).
    pub requested: TagSlug,
    /// `None`: no tag has this slug.
    pub resolution: Option<TagResolution>,
}

/// One page of a public search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkPage {
    /// The query text exactly as received; `None` when absent or blank.
    pub query: Option<String>,
    pub filters: Vec<AppliedFilter>,
    /// Newest first, at most one page.
    pub works: Vec<PublicWorkSummary>,
    /// Present only when another matching work was read after this page.
    pub next: Option<DiscoveryCursor>,
}

fn present(raw: &Option<String>) -> Option<&str> {
    raw.as_deref().filter(|value| !value.trim().is_empty())
}

fn page_size(raw: Option<&str>) -> Result<PageSize, DiscoveryInput> {
    match raw {
        None => Ok(PageSize::DEFAULT),
        Some(raw) => raw
            .parse::<i64>()
            .map_err(|_| PageSizeError::OutOfRange)
            .and_then(PageSize::new)
            .map_err(DiscoveryInput::Limit),
    }
}

/// Public tags of a bound snapshot: active ones in snapshot order with current labels. Missing or
/// wrong-facet catalog rows are corruption, never an empty list.
fn shown_tags(
    snapshot: Option<&MetadataSnapshot>,
    catalog: &[CatalogTag],
) -> Result<Vec<CatalogTag>, StoreError> {
    match snapshot {
        None => Ok(Vec::new()),
        Some(snapshot) => Ok(public_tags(snapshot, catalog)
            .map_err(|_| StoreError::Integrity("public tag catalog rows".into()))?
            .into_iter()
            .cloned()
            .collect()),
    }
}

fn summary(card: PublicCardFacts, catalog: &[CatalogTag]) -> Result<PublicWorkSummary, StoreError> {
    let tags = shown_tags(card.snapshot.as_ref(), catalog)?;
    Ok(PublicWorkSummary {
        work_id: card.work_id,
        title: card.title,
        author_display_name: card.author_display_name,
        published_at: card.published_at,
        tags,
    })
}

fn applied(slug: &TagSlug, facts: &SlugFacts) -> Result<AppliedFilter, DiscoveryError> {
    let resolution = resolution(slug, facts).map_err(|_| DiscoveryError::Internal)?;
    Ok(AppliedFilter {
        requested: slug.clone(),
        resolution,
    })
}

/// The same resolution, borrowed, for the domain's `filter_tags`.
fn borrowed(filter: &AppliedFilter) -> SlugResolution<'_> {
    match &filter.resolution {
        None => SlugResolution::NotFound,
        Some(TagResolution::Canonical(tag)) => SlugResolution::Canonical(tag),
        Some(TagResolution::Redirect(tag)) => SlugResolution::Redirect(tag),
        Some(TagResolution::Retired { tag, .. }) => SlugResolution::Retired(tag),
    }
}

impl<S> Vink<S>
where
    S: PublicationStore,
{
    /// One bounded page of public works matching `input`, newest first. Validation precedes any
    /// read: `q`, then `tag`, then `limit`, then the cursor's shape; its scope is checked against
    /// the canonical search read in the same snapshot.
    pub async fn discover_works(&self, input: DiscoverWorks) -> Result<WorkPage, DiscoveryError> {
        let query = present(&input.query)
            .map(WorkQuery::parse)
            .transpose()
            .map_err(DiscoveryInput::Query)?
            .flatten();
        let filter =
            TagFilter::parse(input.tags.iter().map(String::as_str)).map_err(DiscoveryInput::Tag)?;
        let size = page_size(present(&input.limit))?;
        let cursor = present(&input.cursor)
            .map(DiscoveryCursor::decode)
            .transpose()
            .map_err(DiscoveryInput::Cursor)?;
        let facts = self
            .store
            .discover_works(&DiscoveryRequest {
                query: query.as_ref(),
                filter: &filter,
                after: cursor.map(|cursor| cursor.after()),
                size,
            })
            .await?;
        if facts.filters.len() != filter.slugs().len() {
            return Err(DiscoveryError::Internal);
        }
        let filters = filter
            .slugs()
            .iter()
            .zip(&facts.filters)
            .map(|(slug, facts)| applied(slug, facts))
            .collect::<Result<Vec<_>, _>>()?;
        let resolutions: Vec<SlugResolution<'_>> = filters.iter().map(borrowed).collect();
        let text = query.as_ref().map(|q| q.text().to_owned());
        let Some(tags) = filter_tags(&resolutions) else {
            // No public work carries a tag that is unknown or retired; the adapter searched none.
            if !facts.works.is_empty() {
                return Err(DiscoveryError::Internal);
            }
            return Ok(WorkPage {
                query: text,
                filters,
                works: Vec::new(),
                next: None,
            });
        };
        let scope = SearchScope::of(query.as_ref().map(WorkQuery::key), &tags);
        if cursor.is_some_and(|cursor| cursor.scope() != scope) {
            return Err(DiscoveryInput::CursorScope.into());
        }
        let catalog = facts.catalog;
        let works = facts
            .works
            .into_iter()
            .map(|card| summary(card, &catalog))
            .collect::<Result<Vec<_>, _>>()?;
        let (works, next) = cut_page(works, size, scope, |work| PagePosition {
            published_at: work.published_at,
            work_id: work.work_id,
        });
        Ok(WorkPage {
            query: text,
            filters,
            works,
            next,
        })
    }

    /// The current public snapshot of a work with its public tags, from one read snapshot.
    pub async fn publication(&self, work_id: WorkId) -> Result<PublicWork, ReadError> {
        let PublicationFacts {
            work_id,
            revision_id,
            title,
            author_display_name,
            published_at,
            document,
            snapshot,
            catalog,
        } = self
            .store
            .publication(work_id)
            .await?
            .ok_or(ReadError::NotFound)?;
        let tags = shown_tags(snapshot.as_ref(), &catalog)?;
        Ok(PublicWork {
            work_id,
            revision_id,
            title,
            author_display_name,
            published_at,
            document,
            tags,
        })
    }
}
