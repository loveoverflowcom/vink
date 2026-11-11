//! Publication intent core (#150). No SQL, transport, evaluator or authority adapter.
//!
//! Author commands return a new state and an effect plan, never mutate their inputs. A saved
//! snapshot has no evaluation effect. Promotion checks the *current* exact intent; reading an
//! existing pointer deliberately does not (r1 remains readable while r2 waits).
//!
//! `record_review` accepts an already validated, revision-bound policy decision, NOT a provider
//! result or proof of authority. #151 must validate job/policy/decision versions and admin
//! precedence before calling it. Future storage must reload these facts and principal standing
//! at commit, atomically bind revision + metadata, and commit audit/outbox with the pointer.
//!
//! Baselines chosen for this slice, not new product decisions: start private at generation 1;
//! every accepted audience command (including reaffirmation) increments it; inactive lifecycle
//! facts cannot be restored to active through any command. Restoration checks consistency, not
//! the authenticity of the storage adapter's evidence.

use super::ids::{PrincipalId, RevisionId, WorkId};
use super::revision::{ContentHash, RevisionContent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Private,
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Active,
    AuthorDeleted,
    Removed,
}

/// Facts supplied by the moderation owner, scoped to one immutable revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moderation {
    Unsubmitted,
    Pending,
    Approved,
    Rejected,
    NeedsAdmin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IntentGeneration(i64);

impl IntentGeneration {
    pub const FIRST: Self = Self(1);

    pub fn new(raw: i64) -> Option<Self> {
        (raw >= 1).then_some(Self(raw))
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    fn next(self) -> Result<Self, PublicationError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(PublicationError::GenerationExhausted)
    }
}

/// Immutable content binding. Reuses the canonical v1 hash; metadata remains a separate
/// binding to be captured and bound beside this one in the future publication transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub work: WorkId,
    pub revision: RevisionId,
    pub content: RevisionContent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionFacts {
    pub snapshot: Snapshot,
    pub moderation: Moderation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishIntent {
    pub revision: RevisionId,
    pub generation: IntentGeneration,
}

/// The author intent that authorized an existing pointer. It may be older than the current
/// intent; it is retained so restoration does not equate current intent with public reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicPointer {
    pub revision: RevisionId,
    pub intent_generation: IntentGeneration,
}

/// Untrusted row-loading shape: not itself a publication. No Deserialize bypass for Publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationFacts {
    pub work: WorkId,
    pub owner: PrincipalId,
    pub audience: Audience,
    pub lifecycle: Lifecycle,
    pub generation: IntentGeneration,
    pub intent: Option<PublishIntent>,
    pub pointer: Option<PublicPointer>,
    pub revisions: Vec<RevisionFacts>,
}

