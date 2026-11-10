//! M1 subset replay. Commands run through production, not a test reducer. The canonical model
//! also has job/authority/lifecycle behavior outside #150: admitted decision inputs are supplied
//! at that explicit boundary, and lifecycle mutations are restored as fixtures. Skipped actions
//! must leave the Rust-owned projection unchanged. This is not whole-model conformance.
use super::tests::{generation, owner, revision, snapshot, work};
use super::*;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Vectors {
    scenarios: Vec<Scenario>,
}
#[derive(Deserialize)]
struct Counterexample {
    violation: Vec<Scenario>,
}
#[derive(Deserialize)]
struct Scenario {
    name: String,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
struct Step {
    command: Command,
    outcome: String,
    state: State,
}
#[derive(Deserialize)]
struct Command {
    actor: String,
    expected: i64,
    op: String,
    rev: u128,
    value: String,
}
#[derive(Deserialize)]
struct State {
    revs: BTreeMap<u128, Decision>,
    audience: String,
    desired: u128,
    pointer: u128,
    lifecycle: String,
    generation: i64,
}
#[derive(Deserialize)]
struct Decision {
    decision: String,
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
        "author_deleted" => Lifecycle::AuthorDeleted,
        "removed" => Lifecycle::Removed,
        _ => panic!("unknown lifecycle {value}"),
    }
}
fn project(state: &Publication, expected: &State) {
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
        assert_eq!(
            r.moderation,
            moderation(&expected.revs[&r.snapshot.revision.as_uuid().as_u128()].decision)
        );
    }
    assert_eq!(Publication::restore(facts.clone()), Ok(state.clone()));
}
fn author_outcome(result: &Result<AuthorTransition, PublicationError>) -> &'static str {
    match result {
        Ok(t) => match t.outcome {
            AuthorOutcome::Saved => "saved",
            AuthorOutcome::Submitted => "submitted",
            AuthorOutcome::Waiting => "waiting",
            AuthorOutcome::Published => "published",
            AuthorOutcome::MadePrivate => "made_private",
            AuthorOutcome::Reaffirmed => "reaffirmed",
        },
        Err(PublicationError::NotFound) => "not_found",
        Err(PublicationError::WorkUnavailable) => "work_unavailable",
        Err(PublicationError::UnknownRevision) => "unknown_revision",
        Err(PublicationError::Stale) => "stale",
        Err(PublicationError::RevisionRejected) => "revision_rejected",
        other => panic!("unexpected {other:?}"),
    }
}

// Returns which production boundary was exercised. Effects are checked independently of state.
fn replay_step(state: &mut Publication, step: &Step) -> &'static str {
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
        _ => None,
    };
    if let Some(command) = command {
        let before = state.clone();
        let actor = if c.actor == "author" {
            owner()
        } else {
            PrincipalId::from_uuid(uuid::Uuid::from_u128(2))
        };
        let result = state.decide_author(actor, command);
        assert_eq!(author_outcome(&result), step.outcome);
        if let Ok(t) = result {
            assert_eq!(
                t.effects.evaluation.as_ref().map(|r| r.revision),
                (step.outcome == "submitted").then(|| revision(c.rev))
            );
            if let Some(e) = &t.effects.evaluation {
                assert_eq!(e.hash, snapshot(c.rev).content.hash().clone());
            }
            assert_eq!(
                t.effects.pointer,
                match step.outcome.as_str() {
                    "published" => PointerEffect::Set(t.next.facts().pointer.unwrap()),
                    "made_private" | "reaffirmed" => PointerEffect::Clear,
                    _ => PointerEffect::Unchanged,
                }
            );
            *state = t.next;
        } else {
            assert_eq!(*state, before);
        }
        "author"
    } else if matches!(c.op.as_str(), "ai_result" | "admin_decide")
        && matches!(
            step.outcome.as_str(),
            "published" | "approved_unpublished" | "rejected" | "needs_admin" | "unpublished"
        )
    {
        // Job validation and authority are assumed to have admitted this input. Their model
        // outcomes are not recomputed or claimed as implemented here.
        let t = state
            .record_review(ReviewedRevision {
                work: work(1),
                revision: revision(c.rev),
                hash: snapshot(c.rev).content.hash().clone(),
                expected: state.revision(revision(c.rev)).unwrap().moderation,
                outcome: match c.value.as_str() {
                    "approve" => ReviewOutcome::Approved,
                    "reject" => ReviewOutcome::Rejected,
                    "uncertain" | "invalid_output" => ReviewOutcome::NeedsAdmin,
                    _ => panic!("unadmitted review"),
                },
            })
            .unwrap();
        assert_eq!(t.effects.evaluation, None);
        assert_eq!(
            t.effects.pointer,
            match step.outcome.as_str() {
                "published" => PointerEffect::Set(t.next.facts().pointer.unwrap()),
                "unpublished" => PointerEffect::Clear,
                _ => PointerEffect::Unchanged,
            }
        );
        *state = t.next;
        "review_effect"
    } else if matches!(step.outcome.as_str(), "deleted" | "removed") {
        // No delete/remove command exists. Load terminal facts from the canonical model only.
        let mut facts = state.facts().clone();
        facts.lifecycle = lifecycle(&step.state.lifecycle);
        facts.generation = generation(step.state.generation);
        facts.audience = if step.state.audience == "public" {
            Audience::Public
        } else {
            Audience::Private
        };
        facts.intent = None;
        facts.pointer = None;
        *state = Publication::restore(facts).unwrap();
        "lifecycle_fixture"
    } else {
        // Jobs, policy, authority and rejected results have no implemented command in #150.
        "outside_scope"
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
        for (index, step) in scenario.steps.iter().enumerate() {
            let boundary = replay_step(&mut state, step);
            *counts.entry(boundary).or_insert(0) += 1;
            project(&state, &step.state);
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
            ("author", 22),
            ("review_effect", 8),
            ("lifecycle_fixture", 2),
            ("outside_scope", 19)
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
    for step in &scenario.steps[..3] {
        replay_step(&mut state, step);
        project(&state, &step.state);
    }
    let last = &scenario.steps[3];
    assert_eq!(
        last.state.pointer, 1,
        "the mutant must actually publish after withdrawal"
    );
    let result = state
        .record_review(ReviewedRevision {
            work: work(1),
            revision: revision(last.command.rev),
            hash: snapshot(last.command.rev).content.hash().clone(),
            expected: Moderation::Pending,
            outcome: ReviewOutcome::Approved,
        })
        .unwrap();
    assert_eq!(
        result.next.facts().revisions[0].moderation,
        Moderation::Approved
    );
    assert_eq!(result.next.facts().pointer, None);
    assert_eq!(result.next.readable_revision(), None);
    assert_eq!(result.effects, Effects::NONE);
}
