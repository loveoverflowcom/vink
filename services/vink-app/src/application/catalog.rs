//! Curated catalog use cases (TAG-PR-03, #97): public lookup of the catalog and operator
//! governance. Contract, choices and evidence: docs/development/taxonomy-api.md.
//!
//! **Reads** need no actor and never touch Identity, drafts or audit rows. Each one validates its
//! raw input through the domain parsers first (invalid input never reaches the store), reads one
//! snapshot through [`CatalogReadStore`] and projects it with the pure core: `rank_matches` for
//! suggestions, `resolve_slug` for a tag URL. Only active tags are offered; retired tags are
//! visible only as a resolution of their own slug, with the curator's successor hint. Accent
//! folding finds candidates and never merges two slugs.
//!
//! **Governance** is the operator command (`vink-app tag …`, #97 CHOICE-02 A). Raw operator input
//! becomes a domain [`CatalogCommand`] only through [`ChangeCatalog::validate`]; the command and a
//! [`CuratorAuthority`] go to ONE [`CatalogStore`] transaction that decides with
//! `decide_catalog_change` and writes the change and its audit row together. The application maps
//! the outcome; it restates no catalog rule. A conflict means "re-read the catalog", never
//! "refresh the version and resend".

use super::Vink;
use super::ports::{
    CatalogAuditRecord, CatalogListing, CatalogReadStore, CatalogStore, SlugFacts, StoreError,
};
use crate::domain::ids::TagId;
use crate::domain::taxonomy::{
    CatalogChange, CatalogCommand, CatalogOutcome, CatalogRefusal, CatalogTag, CatalogVersion,
    CatalogWrite, DescriptionError, LabelError, MAX_SUGGESTIONS, MatchTier, QueryError,
    SlugResolution, SuccessorError, TagDescription, TagFacet, TagLabel, TagQuery, TagSlug,
    TagStatus, rank_matches, resolve_slug,
};

/// Most active tags one catalog list returns. The pilot catalog is curated and small; a longer
/// catalog is answered truncated (flagged), never silently paged. Revisit with #98/#99 data.
pub const MAX_LISTED_TAGS: usize = 200;
/// Most audit rows one `tag history` prints.
pub const MAX_HISTORY: i64 = 1000;

/// Authority to change the curated catalog. Only the operator entrypoint constructs one; no HTTP
/// handler does (a test in `http` guards this), and comment-moderator grants never imply it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CuratorAuthority(Curator);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Curator {
    Operator,
}

impl CuratorAuthority {
    /// The operator command `vink-app tag …`: whoever runs it holds the App database credential,
    /// as for `vink-app moderator`. A future principal grant would be another variant whose
    /// store reads the grant under a share lock in the change's transaction.
    pub const fn operator() -> Self {
        Self(Curator::Operator)
    }

    /// The name stored in the audit row.
    pub const fn audit_name(self) -> &'static str {
        match self.0 {
            Curator::Operator => "operator",
        }
    }
}

/// The kind of catalog change, as stored in the audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogOperation {
    Create,
    Rename,
    Describe,
    Alias,
    Retire,
}

impl CatalogOperation {
    pub const fn of(change: &CatalogChange) -> Self {
        match change {
            CatalogChange::Create { .. } => Self::Create,
            CatalogChange::Rename { .. } => Self::Rename,
            CatalogChange::Describe { .. } => Self::Describe,
            CatalogChange::AddAlias { .. } => Self::Alias,
            CatalogChange::Retire { .. } => Self::Retire,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Rename => "rename",
            Self::Describe => "describe",
            Self::Alias => "alias",
            Self::Retire => "retire",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        [
            Self::Create,
            Self::Rename,
            Self::Describe,
            Self::Alias,
            Self::Retire,
        ]
        .into_iter()
        .find(|operation| operation.name() == raw)
    }
}

/// Wire and storage name of a facet.
pub const fn facet_code(facet: TagFacet) -> &'static str {
    match facet {
        TagFacet::Form => "form",
        TagFacet::Theme => "theme",
    }
}

pub fn parse_facet(raw: &str) -> Option<TagFacet> {
    match raw {
        "form" => Some(TagFacet::Form),
        "theme" => Some(TagFacet::Theme),
        _ => None,
    }
}

