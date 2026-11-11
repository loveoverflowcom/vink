//! Review cases and claims (ADMIN-MODERATION-20261111, MOD-12).
//!
//! One case is one exact thing to review: a submitted work revision (content digest + decision
//! version) or a comment (moderation version). History keeps three levels apart: recorded
//! ([`CaseState::Queued`]), assigned ([`CaseState::Claimed`]) and concluded
//! ([`CaseState::Concluded`]). At most one claimant holds a case at a time.
//!
//! Every accepted command advances [`CaseVersion`]; a command carrying an older version is
//! [`CaseRefusal::StaleCase`], so of two moderators racing for a case, or two concurrent
//! conclusions, exactly one applies. A conclusion also fences the decision version of its
//! subject: if the owner's decision moved (an AI result, another reviewer, an appeal
//! conclusion) the case is [`CaseRefusal::StaleDecision`]. Authority is re-resolved from the
//! [`ModerationDirectory`] on every claim, note and conclusion; a claimant who lost every basis
//! cannot conclude, and [`ReviewCase::revalidate`] returns the case to the queue unless another
//! live grant still covers it.
//!
//! A [`CaseConclusion`] carries no authority by itself: the publication or discussion owner
//! still fences its own version when applying it.

use time::OffsetDateTime;

use super::authority::{
    AuthorityBasis, AuthorityRefusal, ModerationDirectory, ReviewTarget, VerifiedAuthority,
};
use super::{DecisionReason, DecisionVersion};
use crate::domain::discussion::lifecycle::ModerationVersion;
use crate::domain::ids::{CaseId, CommentId, PrincipalId, RevisionId, WorkId};
use crate::domain::revision::ContentHash;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CaseVersion(i64);

impl CaseVersion {
    pub const FIRST: Self = Self(1);

    pub fn new(raw: i64) -> Option<Self> {
        (raw >= 1).then_some(Self(raw))
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    fn next(self) -> Result<Self, CaseRefusal> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(CaseRefusal::VersionExhausted)
    }
}

/// The exact subject of a case together with the decision fence it was opened on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewSubject {
    WorkRevision {
        work: WorkId,
        revision: RevisionId,
        digest: ContentHash,
        decision: DecisionVersion,
    },
    Comment {
        work: WorkId,
        comment: CommentId,
        decision: ModerationVersion,
    },
}

impl ReviewSubject {
    pub const fn work(&self) -> WorkId {
        match self {
            Self::WorkRevision { work, .. } | Self::Comment { work, .. } => *work,
        }
    }

    /// Same target, ignoring the decision fence.
    pub fn same_target(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::WorkRevision {
                    work: w1,
                    revision: r1,
                    digest: d1,
                    ..
                },
                Self::WorkRevision {
                    work: w2,
                    revision: r2,
                    digest: d2,
                    ..
                },
            ) => w1 == w2 && r1 == r2 && d1 == d2,
            (
                Self::Comment {
                    work: w1,
                    comment: c1,
                    ..
                },
                Self::Comment {
                    work: w2,
                    comment: c2,
                    ..
                },
            ) => w1 == w2 && c1 == c2,
            _ => false,
        }
    }

    /// Decision fence of the owner as a plain number for comparison within one subject kind.
    pub const fn fence(&self) -> i64 {
        match self {
            Self::WorkRevision { decision, .. } => decision.get(),
            Self::Comment { decision, .. } => decision.get(),
        }
    }
}

/// Why a case exists (audit; not authority).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseOrigin {
    EvaluatorUncertain,
    InvalidOutput,
    RetriesExhausted,
    AutomationPaused,
    PolicyRereview,
    ReviewerIntervention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    /// The author made the work private (PUB-06): its cases and appeals close.
    Withdrawn,
    /// The work was deleted or removed, or the comment removed.
    TargetGone,
    /// The decision moved elsewhere (an appeal conclusion or another case concluded it).
    Superseded,
}

impl CloseReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Withdrawn => "withdrawn",
            Self::TargetGone => "target_gone",
            Self::Superseded => "superseded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseVerdict {
    Approve,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseState {
    /// Recorded, nobody holds it.
    Queued,
    /// Assigned to exactly one reviewer, with the basis checked when it was taken or re-checked.
    Claimed {
        by: PrincipalId,
        basis: AuthorityBasis,
    },
    /// Handed to the admins; only an admin takes it.
    Escalated,
    Concluded {
        by: PrincipalId,
        basis: AuthorityBasis,
        verdict: CaseVerdict,
    },
    Closed(CloseReason),
}

