//! Generated Quint INPUTS (`behavior/discussion/outbox.qnt`, #120 DISC-04) call the production
//! outbox decisions: `decide_claim`, `decide_completion` and `decide_requeue`. The shell keeps one
//! [`DeliveryFacts`] per delivery, the claims each worker believes it holds and a count of
//! applied effects; it serializes commands exactly as the model does (row locks are the postgres
//! lane's evidence). Expected state is read only AFTER each production decision and compared.

use std::collections::BTreeSet;

use serde::Deserialize;

use super::outbox::{
    ClaimDecision, ClaimGeneration, Completion, DeadReason, DeliveryFacts, DeliveryState,
    HandlerResult, OutboxPolicy, Requeue, Transient, decide_claim, decide_completion,
    decide_requeue,
};

/// Tick 0 of the model clock, in microseconds since the Unix epoch.
const EPOCH: i64 = 1_800_000_000_000_000;
const TICK: i64 = 1_000_000;

fn policy() -> OutboxPolicy {
    // The model's bounds: 3 attempts, lease 2 ticks, backoff 1 tick doubling to 2; the deadline
    // is never reached in the model.
    OutboxPolicy::new(3, 2 * TICK, TICK, 2 * TICK, 1_000 * TICK).unwrap()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    format: String,
    quint_version: String,
    lease_ticks: i64,
    max_attempts: u32,
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
    now: i64,
    deliveries: Vec<Observed>,
    holds: Vec<Hold>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    op: String,
    worker: i64,
    delivery: usize,
    generation: i64,
    result: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Observed {
    id: usize,
    state: String,
    attempts: u32,
    generation: i64,
    available: i64,
    outcome: String,
    effects: u32,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[serde(deny_unknown_fields)]
struct Hold {
    worker: i64,
    delivery: usize,
    generation: i64,
}

struct Shell {
    now: i64,
    facts: [DeliveryFacts; 2],
    outcomes: [&'static str; 2],
    effects: [u32; 2],
    holds: BTreeSet<Hold>,
}

impl Shell {
    fn new() -> Self {
        let fresh = DeliveryFacts {
            state: DeliveryState::Ready,
            attempts: 0,
            generation: ClaimGeneration::UNCLAIMED,
            available_at_micros: EPOCH,
            deadline_from_micros: EPOCH,
        };
        Self {
            now: 0,
            facts: [fresh; 2],
            outcomes: ["none"; 2],
            effects: [0; 2],
            holds: BTreeSet::new(),
        }
    }

    fn at(&self) -> i64 {
        EPOCH + self.now * TICK
    }

    /// Runs one model command through production and returns the production outcome code.
    fn apply(&mut self, command: &Command) -> &'static str {
        let index = command.delivery.wrapping_sub(1);
        match command.op.as_str() {
            "claim" => {
                let facts = self.facts[index];
                match decide_claim(facts, policy(), self.at()) {
                    ClaimDecision::NotEligible => "not_eligible",
                    ClaimDecision::Claim {
                        generation,
                        attempts,
                        lease_until_micros,
                        reclaimed,
                    } => {
                        self.facts[index] = DeliveryFacts {
                            state: DeliveryState::Leased,
                            attempts,
                            generation,
                            available_at_micros: lease_until_micros,
                            ..facts
                        };
                        self.outcomes[index] = "none";
                        self.holds.insert(Hold {
                            worker: command.worker,
                            delivery: command.delivery,
                            generation: generation.get(),
                        });
                        if reclaimed { "reclaimed" } else { "claimed" }
                    }
                    ClaimDecision::DeadLetter { reason, .. } => {
                        self.facts[index].state = DeliveryState::Dead;
                        self.outcomes[index] = reason.code();
                        "dead_at_claim"
                    }
                }
            }
            "complete" => {
                let held = Hold {
                    worker: command.worker,
                    delivery: command.delivery,
                    generation: command.generation,
                };
                assert!(
                    self.holds.remove(&held),
                    "the model completes only a held claim"
                );
                let result = match command.result.as_str() {
                    "applied" => HandlerResult::Applied,
                    "retry" => HandlerResult::Retry(Transient::Database),
                    "fail" => HandlerResult::Fail(DeadReason::InvalidPayload),
                    other => panic!("unknown result {other}"),
                };
                let facts = self.facts[index];
                let generation = ClaimGeneration::restore(command.generation).unwrap();
                let completion =
                    decide_completion(facts, generation, result, policy(), self.at(), 0);
                if let Some(state) = completion.state() {
                    self.facts[index].state = state;
                }
                match completion {
                    Completion::Succeed { .. } => {
                        self.effects[index] += 1;
                        self.outcomes[index] = completion.outcome().unwrap();
                        "succeeded"
                    }
                    Completion::Retry {
                        available_at_micros,
                        ..
                    } => {
                        self.facts[index].available_at_micros = available_at_micros;
                        "retry"
                    }
                    Completion::DeadLetter(_) => {
                        self.outcomes[index] = completion.outcome().unwrap();
                        "dead"
                    }
                    Completion::Skip(_) => panic!("the model has no skip"),
                    Completion::Stale(_) => "stale",
                }
            }
            "crash" => {
                self.holds.retain(|hold| hold.worker != command.worker);
                "crashed"
            }
            "tick" => {
                self.now += 1;
                "ticked"
            }
            "requeue" => match decide_requeue(self.facts[index].state) {
                Requeue::Requeued => {
                    let at = self.at();
                    let facts = &mut self.facts[index];
                    facts.state = DeliveryState::Ready;
                    facts.attempts = 0;
                    facts.available_at_micros = at;
                    facts.deadline_from_micros = at;
                    self.outcomes[index] = "none";
                    "requeued"
                }
                Requeue::Refused(_) => "requeue_refused",
            },
            other => panic!("unknown command {other}"),
        }
    }

    fn observed(&self) -> Vec<Observed> {
        (0..2)
            .map(|index| {
                let facts = self.facts[index];
                Observed {
                    id: index + 1,
                    state: facts.state.code().to_owned(),
                    attempts: facts.attempts,
                    generation: facts.generation.get(),
                    available: (facts.available_at_micros - EPOCH) / TICK,
                    outcome: self.outcomes[index].to_owned(),
                    effects: self.effects[index],
                }
            })
            .collect()
    }
}

fn replay(name: &str, steps: &[Step]) {
    let mut shell = Shell::new();
    for (index, step) in steps.iter().enumerate() {
        let outcome = shell.apply(&step.command);
        let at = format!("{name} step {}", index + 1);
        assert_eq!(outcome, step.outcome, "{at}: outcome");
        assert_eq!(shell.now, step.now, "{at}: clock");
        // Ready/leased rows keep `available`; the model leaves it as is on terminal states too.
        assert_eq!(shell.observed(), step.deliveries, "{at}: deliveries");
        let holds: Vec<Hold> = shell.holds.iter().copied().collect();
        assert_eq!(holds, step.holds, "{at}: holds");
    }
}

#[test]
fn quint_outbox_inputs_replay_against_the_production_decisions() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../behavior/discussion/outbox-fixtures.json"
    ))
    .unwrap();
    assert_eq!(fixture.format, "vink-discussion-outbox-conformance-v1");
    assert_eq!(fixture.quint_version, "0.33.1");
    assert_eq!(fixture.lease_ticks * TICK, policy().lease_micros());
    assert_eq!(fixture.max_attempts, policy().max_attempts());
    assert_eq!(fixture.scenarios.len(), 1);
    for scenario in &fixture.scenarios {
        assert!(!scenario.steps.is_empty());
        replay(&scenario.name, &scenario.steps);
    }
}

#[test]
fn the_unfenced_completion_counterexample_is_refused_by_production() {
    let counterexample: Counterexample = serde_json::from_str(include_str!(
        "../../../behavior/discussion/outbox-unfenced-counterexample.json"
    ))
    .unwrap();
    assert_eq!(counterexample.format, "vink-discussion-counterexample-v1");
    assert_eq!(counterexample.quint_version, "0.33.1");
    assert_eq!(counterexample.status, "violation");
    let (last, prefix) = counterexample.steps.split_last().unwrap();
    // The model's prefix is lawful: production agrees step by step.
    replay("unfenced prefix", prefix);
    let mut shell = Shell::new();
    for step in prefix {
        shell.apply(&step.command);
    }
    // The mutated model applied A's late result a second time; production refuses it.
    assert_eq!(last.outcome, "succeeded");
    assert_eq!(last.deliveries[0].effects, 2);
    assert_eq!(shell.apply(&last.command), "stale");
    assert_eq!(shell.effects[0], 1);
}