/// Untrusted lookup input (`?q=&facet=&limit=`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchTags {
    pub query: Option<String>,
    pub facet: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CatalogQueryInput {
    #[error("q is required")]
    MissingQuery,
    #[error("q: {0}")]
    Query(QueryError),
    #[error("facet must be `form` or `theme`")]
    Facet,
    #[error("limit must be between 1 and {MAX_SUGGESTIONS}")]
    Limit,
}

impl CatalogQueryInput {
    pub const fn field(&self) -> &'static str {
        match self {
            Self::MissingQuery | Self::Query(_) => "q",
            Self::Facet => "facet",
            Self::Limit => "limit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogReadError {
    #[error("invalid input: {0}")]
    Invalid(#[from] CatalogQueryInput),
    /// No tag has this slug (a malformed slug cannot name one).
    #[error("not found")]
    NotFound,
    #[error("storage unavailable")]
    Unavailable,
    #[error("internal error")]
    Internal,
}

impl From<StoreError> for CatalogReadError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            StoreError::Integrity(_)
            | StoreError::UnknownPrincipal
            | StoreError::PrincipalSuspended => Self::Internal,
        }
    }
}

/// Active tags of the catalog in display order, at most [`MAX_LISTED_TAGS`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogPage {
    pub tags: Vec<CatalogTag>,
    /// More active tags exist than were returned.
    pub truncated: bool,
}

/// One lookup suggestion: always a canonical active tag, with how it matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagSuggestion {
    pub tag: CatalogTag,
    pub tier: MatchTier,
    /// The alias spelling that matched best, if it was not the tag's own label.
    pub via_alias: Option<TagLabel>,
}

/// What a tag URL names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagResolution {
    Canonical(CatalogTag),
    /// An alias or former slug of an active tag; the client moves to the tag's slug.
    Redirect(CatalogTag),
    /// Any slug of a retired tag, with the curator's successor hint if one was named.
    Retired {
        tag: CatalogTag,
        successor: Option<CatalogTag>,
    },
}

fn display_key(tag: &CatalogTag) -> (u8, String, String, TagId) {
    let facet = match tag.facet() {
        TagFacet::Form => 0,
        TagFacet::Theme => 1,
    };
    (
        facet,
        tag.label().search_key().as_str().to_owned(),
        tag.slug().as_str().to_owned(),
        tag.id(),
    )
}

fn facet_filter(raw: Option<&str>) -> Result<Option<TagFacet>, CatalogQueryInput> {
    raw.map(|raw| parse_facet(raw).ok_or(CatalogQueryInput::Facet))
        .transpose()
}

impl<S> Vink<S>
where
    S: CatalogReadStore,
{
    /// Active tags, optionally of one facet: form first, then by folded label, slug, identity.
    pub async fn catalog_tags(&self, facet: Option<&str>) -> Result<CatalogPage, CatalogReadError> {
        let facet = facet_filter(facet)?;
        let catalog = self.store.active_catalog().await?;
        let mut tags: Vec<CatalogTag> = catalog
            .tags
            .into_iter()
            .filter(|tag| tag.status().is_active() && facet.is_none_or(|f| tag.facet() == f))
            .collect();
        tags.sort_by_cached_key(display_key);
        let truncated = tags.len() > MAX_LISTED_TAGS;
        tags.truncate(MAX_LISTED_TAGS);
        Ok(CatalogPage { tags, truncated })
    }

    /// Bounded suggestions for what someone typed. Validation precedes any read.
    pub async fn search_tags(
        &self,
        input: SearchTags,
    ) -> Result<Vec<TagSuggestion>, CatalogReadError> {
        let raw = input
            .query
            .as_deref()
            .ok_or(CatalogQueryInput::MissingQuery)?;
        let query = TagQuery::parse(raw).map_err(CatalogQueryInput::Query)?;
        let facet = facet_filter(input.facet.as_deref())?;
        let limit = match input.limit {
            None => MAX_SUGGESTIONS,
            Some(n) => usize::try_from(n)
                .ok()
                .filter(|n| (1..=MAX_SUGGESTIONS).contains(n))
                .ok_or(CatalogQueryInput::Limit)?,
        };
        let catalog = self.store.active_catalog().await?;
        let tags: Vec<CatalogTag> = catalog
            .tags
            .into_iter()
            .filter(|tag| facet.is_none_or(|f| tag.facet() == f))
            .collect();
        Ok(rank_matches(&query, &tags, &catalog.aliases, limit)
            .into_iter()
            .map(|found| TagSuggestion {
                tag: found.tag.clone(),
                tier: found.tier,
                via_alias: found.via_alias.map(|alias| alias.label().clone()),
            })
            .collect())
    }

    /// What a tag URL path segment (already percent-decoded) names.
    pub async fn resolve_tag(&self, raw_slug: &str) -> Result<TagResolution, CatalogReadError> {
        let slug = TagSlug::parse(raw_slug).map_err(|_| CatalogReadError::NotFound)?;
        let facts = self.store.slug_facts(&slug).await?;
        resolution(&slug, &facts)?.ok_or(CatalogReadError::NotFound)
    }
}

