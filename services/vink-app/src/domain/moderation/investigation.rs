//! Private investigation of a once-public revision (ADMIN-MODERATION-20261111, MOD-15).
//!
//! An admin who holds a live seat and is not the work's author opens a file with a concrete
//! violation basis, an evidence source and the exact ID of a revision that was once readable by
//! others, on a work its author has since made private. Every read re-checks the seat, the
//! conflict, the file's state and expiry, the work's condition and the exact revision, and
//! produces the audit row *before* the snapshot: an adapter must commit the audit first and
//! return the snapshot only after. A file records its conclusion and basis, never a copy of the
//! text. The right ends when the file is closed or a condition lapses (expiry, the work became
//! public again, was deleted or removed). Default: per-read authority; no bulk export, no draft,
//! no never-public revision.

use time::OffsetDateTime;

use super::authority::{AuthorityRefusal, ModerationDirectory};
use crate::domain::ids::{InvestigationId, PrincipalId, RevisionId, WorkId};
use crate::domain::publication::{Audience, Publication, Snapshot};

fn bounded_text(raw: &str, max: usize) -> Option<String> {
    let text = raw.trim();
    (!text.is_empty()
        && text.chars().count() <= max
        && !text.chars().any(|c| c.is_control() && c != '\n'))
    .then(|| text.to_owned())
}

/// The concrete violation the admin suspects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViolationBasis(String);

