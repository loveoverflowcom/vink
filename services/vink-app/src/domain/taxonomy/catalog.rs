//! The curated catalog (CHOICE-03 A): operators create, rename, describe, alias and retire
//! tags; authors only choose existing ones. Proposals are not part of the pilot.
//!
//! Slug invariant: once a slug names a tag (as its canonical name, an alias or a former name),
//! it names that tag forever. Renaming keeps the identity and turns the old slug into a former
//! name that still resolves (redirect); an alias always points at an active canonical tag, so
//! chains, cycles and self-aliases cannot be expressed; retiring keeps every reference and
//! offers an optional successor hint without rewriting anything. Merging two canonical tags or
//! rewriting history is not an operation. The database must still enforce slug uniqueness:
//! [`decide_catalog_change`] decides on facts the adapter read under its locks.

use super::text::{SearchKey, TagDescription, TagLabel, TagQuery, TagSlug};
use crate::domain::ids::TagId;

/// The closed set of tag groups (CHOICE-01 B, bounded). A facet is fixed when a tag is created.
/// Adding a facet is a reviewed code change, not catalog data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TagFacet {
    /// Dạng bài: thơ, tản văn, truyện ngắn… At most one per work (the slot is an `Option`).
    Form,
    /// Chủ đề: ký ức, gia đình, đô thị… Up to [`super::MAX_THEMES_PER_WORK`] per work.
    Theme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagStatus {
    Active,
    /// Not offered for new use. `successor` is a hint for authors, never applied automatically.
    Retired {
        successor: Option<TagId>,
    },
}

impl TagStatus {
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }
}

positive_token!(
    /// Optimistic-concurrency version of the whole catalog. Every accepted change, including a
    /// same-state one, advances it; a command carrying an older version is a conflict.
    CatalogVersion
);

/// What assignment needs to know about one tag; the adapter can load it without any text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TagFacts {
    pub id: TagId,
    pub facet: TagFacet,
    pub status: TagStatus,
}

/// One canonical tag row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogTag {
    id: TagId,
    facet: TagFacet,
    label: TagLabel,
    slug: TagSlug,
    description: Option<TagDescription>,
    status: TagStatus,
}

impl CatalogTag {
    /// Storage reconstruction. Fails closed when the stored slug is not the label's slug or a
    /// tag names itself as successor; both are states no accepted change produces.
    pub fn restore(
        id: TagId,
        facet: TagFacet,
        label: TagLabel,
        slug: TagSlug,
        description: Option<TagDescription>,
        status: TagStatus,
    ) -> Option<Self> {
        let self_successor = status
            == TagStatus::Retired {
                successor: Some(id),
            };
        (label.slug() == slug && !self_successor).then_some(Self {
            id,
            facet,
            label,
            slug,
            description,
            status,
        })
    }

    pub const fn id(&self) -> TagId {
        self.id
    }

    pub const fn facet(&self) -> TagFacet {
        self.facet
    }

    pub fn label(&self) -> &TagLabel {
        &self.label
    }

    pub fn slug(&self) -> &TagSlug {
        &self.slug
    }

    pub fn description(&self) -> Option<&TagDescription> {
        self.description.as_ref()
    }

    pub const fn status(&self) -> TagStatus {
        self.status
    }

    pub const fn facts(&self) -> TagFacts {
        TagFacts {
            id: self.id,
            facet: self.facet,
            status: self.status,
        }
    }
}

/// Another spelling that resolves to a canonical tag (`Hồi ức` → `Ký ức`). Curated, never
/// inferred from similar meaning or from a shared search key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagAlias {
    label: TagLabel,
    slug: TagSlug,
    target: TagId,
}

impl TagAlias {
    /// Storage reconstruction; the stored slug must be the label's slug.
    pub fn restore(label: TagLabel, slug: TagSlug, target: TagId) -> Option<Self> {
        (label.slug() == slug).then_some(Self {
            label,
            slug,
            target,
        })
    }

    pub fn label(&self) -> &TagLabel {
        &self.label
    }

    pub fn slug(&self) -> &TagSlug {
        &self.slug
    }

