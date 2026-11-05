//! Per-work comments mode (COM-07, D-13). The work's author opens or closes comments for that
//! work; this is independent of system moderation and never edits or hides existing comments.
//!
//! Closing stops new roots and replies at the commit boundary. Approved comments stay readable
//! and reopening restores writing without data changes. A new public work starts open; comments
//! remain premoderated. Both meanings were confirmed by the product owner on 03/11/2026.
//!
//! **Accepted-intent generation** (owner decision 05/11/2026, the same rule as literary
//! endorsement `lve-core-v1` CHOICE-03): a request names the desired mode and the generation the
//! author saw. A request carrying the current generation is accepted and advances it by one,
//! **even when it asks for the mode already set** ([`ModeChange::Reaffirmed`]). That advance is
//! the fence COM-07 asks for: an older request still in flight at the same generation can never
//! be applied afterwards, so it cannot undo the author's newer choice. A request at another
//! generation writes nothing: [`ModeChange::AlreadyCurrent`] when the mode already matches (a
//! retry or duplicate), [`SettingsError::Stale`] otherwise.
//!
//! A raw number is not a generation and the mode is not a writable field:
//! ```compile_fail
//! use vink_app::domain::discussion::settings::SettingsGeneration;
//! let generation = SettingsGeneration(0);
//! ```
//! ```compile_fail
//! use vink_app::domain::discussion::settings::{CommentsMode, DiscussionSettings};
//! let mut settings = DiscussionSettings::INITIAL;
//! settings.mode = CommentsMode::Closed;
//! ```
//! Lawful twin (stable rustdoc does not check `compile_fail` error codes):
//! ```
//! use vink_app::domain::discussion::settings::{
//!     CommentsMode, DiscussionSettings, ModeChange, SettingsGeneration,
//! };
//! assert_eq!(SettingsGeneration::new(0), None);
//! let settings = DiscussionSettings::INITIAL;
//! let Ok(ModeChange::Changed(closed)) =
//!     settings.change_mode(SettingsGeneration::FIRST, CommentsMode::Closed)
//! else {
//!     panic!("the author closes comments at the current generation");
//! };
//! assert_eq!(closed.mode(), CommentsMode::Closed);
//! ```

/// Monotonic version of a thread's settings, bumped by every accepted request (a reaffirmation
/// too); it never moves backwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SettingsGeneration(i64);

impl SettingsGeneration {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentsMode {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscussionSettings {
    mode: CommentsMode,
    generation: SettingsGeneration,
}

/// Result of asking for a mode. Desired-state semantics: asking for the mode already set never
/// fails or flips it, so a retry after a lost response is safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeChange {
    /// Accepted and the mode changes: store these settings.
    Changed(DiscussionSettings),
    /// Accepted; the mode already held. Store these settings: only the generation advanced,
    /// which fences every older request.
    Reaffirmed(DiscussionSettings),
    /// Not accepted: the stored mode already satisfies the request at a generation other than
    /// the caller's (a retry or duplicate). Nothing is written and nothing is fenced.
    AlreadyCurrent(DiscussionSettings),
}

impl ModeChange {
    /// The settings to store, when the request was accepted.
    pub const fn write(self) -> Option<DiscussionSettings> {
        match self {
            Self::Changed(next) | Self::Reaffirmed(next) => Some(next),
            Self::AlreadyCurrent(_) => None,
        }
    }