/// Validated aggregate, with no writable fields or deserialization bypass.
/// ```compile_fail
/// use vink_app::domain::publication::Publication;
/// let state: Publication = serde_json::from_str("{}").unwrap();
/// ```
/// Lawful twin (stable rustdoc ignores compile-fail error codes):
/// ```
/// use vink_app::domain::{ids::{PrincipalId, WorkId}, publication::Publication};
/// let raw: serde_json::Value = serde_json::from_str("{}").unwrap();
/// let state = Publication::new(
///     WorkId::parse("33333333-3333-4333-8333-333333333333").unwrap(),
///     PrincipalId::parse("11111111-1111-4111-8111-111111111111").unwrap(),
/// );
/// assert!(state.readable_revision().is_none());
/// assert!(raw.is_object());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publication(PublicationFacts);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorCommand {
    SaveSnapshot(Snapshot),
    RequestPublic {
        revision: RevisionId,
        expected: IntentGeneration,
    },
    SetPrivate {
        expected: IntentGeneration,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorOutcome {
    Saved,
    Submitted,
    Waiting,
    Published,
    MadePrivate,
    Reaffirmed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationRequest {
    pub work: WorkId,
    pub revision: RevisionId,
    pub hash: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEffect {
    Unchanged,
    Set(PublicPointer),
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effects {
    pub evaluation: Option<EvaluationRequest>,
    pub pointer: PointerEffect,
}

impl Effects {
    const NONE: Self = Self {
        evaluation: None,
        pointer: PointerEffect::Unchanged,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorTransition {
    pub next: Publication,
    pub outcome: AuthorOutcome,
    pub effects: Effects,
}

/// Only a decided outcome, never Pending/Unsubmitted. This is the integration seam for #151;
/// it grants no actor/job authority. `expected` fences a changed moderation fact locally, not
/// a decision-version ABA: the moderation owner must fence that before calling this method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOutcome {
    Approved,
    Rejected,
    NeedsAdmin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedRevision {
    pub work: WorkId,
    pub revision: RevisionId,
    pub hash: ContentHash,
    pub expected: Moderation,
    pub outcome: ReviewOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewTransition {
    pub next: Publication,
    pub effects: Effects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PublicationError {
    #[error("work not found or owned by someone else")]
    NotFound,
    #[error("work is not active")]
    WorkUnavailable,
    #[error("unknown revision")]
    UnknownRevision,
    #[error("revision belongs to another work")]
    WrongWork,
    #[error("revision already exists")]
    DuplicateRevision,
    #[error("audience intent generation changed")]
    Stale,
    #[error("audience intent generation cannot advance")]
    GenerationExhausted,
    #[error("revision was rejected")]
    RevisionRejected,
    #[error("stored publication facts are inconsistent")]
    InvalidFacts,
    #[error("revision content hash does not match")]
    WrongHash,
    #[error("revision has not been submitted")]
    NotSubmitted,
    #[error("moderation fact changed")]
    StaleModeration,
}

impl Publication {
    pub fn new(work: WorkId, owner: PrincipalId) -> Self {
        Self(PublicationFacts {
            work,
            owner,
            audience: Audience::Private,
            lifecycle: Lifecycle::Active,
            generation: IntentGeneration::FIRST,
            intent: None,
            pointer: None,
            revisions: vec![],
        })
    }

    pub fn restore(facts: PublicationFacts) -> Result<Self, PublicationError> {
        let state = Self(facts);
        for (i, revision) in state.0.revisions.iter().enumerate() {
            if revision.snapshot.work != state.0.work
                || state.0.revisions[..i]
                    .iter()
                    .any(|r| r.snapshot.revision == revision.snapshot.revision)
            {
                return Err(PublicationError::InvalidFacts);
            }
        }
        if state.0.lifecycle != Lifecycle::Active
            && (state.0.intent.is_some() || state.0.pointer.is_some())
        {
            return Err(PublicationError::InvalidFacts);
        }
        if (state.0.lifecycle == Lifecycle::AuthorDeleted && state.0.audience != Audience::Private)
            || (state.0.generation == IntentGeneration::FIRST
                && state
                    .0
                    .revisions
                    .iter()
                    .any(|r| r.moderation != Moderation::Unsubmitted))
        {
            return Err(PublicationError::InvalidFacts);
        }
        if (state.0.audience == Audience::Private && state.0.intent.is_some())
            || (state.0.audience == Audience::Public
                && state.0.lifecycle == Lifecycle::Active
                && state.0.intent.is_none())
        {
            return Err(PublicationError::InvalidFacts);
        }
        if let Some(intent) = state.0.intent
            && (intent.generation != state.0.generation
                || intent.generation <= IntentGeneration::FIRST
                || !state
                    .revision(intent.revision)
                    .is_ok_and(|r| r.moderation != Moderation::Unsubmitted))
        {
            return Err(PublicationError::InvalidFacts);
        }
        if let Some(pointer) = state.0.pointer
            && (state.0.lifecycle != Lifecycle::Active
                || state.0.audience != Audience::Public
                || pointer.intent_generation <= IntentGeneration::FIRST
                || pointer.intent_generation > state.0.generation
                || !state
                    .revision(pointer.revision)
                    .is_ok_and(|r| r.moderation == Moderation::Approved)
                || (pointer.intent_generation == state.0.generation
                    && state.0.intent.map(|i| i.revision) != Some(pointer.revision)))
        {
            return Err(PublicationError::InvalidFacts);
        }
        Ok(state)
    }

    pub fn facts(&self) -> &PublicationFacts {
        &self.0
    }

    fn revision(&self, id: RevisionId) -> Result<&RevisionFacts, PublicationError> {
        self.0
            .revisions
            .iter()
            .find(|r| r.snapshot.revision == id)
            .ok_or(PublicationError::UnknownRevision)
    }

    fn active(&self) -> Result<(), PublicationError> {
        if self.0.lifecycle == Lifecycle::Active {
            Ok(())
        } else {
            Err(PublicationError::WorkUnavailable)
        }
    }

    /// Eligibility for changing the pointer, bound to the exact observed intent generation.
    pub fn promotion_eligible(&self, intent: PublishIntent) -> bool {
        self.0.lifecycle == Lifecycle::Active
            && self.0.audience == Audience::Public
            && self.0.intent == Some(intent)
            && self.0.generation == intent.generation
            && self
                .revision(intent.revision)
                .is_ok_and(|r| r.moderation == Moderation::Approved)
    }

    /// Existing public-read eligibility: never require current intent == pointer (PUB-02).
    /// Runtime public-read adapters do not call this core yet.
    pub fn readable_revision(&self) -> Option<&Snapshot> {
        let pointer = self.0.pointer?;
        let revision = self.revision(pointer.revision).ok()?;
        (self.0.lifecycle == Lifecycle::Active
            && self.0.audience == Audience::Public
            && revision.moderation == Moderation::Approved)
            .then_some(&revision.snapshot)
    }

    fn promote(&mut self, effects: &mut Effects) {
        if let Some(intent) = self.0.intent.filter(|i| self.promotion_eligible(*i)) {
            let pointer = PublicPointer {
                revision: intent.revision,
                intent_generation: intent.generation,
            };
            self.0.pointer = Some(pointer);
            effects.pointer = PointerEffect::Set(pointer);
        }
    }

    pub fn decide_author(
        &self,
        actor: PrincipalId,
        command: AuthorCommand,
    ) -> Result<AuthorTransition, PublicationError> {
        if actor != self.0.owner {
            return Err(PublicationError::NotFound);
        }
        self.active()?;
        let mut next = self.clone();
        let mut effects = Effects::NONE;
        let outcome = match command {
            AuthorCommand::SaveSnapshot(snapshot) => {
                if snapshot.work != self.0.work {
                    return Err(PublicationError::WrongWork);
                }
                if self.revision(snapshot.revision).is_ok() {
                    return Err(PublicationError::DuplicateRevision);
                }
                next.0.revisions.push(RevisionFacts {
                    snapshot,
                    moderation: Moderation::Unsubmitted,
                });
                AuthorOutcome::Saved
            }
            AuthorCommand::RequestPublic { revision, expected } => {
                let facts = self.revision(revision)?;
                if expected != self.0.generation {
                    return Err(PublicationError::Stale);
                }
                if facts.moderation == Moderation::Rejected {
                    return Err(PublicationError::RevisionRejected);
                }
                let generation = expected.next()?;
                next.0.generation = generation;
                next.0.audience = Audience::Public;
                next.0.intent = Some(PublishIntent {
                    revision,
                    generation,
                });
                match facts.moderation {
                    Moderation::Unsubmitted => {
                        next.0
                            .revisions
                            .iter_mut()
                            .find(|r| r.snapshot.revision == revision)
                            .ok_or(PublicationError::UnknownRevision)?
                            .moderation = Moderation::Pending;
                        effects.evaluation = Some(EvaluationRequest {
                            work: self.0.work,
                            revision,
                            hash: facts.snapshot.content.hash().clone(),
                        });
                        AuthorOutcome::Submitted
                    }
                    Moderation::Approved => {
                        next.promote(&mut effects);
                        AuthorOutcome::Published
                    }
                    _ => AuthorOutcome::Waiting,
                }
            }
            AuthorCommand::SetPrivate { expected } => {
                if expected != self.0.generation {
                    return Err(PublicationError::Stale);
                }
                next.0.generation = expected.next()?;
                next.0.audience = Audience::Private;
                next.0.intent = None;
                next.0.pointer = None;
                effects.pointer = PointerEffect::Clear;
                if self.0.audience == Audience::Public {
                    AuthorOutcome::MadePrivate
                } else {
                    AuthorOutcome::Reaffirmed
                }
            }
        };
        Ok(AuthorTransition {
            next,
            outcome,
            effects,
        })
    }

    /// Record a decision already admitted by the moderation owner and run the publication guard.
    /// There is deliberately no raw-provider-result or generic save-state method.
    pub fn record_review(
        &self,
        review: ReviewedRevision,
    ) -> Result<ReviewTransition, PublicationError> {
        self.active()?;
        if review.work != self.0.work {
            return Err(PublicationError::WrongWork);
        }
        let revision = self.revision(review.revision)?;
        if revision.snapshot.content.hash() != &review.hash {
            return Err(PublicationError::WrongHash);
        }
        if revision.moderation == Moderation::Unsubmitted {
            return Err(PublicationError::NotSubmitted);
        }
        if revision.moderation != review.expected {
            return Err(PublicationError::StaleModeration);
        }
        let mut next = self.clone();
        let mut effects = Effects::NONE;
        next.0
            .revisions
            .iter_mut()
            .find(|r| r.snapshot.revision == review.revision)
            .ok_or(PublicationError::UnknownRevision)?
            .moderation = match review.outcome {
            ReviewOutcome::Approved => Moderation::Approved,
            ReviewOutcome::Rejected => Moderation::Rejected,
            ReviewOutcome::NeedsAdmin => Moderation::NeedsAdmin,
        };
        // A revised decision on the currently public revision closes its pointer. A rejection
        // of r2 must never close r1. No authority/override decision is inferred here.
        if review.outcome != ReviewOutcome::Approved
            && next
                .0
                .pointer
                .is_some_and(|p| p.revision == review.revision)
        {
            next.0.pointer = None;
            effects.pointer = PointerEffect::Clear;
        }
        // Only this decision's revision may be promoted; an unrelated callback cannot retry a
        // different revision's publication as an accidental side effect.
        if next.0.intent.is_some_and(|i| i.revision == review.revision) {
            next.promote(&mut effects);
        }
        Ok(ReviewTransition { next, effects })
    }
}

#[cfg(test)]
pub(crate) mod replay;
#[cfg(test)]
pub(crate) mod tests;