    pub const fn target(&self) -> TagId {
        self.target
    }
}

/// How a slug names its tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlugKind {
    Canonical,
    Alias,
    /// A slug the tag had before a rename; it keeps resolving to the tag.
    Former,
}

/// The current holder of a slug, as the adapter read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlugHolder {
    pub tag: TagId,
    pub kind: SlugKind,
}

/// A slug row to write. `tag` is always the changed tag; `replaces` is `None` for a new row or
/// the kind of the existing row of the same tag. No write moves a slug to another tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlugWrite {
    pub slug: TagSlug,
    pub tag: TagId,
    pub kind: NameKind,
    pub replaces: Option<SlugKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameKind {
    Canonical,
    Alias(TagLabel),
    Former,
}

impl NameKind {
    pub const fn slug_kind(&self) -> SlugKind {
        match self {
            Self::Canonical => SlugKind::Canonical,
            Self::Alias(_) => SlugKind::Alias,
            Self::Former => SlugKind::Former,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogChange {
    /// `id` comes from [`TagId::generate`] on the server.
    Create {
        id: TagId,
        facet: TagFacet,
        label: TagLabel,
        description: Option<TagDescription>,
    },
    /// New display label; the slug follows the label and the old slug becomes a former name.
    Rename {
        id: TagId,
        label: TagLabel,
    },
    Describe {
        id: TagId,
        description: Option<TagDescription>,
    },
    AddAlias {
        target: TagId,
        label: TagLabel,
    },
    Retire {
        id: TagId,
        successor: Option<TagId>,
    },
}

impl CatalogChange {
    /// The tag whose row the adapter loads as [`CatalogFacts::subject`].
    pub const fn subject(&self) -> TagId {
        match self {
            Self::Create { id, .. }
            | Self::Rename { id, .. }
            | Self::Describe { id, .. }
            | Self::Retire { id, .. } => *id,
            Self::AddAlias { target, .. } => *target,
        }
    }

    /// The slug whose holder the adapter loads as [`CatalogFacts::slug_holder`], if any.
    pub fn claimed_slug(&self) -> Option<TagSlug> {
        match self {
            Self::Create { label, .. }
            | Self::Rename { label, .. }
            | Self::AddAlias { label, .. } => Some(label.slug()),
            Self::Describe { .. } | Self::Retire { .. } => None,
        }
    }

    /// The successor whose facts the adapter loads as [`CatalogFacts::successor`], if any.
    pub const fn successor(&self) -> Option<TagId> {
        match self {
            Self::Retire { successor, .. } => *successor,
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogCommand {
    pub expected: CatalogVersion,
    pub change: CatalogChange,
}

/// Server-owned facts, read in the change's transaction. The decision checks that they belong
/// to the command; it cannot check that they are current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFacts {
    /// The actor's current curator authority (an operator entrypoint or a grant, chosen by #97).
    pub can_curate: bool,
    pub version: CatalogVersion,
    /// Row of [`CatalogChange::subject`].
    pub subject: Option<CatalogTag>,
    /// Holder of [`CatalogChange::claimed_slug`]; `None` for changes that claim no slug.
    pub slug_holder: Option<SlugHolder>,
    /// Facts of [`CatalogChange::successor`]; `None` when no successor is named.
    pub successor: Option<TagFacts>,
}

/// An accepted change: the next catalog version, the subject's new row if it changed and the
/// slug rows to write. All of it commits together or not at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogWrite {
    version: CatalogVersion,
    tag: Option<CatalogTag>,
    slugs: Vec<SlugWrite>,
}

impl CatalogWrite {
    pub const fn version(&self) -> CatalogVersion {
        self.version
    }

    pub fn tag(&self) -> Option<&CatalogTag> {
        self.tag.as_ref()
    }

    pub fn slugs(&self) -> &[SlugWrite] {
        &self.slugs
    }

    /// `false` for a same-state change, which still advances the version.
    pub fn changed(&self) -> bool {
        self.tag.is_some() || !self.slugs.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuccessorError {
    SelfReference,
    Unknown,
    Retired,
    OtherFacet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogRefusal {
    NotCurator,
    UnknownTag,
    TagIdTaken,
    /// The slug already names `holder` (canonically, as an alias or as a former name).
    SlugTaken {
        holder: TagId,
    },
    /// Retired tags accept no rename, description, alias or second retirement in the pilot.
    Retired,
    /// The alias would spell the target's own canonical name.
    AliasIsCanonicalName,
    InvalidSuccessor(SuccessorError),
    /// The facts do not belong to the command. Fail closed; not a curator-facing error.
    InvalidFacts,
    VersionExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogOutcome {
    Accepted(CatalogWrite),
    /// Nothing written; re-read the catalog instead of resubmitting with a refreshed version.
    Conflict {
        current: CatalogVersion,
    },
    Refused(CatalogRefusal),
}

/// The sole catalog transition. Typed facts do not prove where they came from: the adapter
/// loads them under the change's locks and enforces slug uniqueness itself.
pub fn decide_catalog_change(command: &CatalogCommand, facts: &CatalogFacts) -> CatalogOutcome {
    use CatalogOutcome::{Accepted, Conflict, Refused};
    use CatalogRefusal::*;
    if !facts.can_curate {
        return Refused(NotCurator);
    }
    let change = &command.change;
    let foreign_subject = facts
        .subject
        .as_ref()
        .is_some_and(|tag| tag.id != change.subject());
    let foreign_successor = match (change.successor(), facts.successor) {
        (Some(named), Some(found)) => found.id != named,
        (None, Some(_)) => true,
        _ => false,
    };
    let stray_holder = change.claimed_slug().is_none() && facts.slug_holder.is_some();
    if foreign_subject || foreign_successor || stray_holder {
        return Refused(InvalidFacts);
    }
    if command.expected != facts.version {
        return Conflict {
            current: facts.version,
        };
    }
    // Decided before the change's own rules (as for draft tags), so no refusal path drops a
    // half-built write.
    let Some(version) = facts.version.next() else {
        return Refused(VersionExhausted);
    };
    let effect = match change {
        CatalogChange::Create {
            id,
            facet,
            label,
            description,
        } => create(*id, *facet, label, description, facts),
        CatalogChange::Rename { label, .. } => {
            active(facts).and_then(|tag| rename(tag, label, facts))
        }
        CatalogChange::Describe { description, .. } => active(facts).map(|tag| {
            let changed = tag.description != *description;
            let next = changed.then(|| CatalogTag {
                description: description.clone(),
                ..tag.clone()
            });
            (next, Vec::new())
        }),
        CatalogChange::AddAlias { label, .. } => {
            active(facts).and_then(|tag| alias(tag, label, facts))
        }
        CatalogChange::Retire { successor, .. } => {
            active(facts).and_then(|tag| retire(tag, *successor, facts))
        }
    };
    match effect {
        Ok((tag, slugs)) => Accepted(CatalogWrite {
            version,
            tag,
            slugs,
        }),
        Err(refusal) => Refused(refusal),
    }
}

type Effect = Result<(Option<CatalogTag>, Vec<SlugWrite>), CatalogRefusal>;

fn active(facts: &CatalogFacts) -> Result<&CatalogTag, CatalogRefusal> {
    match &facts.subject {
        None => Err(CatalogRefusal::UnknownTag),
        Some(tag) if !tag.status.is_active() => Err(CatalogRefusal::Retired),
        Some(tag) => Ok(tag),
    }
}

fn create(
    id: TagId,
    facet: TagFacet,
    label: &TagLabel,
    description: &Option<TagDescription>,
    facts: &CatalogFacts,
) -> Effect {
    if facts.subject.is_some() {
        return Err(CatalogRefusal::TagIdTaken);
    }
    match facts.slug_holder {
        // A slug held by a tag that does not exist contradicts the facts.
        Some(holder) if holder.tag == id => return Err(CatalogRefusal::InvalidFacts),
        Some(holder) => return Err(CatalogRefusal::SlugTaken { holder: holder.tag }),
        None => {}
    }
    let slug = label.slug();
    let tag = CatalogTag {
        id,
        facet,
        label: label.clone(),
        slug: slug.clone(),
        description: description.clone(),
        status: TagStatus::Active,
    };
    let write = SlugWrite {
        slug,
        tag: id,
        kind: NameKind::Canonical,
        replaces: None,
    };
    Ok((Some(tag), vec![write]))
}

fn rename(tag: &CatalogTag, label: &TagLabel, facts: &CatalogFacts) -> Effect {
    let slug = label.slug();
    let own = |kind| facts.slug_holder == Some(SlugHolder { tag: tag.id, kind });
    let next = CatalogTag {
        label: label.clone(),
        slug: slug.clone(),
        ..tag.clone()
    };
    if slug == tag.slug {
        // Same name, different spelling (case, spacing, punctuation): only the label changes.
        if !own(SlugKind::Canonical) {
            return Err(CatalogRefusal::InvalidFacts);
        }
        let changed = tag.label != *label;
        return Ok((changed.then_some(next), Vec::new()));
    }
    let claim = match facts.slug_holder {
        None => None,
        Some(holder) if holder.tag != tag.id => {
            return Err(CatalogRefusal::SlugTaken { holder: holder.tag });
        }
        // The tag has exactly one canonical slug, and it is not this one.
        Some(SlugHolder {
            kind: SlugKind::Canonical,
            ..
        }) => return Err(CatalogRefusal::InvalidFacts),
        // Taking back one of the tag's own former names or aliases keeps the slug invariant.
        Some(holder) => Some(holder.kind),
    };
    let writes = vec![
        SlugWrite {
            slug,
            tag: tag.id,
            kind: NameKind::Canonical,
            replaces: claim,
        },
        SlugWrite {
            slug: tag.slug.clone(),
            tag: tag.id,
            kind: NameKind::Former,
            replaces: Some(SlugKind::Canonical),
        },
    ];
    Ok((Some(next), writes))
}

fn alias(tag: &CatalogTag, label: &TagLabel, facts: &CatalogFacts) -> Effect {
    let slug = label.slug();
    let own = |kind| facts.slug_holder == Some(SlugHolder { tag: tag.id, kind });
    if slug == tag.slug {
        return if own(SlugKind::Canonical) {
            Err(CatalogRefusal::AliasIsCanonicalName)
        } else {
            Err(CatalogRefusal::InvalidFacts)
        };
    }
    let replaces = match facts.slug_holder {
        None => None,
        Some(holder) if holder.tag != tag.id => {
            return Err(CatalogRefusal::SlugTaken { holder: holder.tag });
        }
        Some(SlugHolder {
            kind: SlugKind::Canonical,
            ..
        }) => return Err(CatalogRefusal::InvalidFacts),
        // Re-spelling an existing alias or reviving a former name of the same tag.
        Some(holder) => Some(holder.kind),
    };
    let write = SlugWrite {
        slug,
        tag: tag.id,
        kind: NameKind::Alias(label.clone()),
        replaces,
    };
    Ok((None, vec![write]))
}

fn retire(tag: &CatalogTag, successor: Option<TagId>, facts: &CatalogFacts) -> Effect {
    if let Some(named) = successor {
        let invalid = |e| Err(CatalogRefusal::InvalidSuccessor(e));
        if named == tag.id {
            return invalid(SuccessorError::SelfReference);
        }
        match facts.successor {
            None => return invalid(SuccessorError::Unknown),
            Some(found) if !found.status.is_active() => return invalid(SuccessorError::Retired),
            Some(found) if found.facet != tag.facet => return invalid(SuccessorError::OtherFacet),
            Some(_) => {}
        }
    }
    let next = CatalogTag {
        status: TagStatus::Retired { successor },
        ..tag.clone()
    };
    Ok((Some(next), Vec::new()))
}

/// Integrity failure: facts that no accepted catalog change produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("catalog facts are inconsistent")]
pub struct CatalogIntegrity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlugResolution<'a> {
    Canonical(&'a CatalogTag),
    /// An alias or former slug of an active tag: answer with the tag's canonical slug.
    Redirect(&'a CatalogTag),
    /// Any slug of a retired tag. Nothing new is filed under it; the successor is a hint.
    Retired(&'a CatalogTag),
    NotFound,
}

/// What a tag URL means. `holder` is the row of `slug`; `tag` is the tag it names.
pub fn resolve_slug<'a>(
    slug: &TagSlug,
    holder: Option<SlugHolder>,
    tag: Option<&'a CatalogTag>,
) -> Result<SlugResolution<'a>, CatalogIntegrity> {
    let (holder, tag) = match (holder, tag) {
        (None, None) => return Ok(SlugResolution::NotFound),
        (Some(holder), Some(tag)) if holder.tag == tag.id => (holder, tag),
        _ => return Err(CatalogIntegrity),
    };
    let canonical = tag.slug == *slug;
    if canonical != (holder.kind == SlugKind::Canonical) {
        return Err(CatalogIntegrity);
    }
    Ok(match (tag.status, holder.kind) {
        (TagStatus::Retired { .. }, _) => SlugResolution::Retired(tag),
        (TagStatus::Active, SlugKind::Canonical) => SlugResolution::Canonical(tag),
        (TagStatus::Active, _) => SlugResolution::Redirect(tag),
    })
}

/// Most suggestions one lookup returns.
pub const MAX_SUGGESTIONS: usize = 10;

/// How a name matched a query, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchTier {
    /// Same slug: same letters and marks, any case or NFC/NFD spelling.
    Exact,
    /// Same accent-folded key (`ma` for `má`); a candidate, not the same name.
    Folded,
    /// The query starts the folded key or one of its words.
    Prefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TagMatch<'a> {
    pub tag: &'a CatalogTag,
    pub tier: MatchTier,
    /// Set when the best match was an alias spelling; the suggestion is still the canonical tag.
    pub via_alias: Option<&'a TagAlias>,
}

fn tier(query: &TagQuery, slug: &TagSlug, key: &SearchKey) -> Option<MatchTier> {
    if query.exact() == Some(slug) {
        Some(MatchTier::Exact)
    } else if key == query.key() {
        Some(MatchTier::Folded)
    } else if key.has_word_prefix(query.key()) {
        Some(MatchTier::Prefix)
    } else {
        None
    }
}

/// Ranks candidate tags and aliases (loaded by the adapter, e.g. by search-key prefix) for a
/// lookup. Active tags only; one suggestion per canonical tag; deterministic order: tier, label
/// before alias, folded label, slug, identity. Never fewer distinctions than the slug makes.
pub fn rank_matches<'a>(
    query: &TagQuery,
    tags: &'a [CatalogTag],
    aliases: &'a [TagAlias],
    limit: usize,
) -> Vec<TagMatch<'a>> {
    let mut found: Vec<TagMatch<'a>> = Vec::new();
    let mut offer =
        |candidate: TagMatch<'a>| match found.iter_mut().find(|m| m.tag.id == candidate.tag.id) {
            Some(best)
                if (candidate.tier, candidate.via_alias.is_some())
                    < (best.tier, best.via_alias.is_some()) =>
            {
                *best = candidate;
            }
            Some(_) => {}
            None => found.push(candidate),
        };
    for tag in tags.iter().filter(|t| t.status.is_active()) {
        if let Some(tier) = tier(query, &tag.slug, &tag.label.search_key()) {
            offer(TagMatch {
                tag,
                tier,
                via_alias: None,
            });
        }
    }
    for alias in aliases {
        let Some(tag) = tags
            .iter()
            .find(|t| t.id == alias.target && t.status.is_active())
        else {
            continue;
        };
        if let Some(tier) = tier(query, &alias.slug, &alias.label.search_key()) {
            offer(TagMatch {
                tag,
                tier,
                via_alias: Some(alias),
            });
        }
    }
    // Keys are computed once per suggestion, not once per comparison.
    found.sort_by_cached_key(|m| {
        (
            m.tier,
            m.via_alias.is_some(),
            m.tag.label.search_key(),
            m.tag.slug.clone(),
            m.tag.id,
        )
    });
    found.truncate(limit.min(MAX_SUGGESTIONS));
    found
}