    /// The settings the author now has, written or not.
    pub const fn settings(self) -> DiscussionSettings {
        match self {
            Self::Changed(settings)
            | Self::Reaffirmed(settings)
            | Self::AlreadyCurrent(settings) => settings,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    /// Another request was accepted since the caller's view and set another mode; a stale tab
    /// must not undo it.
    #[error("comments mode changed since the expected generation")]
    Stale { current: DiscussionSettings },
    #[error("settings generation cannot advance")]
    GenerationExhausted,
}

impl DiscussionSettings {
    /// Settings of a work whose thread has never been configured.
    pub const INITIAL: Self = Self {
        mode: CommentsMode::Open,
        generation: SettingsGeneration::FIRST,
    };

    /// Rebuilds settings read from storage; the generation was already validated.
    pub const fn restore(mode: CommentsMode, generation: SettingsGeneration) -> Self {
        Self { mode, generation }
    }

    pub const fn mode(self) -> CommentsMode {
        self.mode
    }

    pub const fn generation(self) -> SettingsGeneration {
        self.generation
    }

    pub const fn accepts_new_comments(self) -> bool {
        matches!(self.mode, CommentsMode::Open)
    }

    /// Desired mode bound to the generation the author saw. The generation is compared first: a
    /// request at the current generation is accepted whatever it asks for.
    pub fn change_mode(
        self,
        expected: SettingsGeneration,
        mode: CommentsMode,
    ) -> Result<ModeChange, SettingsError> {
        if expected == self.generation {
            let generation = self
                .generation
                .next()
                .ok_or(SettingsError::GenerationExhausted)?;
            let next = Self { mode, generation };
            return Ok(if mode == self.mode {
                ModeChange::Reaffirmed(next)
            } else {
                ModeChange::Changed(next)
            });
        }
        if mode == self.mode {
            Ok(ModeChange::AlreadyCurrent(self))
        } else {
            Err(SettingsError::Stale { current: self })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generation(raw: i64) -> SettingsGeneration {
        SettingsGeneration::new(raw).unwrap()
    }

    #[test]
    fn closing_and_reopening_advance_the_generation() {
        let closed =
            match DiscussionSettings::INITIAL.change_mode(generation(1), CommentsMode::Closed) {
                Ok(ModeChange::Changed(settings)) => settings,
                other => panic!("{other:?}"),
            };
        assert_eq!(closed.mode(), CommentsMode::Closed);
        assert_eq!(closed.generation(), generation(2));
        assert!(!closed.accepts_new_comments());
        let reopened = closed.change_mode(generation(2), CommentsMode::Open);
        assert_eq!(
            reopened,
            Ok(ModeChange::Changed(DiscussionSettings::restore(
                CommentsMode::Open,
                generation(3)
            )))
        );
    }

    #[test]
    fn a_retry_of_the_current_mode_at_an_old_generation_writes_nothing() {
        let closed = DiscussionSettings::restore(CommentsMode::Closed, generation(2));
        for expected in [1, 3, 9] {
            let decided = closed.change_mode(generation(expected), CommentsMode::Closed);
            assert_eq!(decided, Ok(ModeChange::AlreadyCurrent(closed)));
            assert_eq!(decided.unwrap().write(), None);
        }
    }

    #[test]
    fn asking_for_the_current_mode_at_its_generation_is_accepted_and_fences() {
        let closed = DiscussionSettings::restore(CommentsMode::Closed, generation(2));
        let reaffirmed = closed.change_mode(generation(2), CommentsMode::Closed);
        let next = DiscussionSettings::restore(CommentsMode::Closed, generation(3));
        assert_eq!(reaffirmed, Ok(ModeChange::Reaffirmed(next)));
        assert_eq!(reaffirmed.unwrap().write(), Some(next));
        assert!(!next.accepts_new_comments());
    }

    #[test]
    fn a_delayed_request_cannot_undo_a_newer_reaffirmation() {
        // Closed@1. Tab A asks Open@1 and is delayed; tab B asks Closed@1 later and arrives first.
        // B's reaffirmation advances the generation, so A's late request is stale. If a request
        // for the current mode were a no-op, A would match generation 1 and reopen comments.
        let closed = DiscussionSettings::restore(CommentsMode::Closed, generation(1));
        let Ok(ModeChange::Reaffirmed(after_b)) =
            closed.change_mode(generation(1), CommentsMode::Closed)
        else {
            panic!("B carries the current generation");
        };
        assert_eq!(
            after_b.change_mode(generation(1), CommentsMode::Open),
            Err(SettingsError::Stale { current: after_b })
        );
    }

    #[test]
    fn a_stale_tab_cannot_undo_a_newer_choice() {
        // Tab A saw Open@1 and closed it (now Closed@2); tab B reopened it (Open@3).
        let reopened = DiscussionSettings::restore(CommentsMode::Open, generation(3));
        assert_eq!(
            reopened.change_mode(generation(1), CommentsMode::Closed),
            Err(SettingsError::Stale { current: reopened })
        );
    }

    #[test]
    fn generations_are_positive_and_never_wrap() {
        assert_eq!(SettingsGeneration::new(0), None);
        assert_eq!(SettingsGeneration::new(-1), None);
        let last = DiscussionSettings::restore(CommentsMode::Open, generation(i64::MAX));
        for mode in [CommentsMode::Closed, CommentsMode::Open] {
            assert_eq!(
                last.change_mode(generation(i64::MAX), mode),
                Err(SettingsError::GenerationExhausted)
            );
        }
        assert_eq!(
            last.change_mode(generation(1), CommentsMode::Open),
            Ok(ModeChange::AlreadyCurrent(last))
        );
    }
}
