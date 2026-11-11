//! M1 publication + moderation replay (#150, #151, #157).
//!
//! Commands run through the production [`Publication`] and [`WorkModeration`] cores, not a test
//! reducer. All 51 steps across `publication-vectors.json` run through production
//! (`author`: 24, `ai_result`: 13, `admin_decide`: 6, `admin_remove`: 3, `reevaluate`: 4, `policy_bump`: 1).
//! All delete/remove lifecycle steps run through real domain and application transitions (#157).

use super::tests::{generation, owner, revision, snapshot, work};
use super::*;
use crate::domain::moderation::tests::{admin, evaluation_id, reason};
use crate::domain::moderation::{
    AdminAuthority, AdminDecision, AdminVerdict, DecisionBy, DecisionVersion, EvaluationTarget,
    EvaluatorCapability, EvaluatorOutcome, EvaluatorResult, JobState, ModerationError,
    PolicyReasonCode, PolicyVersion, WorkModeration,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub(crate) struct Vectors {
    pub(crate) scenarios: Vec<Scenario>,
}

#[derive(Deserialize)]
pub(crate) struct Counterexample {
    pub(crate) violation: Vec<Scenario>,
}

#[derive(Deserialize)]
pub(crate) struct Scenario {
    pub(crate) name: String,
    pub(crate) steps: Vec<Step>,
}

#[derive(Deserialize)]
pub(crate) struct Step {
    pub(crate) command: Command,
    pub(crate) outcome: String,
    pub(crate) state: State,
}

#[derive(Deserialize)]
pub(crate) struct Command {
    pub(crate) actor: String,
    pub(crate) expected: i64,
    pub(crate) job: u128,
    pub(crate) op: String,
    pub(crate) rev: u128,
    pub(crate) value: String,
}

#[derive(Deserialize)]
pub(crate) struct State {
    pub(crate) revs: BTreeMap<u128, Decision>,
    pub(crate) jobs: BTreeMap<u128, Job>,
    pub(crate) audience: String,
    pub(crate) desired: u128,
    pub(crate) pointer: u128,
    pub(crate) lifecycle: String,
    pub(crate) generation: i64,
    pub(crate) policy: i64,
}

#[derive(Deserialize)]
pub(crate) struct Decision {
    pub(crate) by: String,
    pub(crate) decision: String,
}

#[derive(Deserialize)]
pub(crate) struct Job {
    pub(crate) policy: i64,
    pub(crate) rev: u128,
    pub(crate) state: String,
}

fn moderation(value: &str) -> Moderation {
    match value {
        "none" => Moderation::Unsubmitted,
        "pending" => Moderation::Pending,
        "approved" => Moderation::Approved,
        "rejected" => Moderation::Rejected,
        "needs_admin" => Moderation::NeedsAdmin,
        _ => panic!("unknown moderation {value}"),
    }
}

fn lifecycle(value: &str) -> Lifecycle {
    match value {
        "active" => Lifecycle::Active,
        "author_deleted" => {
            Lifecycle::AuthorDeleted(DeletionProvenance::PublicTombstone(PublicPointer {
                revision: revision(4),
                intent_generation: generation(11),
            }))
        }
        "removed" => Lifecycle::Removed,
        _ => panic!("unknown lifecycle {value}"),
    }
}

fn decision_by(value: &str) -> Option<DecisionBy> {
    match value {
        "none" => None,
        "ai" => Some(DecisionBy::Ai),
        "admin" => Some(DecisionBy::Admin),
        _ => panic!("unknown decision_by {value}"),
    }
}

fn job_state(value: &str) -> JobState {
    match value {
        "open" => JobState::Open,
        "applied" => JobState::Applied,
        "failed" => JobState::Failed,
        _ => panic!("unknown job state {value}"),
    }
}

pub(crate) fn project(state: &Publication, mod_state: &WorkModeration, expected: &State) {
    let facts = state.facts();
    assert_eq!(
        facts.audience,
        if expected.audience == "public" {
            Audience::Public
        } else {
            Audience::Private
        }
    );
    assert_eq!(facts.lifecycle, lifecycle(&expected.lifecycle));
    assert_eq!(facts.generation.get(), expected.generation);
    assert_eq!(
        facts.intent.map_or(0, |i| i.revision.as_uuid().as_u128()),
        expected.desired
    );
    assert_eq!(
        facts.pointer.map_or(0, |i| i.revision.as_uuid().as_u128()),
        expected.pointer
    );
    assert_eq!(
        state
            .readable_revision()
            .map_or(0, |s| s.revision.as_uuid().as_u128()),
        expected.pointer
    );
    assert_eq!(facts.revisions.len(), expected.revs.len());
    for r in &facts.revisions {
        let rev_num = r.snapshot.revision.as_uuid().as_u128();
        let expected_rev = &expected.revs[&rev_num];
        assert_eq!(r.moderation, moderation(&expected_rev.decision));
        let actual_by = mod_state
            .revision_decision(r.snapshot.revision)
            .and_then(|d| d.decision_by());
        assert_eq!(actual_by, decision_by(&expected_rev.by));
    }
    assert_eq!(mod_state.facts().policy.get(), expected.policy);
    assert_eq!(mod_state.facts().jobs.len(), expected.jobs.len());
    for job in &mod_state.facts().jobs {
        let job_num = job.id.as_uuid().as_u128();
        let expected_job = &expected.jobs[&job_num];
        assert_eq!(job.policy.get(), expected_job.policy);
        assert_eq!(job.target.revision().as_uuid().as_u128(), expected_job.rev);
        assert_eq!(job.state, job_state(&expected_job.state));
    }
    assert_eq!(Publication::restore(facts.clone()), Ok(state.clone()));
    assert_eq!(
        WorkModeration::restore(state, mod_state.facts().clone()),
        Ok(mod_state.clone())
    );
}

fn author_outcome(
    result: &Result<crate::domain::moderation::AuthorModerationTransition, ModerationError>,
) -> &'static str {
    match result {
        Ok(t) => match t.outcome {
            AuthorOutcome::Saved => "saved",
            AuthorOutcome::Submitted => "submitted",
            AuthorOutcome::Waiting => "waiting",
            AuthorOutcome::Published => "published",
            AuthorOutcome::MadePrivate => "made_private",
            AuthorOutcome::Reaffirmed => "reaffirmed",
            AuthorOutcome::Deleted => "deleted",
        },
        Err(ModerationError::Publication(PublicationError::NotFound)) => "not_found",
        Err(ModerationError::Publication(PublicationError::WorkUnavailable)) => "work_unavailable",
        Err(ModerationError::Publication(PublicationError::UnknownRevision)) => "unknown_revision",
        Err(ModerationError::Publication(PublicationError::Stale)) => "stale",
        Err(ModerationError::Publication(PublicationError::RevisionRejected)) => {
            "revision_rejected"
        }
        other => panic!("unexpected author result {other:?}"),
    }
}

