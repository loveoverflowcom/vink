//! One private membership per (principal, work). The adapter supplies facts from the
//! same transaction: current standing, public access, active membership count and server time.
//! Clearing membership retains the accepted-intent fence (SAV-INV-04).

use std::cmp::Ordering;
use time::OffsetDateTime;

use super::ids::{PrincipalId, RevisionId, WorkId};
use super::principal::Standing;
use super::publication::WorkAccessPlan;

pub const MAX_SAVED_WORKS: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SavedGeneration(i64);

impl SavedGeneration {
    pub const FIRST: Self = Self(1);

    pub fn new(raw: i64) -> Option<Self> {
        (raw > 0).then_some(Self(raw))
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedWorkKey {
    pub principal: PrincipalId,
    pub work: WorkId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveIntent {
    Saved,
    NotSaved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetSaved {
    /// Principal resolved from the session; work selected by the command.
    pub key: SavedWorkKey,
    pub desired: SaveIntent,
    pub expected_generation: SavedGeneration,
}

/// Access to this work at the time of the command or list read. A missing work is Gone.
/// A read adapter can supply these facts without loading the manuscript.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedWorkAccess {
    Public { work: WorkId, revision: RevisionId },
    Hidden,
    AuthorDeleted,
    Gone,
}

impl From<&WorkAccessPlan<'_>> for SavedWorkAccess {
    fn from(plan: &WorkAccessPlan<'_>) -> Self {
        match plan {
            WorkAccessPlan::PublicBody(snapshot) => Self::Public {
                work: snapshot.work,
                revision: snapshot.revision,
            },
            WorkAccessPlan::AuthorHidden { .. } => Self::Hidden,
            WorkAccessPlan::AuthorTombstone { .. } => Self::AuthorDeleted,
            WorkAccessPlan::ReviewWithdrawn { .. } | WorkAccessPlan::Unavailable => Self::Gone,
        }
    }
}

/// Contains no cached public metadata. Only active membership carries a save time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedWork {
    key: SavedWorkKey,
    generation: SavedGeneration,
    saved_at: Option<OffsetDateTime>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedChange {
    /// Membership changed. Persist the new state.
    Applied(SavedWork),
    /// New same-state intent. Persist the advanced fence, including on an absent membership.
    Reaffirmed(SavedWork),
    /// Old intent already satisfied. Return current state without writing.
    AlreadyCurrent(SavedWork),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveError {
    WrongKey,
    Suspended,
    NotReadable,
    Stale { current: SavedGeneration },
    LimitReached,
    GenerationExhausted,
}

/// Only Readable permits the adapter to join title, form and author from that public
/// revision in the current read snapshot. Neutral rows expose just enough to clear them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedRow {
    Readable {
        work: WorkId,
        generation: SavedGeneration,
        revision: RevisionId,
        saved_at: OffsetDateTime,
    },
    Hidden {
        work: WorkId,
        generation: SavedGeneration,
    },
    AuthorDeleted {
        work: WorkId,
        generation: SavedGeneration,
    },
}

impl SavedWork {
    pub const fn initial(key: SavedWorkKey) -> Self {
        Self {
            key,
            generation: SavedGeneration::FIRST,
            saved_at: None,
        }
    }

    /// Generation one is reserved for a pair that has never accepted a command.
    pub fn restore(
        key: SavedWorkKey,
        generation: SavedGeneration,
        saved_at: Option<OffsetDateTime>,
    ) -> Option<Self> {
        (generation != SavedGeneration::FIRST || saved_at.is_none()).then_some(Self {
            key,
            generation,
            saved_at,
        })
    }

    pub const fn key(self) -> SavedWorkKey {
        self.key
    }
    pub const fn generation(self) -> SavedGeneration {
        self.generation
    }
    pub const fn saved_at(self) -> Option<OffsetDateTime> {
        self.saved_at
    }

    /// active_count counts this principal's Saved memberships, including omitted rows.
    /// The caller serializes that count across works when admitting a new membership.
    pub fn decide(
        self,
        command: SetSaved,
        standing: Standing,
        access: SavedWorkAccess,
        active_count: usize,
        now: OffsetDateTime,
    ) -> Result<SavedChange, SaveError> {
        if command.key != self.key {
            return Err(SaveError::WrongKey);
        }
        if standing != Standing::Active {
            return Err(SaveError::Suspended);
        }
        let wants_saved = command.desired == SaveIntent::Saved;
        if wants_saved
            && !matches!(access, SavedWorkAccess::Public { work, .. } if work == self.key.work)
        {
            return Err(SaveError::NotReadable);
        }
        let already_current = wants_saved == self.saved_at.is_some();
        match command.expected_generation.cmp(&self.generation) {
            Ordering::Less if already_current => return Ok(SavedChange::AlreadyCurrent(self)),
            Ordering::Less | Ordering::Greater => {
                return Err(SaveError::Stale {
                    current: self.generation,
                });
            }
            Ordering::Equal => {}
        }
        if wants_saved && !already_current && active_count >= MAX_SAVED_WORKS {
            return Err(SaveError::LimitReached);
        }
        let generation = self
            .generation
            .0
            .checked_add(1)
            .map(SavedGeneration)
            .ok_or(SaveError::GenerationExhausted)?;
        let next = Self { generation, ..self };
        if already_current {
            return Ok(SavedChange::Reaffirmed(next));
        }
        Ok(SavedChange::Applied(Self {
            saved_at: wants_saved.then_some(now),
            ..next
        }))
    }

    /// None omits the row, without changing membership, saved_at, or the fence.
    pub fn project(
        self,
        viewer: PrincipalId,
        standing: Standing,
        access: SavedWorkAccess,
    ) -> Option<SavedRow> {
        if viewer != self.key.principal || standing != Standing::Active {
            return None;
        }
        let saved_at = self.saved_at?;
        let work = self.key.work;
        let generation = self.generation;
        match access {
            SavedWorkAccess::Public {
                work: public_work,
                revision,
            } if public_work == work => Some(SavedRow::Readable {
                work,
                generation,
                revision,
                saved_at,
            }),
            SavedWorkAccess::Hidden => Some(SavedRow::Hidden { work, generation }),
            SavedWorkAccess::AuthorDeleted => Some(SavedRow::AuthorDeleted { work, generation }),
            SavedWorkAccess::Public { .. } | SavedWorkAccess::Gone => None,
        }
    }
}

#[cfg(test)]
mod tests;
