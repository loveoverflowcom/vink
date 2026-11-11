//! Appeals (ADMIN-MODERATION-20261111, MOD-10).
//!
//! The content owner appeals one adverse decision version made by the evaluator or a moderator.
//! One appeal exists per decision version: a retry with the same or another key returns the
//! existing appeal. An admin decision is final (there is no appeal against it). While an appeal
//! is open only an admin concludes on that decision ([`super::case::CaseRefusal::AppealOpen`]).
//! Upholding or reversing records a reason and creates a new decision version through the owner
//! ([`super::WorkModeration::apply_appeal_conclusion`]); a reversal of an old revision publishes it
//! only if the author's current public intent still names it (the publication guard decides).
//!
//! No notification channel exists: an appeal is recorded, never "sent" ([`Notice::NotSent`]).

use time::OffsetDateTime;

use super::DecisionReason;
use super::authority::{AdminSeat, AuthorityRefusal, ModerationDirectory};
use super::case::{CloseReason, ReviewSubject};
use crate::domain::ids::{AppealId, PrincipalId};

/// Who made the decision under appeal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecidedBy {
    Evaluator,
    Moderator(PrincipalId),
    Admin(PrincipalId),
}

/// The owner's current decision on the subject, read in the same commit as the appeal command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentDecision {
    /// Subject with its *current* decision fence.
    pub subject: ReviewSubject,
    pub owner: PrincipalId,
    pub decided_by: Option<DecidedBy>,
    /// Rejected work revision or hidden/rejected comment: something the owner may contest.
    pub adverse: bool,
    /// The work was made private: its appeals are closed (PUB-06).
    pub withdrawn: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppealVerdict {
    Uphold,
    Reverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppealState {
    Open,
    Upheld,
    Reversed,
    Closed(CloseReason),
}

impl AppealState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Upheld => "upheld",
            Self::Reversed => "reversed",
            Self::Closed(_) => "closed",
        }
    }
}

/// Delivery truth: no channel is connected in this phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    NotSent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppealFacts {
    pub id: AppealId,
    /// Subject with the decision fence that was appealed.
    pub subject: ReviewSubject,
    pub appellant: PrincipalId,
    pub decided_by: DecidedBy,
    pub state: AppealState,
    pub notice: Notice,
}

/// All appeals of one content target (one work's revisions, or one comment).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppealBook(Vec<AppealFacts>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AppealRefusal {
    #[error("not found")]
    NotFound,
    #[error("there is no adverse decision to appeal")]
    NothingToAppeal,
    #[error("an admin decision is final")]
    FinalDecision,
    #[error("the decision changed since it was shown")]
    StaleDecision,
    #[error("the work was made private; its appeals are closed")]
    Withdrawn,
    #[error("unknown appeal")]
    UnknownAppeal,
    #[error("appeal is already resolved")]
    AlreadyResolved,
    #[error("an appeal conclusion needs a reason")]
    ReasonRequired,
    #[error("an admin cannot conclude on their own content")]
    Recused,
    #[error("stored appeal facts are inconsistent")]
    InvalidFacts,
    #[error(transparent)]
    Authority(#[from] AuthorityRefusal),
}

impl AppealRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::NothingToAppeal => "nothing_to_appeal",
            Self::FinalDecision => "final_decision",
            Self::StaleDecision => "stale_decision",
            Self::Withdrawn => "withdrawn",
            Self::UnknownAppeal => "unknown_appeal",
            Self::AlreadyResolved => "already_resolved",
            Self::ReasonRequired => "reason_required",
            Self::Recused => "recused",
            Self::InvalidFacts => "invalid_facts",
            Self::Authority(refusal) => refusal.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filed {
    /// Recorded, not delivered.
    Recorded(AppealId),
    /// The same decision version already has an appeal (any key): returned as is.
    Existing(AppealId),
}

/// An admin's final answer on an appeal, to be applied by the decision's owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppealConclusion {
    pub appeal: AppealId,
    pub subject: ReviewSubject,
    pub admin: AdminSeat,
    pub verdict: AppealVerdict,
    pub reason: DecisionReason,
}

