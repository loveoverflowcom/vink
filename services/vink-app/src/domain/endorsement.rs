//! Literary endorsement (LVE, #73): one positive signal per (actor, target) on a public work
//! ("Đánh giá cao"), a root comment or a flat reply ("Hữu ích"). Pure decisions only: no store,
//! route or UI uses them yet. The API and PostgreSQL slices of #73 call these same functions
//! inside their transaction; they must not restate a rule in a handler or in SQL.
//!
//! Semantics, policy [`POLICY`] (selected for this slice, product confirmation pending; the
//! decision record is `docs/architecture/literary-endorsement.md`):
//!
//! - **Cardinality (CHOICE-01 A):** an endorsement is selected or not; there is no score, reason
//!   facet or downvote. It is independent of reactions, bookmarks and reports.
//! - **Accepted-intent version (CHOICE-03):** a request names the desired state and the version
//!   the client saw. A request carrying the current version is *accepted* and advances the
//!   version by exactly one, **even when it asks for the state already held** (a reaffirmation).
//!   That advance is the fence: a request created earlier, still in flight with the same or an
//!   older version, can never be applied afterwards. A request carrying another version is not
//!   accepted: it is [`Transition::AlreadyCurrent`] when the stored state already satisfies it
//!   (a retry, a duplicate) and [`Transition::Stale`] otherwise. Neither writes. There is no
//!   receipt: a retry is recognized by its effect, and a client reconciles a lost answer by
//!   resending at its last known version or by reading its own state back.
//! - **No version reset (ABA):** storage keeps a cleared endorsement as a row (no selection, its
//!   version); an absent row means [`Endorsement::ABSENT`]. Deleting a row would restart the
//!   version at 1 and let an old duplicate apply again.
//! - **Anchor (CHOICE-02 C):** a work endorsement records the revision the reader saw and is
//!   effective only while that revision is the current publication; a new publication makes it
//!   an [`OwnView::EndorsedEarlierRevision`] until the reader endorses the new text. A comment's
//!   body never changes after submission (D-08), so the comment itself is the anchor
//!   ([`CommentAnchor`]); its moderation version is not a text revision.
//! - **Eligibility** is decided per target in [`work`] and [`comment`]: target binding and
//!   visibility at commit, no self-endorsement (CHOICE-08 A), closed comments stop new comment
//!   endorsements but never clearing (CHOICE-09 B). A hidden target refuses everything, including
//!   clearing, and exposes neither count nor own state.
//! - **Counts are projections** of the selections, per anchor: [`EndorsementCount`] changes only
//!   by an accepted change's delta, never goes negative or wraps, and decides nothing (no ACL,
//!   publication, moderation, curation or ranking reads it).
//!
//! A raw number is not a version, and a stored endorsement is not writable:
//! ```compile_fail
//! use vink_app::domain::endorsement::EndorsementVersion;
//! let version = EndorsementVersion(0);
//! ```
//! ```compile_fail
//! use vink_app::domain::endorsement::{CommentAnchor, Endorsement};
//! let mut endorsement = Endorsement::<CommentAnchor>::ABSENT;
//! endorsement.selection = Some(CommentAnchor);
//! ```
//! Lawful twin (stable rustdoc does not check `compile_fail` error codes):
//! ```
//! use vink_app::domain::endorsement::{CommentAnchor, Endorsement, EndorsementVersion, Transition};
//! assert_eq!(EndorsementVersion::new(0), None);
//! let endorsement = Endorsement::<CommentAnchor>::ABSENT;
//! let Transition::Changed { next, .. } =
//!     endorsement.transition(EndorsementVersion::FIRST, Some(CommentAnchor))
//! else {
//!     panic!("the first endorsement is accepted at the first version");
//! };
//! assert_eq!(next.selection(), Some(CommentAnchor));
//! ```

pub mod comment;
#[cfg(kani)]
mod proofs;
#[cfg(test)]
mod replay;
pub mod work;

/// Policy of this core. Stored projections and the behavior model name it, so a change of
/// meaning (anchor equivalence, self-endorsement, comments-close) is a new policy version.
pub const POLICY: &str = "lve-core-v1";

/// Version of one actor's endorsement of one target. It starts at [`Self::FIRST`] for an actor
/// who never acted and advances by one on every accepted intent, reaffirmations included; it
/// never moves backwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EndorsementVersion(i64);

impl EndorsementVersion {
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

/// The anchor of a comment endorsement: the comment itself. A submitted comment body is
/// immutable (D-08 blocks public edits), so there is nothing else to point at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommentAnchor;

/// One actor's endorsement of one target: the anchor it selects, if any, and its version.
/// `A` is [`crate::domain::ids::RevisionId`] for a work and [`CommentAnchor`] for a comment, so
/// a work endorsement can never carry a comment anchor or the reverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endorsement<A> {
    selection: Option<A>,
    version: EndorsementVersion,
}

