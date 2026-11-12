//! Moderation evaluation orchestration and deterministic local/test scripted mock (#151).
//!
//! No HTTP route, SQL adapter, background queue consumer or real AI provider is wired in `#151`.
//! Future worker integration (`#152+`) must call [`ModerationEvaluator::evaluate`] outside DB
//! locks, then re-read principal standing, revision binding, job state, policy version and
//! decision version inside the apply transaction before committing (`MOD-07`, `MOD-08`).
//! Note: [`crate::persistence::outbox::OutboxEvent`] currently carries only `comment_id`.

use super::ports::{BoundPrincipal, StoreError};
use crate::domain::ids::{EvaluationId, RevisionId, WorkId};
use crate::domain::moderation::automation::AutomationMode;
use crate::domain::moderation::case::CaseRefusal;
use crate::domain::moderation::{
    AdminAuthority, AdminDecision, AdminRemovalTransition, AdminTransition,
    AuthorModerationTransition, EvaluationTicket, EvaluationTransition, EvaluatorCapability,
    EvaluatorResult, ModerationError, WorkModeration,
};
pub use crate::domain::moderation::{
    CanonicalEvaluatorScript, CanonicalEvaluatorVectors, CanonicalScriptResult,
    CanonicalScriptStep, ModerationEvaluator, ScriptKind, ScriptedEvaluator, ScriptedStep,
};
use crate::domain::principal::Standing;
use crate::domain::publication::{AuthorCommand, Publication};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ModerationPlanError {
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("principal suspended")]
    Suspended,
    #[error(transparent)]
    Rejected(#[from] ModerationError),
}

pub fn plan_author_publication(
    actor: Option<BoundPrincipal>,
    publication: &Publication,
    moderation: &WorkModeration,
    command: AuthorCommand,
    new_evaluation_id: Option<EvaluationId>,
) -> Result<AuthorModerationTransition, ModerationPlanError> {
    let actor = actor.ok_or(ModerationPlanError::Unauthenticated)?;
    if actor.standing != Standing::Active {
        return Err(ModerationPlanError::Suspended);
    }
    Ok(moderation.decide_author(publication, actor.principal, command, new_evaluation_id)?)
}

/// [`plan_author_publication`] under the automation mode read in the same commit (MOD-14): a
/// submission while automation is paused goes to the human queue instead of the evaluator.
pub fn plan_author_publication_under(
    actor: Option<BoundPrincipal>,
    publication: &Publication,
    moderation: &WorkModeration,
    command: AuthorCommand,
    new_evaluation_id: Option<EvaluationId>,
    automation: AutomationMode,
) -> Result<AuthorModerationTransition, ModerationPlanError> {
    let actor = actor.ok_or(ModerationPlanError::Unauthenticated)?;
    if actor.standing != Standing::Active {
        return Err(ModerationPlanError::Suspended);
    }
    Ok(moderation.decide_author_with(
        publication,
        actor.principal,
        command,
        new_evaluation_id,
        automation,
    )?)
}

pub fn plan_work_reevaluation(
    publication: &Publication,
    moderation: &WorkModeration,
    revision: RevisionId,
    evaluation_id: EvaluationId,
) -> Result<(WorkModeration, EvaluationTicket), ModerationError> {
    moderation.reevaluate(publication, revision, evaluation_id)
}

pub fn apply_work_evaluation_result(
    publication: &Publication,
    moderation: &WorkModeration,
    result: &EvaluatorResult,
) -> Result<EvaluationTransition, ModerationError> {
    moderation.apply_evaluator_result(publication, Some(EvaluatorCapability::evaluator()), result)
}

pub fn evaluate_and_apply_work_revision<E: ModerationEvaluator>(
    evaluator: &mut E,
    publication: &Publication,
    moderation: &WorkModeration,
    ticket: &EvaluationTicket,
) -> Result<(EvaluatorResult, EvaluationTransition), ModerationError> {
    let result = evaluator.evaluate(ticket);
    let transition = apply_work_evaluation_result(publication, moderation, &result)?;
    Ok((result, transition))
}

pub fn plan_work_admin_decision(
    actor: Option<BoundPrincipal>,
    granted_work: Option<WorkId>,
    publication: &Publication,
    moderation: &WorkModeration,
    decision: AdminDecision,
) -> Result<AdminTransition, ModerationPlanError> {
    let actor = actor.ok_or(ModerationPlanError::Unauthenticated)?;
    if actor.standing != Standing::Active {
        return Err(ModerationPlanError::Suspended);
    }
    let authority = granted_work.map(|work| AdminAuthority::scoped(actor.principal, work));
    Ok(moderation.apply_admin_decision(publication, authority, decision)?)
}

pub fn plan_work_admin_removal(
    actor: Option<BoundPrincipal>,
    granted_work: Option<WorkId>,
    publication: &Publication,
    moderation: &WorkModeration,
) -> Result<AdminRemovalTransition, ModerationPlanError> {
    let actor = actor.ok_or(ModerationPlanError::Unauthenticated)?;
    if actor.standing != Standing::Active {
        return Err(ModerationPlanError::Suspended);
    }
    let authority = granted_work.map(|work| AdminAuthority::scoped(actor.principal, work));
    Ok(moderation.apply_admin_remove(publication, authority)?)
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModerationActionError {
    #[error("not found")]
    NotFound,
    #[error("forbidden: {0}")]
    Forbidden(&'static str),
    #[error("conflict: {0}")]
    Conflict(&'static str),
    #[error("validation failed: {0}")]
    ValidationFailed(&'static str),
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("principal suspended")]
    Suspended,
    #[error("storage unavailable")]
    Unavailable,
    #[error("internal error")]
    Internal,
}

impl From<StoreError> for ModerationActionError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::Unavailable(_) => Self::Unavailable,
            StoreError::UnknownPrincipal => Self::Unauthenticated,
            StoreError::PrincipalSuspended => Self::Suspended,
            StoreError::Integrity(_) => Self::Internal,
        }
    }
}

pub fn case_refusal_to_action_error(e: CaseRefusal) -> ModerationActionError {
    match e {
        CaseRefusal::StaleCase => ModerationActionError::Conflict("case_version_stale"),
        CaseRefusal::StaleDecision => ModerationActionError::Conflict("stale_decision"),
        CaseRefusal::CaseClosed => ModerationActionError::Conflict("case_closed"),
        CaseRefusal::NotClaimant => ModerationActionError::Forbidden("not_claimant"),
        CaseRefusal::AlreadyClaimed => ModerationActionError::Conflict("already_claimed"),
        CaseRefusal::Escalated => ModerationActionError::Forbidden("escalated_to_admin"),
        CaseRefusal::AppealOpen => ModerationActionError::Forbidden("appeal_open"),
        CaseRefusal::ReasonRequired => ModerationActionError::ValidationFailed("reason_required"),
        CaseRefusal::NoteRequired => ModerationActionError::ValidationFailed("note_required"),
        CaseRefusal::WrongSubject => ModerationActionError::ValidationFailed("wrong_subject"),
        CaseRefusal::VersionExhausted => ModerationActionError::Internal,
        CaseRefusal::InvalidFacts => ModerationActionError::Internal,
        CaseRefusal::Authority(a) => ModerationActionError::Forbidden(a.as_str()),
    }
}

#[cfg(test)]
mod tests;
