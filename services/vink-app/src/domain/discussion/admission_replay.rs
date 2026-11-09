//! Generated Quint INPUTS (`behavior/discussion/admission.qnt`, #120 DISC-01) call the production
//! admission, lifecycle and post-review decisions. Standing, trust, the thread, the effective
//! policy and the clock form a synthetic in-memory shell fed only by the model's fact commands: no
//! trust authority, commit-time standing read, store, outbox or worker exists. When the model
//! makes the candidate effective, the shell calls `decide_initial_review` with it directly;
//! production `PolicySelection` cannot express that selection. Expected state is read only AFTER
//! each production decision and compared with it.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use uuid::Uuid;

use super::admission::{
    AdmissionDecision, AdmissionFacts, AdmissionPolicy, AdmissionReason, Assessment,
    PolicySelection, PostReview, PostReviewFinding, PostReviewTicket, RULES_MAX_AGE_MICROS,
    Standing, TRUST_MAX_AGE_MICROS, ThreadReview, Trust, Verdict, decide_admission,
    decide_initial_review, decide_post_review,
};
use super::body::CommentBody;
use super::lifecycle::{CommentState, LifecycleError, Review, Transition};
use super::moderation::ModerationAction;
use super::submission::PayloadDigest;
use crate::domain::ids::WorkId;

/// Tick 0 of the model clock, in microseconds since the Unix epoch.
const EPOCH: i64 = 1_800_000_000_000_000;
/// One model tick: the model's maximum trust age is two ticks.
const TICK: i64 = TRUST_MAX_AGE_MICROS / 2;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    format: String,
    quint_version: String,
    trust_max_age_ticks: i64,
    scenarios: Vec<Scenario>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    name: String,
    steps: Vec<Step>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Counterexample {
    format: String,
    quint_version: String,
    #[allow(dead_code)]
    command: Vec<String>,
    status: String,
    steps: Vec<Step>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    command: Command,
    outcome: String,
    reason: String,
    shadow: String,
    effective: String,
    shadowing: bool,
    now: i64,
    comments: Vec<Observed>,
    tickets: Vec<ObservedTicket>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    op: String,
    actor: i64,
    id: i64,
    value: String,
    age: i64,
    matches: bool,
    flag: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Observed {
    id: i64,
    writer: i64,
    review: String,
    removed: bool,
    version: i64,
    policy: String,
    reason: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ObservedTicket {
    id: i64,
    version: i64,
}

struct Stored {
    writer: i64,
    state: CommentState,
    policy: AdmissionPolicy,
    reason: AdmissionReason,
}

/// Which policy the shell's operator made effective.
#[derive(Clone, Copy)]
enum Selected {
    Pilot(PolicySelection),
    /// Synthetic: only the model may make the candidate effective.
    Candidate,
}

struct Shell {
    standing: BTreeMap<i64, Standing>,
    trust: BTreeMap<i64, Trust>,
    thread: ThreadReview,
    selected: Selected,
    now: i64,
    comments: BTreeMap<i64, Stored>,
    tickets: BTreeMap<i64, PostReviewTicket>,
}

fn digest(text: String) -> PayloadDigest {
    let work = WorkId::from_uuid(Uuid::from_u128(77));
    PayloadDigest::of_submission(work, None, &CommentBody::parse(&text).unwrap())
}

fn verdict(raw: &str) -> Option<Verdict> {
    match raw {
        "unassessed" => None,
        "clear" => Some(Verdict::Clear),
        "flagged" => Some(Verdict::Flagged),
        other => panic!("unknown verdict {other}"),
    }
}

fn review_code(review: Review) -> &'static str {
    match review {
        Review::Pending => "pending",
        Review::Approved => "approved",
        Review::Rejected => "rejected",
    }
}

fn decision_code(decision: AdmissionDecision) -> &'static str {
    match decision {
        AdmissionDecision::Admit(admission) => admission.reason().code(),
        AdmissionDecision::Refuse(refusal) => refusal.code(),
    }
}