/// What an accepted change does to the counts: one selection removed, one added, or both.
/// `removed != added` always holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndorsementDelta<A> {
    removed: Option<A>,
    added: Option<A>,
}

impl<A: Copy> EndorsementDelta<A> {
    pub fn removed(self) -> Option<A> {
        self.removed
    }

    pub fn added(self) -> Option<A> {
        self.added
    }
}

/// Outcome of a desired-state request against the actor's current endorsement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition<A> {
    /// Accepted and the selection changes: store `next`, apply `delta` to the counts.
    Changed {
        next: Endorsement<A>,
        delta: EndorsementDelta<A>,
    },
    /// Accepted; the selection already held. Store `next` (only its version advanced, which
    /// fences every older request); the counts do not change.
    Reaffirmed {
        next: Endorsement<A>,
    },
    /// Not accepted: the stored state already satisfies the request at a newer version than the
    /// caller saw (a retry or duplicate). Nothing is written and nothing is fenced.
    AlreadyCurrent(Endorsement<A>),
    /// Not accepted: an accepted intent changed the selection since the caller's view. It must
    /// not be undone by this request; the client reconciles from `current`.
    Stale {
        current: Endorsement<A>,
    },
    VersionExhausted,
}

impl<A: Copy> Transition<A> {
    /// The endorsement to store, when the request was accepted.
    pub fn write(self) -> Option<Endorsement<A>> {
        match self {
            Self::Changed { next, .. } | Self::Reaffirmed { next } => Some(next),
            Self::AlreadyCurrent(_) | Self::Stale { .. } | Self::VersionExhausted => None,
        }
    }

    /// The count delta, when the request was accepted and changed the selection.
    pub fn delta(self) -> Option<EndorsementDelta<A>> {
        match self {
            Self::Changed { delta, .. } => Some(delta),
            _ => None,
        }
    }
}

impl<A> Endorsement<A> {
    /// An actor who never acted on the target (no stored row).
    pub const ABSENT: Self = Self {
        selection: None,
        version: EndorsementVersion::FIRST,
    };
}

impl<A: Copy + Eq> Endorsement<A> {
    /// Rebuilds an endorsement read from storage; the version was already validated.
    pub fn restore(selection: Option<A>, version: EndorsementVersion) -> Self {
        Self { selection, version }
    }

    pub fn selection(self) -> Option<A> {
        self.selection
    }

    pub fn version(self) -> EndorsementVersion {
        self.version
    }

    /// The one transition: `desired` selection bound to the version the caller saw. The
    /// per-target decisions check eligibility first and then call this.
    pub fn transition(self, expected: EndorsementVersion, desired: Option<A>) -> Transition<A> {
        if expected == self.version {
            let Some(version) = self.version.next() else {
                return Transition::VersionExhausted;
            };
            let next = Self {
                selection: desired,
                version,
            };
            return if desired == self.selection {
                Transition::Reaffirmed { next }
            } else {
                Transition::Changed {
                    next,
                    delta: EndorsementDelta {
                        removed: self.selection,
                        added: desired,
                    },
                }
            };
        }
        if desired == self.selection {
            Transition::AlreadyCurrent(self)
        } else {
            Transition::Stale { current: self }
        }
    }
}

/// How many endorsements select one anchor: a rebuildable projection, never an authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EndorsementCount(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CountError {
    /// The projection disagrees with the selections it summarizes; rebuild it.
    #[error("endorsement count would become negative")]
    Underflow,
    #[error("endorsement count cannot grow")]
    Overflow,
}

impl EndorsementCount {
    pub const ZERO: Self = Self(0);

    /// Rebuilds a count read from storage or recounted from the selections.
    pub const fn restore(count: u32) -> Self {
        Self(count)
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    /// Applies the part of `delta` that concerns `anchor`. On error the caller keeps the old
    /// count and rolls the endorsement write back with it.
    pub fn apply<A: Copy + Eq>(
        self,
        anchor: A,
        delta: EndorsementDelta<A>,
    ) -> Result<Self, CountError> {
        let mut count = self.0;
        if delta.removed == Some(anchor) {
            count = count.checked_sub(1).ok_or(CountError::Underflow)?;
        }
        if delta.added == Some(anchor) {
            count = count.checked_add(1).ok_or(CountError::Overflow)?;
        }
        Ok(Self(count))
    }
}

/// The public aggregate of a target. `Unavailable` is never shown as zero (LVE-FR-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Summary {
    Unavailable,
    Count(EndorsementCount),
}