impl ViolationBasis {
    pub fn parse(raw: &str) -> Result<Self, InvestigationRefusal> {
        bounded_text(raw, 500)
            .map(Self)
            .ok_or(InvestigationRefusal::BasisRequired)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where the evidence came from (a report, a case, an external notice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceSource(String);

impl EvidenceSource {
    pub fn parse(raw: &str) -> Result<Self, InvestigationRefusal> {
        bounded_text(raw, 500)
            .map(Self)
            .ok_or(InvestigationRefusal::EvidenceRequired)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finding {
    Violation,
    NoViolation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvestigationState {
    Open,
    Closed {
        finding: Finding,
        basis: ViolationBasis,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigationFacts {
    pub id: InvestigationId,
    pub admin: PrincipalId,
    pub work: WorkId,
    pub revision: RevisionId,
    pub basis: ViolationBasis,
    pub evidence: EvidenceSource,
    pub expires_at: Option<OffsetDateTime>,
    pub state: InvestigationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvestigationRefusal {
    #[error("a concrete violation basis is required")]
    BasisRequired,
    #[error("an evidence source is required")]
    EvidenceRequired,
    #[error("the admin is the work's author")]
    Conflict,
    #[error("the work is not a private work that was once public")]
    ConditionLapsed,
    #[error("the revision was never public")]
    NeverPublic,
    #[error("the read names another revision than the file")]
    WrongRevision,
    #[error("only the admin who opened the file reads through it")]
    NotInvestigator,
    #[error("the file is closed")]
    Closed,
    #[error("the file expired")]
    Expired,
    #[error("the file belongs to another work")]
    WrongWork,
    #[error(transparent)]
    Authority(#[from] AuthorityRefusal),
}

impl InvestigationRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BasisRequired => "basis_required",
            Self::EvidenceRequired => "evidence_required",
            Self::Conflict => "conflict",
            Self::ConditionLapsed => "condition_lapsed",
            Self::NeverPublic => "never_public",
            Self::WrongRevision => "wrong_revision",
            Self::NotInvestigator => "not_investigator",
            Self::Closed => "closed",
            Self::Expired => "expired",
            Self::WrongWork => "wrong_work",
            Self::Authority(refusal) => refusal.as_str(),
        }
    }
}

/// Audit row that must be committed before the snapshot is returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadAudit {
    pub investigation: InvestigationId,
    pub admin: PrincipalId,
    pub work: WorkId,
    pub revision: RevisionId,
    pub at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigationRead<'a> {
    pub audit: ReadAudit,
    pub snapshot: &'a Snapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Investigation(InvestigationFacts);

/// The work is active, private, not readable and `revision` was once public.
fn eligible(publication: &Publication, revision: RevisionId) -> Result<(), InvestigationRefusal> {
    let facts = publication.facts();
    if !facts.lifecycle.is_active()
        || facts.audience != Audience::Private
        || publication.readable_revision().is_some()
    {
        return Err(InvestigationRefusal::ConditionLapsed);
    }
    if !facts.ever_public.contains(&revision) {
        return Err(InvestigationRefusal::NeverPublic);
    }
    Ok(())
}

impl Investigation {
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        directory: &ModerationDirectory,
        admin: PrincipalId,
        publication: &Publication,
        id: InvestigationId,
        revision: RevisionId,
        basis: ViolationBasis,
        evidence: EvidenceSource,
        expires_at: Option<OffsetDateTime>,
        now: OffsetDateTime,
    ) -> Result<Self, InvestigationRefusal> {
        directory.require_admin(admin, now)?;
        if admin == publication.facts().owner {
            return Err(InvestigationRefusal::Conflict);
        }
        eligible(publication, revision)?;
        if expires_at.is_some_and(|at| at <= now) {
            return Err(InvestigationRefusal::Expired);
        }
        Ok(Self(InvestigationFacts {
            id,
            admin,
            work: publication.facts().work,
            revision,
            basis,
            evidence,
            expires_at,
            state: InvestigationState::Open,
        }))
    }

    pub fn restore(facts: InvestigationFacts) -> Self {
        Self(facts)
    }

    pub fn facts(&self) -> &InvestigationFacts {
        &self.0
    }

    fn live(&self, now: OffsetDateTime) -> Result<(), InvestigationRefusal> {
        if !matches!(self.0.state, InvestigationState::Open) {
            return Err(InvestigationRefusal::Closed);
        }
        if self.0.expires_at.is_some_and(|at| now >= at) {
            return Err(InvestigationRefusal::Expired);
        }
        Ok(())
    }

    /// One read: re-check everything, then hand back the audit row and the exact snapshot.
    pub fn read<'a>(
        &self,
        directory: &ModerationDirectory,
        admin: PrincipalId,
        publication: &'a Publication,
        revision: RevisionId,
        now: OffsetDateTime,
    ) -> Result<InvestigationRead<'a>, InvestigationRefusal> {
        self.live(now)?;
        if admin != self.0.admin {
            return Err(InvestigationRefusal::NotInvestigator);
        }
        directory.require_admin(admin, now)?;
        if publication.facts().work != self.0.work {
            return Err(InvestigationRefusal::WrongWork);
        }
        if admin == publication.facts().owner {
            return Err(InvestigationRefusal::Conflict);
        }
        if revision != self.0.revision {
            return Err(InvestigationRefusal::WrongRevision);
        }
        eligible(publication, revision)?;
        let snapshot = publication
            .facts()
            .revisions
            .iter()
            .find(|r| r.snapshot.revision == revision)
            .map(|r| &r.snapshot)
            .ok_or(InvestigationRefusal::NeverPublic)?;
        Ok(InvestigationRead {
            audit: ReadAudit {
                investigation: self.0.id,
                admin,
                work: self.0.work,
                revision,
                at: now,
            },
            snapshot,
        })
    }

    /// The opening admin closes the file with a finding and its basis (no copy of the text).
    pub fn close(
        &self,
        directory: &ModerationDirectory,
        admin: PrincipalId,
        finding: Finding,
        basis: ViolationBasis,
        now: OffsetDateTime,
    ) -> Result<Self, InvestigationRefusal> {
        if !matches!(self.0.state, InvestigationState::Open) {
            return Err(InvestigationRefusal::Closed);
        }
        if admin != self.0.admin {
            return Err(InvestigationRefusal::NotInvestigator);
        }
        directory.require_admin(admin, now)?;
        let mut next = self.clone();
        next.0.state = InvestigationState::Closed { finding, basis };
        Ok(next)
    }
}
