//! Shared work-view policy values (`VIEW-01`, `VIEW-03`, `UX-07`, `CHOICE-POLICY`): the numbers
//! that the App's counting rule and the Web Reader and its explanation must agree on. Nothing here
//! formats text or performs I/O; copy belongs to the Web and the counting rule to the App.
//!
//! | Value | Owner of the rule | Consumers |
//! | --- | --- | --- |
//! | [`WORK_VIEW_MIN_VISIBLE_MS`] | Reader emission (browser-only) | `vink-web` timer and explanation |
//! | [`VIEW_WINDOW_HOURS`] | App counting rule (`VIEW-03`) | `vink-app` domain, `vink-web` explanation |
//!
//! The threshold decides only when the browser sends a candidate. The App never receives or trusts
//! a duration; it decides with its own rules (receipts, window, horizon, ownership).
//!
//! To change a value, update it here and rebuild the workspace.

use core::fmt;
use core::time::Duration;

/// Canonical default minimum foreground visible duration (milliseconds) before emitting one
/// work-view candidate (`VIEW-01`: 5 seconds). Single default source across the workspace.
pub const WORK_VIEW_MIN_VISIBLE_MS: u32 = 5_000;

/// Conservative upper bound (10 minutes) preventing nonsensical configuration values.
pub const MAX_MIN_VISIBLE_MS: u32 = 600_000;

/// `VIEW-03`: a reader has at most one counted view per work per sliding window of this many
/// hours. The App derives its `VIEW_WINDOW` from it; the Web explanation prints it.
pub const VIEW_WINDOW_HOURS: u32 = 24;

/// Validated foreground threshold used by the Reader timer and its explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkViewDurationPolicy {
    min_visible_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    Zero,
    TooLarge { max_ms: u32 },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => f.write_str("minimum visible threshold must be positive"),
            Self::TooLarge { max_ms } => {
                write!(f, "minimum visible threshold exceeds {max_ms} ms")
            }
        }
    }
}

impl std::error::Error for PolicyError {}

impl WorkViewDurationPolicy {
    /// The workspace default policy built from [`WORK_VIEW_MIN_VISIBLE_MS`].
    pub const DEFAULT: Self = Self {
        min_visible_ms: WORK_VIEW_MIN_VISIBLE_MS,
    };

    /// Constructs a policy with a validated non-zero duration override (used in tests and config).
    pub const fn from_millis(min_visible_ms: u32) -> Result<Self, PolicyError> {
        if min_visible_ms == 0 {
            return Err(PolicyError::Zero);
        }
        if min_visible_ms > MAX_MIN_VISIBLE_MS {
            return Err(PolicyError::TooLarge {
                max_ms: MAX_MIN_VISIBLE_MS,
            });
        }
        Ok(Self { min_visible_ms })
    }

    pub const fn min_visible_ms(self) -> u32 {
        self.min_visible_ms
    }

    pub const fn min_visible_duration(self) -> Duration {
        Duration::from_millis(self.min_visible_ms as u64)
    }
}

impl Default for WorkViewDurationPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_the_single_threshold_constant() {
        let policy = WorkViewDurationPolicy::default();
        assert_eq!(policy, WorkViewDurationPolicy::DEFAULT);
        assert_eq!(policy.min_visible_ms(), WORK_VIEW_MIN_VISIBLE_MS);
        assert_eq!(
            policy.min_visible_duration(),
            Duration::from_millis(u64::from(WORK_VIEW_MIN_VISIBLE_MS))
        );
        assert_eq!(VIEW_WINDOW_HOURS, 24);
    }

    #[test]
    fn custom_override_changes_the_threshold_and_duration_together() {
        let fractional = WorkViewDurationPolicy::from_millis(1_500).unwrap();
        assert_eq!(fractional.min_visible_ms(), 1_500);
        assert_eq!(
            fractional.min_visible_duration(),
            Duration::from_millis(1_500)
        );
    }

    #[test]
    fn rejects_zero_or_excessive_durations() {
        assert_eq!(
            WorkViewDurationPolicy::from_millis(0),
            Err(PolicyError::Zero)
        );
        assert_eq!(
            WorkViewDurationPolicy::from_millis(MAX_MIN_VISIBLE_MS + 1),
            Err(PolicyError::TooLarge {
                max_ms: MAX_MIN_VISIBLE_MS
            })
        );
        assert!(WorkViewDurationPolicy::from_millis(MAX_MIN_VISIBLE_MS).is_ok());
    }
}