/// What the acting principal may see of their own endorsement. Private to that principal: it
/// never enters a shared cache or another viewer's response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnView {
    /// The target is missing, foreign or hidden: nothing about it is shown.
    Unavailable,
    NotEndorsed,
    /// Selected and effective: counted in the summary.
    Endorsed,
    /// Selected at a revision that is no longer the public one: not counted; the reader may
    /// endorse the current text.
    EndorsedEarlierRevision,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(raw: i64) -> EndorsementVersion {
        EndorsementVersion::new(raw).unwrap()
    }

    /// Two distinguishable anchors, standing for two revisions of a work.
    const R1: u8 = 1;
    const R2: u8 = 2;

    #[test]
    fn versions_are_positive_and_never_wrap() {
        assert_eq!(EndorsementVersion::new(0), None);
        assert_eq!(EndorsementVersion::new(-1), None);
        assert_eq!(EndorsementVersion::new(1), Some(EndorsementVersion::FIRST));
        let last = Endorsement::restore(None, version(i64::MAX));
        assert_eq!(
            last.transition(version(i64::MAX), Some(R1)),
            Transition::VersionExhausted
        );
        assert_eq!(
            last.transition(version(i64::MAX), None),
            Transition::VersionExhausted,
            "a reaffirmation needs a next version too"
        );
    }

    #[test]
    fn select_reconfirm_and_clear_advance_one_version_each() {
        let Transition::Changed { next, delta } =
            Endorsement::<u8>::ABSENT.transition(version(1), Some(R1))
        else {
            panic!("first selection");
        };
        assert_eq!(next, Endorsement::restore(Some(R1), version(2)));
        assert_eq!((delta.removed(), delta.added()), (None, Some(R1)));

        let Transition::Changed { next, delta } = next.transition(version(2), Some(R2)) else {
            panic!("reconfirm at a new anchor");
        };
        assert_eq!(next, Endorsement::restore(Some(R2), version(3)));
        assert_eq!((delta.removed(), delta.added()), (Some(R1), Some(R2)));

        let Transition::Changed { next, delta } = next.transition(version(3), None) else {
            panic!("clear");
        };
        assert_eq!(next, Endorsement::restore(None, version(4)));
        assert_eq!((delta.removed(), delta.added()), (Some(R2), None));
    }

    #[test]
    fn a_request_for_the_held_state_at_the_current_version_is_accepted_and_fences() {
        // LVE adversarial trace: OFF@1. A = ON@1 was created first but is delayed; B = OFF@1
        // arrives first. B is accepted (reaffirmed) and moves the version, so A is stale.
        let off = Endorsement::<u8>::ABSENT;
        let Transition::Reaffirmed { next } = off.transition(version(1), None) else {
            panic!("B is accepted");
        };
        assert_eq!(next, Endorsement::restore(None, version(2)));
        assert_eq!(off.transition(version(1), None).delta(), None);
        assert_eq!(
            next.transition(version(1), Some(R1)),
            Transition::Stale { current: next },
            "the delayed A cannot revive the older intent"
        );
    }

    #[test]
    fn retries_and_duplicates_are_recognized_by_their_effect_and_never_write() {
        let on = Endorsement::restore(Some(R1), version(2));
        for expected in [1, 3, 9] {
            assert_eq!(
                on.transition(version(expected), Some(R1)),
                Transition::AlreadyCurrent(on)
            );
            assert_eq!(on.transition(version(expected), Some(R1)).write(), None);
        }
        // A retry of an older select after a clear: the cleared row kept its version, so the
        // duplicate is stale instead of applying again (no ABA).
        let cleared = Endorsement::restore(None, version(3));
        assert_eq!(
            cleared.transition(version(1), Some(R1)),
            Transition::Stale { current: cleared }
        );
        assert_eq!(
            cleared.transition(version(2), None),
            Transition::AlreadyCurrent(cleared),
            "retrying the clear is not a toggle"
        );
    }

    #[test]
    fn counts_follow_deltas_per_anchor_and_refuse_to_underflow_or_wrap() {
        let Transition::Changed { delta, .. } =
            Endorsement::restore(Some(R1), version(2)).transition(version(2), Some(R2))
        else {
            panic!("reconfirm");
        };
        let r1 = EndorsementCount::restore(3).apply(R1, delta).unwrap();
        let r2 = EndorsementCount::restore(0).apply(R2, delta).unwrap();
        assert_eq!((r1.get(), r2.get()), (2, 1));
        assert_eq!(
            EndorsementCount::restore(7).apply(3, delta),
            Ok(EndorsementCount::restore(7)),
            "an unrelated anchor is untouched"
        );
        assert_eq!(
            EndorsementCount::ZERO.apply(R1, delta),
            Err(CountError::Underflow)
        );
        assert_eq!(
            EndorsementCount::restore(u32::MAX).apply(R2, delta),
            Err(CountError::Overflow)
        );
    }
}
