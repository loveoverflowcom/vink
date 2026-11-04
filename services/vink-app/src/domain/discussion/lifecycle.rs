//! Comment lifecycle on two independent axes (#41): **presence** (still there, or removed by its
//! writer or by moderation) and **review** (pending, approved, rejected). Public visibility is
//! derived from both plus thread access in [`super::visibility`]; neither axis alone decides it.
//!
//! Every new comment starts pending (COM-02, D-08 premoderation). Decisions are bound to the
//! [`ModerationVersion`] the moderator saw (MOD-02): a late or duplicate decision cannot overwrite a
//! newer one and nothing restores a removed comment (D2T-INV-07). Removal is terminal in this
//! slice; restore and appeal reopening need their own reviewed transitions. Editing does not
//! exist: D-08 keeps public edits blocked until an edit gate is proven.
//!
//! A raw number is not a version and a state's axes are not writable fields; request JSON goes
//! through [`ModerationVersion::new`], and only transitions advance a state:
//! ```compile_fail
//! use vink_app::domain::discussion::lifecycle::ModerationVersion;
//! let version: ModerationVersion = serde_json::from_str("0").unwrap();
//! ```
//! ```compile_fail
//! use vink_app::domain::discussion::lifecycle::ModerationVersion;
//! let version = ModerationVersion(0);
//! ```
//! ```compile_fail
//! use vink_app::domain::discussion::lifecycle::{CommentState, Review};
//! let mut state = CommentState::SUBMITTED;
//! state.review = Review::Approved;
//! ```
//! Lawful twin of the three blocks (stable rustdoc does not check `compile_fail` error codes):
//! ```
//! use vink_app::domain::discussion::lifecycle::{
//!     CommentState, Decision, ModerationVersion, Review, Transition,
//! };
//! let raw: i64 = serde_json::from_str("0").unwrap();
//! assert_eq!(ModerationVersion::new(raw), None);
//! let state = CommentState::SUBMITTED;
//! let Ok(Transition::Changed(approved)) = state.decide(ModerationVersion::FIRST, Decision::Approve)
//! else {
//!     panic!("a pending comment at its current version can be approved");
//! };
//! assert_eq!(approved.review(), Review::Approved);
//! ```

/// Version of a comment's moderation/presence state. Starts at 1 and advances on every change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModerationVersion(i64);

impl ModerationVersion {
    pub const FIRST: Self = Self(1);

