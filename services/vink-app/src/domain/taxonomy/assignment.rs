//! An author's tags for a work: the private draft assignment and its decision, the immutable
//! metadata snapshot a publication binds, and the public projection (CHOICE-02 B).
//!
//! Draft tags are private working state with their own [`MetadataVersion`]. Saving them never
//! touches a revision, its `ContentHash` v1, the publication pointer or the endorsement anchor.
//! A publication binds a content revision AND a [`MetadataSnapshot`]; only the publication
//! owner's transaction may move that pair, and public reads project only the bound snapshot.

use sha2::{Digest, Sha256};

use super::catalog::{CatalogIntegrity, CatalogTag, TagFacet, TagFacts, TagStatus};
use crate::domain::ids::{PrincipalId, TagId};
use crate::domain::principal::Standing;

/// Most distinct theme tags on one work, after alias resolution and deduplication. A tag says
/// what a work is mainly about; three covers "Ký ức / Gia đình" plus one more while keeping each
/// filter meaningful. Provisional (ADR §CHOICE-01): revisit with pilot data, not by copying the
/// five of other platforms. The form slot holds at most one tag by type.
pub const MAX_THEMES_PER_WORK: usize = 3;
/// Most raw theme references one request may carry, before deduplication: every allowed theme
/// twice (once picked through an alias, once directly) is still a choice; more is malformed.
pub const MAX_THEME_REFS: usize = 2 * MAX_THEMES_PER_WORK;

positive_token!(
    /// Optimistic-concurrency version of a work's draft metadata. Starts at 1 with no tags.
    /// Every accepted save advances it, including one that changes nothing, so an older command
    /// carrying the same expected version can never land after a newer one.
    MetadataVersion
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    #[error("more than {MAX_THEME_REFS} theme references")]
    TooManyThemeRefs,
    #[error("form is not a tag id")]
    Form,
    #[error("theme {index} is not a tag id")]
    Theme { index: usize },
}

/// A syntactically valid choice: tag identities only, themes deduplicated in first-seen order.
/// Not yet checked against the catalog; only [`decide_tag_change`] turns it into [`WorkTags`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagSelection {
    form: Option<TagId>,
    themes: Vec<TagId>,
}

impl TagSelection {
    pub fn new(form: Option<TagId>, themes: &[TagId]) -> Result<Self, SelectionError> {
        if themes.len() > MAX_THEME_REFS {
            return Err(SelectionError::TooManyThemeRefs);
        }
        // Keep an identity at its first position: compare with the earlier input, not with the
        // output, so every loop is bounded by the (already bounded) input length.
        let mut unique = Vec::with_capacity(themes.len());
        for (i, id) in themes.iter().enumerate() {
            if !themes[..i].contains(id) {
                unique.push(*id);
            }
        }
        Ok(Self {
            form,
            themes: unique,
        })
    }

    /// Parses raw identities. The reference count is bounded before any element is parsed.
    pub fn parse(form: Option<&str>, themes: &[String]) -> Result<Self, SelectionError> {
        if themes.len() > MAX_THEME_REFS {
            return Err(SelectionError::TooManyThemeRefs);
        }
        let form = form
            .map(|raw| TagId::parse(raw).ok_or(SelectionError::Form))
            .transpose()?;
        let mut ids = Vec::with_capacity(themes.len());
        for (index, raw) in themes.iter().enumerate() {
            ids.push(TagId::parse(raw).ok_or(SelectionError::Theme { index })?);
        }
        Self::new(form, &ids)
    }

    pub const fn form(&self) -> Option<TagId> {
        self.form
    }

    pub fn themes(&self) -> &[TagId] {
        &self.themes
    }
}

/// A work's tags as the catalog accepted them: at most one form, at most
/// [`MAX_THEMES_PER_WORK`] distinct themes in the author's order, no identity in both slots.
/// ```compile_fail
/// use vink_app::domain::taxonomy::WorkTags;
/// let tags = WorkTags { form: None, themes: Vec::new() };
/// ```
/// Lawful twin: the same empty value comes from the constant or the checked restore.
/// ```
/// use vink_app::domain::taxonomy::WorkTags;
/// let tags: WorkTags = WorkTags::restore(None, Vec::new()).unwrap();
/// assert_eq!(tags, WorkTags::EMPTY);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkTags {
    form: Option<TagId>,
    themes: Vec<TagId>,
}

impl WorkTags {
    pub const EMPTY: Self = Self {
        form: None,
        themes: Vec::new(),
    };

    /// Storage reconstruction: structure only (bounds, distinctness). Facets were checked when
    /// the tags were accepted and cannot change afterwards.
    pub fn restore(form: Option<TagId>, themes: Vec<TagId>) -> Option<Self> {
        let distinct = themes
            .iter()
            .enumerate()
            .all(|(i, id)| !themes[..i].contains(id) && Some(*id) != form);
        (themes.len() <= MAX_THEMES_PER_WORK && distinct).then_some(Self { form, themes })
    }

