//! Generated model INPUTS call production review/lifecycle/ownership decisions. This shell
//! assumes an authorized immutable operator ticket and a locked leased delivery; it does not
//! prove authority, hashing, SQL atomicity or process recovery (the PostgreSQL lane does).
use super::admission::PostReviewFinding;
use super::lifecycle::{CommentState, ModerationVersion, Review, Transition};
use super::moderation::ModerationAction;
use super::outbox::{ClaimGeneration, DeliveryFacts, DeliveryState, Ownership, check_ownership};
use super::post_review::{ReviewBinding, ReviewDecision, decide_review};
use super::submission::PayloadDigest;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    format: String,
    quint_version: String,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    op: String,
    value: String,
    held: i64,
    matches: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    command: Command,
    outcome: String,
    review: String,
    version: i64,
    removed: bool,
    now: i64,
    deadline: i64,
    generation: i64,
    finding: String,
    exhausted: bool,
    receipt: String,
}

struct Shell {
    state: CommentState,
    now: i64,
    deadline: i64,
    generation: i64,
    finding: Option<PostReviewFinding>,
    exhausted: bool,
    receipt: &'static str,
}
impl Shell {
    fn new() -> Self {
        Self {
            state: CommentState::restore(
                Review::Approved,
                None,
                ModerationVersion::new(2).unwrap(),
            ),
            now: 0,
            deadline: 2,
            generation: 1,
            finding: None,
            exhausted: false,
            receipt: "none",
        }
    }
    fn apply(&mut self, command: &Command) -> &'static str {
        match command.op.as_str() {
            "reset" => {
                *self = Self::new();
                "reset"
            }
            "result" => {
                self.finding = Some(match command.value.as_str() {
                    "clear" => PostReviewFinding::Clear,
                    "flagged" => PostReviewFinding::Flagged,
                    "unresolved" => PostReviewFinding::Unresolved,
                    _ => panic!("finding"),
                });
                "result"
            }
            "tick" => {
                self.now += 1;
                "tick"
            }
            "reclaim" => {
                self.generation = 2;
                "reclaim"
            }
            "exhaust" => {
                self.exhausted = true;
                "exhaust"
            }
            "requeue" => "requeue", // Delivery retry never changes the immutable ticket.
            "human" => {
                if !self.state.is_present() {
                    return "removed";
                }
                let action = if command.value == "approved" {
                    ModerationAction::Approve
                } else {
                    ModerationAction::Reject
                };
                let Ok(Transition::Changed(next)) = action.apply(self.state, self.state.version())
                else {
                    panic!("accepted human intent");
                };
                self.state = next;
                "human"
            }
            "remove" => {
                if let Transition::Changed(next) = self.state.remove_by_author().unwrap() {
                    self.state = next;
                }
                "remove"
            }
            "apply" => {
                let facts = DeliveryFacts {
                    state: DeliveryState::Leased,
                    attempts: 1,
                    generation: ClaimGeneration::restore(self.generation).unwrap(),
                    available_at_micros: 1,
                    deadline_from_micros: 0,
                };
                if matches!(
                    check_ownership(facts, ClaimGeneration::restore(command.held).unwrap()),
                    Ownership::Stale(_)
                ) {
                    return "stale";
                }
                if self.receipt != "none" {
                    return "already";
                }
                let payload = PayloadDigest::from_bytes(&[0; 32]).unwrap();
                let current = if command.matches {
                    payload
                } else {
                    PayloadDigest::from_bytes(&[1; 32]).unwrap()
                };
                let ticket = ReviewBinding {
                    version: ModerationVersion::new(2).unwrap(),
                    payload,
                    deadline_micros: self.deadline,
                };
                let outcome = match decide_review(
                    ticket,
                    self.state,
                    current,
                    self.finding,
                    self.now,
                    self.exhausted,
                )
                .unwrap()
                {
                    ReviewDecision::AwaitResult => "waiting",
                    ReviewDecision::Keep => "kept",
                    ReviewDecision::Hold(next) => {
                        self.state = next;
                        "held"
                    }
                    ReviewDecision::Superseded => "superseded",
                    ReviewDecision::Removed => "removed",
                    ReviewDecision::InvalidBinding => "invalid",
                };
                if matches!(outcome, "kept" | "held" | "superseded" | "removed") {
                    self.receipt = outcome;
                }
                outcome
            }
            _ => panic!("command"),
        }
    }
}

#[test]
fn quint_operator_review_inputs_replay_through_production_guards() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../behavior/discussion/post-review-fixtures.json"
    ))
    .unwrap();
    assert_eq!(fixture.format, "vink-post-review-v1");
    assert_eq!(fixture.quint_version, "0.33.1");
    assert_eq!(fixture.steps.len(), 32);
    let mut shell = Shell::new();
    for (index, step) in fixture.steps.iter().enumerate() {
        assert_eq!(
            shell.apply(&step.command),
            step.outcome,
            "step {}",
            index + 1
        );
        let review = match shell.state.review() {
            Review::Approved => "approved",
            Review::Pending => "pending",
            Review::Rejected => "rejected",
        };
        assert_eq!(
            (
                review,
                shell.state.version().get(),
                !shell.state.is_present()
            ),
            (step.review.as_str(), step.version, step.removed),
            "state at {}",
            index + 1
        );
        assert_eq!(
            (
                shell.now,
                shell.deadline,
                shell.generation,
                shell.exhausted,
                shell.receipt
            ),
            (
                step.now,
                step.deadline,
                step.generation,
                step.exhausted,
                step.receipt.as_str()
            )
        );
        let finding = match shell.finding {
            None => "none",
            Some(PostReviewFinding::Clear) => "clear",
            Some(PostReviewFinding::Flagged) => "flagged",
            Some(PostReviewFinding::Unresolved) => "unresolved",
        };
        assert_eq!(finding, step.finding);
    }
}
