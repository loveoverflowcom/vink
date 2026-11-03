//! Per-work comments mode (COM-07, D-13). The work's author opens or closes comments for that
//! work; this is independent of system moderation and never edits or hides existing comments.
//!
//! Closing stops new roots and replies at the commit boundary. Approved comments stay readable
//! and reopening restores writing without data changes (default meaning recorded in the ownership
//! ADR, pending product confirmation). A new public work starts open; comments remain premoderated.

/// Monotonic version of a thread's settings, bumped only by an actual mode change.
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

/// Result of asking for a mode. Desired-state semantics: asking for the current mode is a safe
/// no-op, so a retry after a lost response cannot fail or flip the mode back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeChange {
    Changed(DiscussionSettings),
    Unchanged(DiscussionSettings),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    /// Another change landed since the caller's view; a stale tab must not undo it.
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

    pub fn change_mode(
        self,
        expected: SettingsGeneration,
        mode: CommentsMode,
    ) -> Result<ModeChange, SettingsError> {
        if mode == self.mode {
            return Ok(ModeChange::Unchanged(self));
        }
        if expected != self.generation {
            return Err(SettingsError::Stale { current: self });
        }
        let generation = self
            .generation
            .next()
            .ok_or(SettingsError::GenerationExhausted)?;
        Ok(ModeChange::Changed(Self { mode, generation }))
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
    fn a_retry_of_the_current_mode_is_a_no_op_even_with_an_old_generation() {
        let closed = DiscussionSettings::restore(CommentsMode::Closed, generation(2));
        assert_eq!(
            closed.change_mode(generation(1), CommentsMode::Closed),
            Ok(ModeChange::Unchanged(closed))
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
        assert_eq!(
            last.change_mode(generation(i64::MAX), CommentsMode::Closed),
            Err(SettingsError::GenerationExhausted)
        );
    }
}