/// What `slug` names according to `facts` (read in one snapshot); `None` when it names no tag.
/// Shared by tag URLs and discovery filters (TAG-PR-04), so both resolve a slug the same way.
pub(super) fn resolution(
    slug: &TagSlug,
    facts: &SlugFacts,
) -> Result<Option<TagResolution>, CatalogReadError> {
    let resolution = resolve_slug(slug, facts.holder, facts.tag.as_ref())
        .map_err(|_| CatalogReadError::Internal)?;
    Ok(match resolution {
        SlugResolution::NotFound => None,
        SlugResolution::Canonical(tag) => Some(TagResolution::Canonical(tag.clone())),
        SlugResolution::Redirect(tag) => Some(TagResolution::Redirect(tag.clone())),
        SlugResolution::Retired(tag) => {
            // The successor row must be exactly the hint the retired tag names.
            let hinted = match tag.status() {
                TagStatus::Retired { successor } => successor,
                TagStatus::Active => None,
            };
            let successor = match (hinted, &facts.successor) {
                (None, None) => None,
                (Some(id), Some(row)) if row.id() == id => Some(row.clone()),
                _ => return Err(CatalogReadError::Internal),
            };
            Some(TagResolution::Retired {
                tag: tag.clone(),
                successor,
            })
        }
    })
}

/// Untrusted operator input for one catalog change. Labels and descriptions are raw text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogRequest {
    Create {
        facet: String,
        label: String,
    },
    Rename {
        tag: String,
        label: String,
    },
    /// `None` clears the description.
    Describe {
        tag: String,
        description: Option<String>,
    },
    AddAlias {
        tag: String,
        label: String,
    },
    Retire {
        tag: String,
        successor: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeCatalog {
    /// The catalog version the operator last read (`vink-app tag list`).
    pub expected_version: i64,
    pub request: CatalogRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CatalogInput {
    #[error("catalog version must be an integer >= 1")]
    ExpectedVersion,
    #[error("facet must be `form` or `theme`")]
    Facet,
    #[error("tag must be a tag UUID")]
    Tag,
    #[error("successor must be a tag UUID")]
    Successor,
    #[error("label: {0}")]
    Label(LabelError),
    #[error("description: {0}")]
    Description(DescriptionError),
    #[error("history limit must be between 1 and {MAX_HISTORY}")]
    Limit,
}

impl CatalogInput {
    pub const fn field(&self) -> &'static str {
        match self {
            Self::ExpectedVersion => "catalog_version",
            Self::Facet => "facet",
            Self::Tag => "tag",
            Self::Successor => "successor",
            Self::Label(_) => "label",
            Self::Description(_) => "description",
            Self::Limit => "limit",
        }
    }
}

fn tag_id(raw: &str, error: CatalogInput) -> Result<TagId, CatalogInput> {
    TagId::parse(raw).ok_or(error)
}

fn label(raw: &str) -> Result<TagLabel, CatalogInput> {
    TagLabel::parse(raw).map_err(CatalogInput::Label)
}

impl ChangeCatalog {
    /// The only path from raw operator input to a domain command. Pure: it reads no catalog, so
    /// it runs before any connection. A `Create` gets a fresh server-chosen identity.
    pub fn validate(&self) -> Result<CatalogCommand, CatalogInput> {
        let expected =
            CatalogVersion::new(self.expected_version).ok_or(CatalogInput::ExpectedVersion)?;
        let change = match &self.request {
            CatalogRequest::Create { facet, label: raw } => CatalogChange::Create {
                id: TagId::generate(),
                facet: parse_facet(facet).ok_or(CatalogInput::Facet)?,
                label: label(raw)?,
                description: None,
            },
            CatalogRequest::Rename { tag, label: raw } => CatalogChange::Rename {
                id: tag_id(tag, CatalogInput::Tag)?,
                label: label(raw)?,
            },
            CatalogRequest::Describe { tag, description } => CatalogChange::Describe {
                id: tag_id(tag, CatalogInput::Tag)?,
                description: description
                    .as_deref()
                    .map(TagDescription::parse)
                    .transpose()
                    .map_err(CatalogInput::Description)?,
            },
            CatalogRequest::AddAlias { tag, label: raw } => CatalogChange::AddAlias {
                target: tag_id(tag, CatalogInput::Tag)?,
                label: label(raw)?,
            },
            CatalogRequest::Retire { tag, successor } => CatalogChange::Retire {
                id: tag_id(tag, CatalogInput::Tag)?,
                successor: successor
                    .as_deref()
                    .map(|raw| tag_id(raw, CatalogInput::Successor))
                    .transpose()?,
            },
        };
        Ok(CatalogCommand { expected, change })
    }
}

/// Curator-facing refusals; nothing was written, audit included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogRejection {
    NotCurator,
    UnknownTag,
    TagIdTaken,
    /// The label's slug already names `holder` (canonically, as an alias or a former name).
    SlugTaken {
        holder: TagId,
    },
    /// Retired tags take no rename, description, alias or second retirement in the pilot.
    Retired,
    AliasIsCanonicalName,
    InvalidSuccessor(SuccessorError),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    #[error("invalid input: {0}")]
    Invalid(#[from] CatalogInput),
    /// Nothing written; re-read the catalog and decide again.
    #[error("catalog changed; current version is {}", current.get())]
    Conflict { current: CatalogVersion },
    #[error("catalog change refused")]
    Rejected(CatalogRejection),
    #[error("storage unavailable")]
    Unavailable,
    #[error("internal error")]
    Internal,
}

impl From<StoreError> for CatalogError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            StoreError::Integrity(_)
            | StoreError::UnknownPrincipal
            | StoreError::PrincipalSuspended => Self::Internal,
        }
    }
}