pub(crate) fn parse_evaluator_outcome(value: &str) -> EvaluatorOutcome {
    match value {
        "approve" => EvaluatorOutcome::Approve,
        "reject" => EvaluatorOutcome::Reject(PolicyReasonCode::SafetyPolicy),
        "uncertain" => EvaluatorOutcome::Uncertain,
        "timeout" => EvaluatorOutcome::Timeout,
        "provider_error" => EvaluatorOutcome::ProviderError,
        "invalid_output" => {
            EvaluatorOutcome::from_raw_json(r#"{"verdict":"invalid_output","authority":"forged"}"#)
        }
        other => panic!("unknown evaluator outcome {other}"),
    }
}

pub(crate) fn replay_step(
    state: &mut Publication,
    mod_state: &mut WorkModeration,
    step: &Step,
) -> &'static str {
    let c = &step.command;
    let command = match c.op.as_str() {
        "snapshot" => Some(AuthorCommand::SaveSnapshot(snapshot(
            state.facts().revisions.len() as u128 + 1,
        ))),
        "request_public" => Some(AuthorCommand::RequestPublic {
            revision: revision(c.rev),
            expected: generation(c.expected),
        }),
        "set_private" => Some(AuthorCommand::SetPrivate {
            expected: generation(c.expected),
        }),
        "author_delete" => Some(AuthorCommand::Delete {
            expected: IntentGeneration::new(c.expected),
        }),
        _ => None,
    };
    if let Some(command) = command {
        let before_pub = state.clone();
        let before_mod = mod_state.clone();
        let actor = if c.actor == "author" {
            owner()
        } else {
            PrincipalId::from_uuid(uuid::Uuid::from_u128(2))
        };
        let next_eval_id = evaluation_id(mod_state.facts().jobs.len() as u128 + 1);
        let result = mod_state.decide_author(state, actor, command, Some(next_eval_id));
        assert_eq!(author_outcome(&result), step.outcome);
        if let Ok(t) = result {
            assert_eq!(
                t.effects.evaluation.as_ref().map(|r| r.revision),
                (step.outcome == "submitted").then(|| revision(c.rev))
            );
            assert_eq!(
                t.ticket.as_ref().map(|ticket| ticket.id),
                (step.outcome == "submitted").then_some(next_eval_id)
            );
            if let Some(e) = &t.effects.evaluation {
                assert_eq!(e.hash, snapshot(c.rev).content.hash().clone());
            }
            assert_eq!(
                t.effects.pointer,
                match step.outcome.as_str() {
                    "published" => PointerEffect::Set(t.publication.facts().pointer.unwrap()),
                    "made_private" | "reaffirmed" | "deleted" => PointerEffect::Clear,
                    _ => PointerEffect::Unchanged,
                }
            );
            *state = t.publication;
            *mod_state = t.moderation;
        } else {
            assert_eq!(*state, before_pub);
            assert_eq!(*mod_state, before_mod);
        }
        "author"
    } else if c.op == "ai_result" {
        let before_pub = state.clone();
        let before_mod = mod_state.clone();
        let capability = (c.actor == "evaluator").then(EvaluatorCapability::evaluator);
        let job_policy = mod_state
            .facts()
            .jobs
            .iter()
            .find(|j| j.id == evaluation_id(c.job))
            .map_or(mod_state.facts().policy, |j| j.policy);
        let result = EvaluatorResult {
            evaluation_id: evaluation_id(c.job),
            target: EvaluationTarget::WorkRevision {
                work: work(1),
                revision: revision(c.rev),
                digest: snapshot(c.rev).content.hash().clone(),
            },
            policy: job_policy,
            outcome: parse_evaluator_outcome(&c.value),
        };
        let applied = mod_state.apply_evaluator_result(state, capability, &result);
        match applied {
            Ok(t) => {
                assert_eq!(t.outcome.as_str(), step.outcome);
                assert_eq!(t.effects.evaluation, None);
                assert_eq!(
                    t.effects.pointer,
                    match step.outcome.as_str() {
                        "published" => PointerEffect::Set(t.publication.facts().pointer.unwrap()),
                        _ => PointerEffect::Unchanged,
                    }
                );
                *state = t.publication;
                *mod_state = t.moderation;
            }
            Err(ModerationError::Refused(refusal)) => {
                assert_eq!(refusal.as_str(), step.outcome);
                assert_eq!(*state, before_pub);
                assert_eq!(*mod_state, before_mod);
            }
            other => panic!("unexpected ai_result outcome {other:?}"),
        }
        "ai_result"
    } else if c.op == "admin_decide" {
        let before_pub = state.clone();
        let before_mod = mod_state.clone();
        let authority = (c.actor == "admin").then(|| AdminAuthority::scoped(admin(), work(1)));
        let expected_version = mod_state
            .revision_decision(revision(c.rev))
            .map_or(DecisionVersion::FIRST, |d| d.version);
        let verdict = match c.value.as_str() {
            "approve" => AdminVerdict::Approve,
            "reject" => AdminVerdict::Reject,
            other => panic!("unexpected admin verdict {other}"),
        };
        let applied = mod_state.apply_admin_decision(
            state,
            authority,
            AdminDecision {
                revision: revision(c.rev),
                digest: snapshot(c.rev).content.hash().clone(),
                expected: expected_version,
                verdict,
                reason: reason("Quyết định hậu kiểm của quản trị viên"),
            },
        );
        match applied {
            Ok(t) => {
                assert_eq!(t.outcome.as_str(), step.outcome);
                assert_eq!(t.effects.evaluation, None);
                assert_eq!(
                    t.effects.pointer,
                    match step.outcome.as_str() {
                        "published" => PointerEffect::Set(t.publication.facts().pointer.unwrap()),
                        "unpublished" => PointerEffect::Clear,
                        _ => PointerEffect::Unchanged,
                    }
                );
                *state = t.publication;
                *mod_state = t.moderation;
            }
            Err(err) => {
                let code = match err {
                    ModerationError::NotAdmin => "not_admin",
                    ModerationError::UnknownRevision => "unknown_revision",
                    ModerationError::TargetGone => "target_gone",
                    ModerationError::NotSubmitted => "not_submitted",
                    other => panic!("unexpected admin refusal {other:?}"),
                };
                assert_eq!(code, step.outcome);
                assert_eq!(*state, before_pub);
                assert_eq!(*mod_state, before_mod);
            }
        }
        "admin_decide"
    } else if c.op == "reevaluate" {
        let before_mod = mod_state.clone();
        let next_eval_id = evaluation_id(mod_state.facts().jobs.len() as u128 + 1);
        match mod_state.reevaluate(state, revision(c.rev), next_eval_id) {
            Ok((next_mod, ticket)) => {
                assert_eq!(step.outcome, "requeued");
                assert_eq!(ticket.id, next_eval_id);
                assert_eq!(ticket.policy, mod_state.facts().policy);
                *mod_state = next_mod;
            }
            Err(ModerationError::Reevaluate(refusal)) => {
                assert_eq!(refusal.as_quint_outcome(), step.outcome);
                assert_eq!(*mod_state, before_mod);
            }
            other => panic!("unexpected reevaluate outcome {other:?}"),
        }
        "reevaluate"
    } else if c.op == "policy_bump" {
        assert_eq!(step.outcome, "policy_changed");
        *mod_state = mod_state.bump_policy().unwrap();
        assert_eq!(
            mod_state.facts().policy,
            PolicyVersion::new(step.state.policy).unwrap()
        );
        "policy_bump"
    } else if c.op == "admin_remove" {
        let before_pub = state.clone();
        let before_mod = mod_state.clone();
        let authority = (c.actor == "admin").then(|| AdminAuthority::scoped(admin(), work(1)));
        let applied = mod_state.apply_admin_remove(state, authority);
        match applied {
            Ok(t) => {
                assert_eq!(step.outcome, "removed");
                assert_eq!(t.effects.evaluation, None);
                assert_eq!(t.effects.pointer, PointerEffect::Clear);
                *state = t.publication;
                *mod_state = t.moderation;
            }
            Err(err) => {
                let code = match err {
                    ModerationError::NotAdmin => "not_admin",
                    ModerationError::TargetGone => "target_gone",
                    other => panic!("unexpected admin_remove refusal {other:?}"),
                };
                assert_eq!(code, step.outcome);
                assert_eq!(*state, before_pub);
                assert_eq!(*mod_state, before_mod);
            }
        }
        "admin_remove"
    } else {
        panic!("unexpected command op {}", c.op);
    }
}

