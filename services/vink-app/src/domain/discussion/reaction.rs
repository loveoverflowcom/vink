//! Reactions on a comment (D2T-INV-08, COM-06). Pure decisions only: no store, route or UI uses
//! them yet; persistence, counts in SQL and the HTTP contract belong to #43/#45.
//!
//! Semantics chosen for this core (proposed, pending product confirmation; see
//! `docs/development/discussion-core.md`):
//!
//! - **Desired state, never a toggle.** A request names the reaction the actor wants on a comment
//!   (`Some(kind)`) or none (`None`). Asking for what is already there is a no-op, so a retry
//!   after a lost response cannot undo itself.
//! - **Cardinality:** at most one reaction per (actor, comment), from the closed
//!   [`ReactionKind`] catalog. Choosing another kind replaces the previous one.
//! - **Generation fence:** every actual change advances the actor's [`ReactionGeneration`] on
//!   that comment by one and needs the generation the client saw. A delayed duplicate from an
//!   older view is `Stale`, so it cannot revert a newer choice. Storage must therefore keep a
//!   cleared reaction's row (kind `None`, its generation) instead of deleting it.
//! - **Interaction limits:** only a public comment of a public work accepts reactions; a hidden
//!   comment answers like a missing one. Nobody reacts to their own comment. A work whose author
//!   closed comments accepts no new or changed reaction, but an actor may always clear their own.
//! - **Counts are projections.** [`ReactionTally`] is rebuilt from reactions and changes only by
//!   the delta of an applied change; it never goes negative or wraps, and it decides nothing
//!   (no ACL, ranking or removal reads it). A hidden comment exposes no tally.
//!
//! A raw number is not a generation, and a stored reaction is not writable:
//! ```compile_fail
//! use vink_app::domain::discussion::reaction::ReactionGeneration;
//! let generation = ReactionGeneration(0);
//! ```
//! ```compile_fail
//! use vink_app::domain::discussion::reaction::{ActorReaction, ReactionKind};
//! let mut reaction = ActorReaction::NONE;
//! reaction.kind = Some(ReactionKind::Heart);
//! ```
//! Lawful twin (stable rustdoc does not check `compile_fail` error codes):
//! ```
//! use vink_app::domain::discussion::reaction::{
//!     ActorReaction, ReactionChange, ReactionGeneration, ReactionKind,
//! };
//! assert_eq!(ReactionGeneration::new(0), None);
//! let reaction = ActorReaction::NONE;
//! let Ok(ReactionChange::Changed { next, .. }) =
//!     reaction.desire(ReactionGeneration::FIRST, Some(ReactionKind::Heart))
//! else {
//!     panic!("the first reaction applies at the first generation");
//! };
//! assert_eq!(next.kind(), Some(ReactionKind::Heart));
//! ```

use super::lifecycle::CommentState;
use super::settings::DiscussionSettings;
use crate::domain::ids::PrincipalId;

/// The closed reaction catalog. Codes are the wire/storage spelling; adding a kind is a contract
/// change, never a client-supplied string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReactionKind {
    /// Thích, thương.
    Heart,
    /// Xúc động.
    Moved,
    /// Suy ngẫm.
    Thoughtful,
    /// Tán thưởng.
    Applause,
}

pub const KINDS: usize = 4;

impl ReactionKind {
    pub const ALL: [Self; KINDS] = [Self::Heart, Self::Moved, Self::Thoughtful, Self::Applause];

    pub const fn code(self) -> &'static str {
        match self {
            Self::Heart => "heart",
            Self::Moved => "moved",
            Self::Thoughtful => "thoughtful",
            Self::Applause => "applause",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.code() == raw)
    }

    const fn index(self) -> usize {
        match self {
            Self::Heart => 0,
            Self::Moved => 1,
            Self::Thoughtful => 2,
            Self::Applause => 3,
        }
    }
}

/// Monotonic version of one actor's reaction on one comment, bumped only by an actual change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReactionGeneration(i64);

impl ReactionGeneration {
    pub const FIRST: Self = Self(1);

    pub fn new(raw: i64) -> Option<Self> {
        (raw >= 1).then_some(Self(raw))
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// One actor's current reaction on one comment, with its generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorReaction {
    kind: Option<ReactionKind>,
    generation: ReactionGeneration,
}

/// What an applied change does to the comment's counts. `removed != added` always holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReactionDelta {
    removed: Option<ReactionKind>,
    added: Option<ReactionKind>,
}

impl ReactionDelta {
    pub const fn removed(self) -> Option<ReactionKind> {
        self.removed
    }

