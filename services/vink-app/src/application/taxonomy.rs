//! Author tag use cases (TAG-PR-01, #95): read and save the private tags of the actor's draft.
//!
//! Each save authenticates the actor, validates raw input into a [`TagChange`] (expected
//! metadata version, form + deduplicated themes as tag identities) and hands it to ONE
//! [`WorkTagStore`] transaction, which loads the facts under locks and calls
//! [`crate::domain::taxonomy::decide_tag_change`]. This module only maps the outcome; it restates
//! no catalog, bound or version rule. Saving tags publishes nothing.
//!
//! Retry semantics: every accepted save advances the version, so resending a command whose
//! acknowledgement was lost is a [`TagError::VersionConflict`]. Its `observed` tags tell the
//! client whether its desired state holds now ([`WorkTags::matches`]); that is a statement
//! about state, not a receipt naming the command that produced it. #97 CHOICE-03 A keeps it
//! that way: no command-key receipt, because the author needs the current tags, not the
//! history of which tab's command committed.
//!
//! Public tags (TAG-PR-02, #96) are read through the current publication's metadata binding
//! only ([`Vink::public_work_tags`]); nothing here writes a snapshot or a binding, which belong
//! to the publication owner's transaction.
//!
//! Catalog lookup and curator changes are `application::catalog` (TAG-PR-03, #97). HTTP v1 maps
//! these use cases to `/api/v1/drafts/{work_id}/tags`, a resource of its own with this version
//! (#97 CHOICE-01 B): saving text never carries or clears tags, and a stale tag save is a
//! conflict whose `observed` tags the client reconciles from (#97 CHOICE-03 A).

use super::ports::{PublicTagStore, StoreError, WorkTagStore};
use super::{ReadError, Vink};
use crate::domain::ids::{PrincipalId, RevisionId, TagId, WorkId};
use crate::domain::taxonomy::{
    CatalogTag, MetadataVersion, SelectionError, TagChange, TagFacet, TagOutcome, TagRefusal,
    TagSelection, TagStatus, WorkTags, draft_tags, public_tags,
};

/// Untrusted save input. Only [`Vink::save_work_tags`] turns it into a [`TagChange`]. It is the
/// whole desired state: `form: None` clears the form slot and empty `themes` clears the themes.
/// The HTTP contract requires both fields to be present, so an omitted field is never a clear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveWorkTags {
    pub work_id: WorkId,
    pub expected_version: i64,
    pub form: Option<String>,
    pub themes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedWorkTags {
    pub work_id: WorkId,
    pub version: MetadataVersion,
    pub tags: WorkTags,
    /// `false` for an accepted save that changed nothing (the version still advanced).
    pub changed: bool,
}

/// The author's view of their draft tags: the stored assignment and its catalog rows in display
/// order (form, then themes), retired tags included so they can be kept or removed, and the rows
/// of the successor hints those retired tags name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftWorkTags {
    pub work_id: WorkId,
    pub version: MetadataVersion,
    pub tags: WorkTags,
    pub shown: Vec<CatalogTag>,
    pub successors: Vec<CatalogTag>,
}

impl DraftWorkTags {
    /// The successor row a shown retired tag names, if any.
    pub fn successor_of(&self, tag: &CatalogTag) -> Option<&CatalogTag> {
        match tag.status() {
            TagStatus::Retired {
                successor: Some(id),
            } => self.successors.iter().find(|row| row.id() == id),
            _ => None,
        }
    }
}

/// What a public reader sees: the active tags of the snapshot bound to the current publication,
/// in snapshot order with current catalog labels, and the revision bound in that same row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicWorkTags {
    pub work_id: WorkId,
    pub revision_id: RevisionId,
    pub tags: Vec<CatalogTag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TagInput {
    #[error("expected_version must be >= 1")]
    ExpectedVersion,
    #[error("{0}")]
    Selection(#[from] SelectionError),
}

impl TagInput {
    pub fn field(&self) -> &'static str {
        match self {
            Self::ExpectedVersion => "expected_version",
            Self::Selection(SelectionError::Form) => "form",
            Self::Selection(SelectionError::Theme { .. } | SelectionError::TooManyThemeRefs) => {
                "themes"
            }
        }
    }
}

/// Catalog-dependent refusals the author can act on. Nothing was written. `slot` is where the
/// request put the tag: the form when it was the requested form, otherwise the themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagRejection {
    /// "Chưa có thẻ này": not in the curated catalog; the pilot has no author proposals.
    UnknownTag {
        tag: TagId,
        slot: TagFacet,
    },
    RetiredTag {
        tag: TagId,
        slot: TagFacet,
        successor: Option<TagId>,
    },
    WrongFacet {
        tag: TagId,
        slot: TagFacet,
    },
    TooManyThemes,
}

