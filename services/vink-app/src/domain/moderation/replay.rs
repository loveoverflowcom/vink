//! Quint replay for the moderation evaluator contract, scripted mock and admin-override negative
//! control (#151, AC-07).
//!
//! Explicitly separates the two scopes in `evaluator-vectors.json`:
//! - **15 `publication.qnt` steps** execute end-to-end through [`ScriptedEvaluator`],
//!   [`apply_work_evaluation_result`], [`WorkModeration::apply_evaluator_result`] and
//!   [`Publication::record_review`].
//! - **9 `comment_edit.qnt` steps** execute through [`ScriptedEvaluator`] and the shared
//!   [`validate_evaluation_result`] contract validator (`EvaluationTarget::CommentRevision`),
//!   verifying contract/script compatibility (including `superseded`) without claiming a
//!   comment-edit domain consumer exists (`#154`).

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use super::tests::{comment, evaluation_id};
use super::*;
use crate::domain::discussion::submission::PayloadDigest;
use crate::domain::publication::Publication;
use crate::domain::publication::replay::{
    Counterexample, Vectors as PublicationVectors, parse_evaluator_outcome, project, replay_step,
};
use crate::domain::publication::tests::{owner, revision, snapshot, work};

#[derive(Deserialize)]
struct CommentEditVectors {
    scenarios: Vec<CommentScenario>,
}

#[derive(Deserialize)]
struct CommentScenario {
    name: String,
    steps: Vec<CommentStep>,
}

#[derive(Deserialize)]
struct CommentStep {
    state: CommentState,
}

#[derive(Clone, Deserialize)]
struct CommentState {
    revs: BTreeMap<u128, CommentRev>,
    jobs: BTreeMap<u128, CommentJob>,
    shown: u128,
    candidate: u128,
    removal: String,
}

#[derive(Clone, Deserialize)]
struct CommentRev {
    by: String,
    decision: String,
}

#[derive(Clone, Deserialize)]
struct CommentJob {
    rev: u128,
    state: String,
}

fn comment_digest(rev: u128) -> PayloadDigest {
    PayloadDigest::from_bytes(&[rev as u8; 32]).unwrap()
}

fn initial_comment_state() -> CommentState {
    CommentState {
        revs: BTreeMap::from([(
            1,
            CommentRev {
                by: "none".to_owned(),
                decision: "pending".to_owned(),
            },
        )]),
        jobs: BTreeMap::from([(
            1,
            CommentJob {
                rev: 1,
                state: "open".to_owned(),
            },
        )]),
        shown: 0,
        candidate: 1,
        removal: "none".to_owned(),
    }
}

fn comment_job_state(raw: &str) -> JobState {
    match raw {
        "open" => JobState::Open,
        "applied" => JobState::Applied,
        "failed" => JobState::Failed,
        other => panic!("unknown comment job state {other}"),
    }
}