/// An accepted change as committed with its audit row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogChanged {
    pub operation: CatalogOperation,
    pub subject: TagId,
    pub write: CatalogWrite,
}

impl<S> Vink<S>
where
    S: CatalogStore,
{
    pub async fn change_catalog(
        &self,
        authority: CuratorAuthority,
        command: CatalogCommand,
    ) -> Result<CatalogChanged, CatalogError> {
        use CatalogRefusal as R;
        let outcome = self.store.change_catalog(authority, &command).await?;
        let rejected = |rejection| Err(CatalogError::Rejected(rejection));
        match outcome {
            CatalogOutcome::Accepted(write) => Ok(CatalogChanged {
                operation: CatalogOperation::of(&command.change),
                subject: command.change.subject(),
                write,
            }),
            CatalogOutcome::Conflict { current } => Err(CatalogError::Conflict { current }),
            CatalogOutcome::Refused(refusal) => match refusal {
                R::NotCurator => rejected(CatalogRejection::NotCurator),
                R::UnknownTag => rejected(CatalogRejection::UnknownTag),
                R::TagIdTaken => rejected(CatalogRejection::TagIdTaken),
                R::SlugTaken { holder } => rejected(CatalogRejection::SlugTaken { holder }),
                R::Retired => rejected(CatalogRejection::Retired),
                R::AliasIsCanonicalName => rejected(CatalogRejection::AliasIsCanonicalName),
                R::InvalidSuccessor(error) => rejected(CatalogRejection::InvalidSuccessor(error)),
                R::InvalidFacts | R::VersionExhausted => Err(CatalogError::Internal),
            },
        }
    }

    pub async fn catalog_listing(&self) -> Result<CatalogListing, CatalogError> {
        Ok(self.store.catalog_listing().await?)
    }

    pub async fn catalog_history(
        &self,
        limit: i64,
    ) -> Result<Vec<CatalogAuditRecord>, CatalogError> {
        if !(1..=MAX_HISTORY).contains(&limit) {
            return Err(CatalogInput::Limit.into());
        }
        Ok(self.store.catalog_history(limit).await?)
    }
}