impl CaseState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Claimed { .. } => "claimed",
            Self::Escalated => "escalated",
            Self::Concluded { .. } => "concluded",
            Self::Closed(_) => "closed",
        }
    }

    const fn is_open(self) -> bool {
        !matches!(self, Self::Concluded { .. } | Self::Closed(_))
    }
}

/// Validated internal note, never sent to the writer (separate from [`DecisionReason`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalNote(String);

impl InternalNote {
    pub const MAX_CHARS: usize = 2000;

    pub fn parse(raw: &str) -> Result<Self, CaseRefusal> {
        let text = raw.trim();
        if text.is_empty()
            || text.chars().count() > Self::MAX_CHARS
            || text.chars().any(|c| c.is_control() && c != '\n')
        {
            return Err(CaseRefusal::NoteRequired);
        }
        Ok(Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteFacts {
    pub author: PrincipalId,
    pub note: InternalNote,
}

/// Untrusted row-loading shape; validated by [`ReviewCase::restore`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseFacts {
    pub id: CaseId,
    pub subject: ReviewSubject,
    pub target: ReviewTarget,
    pub origin: CaseOrigin,
    pub version: CaseVersion,
    pub state: CaseState,
    pub notes: Vec<NoteFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCase(CaseFacts);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CaseRefusal {
    #[error("case changed since the expected version")]
    StaleCase,
    #[error("the decision changed since the case was opened")]
    StaleDecision,
    #[error("case is concluded or closed")]
    CaseClosed,
    #[error("case is held by someone else")]
    NotClaimant,
    #[error("case is already claimed")]
    AlreadyClaimed,
    #[error("an escalated case is taken only by an admin")]
    Escalated,
    #[error("an open appeal reserves this decision for an admin")]
    AppealOpen,
    #[error("a takeover needs a reason")]
    ReasonRequired,
    #[error("a note must be non-empty plain text")]
    NoteRequired,
    #[error("case subject does not match")]
    WrongSubject,
    #[error("case version cannot advance")]
    VersionExhausted,
    #[error("stored case facts are inconsistent")]
    InvalidFacts,
    #[error(transparent)]
    Authority(#[from] AuthorityRefusal),
}

impl CaseRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaleCase => "stale_case",
            Self::StaleDecision => "stale_decision",
            Self::CaseClosed => "case_closed",
            Self::NotClaimant => "not_claimant",
            Self::AlreadyClaimed => "already_claimed",
            Self::Escalated => "escalated_to_admin",
            Self::AppealOpen => "appeal_open",
            Self::ReasonRequired => "reason_required",
            Self::NoteRequired => "note_required",
            Self::WrongSubject => "wrong_subject",
            Self::VersionExhausted => "version_exhausted",
            Self::InvalidFacts => "invalid_facts",
            Self::Authority(refusal) => refusal.as_str(),
        }
    }
}

/// What happened to a case (history row: recorded / assigned / concluded are distinct).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseEvent {
    Claimed,
    Released,
    Noted,
    Escalated,
    TakenOver,
    AuthorityKept,
    AuthorityLost,
    Concluded,
    Closed(CloseReason),
}

impl CaseEvent {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claimed => "claimed",
            Self::Released => "released",
            Self::Noted => "noted",
            Self::Escalated => "escalated",
            Self::TakenOver => "taken_over",
            Self::AuthorityKept => "authority_kept",
            Self::AuthorityLost => "authority_lost",
            Self::Concluded => "concluded",
            Self::Closed(_) => "closed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseTransition {
    pub next: ReviewCase,
    pub event: CaseEvent,
}

/// The reviewer's verdict on the exact subject, checked against the case and decision versions.
/// The owner applies it under its own version fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseConclusion {
    pub case: CaseId,
    pub subject: ReviewSubject,
    pub authority: VerifiedAuthority,
    pub verdict: CaseVerdict,
    pub reason: Option<DecisionReason>,
}