fn replay_comment_contract_step(
    evaluator: &mut ScriptedEvaluator,
    scenarios: &[CommentScenario],
    script_step: &CanonicalScriptStep,
) -> &'static str {
    let scenario = scenarios
        .iter()
        .find(|s| s.name == script_step.scenario)
        .unwrap_or_else(|| panic!("unknown comment scenario {}", script_step.scenario));
    let before = if script_step.step == 1 {
        initial_comment_state()
    } else {
        scenario.steps[script_step.step - 2].state.clone()
    };
    let after = &scenario.steps[script_step.step - 1].state;

    let jobs: Vec<EvaluationJobFacts> = before
        .jobs
        .iter()
        .map(|(&job_num, job)| EvaluationJobFacts {
            id: evaluation_id(job_num),
            target: EvaluationTarget::CommentRevision {
                comment: comment(1),
                revision: revision(job.rev),
                digest: comment_digest(job.rev),
            },
            policy: PolicyVersion::FIRST,
            state: comment_job_state(&job.state),
        })
        .collect();

    let ticket_rev = before
        .jobs
        .get(&script_step.result.job)
        .map_or(script_step.result.revision, |j| j.rev);
    let ticket = EvaluationTicket {
        id: evaluation_id(script_step.result.job),
        target: EvaluationTarget::CommentRevision {
            comment: comment(1),
            revision: revision(ticket_rev),
            digest: comment_digest(ticket_rev),
        },
        policy: PolicyVersion::FIRST,
    };
    let result_target = EvaluationTarget::CommentRevision {
        comment: comment(1),
        revision: revision(script_step.result.revision),
        digest: comment_digest(script_step.result.revision),
    };
    evaluator.push(ScriptedStep {
        kind: script_step.script,
        result: EvaluatorResult {
            evaluation_id: evaluation_id(script_step.result.job),
            target: result_target,
            policy: PolicyVersion::FIRST,
            outcome: script_step
                .result
                .parse_outcome()
                .expect("canonical script outcome must parse"),
        },
    });

    let delivered = evaluator.evaluate(&ticket);
    let validated = validate_evaluation_result(
        Some(EvaluatorCapability::evaluator()),
        Some(EvaluationSubject::Comment(comment(1))),
        &jobs,
        PolicyVersion::FIRST,
        &delivered,
        |rev_id| {
            let rev_num = rev_id.as_uuid().as_u128();
            let rev = before.revs.get(&rev_num)?;
            Some(TargetRevisionFacts {
                target: EvaluationTarget::CommentRevision {
                    comment: comment(1),
                    revision: rev_id,
                    digest: comment_digest(rev_num),
                },
                target_active: before.removal == "none",
                overridden_by_admin: rev.by == "admin",
                candidate_pending: before.candidate == rev_num && rev.decision == "pending",
            })
        },
    );

    let actual_outcome = match validated {
        Ok(_) => match delivered.outcome {
            EvaluatorOutcome::Approve => "published",
            EvaluatorOutcome::Reject(_) => "rejected",
            EvaluatorOutcome::Uncertain | EvaluatorOutcome::InvalidOutput => "needs_admin",
            EvaluatorOutcome::Timeout | EvaluatorOutcome::ProviderError => "retryable",
        },
        Err(refusal) => refusal.as_str(),
    };
    assert_eq!(actual_outcome, script_step.applied);
    assert_eq!(after.shown, script_step.public_revision);
    "comment_contract_only"
}

#[test]
fn quint_evaluator_vectors_replay_publication_steps_and_check_comment_contract() {
    let script_vectors: CanonicalEvaluatorVectors = serde_json::from_str(include_str!(
        "../../../behavior/target/evaluator-vectors.json"
    ))
    .unwrap();
    assert_eq!(script_vectors.format, "vink-moderation-evaluator-script-v1");
    assert_eq!(script_vectors.scripts.len(), 1);
    let script_steps = &script_vectors.scripts[0].steps;
    assert_eq!(script_steps.len(), 24);

    let pub_vectors: PublicationVectors = serde_json::from_str(include_str!(
        "../../../behavior/publication/publication-vectors.json"
    ))
    .unwrap();
    let comment_vectors: CommentEditVectors = serde_json::from_str(include_str!(
        "../../../behavior/target/comment_edit-vectors.json"
    ))
    .unwrap();

    let mut evaluator = ScriptedEvaluator::new([]);
    let mut counts = BTreeMap::new();
    let mut current_pub_scenario: Option<(String, usize, Publication, WorkModeration)> = None;

    for script_step in script_steps {
        let boundary = match script_step.model.as_str() {
            "publication.qnt" => {
                let scenario = pub_vectors
                    .scenarios
                    .iter()
                    .find(|s| s.name == script_step.scenario)
                    .unwrap();
                let need_reset =
                    current_pub_scenario
                        .as_ref()
                        .is_none_or(|(name, cursor, _, _)| {
                            name != &scenario.name || *cursor >= script_step.step
                        });
                if need_reset {
                    current_pub_scenario = Some((
                        scenario.name.clone(),
                        0,
                        Publication::new(work(1), owner()),
                        WorkModeration::new(work(1)),
                    ));
                }
                let (_, cursor, pub_state, mod_state) = current_pub_scenario.as_mut().unwrap();
                while *cursor + 1 < script_step.step {
                    let prior = &scenario.steps[*cursor];
                    replay_step(pub_state, mod_state, prior);
                    project(pub_state, mod_state, &prior.state);
                    *cursor += 1;
                }

                let target_step = &scenario.steps[script_step.step - 1];
                let job_facts = mod_state
                    .facts()
                    .jobs
                    .iter()
                    .find(|j| j.id == evaluation_id(script_step.result.job))
                    .expect("scripted publication step must reference a tracked job")
                    .clone();
                let ticket = EvaluationTicket {
                    id: job_facts.id,
                    target: job_facts.target.clone(),
                    policy: job_facts.policy,
                };
                evaluator.push(ScriptedStep {
                    kind: script_step.script,
                    result: EvaluatorResult {
                        evaluation_id: evaluation_id(script_step.result.job),
                        target: EvaluationTarget::WorkRevision {
                            work: work(1),
                            revision: revision(script_step.result.revision),
                            digest: snapshot(script_step.result.revision).content.hash().clone(),
                        },
                        policy: job_facts.policy,
                        outcome: script_step
                            .result
                            .parse_outcome()
                            .expect("canonical script outcome must parse"),
                    },
                });

                let before_pub = pub_state.clone();
                let before_mod = mod_state.clone();
                let delivered = evaluator.evaluate(&ticket);
                match mod_state.apply_evaluator_result(
                    pub_state,
                    Some(EvaluatorCapability::evaluator()),
                    &delivered,
                ) {
                    Ok(transition) => {
                        assert_eq!(transition.outcome.as_str(), script_step.applied);
                        *pub_state = transition.publication;
                        *mod_state = transition.moderation;
                    }
                    Err(ModerationError::Refused(refusal)) => {
                        assert_eq!(refusal.as_str(), script_step.applied);
                        assert_eq!(*pub_state, before_pub);
                        assert_eq!(*mod_state, before_mod);
                    }
                    other => panic!("unexpected publication evaluation result {other:?}"),
                }
                assert_eq!(
                    pub_state
                        .readable_revision()
                        .map_or(0, |s| s.revision.as_uuid().as_u128()),
                    script_step.public_revision
                );
                project(pub_state, mod_state, &target_step.state);
                *cursor = script_step.step;
                "publication_consumer"
            }
            "comment_edit.qnt" => replay_comment_contract_step(
                &mut evaluator,
                &comment_vectors.scenarios,
                script_step,
            ),
            other => panic!("unexpected model in evaluator-vectors.json: {other}"),
        };
        *counts.entry(boundary).or_insert(0) += 1;
    }

    assert_eq!(evaluator.remaining(), 0);
    assert_eq!(evaluator.unscripted_calls(), 0);
    assert_eq!(evaluator.executed_kinds().len(), 24);
    let covered: BTreeSet<ScriptKind> = evaluator.executed_kinds().iter().copied().collect();
    assert_eq!(covered, BTreeSet::from(ScriptKind::ALL));
    assert_eq!(
        counts,
        BTreeMap::from([("comment_contract_only", 9), ("publication_consumer", 15),])
    );
}