    pub fn new(raw: i64) -> Option<Self> {
        (raw >= 1).then_some(Self(raw))
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    fn next(self) -> Result<Self, LifecycleError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(LifecycleError::VersionExhausted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Review {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    /// The comment's own writer removed it; storage drops the text.
    ByAuthor,
    /// A moderator removed it; storage keeps the text for audit but no read path returns it.
    ByModeration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentState {
    review: Review,
    removal: Option<Removal>,
    version: ModerationVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Changed(CommentState),
    /// The requested outcome already holds; nothing is written (safe retry).
    Unchanged(CommentState),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    /// Removal is terminal: no decision brings a removed comment back.
    #[error("comment was removed")]
    Removed,
    #[error("comment changed since the expected moderation version")]
    Stale { current: ModerationVersion },
    #[error("moderation version cannot advance")]
    VersionExhausted,
}

impl CommentState {
    /// State of a freshly submitted comment.
    pub const SUBMITTED: Self = Self {
        review: Review::Pending,
        removal: None,
        version: ModerationVersion::FIRST,
    };

    /// Rebuilds state read from storage; the version was already validated.
    pub const fn restore(
        review: Review,
        removal: Option<Removal>,
        version: ModerationVersion,
    ) -> Self {
        Self {
            review,
            removal,
            version,
        }
    }

    pub const fn review(self) -> Review {
        self.review
    }

    pub const fn removal(self) -> Option<Removal> {
        self.removal
    }

    pub const fn version(self) -> ModerationVersion {
        self.version
    }

    pub const fn is_present(self) -> bool {
        self.removal.is_none()
    }

    /// Present and approved: the only state anyone but the writer may read.
    pub const fn is_public(self) -> bool {
        self.removal.is_none() && matches!(self.review, Review::Approved)
    }

    /// A moderator's approve/reject, bound to the version they reviewed. Reversing an earlier
    /// decision (approved → rejected to hide, rejected → approved on review) is allowed with the
    /// current version; a decision equal to the current review is a no-op.
    pub fn decide(
        self,
        expected: ModerationVersion,
        decision: Decision,
    ) -> Result<Transition, LifecycleError> {
        if !self.is_present() {
            return Err(LifecycleError::Removed);
        }
        if expected != self.version {
            return Err(LifecycleError::Stale {
                current: self.version,
            });
        }
        let review = match decision {
            Decision::Approve => Review::Approved,
            Decision::Reject => Review::Rejected,
        };
        if review == self.review {
            return Ok(Transition::Unchanged(self));
        }
        Ok(Transition::Changed(Self {
            review,
            removal: None,
            version: self.version.next()?,
        }))
    }

    /// The writer removes their own comment. Needs no version: the writer's intent to withdraw
    /// their words wins over any pending review. Removing an already removed comment is a no-op.
    pub fn remove_by_author(self) -> Result<Transition, LifecycleError> {
        if !self.is_present() {
            return Ok(Transition::Unchanged(self));
        }
        Ok(Transition::Changed(Self {
            removal: Some(Removal::ByAuthor),
            version: self.version.next()?,
            ..self
        }))
    }

    /// A moderator removes a comment they reviewed at `expected`.
    pub fn remove_by_moderation(
        self,
        expected: ModerationVersion,
    ) -> Result<Transition, LifecycleError> {
        if !self.is_present() {
            return Ok(Transition::Unchanged(self));
        }
        if expected != self.version {
            return Err(LifecycleError::Stale {
                current: self.version,
            });
        }
        Ok(Transition::Changed(Self {
            removal: Some(Removal::ByModeration),
            version: self.version.next()?,
            ..self
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(raw: i64) -> ModerationVersion {
        ModerationVersion::new(raw).unwrap()
    }

    fn changed(transition: Result<Transition, LifecycleError>) -> CommentState {
        match transition {
            Ok(Transition::Changed(state)) => state,
            other => panic!("expected a change, got {other:?}"),
        }
    }

    #[test]
    fn a_new_comment_is_pending_until_a_versioned_decision() {
        let state = CommentState::SUBMITTED;
        assert!(!state.is_public());
        let approved = changed(state.decide(version(1), Decision::Approve));
        assert!(approved.is_public());
        assert_eq!(approved.version(), version(2));
    }

    #[test]
    fn a_late_decision_cannot_overwrite_a_newer_one() {
        let rejected = changed(CommentState::SUBMITTED.decide(version(1), Decision::Reject));
        // A second moderator (or a retried request) acting on what they saw at version 1.
        assert_eq!(
            rejected.decide(version(1), Decision::Approve),
            Err(LifecycleError::Stale {
                current: version(2)
            })
        );
        assert_eq!(
            rejected.decide(version(2), Decision::Reject),
            Ok(Transition::Unchanged(rejected))
        );
    }

    #[test]
    fn removal_is_terminal_for_every_later_decision() {
        let approved = changed(CommentState::SUBMITTED.decide(version(1), Decision::Approve));
        for removed in [
            changed(approved.remove_by_author()),
            changed(approved.remove_by_moderation(version(2))),
        ] {
            assert!(!removed.is_present() && !removed.is_public());
            assert_eq!(removed.version(), version(3));
            for decision in [Decision::Approve, Decision::Reject] {
                for expected in [version(2), version(3)] {
                    assert_eq!(
                        removed.decide(expected, decision),
                        Err(LifecycleError::Removed)
                    );
                }
            }
            assert_eq!(
                removed.remove_by_author(),
                Ok(Transition::Unchanged(removed))
            );
            assert_eq!(
                removed.remove_by_moderation(version(1)),
                Ok(Transition::Unchanged(removed))
            );
        }
    }

    #[test]
    fn the_writer_may_remove_a_pending_comment_without_a_version() {
        let removed = changed(CommentState::SUBMITTED.remove_by_author());
        assert_eq!(removed.removal(), Some(Removal::ByAuthor));
        assert_eq!(removed.review(), Review::Pending);
    }

    #[test]
    fn moderation_removal_is_version_bound() {
        assert_eq!(
            CommentState::SUBMITTED.remove_by_moderation(version(7)),
            Err(LifecycleError::Stale {
                current: version(1)
            })
        );
    }

    #[test]
    fn every_reachable_state_keeps_the_axes_consistent() {
        // Explore all command sequences of length 4 from SUBMITTED; versions never decrease, a
        // removed state never becomes present again and only approved+present is public.
        fn explore(state: CommentState, depth: usize, seen: &mut usize) {
            *seen += 1;
            assert_eq!(
                state.is_public(),
                state.is_present() && state.review() == Review::Approved
            );
            if depth == 0 {
                return;
            }
            let expected_values = [state.version(), version(state.version().get() + 1)];
            let mut next = Vec::new();
            for expected in expected_values {
                for decision in [Decision::Approve, Decision::Reject] {
                    next.push(state.decide(expected, decision));
                }
                next.push(state.remove_by_moderation(expected));
            }
            next.push(state.remove_by_author());
            for outcome in next {
                match outcome {
                    Ok(Transition::Changed(after)) => {
                        assert!(after.version() > state.version());
                        if !state.is_present() {
                            panic!("a removed comment changed: {state:?} -> {after:?}");
                        }
                        explore(after, depth - 1, seen);
                    }
                    Ok(Transition::Unchanged(after)) => assert_eq!(after, state),
                    Err(_) => {}
                }
            }
        }
        let mut seen = 0;
        explore(CommentState::SUBMITTED, 4, &mut seen);
        assert!(
            seen > 20,
            "exploration must reach removed and decided states"
        );
    }
}
