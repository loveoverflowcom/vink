//! Shared work-view threshold policy (`VIEW-01`, `UX-07`, `CHOICE-POLICY`).
//!
//! This crate owns the single source of truth for the minimum foreground display duration
//! required before the browser Reader emits a work-view candidate (`WORK_VIEW_MIN_VISIBLE_MS`).
//! Both `vink-app` and `vink-web` (CSR standalone and SSR/hydrate) consume this type and constant
//! so the Reader timer, author explanation contract (`UX-07`), and verification share one value.
//!
//! To change the default threshold later, update [`WORK_VIEW_MIN_VISIBLE_MS`] here and rebuild the
//! workspace.

use core::fmt;
use core::time::Duration;

/// Canonical default minimum foreground visible duration (milliseconds) before emitting one
/// work-view candidate (`VIEW-01`: 5 seconds). Single default source across the workspace.
pub const WORK_VIEW_MIN_VISIBLE_MS: u32 = 5_000;

/// Conservative upper bound (10 minutes) preventing nonsensical configuration values.
pub const MAX_MIN_VISIBLE_MS: u32 = 600_000;

/// Validated view threshold policy used by the Reader timer and author explanation contract.
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

    /// Formats the threshold for Vietnamese author-facing copy (`5 giây`, `1,5 giây`, `250 ms`).
    pub fn threshold_label_vi(self) -> String {
        if self.min_visible_ms < 1_000 {
            return format!("{} ms", self.min_visible_ms);
        }
        let whole = self.min_visible_ms / 1_000;
        let rem = self.min_visible_ms % 1_000;
        if rem == 0 {
            return format!("{whole} giây");
        }
        let frac = format!("{rem:03}");
        let trimmed = frac.trim_end_matches('0');
        format!("{whole},{trimmed} giây")
    }

    /// Canonical `UX-07` coverage explanation driven dynamically by this policy's threshold.
    pub fn ux_explanation_vi(self) -> String {
        format!(
            "Chỉ tính khi bài hiển thị tối thiểu {} ở tab đang xem; mỗi người tối đa một lượt trong 24 giờ. Khách được tính theo trình duyệt; lượt bạn tự xem khi đăng nhập và trình duyệt tắt JavaScript không được tính.",
            self.threshold_label_vi()
        )
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
    fn default_policy_matches_single_constant_and_formats_vietnamese_copy() {
        let policy = WorkViewDurationPolicy::default();
        assert_eq!(policy.min_visible_ms(), WORK_VIEW_MIN_VISIBLE_MS);
        assert_eq!(
            policy.min_visible_duration(),
            Duration::from_millis(u64::from(WORK_VIEW_MIN_VISIBLE_MS))
        );
        assert_eq!(policy.threshold_label_vi(), "5 giây");
        let explanation = policy.ux_explanation_vi();
        assert!(explanation.contains("5 giây"), "{explanation}");
        assert!(explanation.contains("24 giờ"), "{explanation}");
        assert!(explanation.contains("trình duyệt"), "{explanation}");
        assert!(explanation.contains("tự xem"), "{explanation}");
        assert!(explanation.contains("JavaScript"), "{explanation}");
    }

    #[test]
    fn custom_override_changes_threshold_duration_and_explanation_together() {
        let fractional = WorkViewDurationPolicy::from_millis(1_500).unwrap();
        assert_eq!(fractional.min_visible_ms(), 1_500);
        assert_eq!(fractional.threshold_label_vi(), "1,5 giây");
        let text = fractional.ux_explanation_vi();
        assert!(text.contains("tối thiểu 1,5 giây"), "{text}");
        assert!(!text.contains("tối thiểu 5 giây"), "{text}");

        let subsecond = WorkViewDurationPolicy::from_millis(250).unwrap();
        assert_eq!(subsecond.threshold_label_vi(), "250 ms");
        assert!(subsecond.ux_explanation_vi().contains("250 ms"));
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