#[test]
fn quint_publication_author_inputs_and_admitted_decision_effects_replay_through_production() {
    let vectors: Vectors = serde_json::from_str(include_str!(
        "../../../behavior/publication/publication-vectors.json"
    ))
    .unwrap();
    let mut counts = BTreeMap::new();
    for scenario in vectors.scenarios {
        let mut state = Publication::new(work(1), owner());
        let mut mod_state = WorkModeration::new(work(1));
        for (index, step) in scenario.steps.iter().enumerate() {
            let boundary = replay_step(&mut state, &mut mod_state, step);
            *counts.entry(boundary).or_insert(0) += 1;
            project(&state, &mod_state, &step.state);
            println!(
                "{} step {}: {boundary} {}",
                scenario.name,
                index + 1,
                step.outcome
            );
        }
    }
    // Exact selection counts make a reduced/empty replay fail instead of silently passing.
    assert_eq!(
        counts,
        BTreeMap::from([
            ("admin_decide", 6),
            ("admin_remove", 3),
            ("ai_result", 13),
            ("author", 24),
            ("policy_bump", 1),
            ("reevaluate", 4),
        ])
    );
}

#[test]
fn the_withdrawn_intent_counterexample_is_refused_by_the_production_publication_guard() {
    let vectors: Counterexample = serde_json::from_str(include_str!(
        "../../../behavior/publication/publication-withdrawn-intent-counterexample.json"
    ))
    .unwrap();
    let scenario = &vectors.violation[0];
    assert_eq!(scenario.steps.len(), 4);
    let mut state = Publication::new(work(1), owner());
    let mut mod_state = WorkModeration::new(work(1));
    for step in &scenario.steps[..3] {
        replay_step(&mut state, &mut mod_state, step);
        project(&state, &mod_state, &step.state);
    }
    let last = &scenario.steps[3];
    assert_eq!(
        last.state.pointer, 1,
        "the mutant must actually publish after withdrawal"
    );
    let result = mod_state
        .apply_evaluator_result(
            &state,
            Some(EvaluatorCapability::evaluator()),
            &EvaluatorResult {
                evaluation_id: evaluation_id(last.command.job),
                target: EvaluationTarget::WorkRevision {
                    work: work(1),
                    revision: revision(last.command.rev),
                    digest: snapshot(last.command.rev).content.hash().clone(),
                },
                policy: PolicyVersion::FIRST,
                outcome: EvaluatorOutcome::Approve,
            },
        )
        .unwrap();
    assert_eq!(
        result.outcome,
        crate::domain::moderation::AppliedEvaluation::ApprovedUnpublished
    );
    assert_eq!(
        result.publication.facts().revisions[0].moderation,
        Moderation::Approved
    );
    assert_eq!(result.publication.facts().pointer, None);
    assert_eq!(result.publication.readable_revision(), None);
    assert_eq!(result.effects, Effects::NONE);
    assert_eq!(
        WorkModeration::restore(&result.publication, result.moderation.facts().clone()),
        Ok(result.moderation)
    );
}
