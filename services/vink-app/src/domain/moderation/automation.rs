//! Automatic moderation mode (ADMIN-MODERATION-20261111, MOD-14).
//!
//! While paused the evaluator applies nothing: new submissions go straight to the human queue,
//! open evaluation jobs are rerouted ([`super::JobState::Rerouted`]) and any result that still
//! arrives is refused ([`super::EvaluationRefusal::AutomationPaused`]). Resuming applies to new
//! submissions only; rerouted work stays with human reviewers. Pausing never stops moderators.

use time::OffsetDateTime;

use super::DecisionReason;
use super::authority::{AuthorityRefusal, ModerationDirectory};
use crate::domain::ids::PrincipalId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationMode {
    Enabled,
    Paused,
}

impl AutomationMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Paused => "paused",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AutomationVersion(i64);

impl AutomationVersion {
    pub const FIRST: Self = Self(1);

    pub fn new(raw: i64) -> Option<Self> {
        (raw >= 1).then_some(Self(raw))
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AutomationError {
    #[error("automation mode changed since the expected version")]
    Stale,
    #[error("the requested mode already holds")]
    Unchanged,
    #[error("a mode change needs a reason")]
    ReasonRequired,
    #[error("automation version cannot advance")]
    VersionExhausted,
    #[error(transparent)]
    Authority(#[from] AuthorityRefusal),
}

/// Audit row of an accepted mode change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationAudit {
    pub admin: PrincipalId,
    pub mode: AutomationMode,
    pub version: AutomationVersion,
    pub reason: DecisionReason,
    pub at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Automation {
    mode: AutomationMode,
    version: AutomationVersion,
}

impl Automation {
    pub const INITIAL: Self = Self {
        mode: AutomationMode::Enabled,
        version: AutomationVersion::FIRST,
    };

    pub const fn restore(mode: AutomationMode, version: AutomationVersion) -> Self {
        Self { mode, version }
    }

    pub const fn mode(self) -> AutomationMode {
        self.mode
    }

    pub const fn version(self) -> AutomationVersion {
        self.version
    }

    /// Admin pauses or resumes, fenced by the version they saw.
    pub fn set(
        self,
        directory: &ModerationDirectory,
        admin: PrincipalId,
        expected: AutomationVersion,
        mode: AutomationMode,
        reason: Option<DecisionReason>,
        now: OffsetDateTime,
    ) -> Result<(Self, AutomationAudit), AutomationError> {
        directory.require_admin(admin, now)?;
        if expected != self.version {
            return Err(AutomationError::Stale);
        }
        if mode == self.mode {
            return Err(AutomationError::Unchanged);
        }
        let reason = reason.ok_or(AutomationError::ReasonRequired)?;
        let version = AutomationVersion(
            self.version
                .0
                .checked_add(1)
                .ok_or(AutomationError::VersionExhausted)?,
        );
        let next = Self { mode, version };
        Ok((
            next,
            AutomationAudit {
                admin,
                mode,
                version,
                reason,
                at: now,
            },
        ))
    }
}