#[test]
fn the_admin_late_result_counterexample_is_refused_by_the_production_moderation_guard() {
    let vectors: Counterexample = serde_json::from_str(include_str!(
        "../../../behavior/publication/publication-late-result-counterexample.json"
    ))
    .unwrap();
    let scenario = &vectors.violation[0];
    assert_eq!(scenario.steps.len(), 4);

    let mut pub_state = Publication::new(work(1), owner());
    let mut mod_state = WorkModeration::new(work(1));
    for step in &scenario.steps[..3] {
        replay_step(&mut pub_state, &mut mod_state, step);
        project(&pub_state, &mod_state, &step.state);
    }
    assert_eq!(pub_state.readable_revision(), Some(&snapshot(1)));
    assert_eq!(
        mod_state
            .revision_decision(revision(1))
            .unwrap()
            .decision_by(),
        Some(DecisionBy::Admin)
    );

    let last = &scenario.steps[3];
    assert_eq!(
        last.state.revs[&1].decision, "rejected",
        "the mutant must overwrite the admin approval with the late AI rejection"
    );
    let before_pub = pub_state.clone();
    let before_mod = mod_state.clone();
    let result = mod_state.apply_evaluator_result(
        &pub_state,
        Some(EvaluatorCapability::evaluator()),
        &EvaluatorResult {
            evaluation_id: evaluation_id(last.command.job),
            target: EvaluationTarget::WorkRevision {
                work: work(1),
                revision: revision(last.command.rev),
                digest: snapshot(last.command.rev).content.hash().clone(),
            },
            policy: PolicyVersion::FIRST,
            outcome: parse_evaluator_outcome(&last.command.value),
        },
    );
    assert_eq!(result, Err(EvaluationRefusal::Overridden.into()));
    assert_eq!(pub_state, before_pub);
    assert_eq!(mod_state, before_mod);
    assert_eq!(pub_state.readable_revision(), Some(&snapshot(1)));
}