    pub const fn form(&self) -> Option<TagId> {
        self.form
    }

    pub fn themes(&self) -> &[TagId] {
        &self.themes
    }

    /// Form first, then themes in order.
    pub fn ids(&self) -> impl Iterator<Item = TagId> + '_ {
        self.form.into_iter().chain(self.themes.iter().copied())
    }

    pub fn contains(&self, id: TagId) -> bool {
        self.form == Some(id) || self.themes.contains(&id)
    }

    /// Whether these tags are exactly the desired selection. After a lost acknowledgement this
    /// says the desired state holds now; it does not prove which command produced it.
    pub fn matches(&self, desired: &TagSelection) -> bool {
        self.form == desired.form && self.themes == desired.themes
    }
}

/// A validated save: the version it replaces and the desired tags. No `Deserialize`.
/// ```compile_fail
/// use vink_app::domain::taxonomy::{TagChange, TagSelection};
/// let change = TagChange::new(1, TagSelection::new(None, &[]).unwrap());
/// ```
/// Lawful twin: the expected version must be a [`MetadataVersion`].
/// ```
/// use vink_app::domain::taxonomy::{MetadataVersion, TagChange, TagSelection};
/// let change = TagChange::new(MetadataVersion::FIRST, TagSelection::new(None, &[]).unwrap());
/// assert_eq!(change.expected(), MetadataVersion::FIRST);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagChange {
    expected: MetadataVersion,
    selection: TagSelection,
}

impl TagChange {
    pub const fn new(expected: MetadataVersion, selection: TagSelection) -> Self {
        Self {
            expected,
            selection,
        }
    }

    pub const fn expected(&self) -> MetadataVersion {
        self.expected
    }

    pub fn selection(&self) -> &TagSelection {
        &self.selection
    }
}

/// Server-owned facts read in the save's transaction: the work's owner, the actor's standing,
/// the current draft metadata and the catalog facts of the selected tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkTagFacts {
    pub owner: PrincipalId,
    pub standing: Standing,
    pub version: MetadataVersion,
    pub current: WorkTags,
    /// Facts of the selected tags that exist; a missing one is unknown. Duplicates are invalid.
    pub catalog: Vec<TagFacts>,
}

/// An accepted save: write `version` and `tags` to the draft metadata, nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagWrite {
    version: MetadataVersion,
    tags: WorkTags,
    changed: bool,
}

impl TagWrite {
    pub const fn version(&self) -> MetadataVersion {
        self.version
    }

    pub fn tags(&self) -> &WorkTags {
        &self.tags
    }