impl ReviewCase {
    /// Open a recorded case for the exact subject.
    pub fn open(
        id: CaseId,
        subject: ReviewSubject,
        target: ReviewTarget,
        origin: CaseOrigin,
    ) -> Self {
        Self(CaseFacts {
            id,
            subject,
            target,
            origin,
            version: CaseVersion::FIRST,
            state: CaseState::Queued,
            notes: vec![],
        })
    }

    pub fn restore(facts: CaseFacts) -> Result<Self, CaseRefusal> {
        if facts.subject.work() != facts.target.work {
            return Err(CaseRefusal::InvalidFacts);
        }
        let kind_matches = matches!(
            (&facts.subject, facts.target.capability),
            (
                ReviewSubject::WorkRevision { .. },
                super::authority::ReviewCapability::Works
            ) | (
                ReviewSubject::Comment { .. },
                super::authority::ReviewCapability::Comments
            )
        );
        if !kind_matches {
            return Err(CaseRefusal::InvalidFacts);
        }
        if facts.version == CaseVersion::FIRST && facts.state != CaseState::Queued {
            return Err(CaseRefusal::InvalidFacts);
        }
        Ok(Self(facts))
    }

    pub fn facts(&self) -> &CaseFacts {
        &self.0
    }

    pub const fn state(&self) -> CaseState {
        self.0.state
    }

    pub const fn version(&self) -> CaseVersion {
        self.0.version
    }

    fn fenced(&self, expected: CaseVersion) -> Result<(), CaseRefusal> {
        if !self.0.state.is_open() {
            return Err(CaseRefusal::CaseClosed);
        }
        if expected != self.0.version {
            return Err(CaseRefusal::StaleCase);
        }
        Ok(())
    }

    fn advance(&self, state: CaseState, event: CaseEvent) -> Result<CaseTransition, CaseRefusal> {
        let mut next = self.clone();
        next.0.version = self.0.version.next()?;
        next.0.state = state;
        Ok(CaseTransition { next, event })
    }

    /// Take a queued case. An escalated case is taken only through [`Self::take_over`].
    pub fn claim(
        &self,
        directory: &ModerationDirectory,
        actor: PrincipalId,
        expected: CaseVersion,
        now: OffsetDateTime,
    ) -> Result<CaseTransition, CaseRefusal> {
        self.fenced(expected)?;
        match self.0.state {
            CaseState::Queued => {}
            CaseState::Escalated => return Err(CaseRefusal::Escalated),
            _ => return Err(CaseRefusal::AlreadyClaimed),
        }
        let authority = directory.resolve(actor, &self.0.target, now)?;
        self.advance(
            CaseState::Claimed {
                by: actor,
                basis: authority.basis(),
            },
            CaseEvent::Claimed,
        )
    }

    pub fn release(
        &self,
        actor: PrincipalId,
        expected: CaseVersion,
    ) -> Result<CaseTransition, CaseRefusal> {
        self.fenced(expected)?;
        match self.0.state {
            CaseState::Claimed { by, .. } if by == actor => {
                self.advance(CaseState::Queued, CaseEvent::Released)
            }
            _ => Err(CaseRefusal::NotClaimant),
        }
    }

    /// The claimant (or an admin) adds an internal note. Never part of the writer's reason.
    pub fn note(
        &self,
        directory: &ModerationDirectory,
        actor: PrincipalId,
        expected: CaseVersion,
        note: InternalNote,
        now: OffsetDateTime,
    ) -> Result<CaseTransition, CaseRefusal> {
        self.fenced(expected)?;
        let authority = directory.resolve(actor, &self.0.target, now)?;
        let holds = matches!(self.0.state, CaseState::Claimed { by, .. } if by == actor);
        if !holds && !authority.is_admin() {
            return Err(CaseRefusal::NotClaimant);
        }
        let mut transition = self.advance(self.0.state, CaseEvent::Noted)?;
        transition.next.0.notes.push(NoteFacts {
            author: actor,
            note,
        });
        Ok(transition)
    }

    /// The claimant hands the case to the admins.
    pub fn escalate(
        &self,
        actor: PrincipalId,
        expected: CaseVersion,
    ) -> Result<CaseTransition, CaseRefusal> {
        self.fenced(expected)?;
        match self.0.state {
            CaseState::Claimed { by, .. } if by == actor => {
                self.advance(CaseState::Escalated, CaseEvent::Escalated)
            }
            _ => Err(CaseRefusal::NotClaimant),
        }
    }