fn transition_outcome(
    result: Result<Transition, LifecycleError>,
) -> (&'static str, Option<CommentState>) {
    match result {
        Ok(Transition::Changed(state)) => ("changed", Some(state)),
        Ok(Transition::Unchanged(_)) => ("unchanged", None),
        Err(LifecycleError::Removed) => ("removed", None),
        Err(other) => panic!("the model never reaches {other:?}"),
    }
}

impl Shell {
    fn new() -> Self {
        Self {
            standing: (1..=3).map(|writer| (writer, Standing::Active)).collect(),
            trust: (1..=3).map(|writer| (writer, Trust::Unknown)).collect(),
            thread: ThreadReview::Standard,
            selected: Selected::Pilot(PolicySelection::PREMODERATION),
            now: 0,
            comments: BTreeMap::new(),
            tickets: BTreeMap::new(),
        }
    }

    fn micros(tick: i64) -> i64 {
        EPOCH + tick * TICK
    }

    /// Facts for a submission: the rules assessment is built from the command, never from state.
    fn facts(&self, command: &Command) -> AdmissionFacts {
        let now = Self::micros(self.now);
        let payload = digest(format!("Bình luận {}", command.id));
        let rules = verdict(&command.value).map_or(Assessment::Unassessed, |verdict| {
            Assessment::Assessed {
                verdict,
                payload: if command.matches {
                    payload
                } else {
                    digest(format!("Bài khác {}", command.id))
                },
                at_micros: match command.age {
                    -1 => now + 1,
                    0 => now,
                    1 => now - RULES_MAX_AGE_MICROS - 1,
                    other => panic!("unknown rules age {other}"),
                },
            }
        });
        let classifier =
            verdict(&command.flag).map_or(Assessment::Unassessed, |verdict| Assessment::Assessed {
                verdict,
                payload,
                at_micros: now,
            });
        AdmissionFacts {
            standing: self.standing[&command.actor],
            trust: self.trust[&command.actor],
            thread: self.thread,
            rules,
            classifier,
            payload,
            now_micros: now,
        }
    }