    pub const fn added(self) -> Option<ReactionKind> {
        self.added
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionChange {
    Changed {
        next: ActorReaction,
        delta: ReactionDelta,
    },
    /// The actor already has exactly the requested reaction: a safe retry, nothing to write.
    Unchanged(ActorReaction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReactionError {
    /// Another change by this actor landed since the caller's view; it must not be undone.
    #[error("reaction changed since the expected generation")]
    Stale { current: ActorReaction },
    #[error("reaction generation cannot advance")]
    GenerationExhausted,
}

impl ActorReaction {
    /// An actor who never reacted to the comment (no stored row).
    pub const NONE: Self = Self {
        kind: None,
        generation: ReactionGeneration::FIRST,
    };

    /// Rebuilds a reaction read from storage; the generation was already validated.
    pub const fn restore(kind: Option<ReactionKind>, generation: ReactionGeneration) -> Self {
        Self { kind, generation }
    }

    pub const fn kind(self) -> Option<ReactionKind> {
        self.kind
    }

    pub const fn generation(self) -> ReactionGeneration {
        self.generation
    }

    /// Desired-state change bound to the generation the caller saw.
    pub fn desire(
        self,
        expected: ReactionGeneration,
        wanted: Option<ReactionKind>,
    ) -> Result<ReactionChange, ReactionError> {
        if wanted == self.kind {
            return Ok(ReactionChange::Unchanged(self));
        }
        if expected != self.generation {
            return Err(ReactionError::Stale { current: self });
        }
        let generation = self
            .generation
            .next()
            .ok_or(ReactionError::GenerationExhausted)?;
        Ok(ReactionChange::Changed {
            next: Self {
                kind: wanted,
                generation,
            },
            delta: ReactionDelta {
                removed: self.kind,
                added: wanted,
            },
        })
    }
}

/// Facts the adapter reads in the write transaction, under the same locks as a submission.
/// `target` is `None` when the comment is not in this work's thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReactionFacts {
    pub work_public: bool,
    pub settings: DiscussionSettings,
    /// The comment's writer and lifecycle state.
    pub target: Option<(PrincipalId, CommentState)>,
    /// The acting principal's stored reaction on it, or [`ActorReaction::NONE`].
    pub current: ActorReaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionDecision {
    /// Store `next` for the actor and apply `delta` to the comment's tally.
    Apply {
        next: ActorReaction,
        delta: ReactionDelta,
    },
    /// Already in the requested state; write nothing.
    Unchanged(ActorReaction),
    Stale {
        current: ActorReaction,
    },
    /// The work has no current public publication.
    WorkNotPublic,
    /// Missing, foreign or not public: indistinguishable on purpose.
    TargetUnavailable,
    /// Writers do not react to their own comments.
    OwnComment,
    /// The work's author closed comments; only clearing an existing reaction is accepted.
    CommentsClosed,
    GenerationExhausted,
}

/// The single production decision for setting or clearing a reaction.
pub fn decide_reaction(
    actor: PrincipalId,
    expected: ReactionGeneration,
    wanted: Option<ReactionKind>,
    facts: ReactionFacts,
) -> ReactionDecision {
    if !facts.work_public {
        return ReactionDecision::WorkNotPublic;
    }
    let Some((writer, _)) = facts.target.filter(|(_, state)| state.is_public()) else {
        return ReactionDecision::TargetUnavailable;
    };
    // Limits gate only new interaction; a retry of the current state stays a no-op, and clearing
    // is always the actor's to do.
    if wanted.is_some() && wanted != facts.current.kind() {
        if actor == writer {
            return ReactionDecision::OwnComment;
        }
        if !facts.settings.accepts_new_comments() {
            return ReactionDecision::CommentsClosed;
        }
    }
    match facts.current.desire(expected, wanted) {
        Ok(ReactionChange::Changed { next, delta }) => ReactionDecision::Apply { next, delta },
        Ok(ReactionChange::Unchanged(current)) => ReactionDecision::Unchanged(current),
        Err(ReactionError::Stale { current }) => ReactionDecision::Stale { current },
        Err(ReactionError::GenerationExhausted) => ReactionDecision::GenerationExhausted,
    }
}

/// Per-kind reaction counts of one comment: a rebuildable projection, never an authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReactionTally([u32; KINDS]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TallyError {
    /// The projection disagrees with the reactions it summarizes; rebuild it.
    #[error("reaction count would become negative")]
    Underflow,
    #[error("reaction count cannot grow")]
    Overflow,
}

impl ReactionTally {
    pub const EMPTY: Self = Self([0; KINDS]);

    /// Rebuilds counts read from storage, in [`ReactionKind::ALL`] order.
    pub const fn restore(counts: [u32; KINDS]) -> Self {
        Self(counts)
    }

    pub const fn count(self, kind: ReactionKind) -> u32 {
        self.0[kind.index()]
    }

    pub fn total(self) -> u64 {
        self.0.iter().map(|&count| u64::from(count)).sum()
    }

    /// Applies one change atomically: on error the caller keeps the old tally.
    pub fn apply(self, delta: ReactionDelta) -> Result<Self, TallyError> {
        let mut counts = self.0;
        if let Some(kind) = delta.removed {
            let slot = &mut counts[kind.index()];
            *slot = slot.checked_sub(1).ok_or(TallyError::Underflow)?;
        }
        if let Some(kind) = delta.added {
            let slot = &mut counts[kind.index()];
            *slot = slot.checked_add(1).ok_or(TallyError::Overflow)?;
        }
        Ok(Self(counts))
    }
}

/// The tally a reader may see: only a public comment shows counts.
pub fn public_tally(state: CommentState, tally: ReactionTally) -> Option<ReactionTally> {
    state.is_public().then_some(tally)
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::discussion::lifecycle::{ModerationVersion, Removal, Review};
    use crate::domain::discussion::settings::{CommentsMode, SettingsGeneration};

    fn principal(id: u128) -> PrincipalId {
        PrincipalId::from_uuid(Uuid::from_u128(id))
    }

    fn generation(raw: i64) -> ReactionGeneration {
        ReactionGeneration::new(raw).unwrap()
    }

    fn public() -> CommentState {
        CommentState::restore(Review::Approved, None, ModerationVersion::FIRST)
    }

    fn closed() -> DiscussionSettings {
        DiscussionSettings::restore(CommentsMode::Closed, SettingsGeneration::new(2).unwrap())
    }

    /// Writer 2's public comment on an open public work; actor 3 has `current`.
    fn facts(current: ActorReaction) -> ReactionFacts {
        ReactionFacts {
            work_public: true,
            settings: DiscussionSettings::INITIAL,
            target: Some((principal(2), public())),
            current,
        }
    }

    fn applied(decision: ReactionDecision) -> (ActorReaction, ReactionDelta) {
        match decision {
            ReactionDecision::Apply { next, delta } => (next, delta),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn kinds_parse_exactly_their_codes() {
        for kind in ReactionKind::ALL {
            assert_eq!(ReactionKind::parse(kind.code()), Some(kind));
        }
        for bad in ["", "Heart", "HEART", "like", "heart ", "❤️"] {
            assert_eq!(ReactionKind::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn set_replace_and_clear_advance_one_generation_each() {
        let actor = principal(3);
        let (heart, delta) = applied(decide_reaction(
            actor,
            generation(1),
            Some(ReactionKind::Heart),
            facts(ActorReaction::NONE),
        ));
        assert_eq!(
            heart,
            ActorReaction::restore(Some(ReactionKind::Heart), generation(2))
        );
        assert_eq!(
            (delta.removed(), delta.added()),
            (None, Some(ReactionKind::Heart))
        );

        let (moved, delta) = applied(decide_reaction(
            actor,
            generation(2),
            Some(ReactionKind::Moved),
            facts(heart),
        ));
        assert_eq!(moved.generation(), generation(3));
        assert_eq!(
            (delta.removed(), delta.added()),
            (Some(ReactionKind::Heart), Some(ReactionKind::Moved))
        );

        let (cleared, delta) = applied(decide_reaction(actor, generation(3), None, facts(moved)));
        assert_eq!(cleared, ActorReaction::restore(None, generation(4)));
        assert_eq!(
            (delta.removed(), delta.added()),
            (Some(ReactionKind::Moved), None)
        );
    }

    #[test]
    fn retrying_the_current_state_is_a_no_op_at_any_generation() {
        let heart = ActorReaction::restore(Some(ReactionKind::Heart), generation(2));
        for expected in [1, 2, 9] {
            assert_eq!(
                decide_reaction(
                    principal(3),
                    generation(expected),
                    Some(ReactionKind::Heart),
                    facts(heart)
                ),
                ReactionDecision::Unchanged(heart)
            );
        }
        // Clearing twice is not a toggle back to the old reaction.
        let cleared = ActorReaction::restore(None, generation(3));
        assert_eq!(
            decide_reaction(principal(3), generation(2), None, facts(cleared)),
            ReactionDecision::Unchanged(cleared)
        );
    }

    #[test]
    fn a_delayed_duplicate_cannot_revert_a_newer_choice() {
        // Tab A set Heart at generation 1 (now 2); tab B then cleared it (now 3). A's delayed
        // retry of Heart@1 arrives last and must not bring Heart back.
        let cleared = ActorReaction::restore(None, generation(3));
        assert_eq!(
            decide_reaction(
                principal(3),
                generation(1),
                Some(ReactionKind::Heart),
                facts(cleared)
            ),
            ReactionDecision::Stale { current: cleared }
        );
    }

    #[test]
    fn interaction_limits_refuse_new_reactions_but_never_block_clearing() {
        let actor = principal(3);
        let heart = ActorReaction::restore(Some(ReactionKind::Heart), generation(2));

        let own = ReactionFacts {
            target: Some((actor, public())),
            ..facts(ActorReaction::NONE)
        };
        assert_eq!(
            decide_reaction(actor, generation(1), Some(ReactionKind::Heart), own),
            ReactionDecision::OwnComment
        );

        let shut = ReactionFacts {
            settings: closed(),
            ..facts(heart)
        };
        assert_eq!(
            decide_reaction(actor, generation(2), Some(ReactionKind::Moved), shut),
            ReactionDecision::CommentsClosed
        );
        assert_eq!(
            decide_reaction(actor, generation(2), Some(ReactionKind::Heart), shut),
            ReactionDecision::Unchanged(heart),
            "a lost-response retry after closing still succeeds"
        );
        let (cleared, _) = applied(decide_reaction(actor, generation(2), None, shut));
        assert_eq!(cleared.kind(), None);
    }

    #[test]
    fn hidden_comments_and_withdrawn_works_accept_nothing() {
        let actor = principal(3);
        let heart = ActorReaction::restore(Some(ReactionKind::Heart), generation(2));
        let hidden = [
            CommentState::SUBMITTED,
            CommentState::restore(Review::Rejected, None, ModerationVersion::FIRST),
            CommentState::restore(
                Review::Approved,
                Some(Removal::ByAuthor),
                ModerationVersion::new(2).unwrap(),
            ),
            CommentState::restore(
                Review::Approved,
                Some(Removal::ByModeration),
                ModerationVersion::new(2).unwrap(),
            ),
        ];
        for state in hidden {
            for wanted in [Some(ReactionKind::Moved), None] {
                let target_hidden = ReactionFacts {
                    target: Some((principal(2), state)),
                    ..facts(heart)
                };
                assert_eq!(
                    decide_reaction(actor, generation(2), wanted, target_hidden),
                    ReactionDecision::TargetUnavailable
                );
            }
            assert_eq!(public_tally(state, ReactionTally::EMPTY), None);
        }
        let missing = ReactionFacts {
            target: None,
            ..facts(heart)
        };
        assert_eq!(
            decide_reaction(actor, generation(2), None, missing),
            ReactionDecision::TargetUnavailable
        );
        let withdrawn = ReactionFacts {
            work_public: false,
            ..facts(heart)
        };
        assert_eq!(
            decide_reaction(actor, generation(2), None, withdrawn),
            ReactionDecision::WorkNotPublic
        );
    }

    #[test]
    fn generations_are_positive_and_never_wrap() {
        assert_eq!(ReactionGeneration::new(0), None);
        assert_eq!(ReactionGeneration::new(-1), None);
        let last = ActorReaction::restore(None, generation(i64::MAX));
        assert_eq!(
            last.desire(generation(i64::MAX), Some(ReactionKind::Heart)),
            Err(ReactionError::GenerationExhausted)
        );
    }

    #[test]
    fn tallies_follow_deltas_and_refuse_to_underflow_or_wrap() {
        let (heart, delta) =
            match ActorReaction::NONE.desire(generation(1), Some(ReactionKind::Heart)) {
                Ok(ReactionChange::Changed { next, delta }) => (next, delta),
                other => panic!("{other:?}"),
            };
        let tally = ReactionTally::EMPTY.apply(delta).unwrap();
        assert_eq!(tally.count(ReactionKind::Heart), 1);
        assert_eq!(tally.total(), 1);
        let Ok(ReactionChange::Changed { delta: replace, .. }) =
            heart.desire(generation(2), Some(ReactionKind::Applause))
        else {
            panic!("replace applies");
        };
        let tally = tally.apply(replace).unwrap();
        assert_eq!(tally.count(ReactionKind::Heart), 0);
        assert_eq!(tally.count(ReactionKind::Applause), 1);
        assert_eq!(tally.total(), 1, "replacing keeps one reaction per actor");

        assert_eq!(
            ReactionTally::EMPTY.apply(replace),
            Err(TallyError::Underflow)
        );
        let full = ReactionTally::restore([u32::MAX, 0, 0, 0]);
        assert_eq!(full.apply(delta), Err(TallyError::Overflow));
        assert_eq!(public_tally(public(), tally), Some(tally));
    }
}