impl AppealBook {
    pub fn restore(appeals: Vec<AppealFacts>) -> Result<Self, AppealRefusal> {
        for (i, appeal) in appeals.iter().enumerate() {
            if appeals[..i].iter().any(|a| {
                a.id == appeal.id
                    || (a.subject.same_target(&appeal.subject)
                        && a.subject.fence() == appeal.subject.fence())
            }) {
                return Err(AppealRefusal::InvalidFacts);
            }
            if matches!(appeal.decided_by, DecidedBy::Admin(_)) {
                return Err(AppealRefusal::InvalidFacts);
            }
        }
        Ok(Self(appeals))
    }

    pub fn appeals(&self) -> &[AppealFacts] {
        &self.0
    }

    pub fn get(&self, id: AppealId) -> Option<&AppealFacts> {
        self.0.iter().find(|a| a.id == id)
    }

    /// Whether an open appeal reserves the current decision of `subject` for an admin.
    pub fn open_on(&self, subject: &ReviewSubject) -> bool {
        self.0.iter().any(|a| {
            a.state == AppealState::Open
                && a.subject.same_target(subject)
                && a.subject.fence() == subject.fence()
        })
    }

    /// The owner files an appeal on the decision version they saw (`seen`).
    pub fn file(
        &self,
        actor: PrincipalId,
        current: &CurrentDecision,
        seen: &ReviewSubject,
        new_id: AppealId,
    ) -> Result<(Self, Filed), AppealRefusal> {
        if actor != current.owner || !seen.same_target(&current.subject) {
            return Err(AppealRefusal::NotFound);
        }
        if current.withdrawn {
            return Err(AppealRefusal::Withdrawn);
        }
        if let Some(existing) = self
            .0
            .iter()
            .find(|a| a.subject.same_target(seen) && a.subject.fence() == seen.fence())
        {
            return Ok((self.clone(), Filed::Existing(existing.id)));
        }
        if seen.fence() != current.subject.fence() {
            return Err(AppealRefusal::StaleDecision);
        }
        let decided_by = match current.decided_by {
            Some(DecidedBy::Admin(_)) => return Err(AppealRefusal::FinalDecision),
            Some(by) if current.adverse => by,
            _ => return Err(AppealRefusal::NothingToAppeal),
        };
        if self.0.iter().any(|a| a.id == new_id) {
            return Err(AppealRefusal::InvalidFacts);
        }
        let mut next = self.clone();
        next.0.push(AppealFacts {
            id: new_id,
            subject: current.subject.clone(),
            appellant: actor,
            decided_by,
            state: AppealState::Open,
            notice: Notice::NotSent,
        });
        Ok((next, Filed::Recorded(new_id)))
    }

    /// An admin concludes an open appeal on the decision version it names. The owner applies the
    /// conclusion under its own decision-version fence.
    #[allow(clippy::too_many_arguments)]
    pub fn conclude(
        &self,
        directory: &ModerationDirectory,
        admin: PrincipalId,
        id: AppealId,
        current: &CurrentDecision,
        verdict: AppealVerdict,
        reason: Option<DecisionReason>,
        now: OffsetDateTime,
    ) -> Result<(Self, AppealConclusion), AppealRefusal> {
        let seat = directory.require_admin(admin, now)?;
        let appeal = self.get(id).ok_or(AppealRefusal::UnknownAppeal)?;
        if appeal.state != AppealState::Open {
            return Err(AppealRefusal::AlreadyResolved);
        }
        if current.withdrawn {
            return Err(AppealRefusal::Withdrawn);
        }
        if admin == current.owner || !appeal.subject.same_target(&current.subject) {
            return Err(AppealRefusal::Recused);
        }
        if appeal.subject.fence() != current.subject.fence() {
            return Err(AppealRefusal::StaleDecision);
        }
        let reason = reason.ok_or(AppealRefusal::ReasonRequired)?;
        let mut next = self.clone();
        let slot = next
            .0
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(AppealRefusal::UnknownAppeal)?;
        slot.state = match verdict {
            AppealVerdict::Uphold => AppealState::Upheld,
            AppealVerdict::Reverse => AppealState::Reversed,
        };
        Ok((
            next,
            AppealConclusion {
                appeal: id,
                subject: appeal.subject.clone(),
                admin: seat,
                verdict,
                reason,
            },
        ))
    }

    /// Close every open appeal (the author made the work private, or the target is gone).
    pub fn close_open(&self, reason: CloseReason) -> Self {
        let mut next = self.clone();
        for appeal in &mut next.0 {
            if appeal.state == AppealState::Open {
                appeal.state = AppealState::Closed(reason);
            }
        }
        next
    }
}