    /// `false` when the tags equal the current ones; the version still advances.
    pub const fn changed(&self) -> bool {
        self.changed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagRefusal {
    /// No such work, or not the actor's: deliberately indistinguishable.
    NotFound,
    Suspended,
    /// Not in the catalog ("chưa có thẻ này"). The pilot has no author proposals.
    UnknownTag(TagId),
    /// Retired and not already on the draft; `successor` is the curator's hint.
    RetiredTag {
        tag: TagId,
        successor: Option<TagId>,
    },
    /// The tag belongs to the other slot.
    WrongFacet {
        tag: TagId,
        slot: TagFacet,
    },
    TooManyThemes,
    /// The facts do not belong to the command. Fail closed; not an author-facing error.
    InvalidFacts,
    VersionExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagOutcome {
    Accepted(TagWrite),
    /// Nothing written. `observed` lets the client reconcile; never refresh the expected
    /// version and resubmit the old command blindly.
    Conflict {
        current: MetadataVersion,
        observed: WorkTags,
    },
    Refused(TagRefusal),
}

/// The sole draft-metadata transition. The adapter loads `facts` under its locks (work row,
/// metadata row, selected catalog rows) and writes exactly an accepted [`TagWrite`].
///
/// Precedence: missing/foreign work → suspension → stale version → exhausted version → theme
/// bound → per-tag validity (form, then themes in order).
/// A stale tab therefore always learns about the conflict before validation against a catalog
/// that may have changed since it rendered.
pub fn decide_tag_change(
    actor: PrincipalId,
    change: &TagChange,
    facts: Option<&WorkTagFacts>,
) -> TagOutcome {
    use TagOutcome::{Accepted, Conflict, Refused};
    let Some(facts) = facts else {
        return Refused(TagRefusal::NotFound);
    };
    if facts.owner != actor {
        return Refused(TagRefusal::NotFound);
    }
    if facts.standing == Standing::Suspended {
        return Refused(TagRefusal::Suspended);
    }
    let duplicate_facts = facts
        .catalog
        .iter()
        .enumerate()
        .any(|(i, t)| facts.catalog[..i].iter().any(|u| u.id == t.id));
    if duplicate_facts {
        return Refused(TagRefusal::InvalidFacts);
    }
    if change.expected != facts.version {
        return Conflict {
            current: facts.version,
            observed: facts.current.clone(),
        };
    }
    let Some(version) = facts.version.next() else {
        return Refused(TagRefusal::VersionExhausted);
    };
    let selection = &change.selection;
    if selection.themes.len() > MAX_THEMES_PER_WORK {
        return Refused(TagRefusal::TooManyThemes);
    }
    let admit = |id: TagId, slot: TagFacet| -> Result<(), TagRefusal> {
        let Some(tag) = facts.catalog.iter().find(|t| t.id == id) else {
            return Err(TagRefusal::UnknownTag(id));
        };
        if tag.facet != slot {
            return Err(TagRefusal::WrongFacet { tag: id, slot });
        }
        // A retired tag may stay where it already is (no silent rewrite), never be added.
        match tag.status {
            TagStatus::Retired { successor } if !facts.current.contains(id) => {
                Err(TagRefusal::RetiredTag { tag: id, successor })
            }
            _ => Ok(()),
        }
    };
    // Form first, then themes in order; the first inadmissible tag is reported.
    if let Some(Err(refusal)) = selection.form.map(|id| admit(id, TagFacet::Form)) {
        return Refused(refusal);
    }
    for id in &selection.themes {
        if let Err(refusal) = admit(*id, TagFacet::Theme) {
            return Refused(refusal);
        }
    }
    let tags = WorkTags {
        form: selection.form,
        themes: selection.themes.clone(),
    };
    let changed = tags != facts.current;
    Accepted(TagWrite {
        version,
        tags,
        changed,
    })
}

/// Content identity of a metadata snapshot, separate from `ContentHash` v1 (which stays title +
/// document only). SHA-256 over `vink-work-metadata-v1`, then the form as `0x00` or `0x01` +
/// 16 UUID bytes, then the theme count as u64 big-endian and each theme's 16 UUID bytes.
/// Order is significant: the author's theme order is part of what was published.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MetadataDigest(String);

impl MetadataDigest {
    pub fn of(tags: &WorkTags) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"vink-work-metadata-v1");
        match tags.form {
            None => hasher.update([0u8]),
            Some(form) => {
                hasher.update([1u8]);
                hasher.update(form.as_uuid().as_bytes());
            }
        }
        hasher.update((tags.themes.len() as u64).to_be_bytes());
        for theme in &tags.themes {
            hasher.update(theme.as_uuid().as_bytes());
        }
        let mut hex = String::with_capacity(64);
        for byte in hasher.finalize().iter() {
            hex.push_str(&format!("{byte:02x}"));
        }
        Self(hex)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Immutable tags captured for a publication. A later draft edit makes a new snapshot; it never
/// changes this one or the revision it is published with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataSnapshot {
    tags: WorkTags,
    digest: MetadataDigest,
}

impl MetadataSnapshot {
    pub fn capture(tags: WorkTags) -> Self {
        let digest = MetadataDigest::of(&tags);
        Self { tags, digest }
    }

    /// Storage reconstruction; fails closed when the stored digest does not match the tags.
    pub fn restore(tags: WorkTags, digest: &str) -> Option<Self> {
        let snapshot = Self::capture(tags);
        (snapshot.digest.as_str() == digest).then_some(snapshot)
    }

    pub fn tags(&self) -> &WorkTags {
        &self.tags
    }

    pub fn digest(&self) -> &MetadataDigest {
        &self.digest
    }
}

fn project<'a>(
    tags: &WorkTags,
    catalog: &'a [CatalogTag],
    include_retired: bool,
) -> Result<Vec<&'a CatalogTag>, CatalogIntegrity> {
    let slots = tags
        .form
        .into_iter()
        .map(|id| (id, TagFacet::Form))
        .chain(tags.themes.iter().map(|id| (*id, TagFacet::Theme)));
    let mut shown = Vec::with_capacity(tags.themes.len() + 1);
    for (id, slot) in slots {
        // Catalog rows are never deleted and facets never change: anything else is corruption.
        let tag = catalog
            .iter()
            .find(|t| t.id() == id && t.facet() == slot)
            .ok_or(CatalogIntegrity)?;
        if include_retired || tag.status().is_active() {
            shown.push(tag);
        }
    }
    Ok(shown)
}

/// Tags a public reader sees for the bound snapshot: active tags in snapshot order, with their
/// current label (a rename shows everywhere, because snapshots hold identities). Retired tags
/// stay in the snapshot but are not shown or linked.
pub fn public_tags<'a>(
    snapshot: &MetadataSnapshot,
    catalog: &'a [CatalogTag],
) -> Result<Vec<&'a CatalogTag>, CatalogIntegrity> {
    project(&snapshot.tags, catalog, false)
}

/// Tags the author sees on their draft, retired ones included so they can be kept or removed.
pub fn draft_tags<'a>(
    tags: &WorkTags,
    catalog: &'a [CatalogTag],
) -> Result<Vec<&'a CatalogTag>, CatalogIntegrity> {
    project(tags, catalog, true)
}