    /// Runs one command through production; returns (outcome, reason, shadow).
    fn apply(&mut self, command: &Command) -> (String, String, String) {
        let id = command.id;
        let mut reason = "none";
        let mut shadow = "none";
        let outcome = match command.op.as_str() {
            "submit" => {
                assert!(
                    !self.comments.contains_key(&id),
                    "server identities are fresh"
                );
                let facts = self.facts(command);
                let (effective, measured) = match self.selected {
                    Selected::Pilot(selection) => {
                        let outcome = decide_admission(selection, facts);
                        (outcome.effective, outcome.shadow)
                    }
                    Selected::Candidate => (
                        decide_initial_review(AdmissionPolicy::TrustedCohortV1, facts),
                        None,
                    ),
                };
                reason = decision_code(effective);
                shadow = measured.map_or("none", decision_code);
                match effective {
                    AdmissionDecision::Admit(admission) => {
                        self.comments.insert(
                            id,
                            Stored {
                                writer: command.actor,
                                state: admission.initial_state(),
                                policy: admission.policy(),
                                reason: admission.reason(),
                            },
                        );
                        if let Some(ticket) = admission.post_review() {
                            self.tickets.insert(id, ticket);
                        }
                        "created"
                    }
                    AdmissionDecision::Refuse(_) => "refused",
                }
            }
            "moderate" => {
                let action = match command.value.as_str() {
                    "approve" => ModerationAction::Approve,
                    "reject" => ModerationAction::Reject,
                    "remove" => ModerationAction::Remove,
                    other => panic!("unknown action {other}"),
                };
                let stored = self.comments.get_mut(&id).unwrap();
                let (outcome, next) =
                    transition_outcome(action.apply(stored.state, stored.state.version()));
                if let Some(next) = next {
                    stored.state = next;
                }
                outcome
            }
            "remove" => {
                let stored = self.comments.get_mut(&id).unwrap();
                assert_eq!(stored.writer, command.actor, "only the writer withdraws");
                let (outcome, next) = transition_outcome(stored.state.remove_by_author());
                if let Some(next) = next {
                    stored.state = next;
                }
                outcome
            }
            "post_review" => {
                let ticket = self.tickets.remove(&id).expect("an owed post-review");
                let finding = match command.value.as_str() {
                    "clear" => PostReviewFinding::Clear,
                    "flagged" => PostReviewFinding::Flagged,
                    "unresolved" => PostReviewFinding::Unresolved,
                    other => panic!("unknown finding {other}"),
                };
                let stored = self.comments.get_mut(&id).unwrap();
                match decide_post_review(ticket, stored.state, finding) {
                    Ok(PostReview::Keep) => "kept",
                    Ok(PostReview::Hold(Transition::Changed(next))) => {
                        stored.state = next;
                        "held"
                    }
                    Ok(PostReview::Hold(Transition::Unchanged(_))) => "already_hidden",
                    Err(LifecycleError::Stale { .. }) => "stale",
                    Err(LifecycleError::Removed) => "removed",
                    Err(other) => panic!("the model never reaches {other:?}"),
                }
            }
            "standing" => {
                let standing = match command.value.as_str() {
                    "active" => Standing::Active,
                    "suspended" => Standing::Suspended,
                    "unknown" => Standing::Unknown,
                    other => panic!("unknown standing {other}"),
                };
                self.standing.insert(command.actor, standing);
                "facts_set"
            }
            "attest" => {
                let attested_at_micros = Self::micros(self.now);
                self.trust
                    .insert(command.actor, Trust::Trusted { attested_at_micros });
                "facts_set"
            }
            "trust" => {
                let trust = match command.value.as_str() {
                    "unknown" => Trust::Unknown,
                    "not_trusted" => Trust::NotTrusted,
                    "revoked" => Trust::Revoked,
                    other => panic!("unknown trust {other}"),
                };
                self.trust.insert(command.actor, trust);
                "facts_set"
            }
            "thread" => {
                self.thread = match command.value.as_str() {
                    "standard" => ThreadReview::Standard,
                    "reviewed" => ThreadReview::AllReviewed,
                    other => panic!("unknown thread review {other}"),
                };
                "facts_set"
            }
            "policy" => {
                self.selected = match command.value.as_str() {
                    "premoderation" => Selected::Pilot(PolicySelection::PREMODERATION),
                    "shadow" => Selected::Pilot(PolicySelection::shadowing(
                        AdmissionPolicy::TrustedCohortV1,
                    )),
                    "trusted_cohort" => Selected::Candidate,
                    other => panic!("unknown policy {other}"),
                };
                "facts_set"
            }
            "tick" => {
                self.now += 1;
                "facts_set"
            }
            other => panic!("unknown command {other}"),
        };
        (outcome.into(), reason.into(), shadow.into())
    }

    fn effective(&self) -> (&'static str, bool) {
        match self.selected {
            Selected::Pilot(selection) => {
                (selection.effective().code(), selection.shadow().is_some())
            }
            Selected::Candidate => (AdmissionPolicy::TrustedCohortV1.code(), false),
        }
    }

    fn observed(&self) -> (Vec<Observed>, Vec<ObservedTicket>) {
        let comments = self
            .comments
            .iter()
            .map(|(id, stored)| Observed {
                id: *id,
                writer: stored.writer,
                review: review_code(stored.state.review()).into(),
                removed: !stored.state.is_present(),
                version: stored.state.version().get(),
                policy: stored.policy.code().into(),
                reason: stored.reason.code().into(),
            })
            .collect();
        let tickets = self
            .tickets
            .iter()
            .map(|(id, ticket)| ObservedTicket {
                id: *id,
                version: ticket.version().get(),
            })
            .collect();
        (comments, tickets)
    }