    /// An admin takes the case from the queue, the escalation or another claimant, with a reason.
    pub fn take_over(
        &self,
        directory: &ModerationDirectory,
        admin: PrincipalId,
        expected: CaseVersion,
        reason: Option<&DecisionReason>,
        now: OffsetDateTime,
    ) -> Result<CaseTransition, CaseRefusal> {
        self.fenced(expected)?;
        if reason.is_none() {
            return Err(CaseRefusal::ReasonRequired);
        }
        directory.require_admin(admin, now)?;
        let authority = directory.resolve(admin, &self.0.target, now)?;
        if matches!(self.0.state, CaseState::Claimed { by, .. } if by == admin) {
            return Err(CaseRefusal::AlreadyClaimed);
        }
        self.advance(
            CaseState::Claimed {
                by: admin,
                basis: authority.basis(),
            },
            CaseEvent::TakenOver,
        )
    }

    /// Re-check the claimant's authority (after a revocation, expiry, group change or
    /// suspension). Lost every basis → back to the queue; another live basis → kept on it.
    pub fn revalidate(
        &self,
        directory: &ModerationDirectory,
        now: OffsetDateTime,
    ) -> Result<CaseTransition, CaseRefusal> {
        let CaseState::Claimed { by, basis } = self.0.state else {
            return Err(CaseRefusal::NotClaimant);
        };
        match directory.resolve_preferring(by, &self.0.target, now, Some(basis)) {
            Ok(authority) if authority.basis() == basis => Ok(CaseTransition {
                next: self.clone(),
                event: CaseEvent::AuthorityKept,
            }),
            Ok(authority) => self.advance(
                CaseState::Claimed {
                    by,
                    basis: authority.basis(),
                },
                CaseEvent::AuthorityKept,
            ),
            Err(_) => self.advance(CaseState::Queued, CaseEvent::AuthorityLost),
        }
    }

    /// Conclude the case. `current` is the owner's subject with its current decision fence,
    /// read in the same commit; `appeal_open` reserves the decision for an admin (MOD-10).
    #[allow(clippy::too_many_arguments)]
    pub fn conclude(
        &self,
        directory: &ModerationDirectory,
        actor: PrincipalId,
        expected: CaseVersion,
        current: &ReviewSubject,
        verdict: CaseVerdict,
        reason: Option<DecisionReason>,
        appeal_open: bool,
        now: OffsetDateTime,
    ) -> Result<(CaseTransition, CaseConclusion), CaseRefusal> {
        self.fenced(expected)?;
        let CaseState::Claimed { by, basis } = self.0.state else {
            return Err(CaseRefusal::NotClaimant);
        };
        if by != actor {
            return Err(CaseRefusal::NotClaimant);
        }
        let authority = directory.resolve_preferring(actor, &self.0.target, now, Some(basis))?;
        if !self.0.subject.same_target(current) {
            return Err(CaseRefusal::WrongSubject);
        }
        if current.fence() != self.0.subject.fence() {
            return Err(CaseRefusal::StaleDecision);
        }
        if appeal_open && !authority.is_admin() {
            return Err(CaseRefusal::AppealOpen);
        }
        if verdict == CaseVerdict::Reject && reason.is_none() {
            return Err(CaseRefusal::ReasonRequired);
        }
        let transition = self.advance(
            CaseState::Concluded {
                by: actor,
                basis: authority.basis(),
                verdict,
            },
            CaseEvent::Concluded,
        )?;
        Ok((
            transition,
            CaseConclusion {
                case: self.0.id,
                subject: self.0.subject.clone(),
                authority,
                verdict,
                reason,
            },
        ))
    }

    /// System close (private work, deleted/removed target, superseded decision). Idempotent on a
    /// case that is already concluded or closed.
    pub fn close(&self, reason: CloseReason) -> Result<CaseTransition, CaseRefusal> {
        if !self.0.state.is_open() {
            return Err(CaseRefusal::CaseClosed);
        }
        self.advance(CaseState::Closed(reason), CaseEvent::Closed(reason))
    }
}