impl TagRejection {
    /// The request field the rejection is about.
    pub const fn field(&self) -> &'static str {
        match self {
            Self::UnknownTag { slot, .. }
            | Self::RetiredTag { slot, .. }
            | Self::WrongFacet { slot, .. } => match slot {
                TagFacet::Form => "form",
                TagFacet::Theme => "themes",
            },
            Self::TooManyThemes => "themes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TagError {
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("suspended")]
    Suspended,
    /// Missing, or owned by someone else; deliberately indistinguishable.
    #[error("not found")]
    NotFound,
    /// Nothing written; reconcile from `observed` instead of resending with `current`.
    #[error("version conflict; current version is {}", current.get())]
    VersionConflict {
        current: MetadataVersion,
        observed: WorkTags,
    },
    #[error("invalid input: {0}")]
    Invalid(#[from] TagInput),
    #[error("tag selection rejected")]
    Rejected(TagRejection),
    #[error("storage unavailable")]
    Unavailable,
    #[error("internal error")]
    Internal,
}

impl From<StoreError> for TagError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            StoreError::UnknownPrincipal => Self::Unauthenticated,
            StoreError::PrincipalSuspended => Self::Suspended,
            StoreError::Integrity(_) => Self::Internal,
        }
    }
}

fn refused(refusal: TagRefusal, selection: &TagSelection) -> TagError {
    // The core checks the form first, so a tag that is the requested form was refused there.
    let slot = |tag| {
        if selection.form() == Some(tag) {
            TagFacet::Form
        } else {
            TagFacet::Theme
        }
    };
    match refusal {
        TagRefusal::NotFound => TagError::NotFound,
        TagRefusal::Suspended => TagError::Suspended,
        TagRefusal::UnknownTag(tag) => TagError::Rejected(TagRejection::UnknownTag {
            tag,
            slot: slot(tag),
        }),
        TagRefusal::RetiredTag { tag, successor } => TagError::Rejected(TagRejection::RetiredTag {
            tag,
            slot: slot(tag),
            successor,
        }),
        TagRefusal::WrongFacet { tag, slot } => {
            TagError::Rejected(TagRejection::WrongFacet { tag, slot })
        }
        TagRefusal::TooManyThemes => TagError::Rejected(TagRejection::TooManyThemes),
        TagRefusal::InvalidFacts | TagRefusal::VersionExhausted => TagError::Internal,
    }
}

impl<S> Vink<S>
where
    S: WorkTagStore,
{
    pub async fn work_tags(
        &self,
        actor: Option<PrincipalId>,
        work_id: WorkId,
    ) -> Result<DraftWorkTags, TagError> {
        let owner = actor.ok_or(TagError::Unauthenticated)?;
        let stored = self
            .store
            .work_tags(owner, work_id)
            .await?
            .ok_or(TagError::NotFound)?;
        let shown: Vec<CatalogTag> = draft_tags(&stored.tags, &stored.catalog)
            .map_err(|_| TagError::Internal)?
            .into_iter()
            .cloned()
            .collect();
        // Catalog rows are never deleted: a named successor without its row is corruption.
        let mut successors: Vec<CatalogTag> = Vec::new();
        for tag in &shown {
            if let TagStatus::Retired {
                successor: Some(id),
            } = tag.status()
                && !successors.iter().any(|row| row.id() == id)
            {
                let row = stored
                    .catalog
                    .iter()
                    .find(|row| row.id() == id)
                    .ok_or(TagError::Internal)?;
                successors.push(row.clone());
            }
        }
        Ok(DraftWorkTags {
            work_id,
            version: stored.version,
            tags: stored.tags,
            shown,
            successors,
        })
    }

    /// Conditional write of the draft tags: authentication, then input validation, then one
    /// store transaction that decides on current facts. Invalid input never reaches the store.
    pub async fn save_work_tags(
        &self,
        actor: Option<PrincipalId>,
        command: SaveWorkTags,
    ) -> Result<SavedWorkTags, TagError> {
        let owner = actor.ok_or(TagError::Unauthenticated)?;
        let expected =
            MetadataVersion::new(command.expected_version).ok_or(TagInput::ExpectedVersion)?;
        let selection = TagSelection::parse(command.form.as_deref(), &command.themes)
            .map_err(TagInput::from)?;
        let change = TagChange::new(expected, selection.clone());
        match self
            .store
            .save_work_tags(owner, command.work_id, &change)
            .await?
        {
            TagOutcome::Accepted(write) => Ok(SavedWorkTags {
                work_id: command.work_id,
                version: write.version(),
                tags: write.tags().clone(),
                changed: write.changed(),
            }),
            TagOutcome::Conflict { current, observed } => {
                Err(TagError::VersionConflict { current, observed })
            }
            TagOutcome::Refused(refusal) => Err(refused(refusal, &selection)),
        }
    }
}

impl<S> Vink<S>
where
    S: PublicTagStore,
{
    /// Public tags of a work: no actor, no draft. A publication without a bound snapshot shows no
    /// tags; a snapshot whose catalog rows are missing or of the wrong facet is an integrity
    /// failure, never an empty list.
    pub async fn public_work_tags(&self, work_id: WorkId) -> Result<PublicWorkTags, ReadError> {
        let facts = self
            .store
            .public_work_tags(work_id)
            .await?
            .ok_or(ReadError::NotFound)?;
        let tags = match &facts.snapshot {
            None => Vec::new(),
            Some(snapshot) => public_tags(snapshot, &facts.catalog)
                .map_err(|_| ReadError::Internal)?
                .into_iter()
                .cloned()
                .collect(),
        };
        Ok(PublicWorkTags {
            work_id,
            revision_id: facts.revision_id,
            tags,
        })
    }
}