    /// Applies one step's command and compares everything the model exported for it.
    fn replay(&mut self, step: &Step, at: &str) -> String {
        let (outcome, reason, shadow) = self.apply(&step.command);
        assert_eq!(outcome, step.outcome, "{at}: outcome");
        assert_eq!(reason, step.reason, "{at}: reason");
        assert_eq!(shadow, step.shadow, "{at}: shadow");
        let (effective, shadowing) = self.effective();
        assert_eq!(
            (effective, shadowing, self.now),
            (step.effective.as_str(), step.shadowing, step.now),
            "{at}: shell facts"
        );
        let (comments, tickets) = self.observed();
        assert_eq!(comments, step.comments, "{at}: comments");
        assert_eq!(tickets, step.tickets, "{at}: owed post-reviews");
        outcome
    }
}

#[test]
fn quint_admission_inputs_replay_against_the_production_decisions() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../behavior/discussion/admission-fixtures.json"
    ))
    .unwrap();
    let tool: serde_json::Value =
        serde_json::from_str(include_str!("../../../behavior/discussion/package.json")).unwrap();
    assert_eq!(fixture.format, "vink-discussion-admission-conformance-v1");
    assert_eq!(
        fixture.quint_version,
        tool["dependencies"]["@informalsystems/quint"]
    );
    assert_eq!(TICK * fixture.trust_max_age_ticks, TRUST_MAX_AGE_MICROS);
    assert!(!fixture.scenarios.is_empty());
    let mut outcomes = BTreeSet::new();
    let mut reasons = BTreeSet::new();
    for scenario in &fixture.scenarios {
        assert!(!scenario.steps.is_empty());
        let mut shell = Shell::new();
        for (index, step) in scenario.steps.iter().enumerate() {
            let at = format!("{} step {index}", scenario.name);
            outcomes.insert(shell.replay(step, &at));
            reasons.insert(step.reason.clone());
        }
    }
    for expected in ["created", "refused", "held", "kept", "stale", "removed"] {
        assert!(
            outcomes.contains(expected),
            "replay never reaches {expected}"
        );
    }
    assert!(reasons.contains("trusted_and_clear") && reasons.contains("trust_stale"));
}

#[test]
fn the_admission_counterexamples_are_refused_by_production() {
    for (name, json) in [
        (
            "shadow",
            include_str!("../../../behavior/discussion/admission-shadow-counterexample.json"),
        ),
        (
            "stale trust",
            include_str!("../../../behavior/discussion/admission-stale-trust-counterexample.json"),
        ),
        (
            "unfenced",
            include_str!("../../../behavior/discussion/admission-unfenced-counterexample.json"),
        ),
    ] {
        let counterexample: Counterexample = serde_json::from_str(json).unwrap();
        assert_eq!(counterexample.format, "vink-discussion-counterexample-v1");
        assert_eq!(counterexample.status, "violation");
        assert!(!counterexample.quint_version.is_empty());
        let (last, prefix) = counterexample.steps.split_last().unwrap();
        let mut shell = Shell::new();
        // The prefix is lawful: production agrees with the mutant until its violating step.
        for (index, step) in prefix.iter().enumerate() {
            shell.replay(step, &format!("{name} step {index}"));
        }
        let (outcome, reason, _) = shell.apply(&last.command);
        let (comments, _) = shell.observed();
        let comment = &comments.iter().find(|c| c.id == last.command.id).unwrap();
        match name {
            "shadow" => {
                assert_eq!(
                    (outcome.as_str(), reason.as_str()),
                    ("created", "policy_premoderation")
                );
                assert_eq!(
                    comment.review, "pending",
                    "the shadow approval is not stored"
                );
            }
            "stale trust" => {
                assert_eq!(
                    (outcome.as_str(), reason.as_str()),
                    ("created", "trust_stale")
                );
                assert_eq!(comment.review, "pending");
            }
            _ => {
                assert_eq!(outcome, "stale", "the human decision wins over the job");
                assert_eq!((comment.review.as_str(), comment.version), ("approved", 3));
            }
        }
        assert_ne!(
            comments, last.comments,
            "{name}: production must not reach the mutant's state"
        );
    }
}
