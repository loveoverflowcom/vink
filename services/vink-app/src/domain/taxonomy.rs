//! Work taxonomy core (TAG-PR-01, #95): what a tag means, independent of SQL, HTTP and UI.
//!
//! Policy and alternatives: docs/architecture/work-taxonomy.md; requirement → function → test
//! registry: docs/development/taxonomy-core.md. The pilot selects one curated catalog with two
//! closed facets (form, theme), draft tags with their own version, and an immutable metadata
//! snapshot bound beside the content revision. A tag describes a work; it grants no permission,
//! publishes nothing, curates nothing and is not a quality mark.
//!
//! Nothing here reads a clock, a database, a request or a session. Actor, standing, ownership,
//! versions and catalog rows arrive as explicit facts; the adapter that loads them under its
//! locks (TAG-PR-02, #96) is responsible for their currency and for uniqueness constraints.
//!
//! Identities cannot stand in for one another:
//! ```compile_fail
//! use vink_app::domain::ids::{TagId, WorkId};
//! let tag = TagId::parse("a1111111-1111-4111-8111-111111111111").unwrap();
//! let work: WorkId = tag;
//! ```
//! Lawful twin (stable rustdoc does not check `compile_fail` error codes): the same setup
//! compiles; only the substitution fails.
//! ```
//! use vink_app::domain::ids::{TagId, WorkId};
//! let tag = TagId::parse("a1111111-1111-4111-8111-111111111111").unwrap();
//! let work: WorkId = WorkId::from_uuid(tag.as_uuid());
//! assert_eq!(work.as_uuid(), tag.as_uuid());
//! ```

/// A positive version token: starts at 1, advances by checked +1.
macro_rules! positive_token {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(i64);

        impl $name {
            pub const FIRST: Self = Self(1);

            pub fn new(raw: i64) -> Option<Self> {
                (raw >= 1).then_some(Self(raw))
            }

            pub const fn get(self) -> i64 {
                self.0
            }

            /// `None` at `i64::MAX`: the caller refuses instead of wrapping.
            pub fn next(self) -> Option<Self> {
                self.0.checked_add(1).map(Self)
            }
        }
    };
}

pub mod assignment;
pub mod catalog;
pub mod text;

#[cfg(kani)]
mod proofs;
#[cfg(test)]
mod tests;

pub use assignment::{
    MAX_THEME_REFS, MAX_THEMES_PER_WORK, MetadataDigest, MetadataSnapshot, MetadataVersion,
    SelectionError, TagChange, TagOutcome, TagRefusal, TagSelection, TagWrite, WorkTagFacts,
    WorkTags, decide_tag_change, draft_tags, public_tags,
};
pub use catalog::{
    CatalogChange, CatalogCommand, CatalogFacts, CatalogIntegrity, CatalogOutcome, CatalogRefusal,
    CatalogTag, CatalogVersion, CatalogWrite, MAX_SUGGESTIONS, MatchTier, NameKind, SlugHolder,
    SlugKind, SlugResolution, SlugWrite, SuccessorError, TagAlias, TagFacet, TagFacts, TagMatch,
    TagStatus, decide_catalog_change, rank_matches, resolve_slug,
};
pub use text::{
    DescriptionError, LabelError, MAX_DESCRIPTION_CHARS, MAX_LABEL_CHARS, MAX_QUERY_CHARS,
    MAX_RAW_TEXT_BYTES, QueryError, SearchKey, SlugError, TagDescription, TagLabel, TagQuery,
    TagSlug,
};
