//! Quint inputs call the production owner/chain, never a second Rust reducer.
//! Symbolic ticket IDs map to allocated production tickets; the global atomic ID is opaque.
//! The synthetic shell remounts an empty chain. Scope changes alone do not clear DTO/chain:
//! that is an adapter obligation, covered separately by browser tests.
use super::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    op: String,
    intent: String,
    ticket: usize,
    scope: u64,
    input: u64,
    rows: Vec<u64>,
    next: u64,
}

fn cursor(value: u64) -> Option<String> {
    (value != 0).then(|| value.to_string())
}
fn numeric(value: &Option<String>) -> u64 {
    value.as_ref().map_or(0, |s| s.parse().unwrap())
}
fn intent(value: &str) -> Intent {
    match value {
        "initial" => Intent::Initial,
        "refresh" => Intent::Refresh,
        "append" => Intent::Append,
        "restart" => Intent::Restart,
        _ => panic!("unknown intent {value}"),
    }
}
fn intent_name(value: Intent) -> &'static str {
    match value {
        Intent::Initial => "initial",
        Intent::Refresh => "refresh",
        Intent::Append => "append",
        Intent::Restart => "restart",
    }
}

struct Shell {
    owner: ReadOwner<u64>,
    chain: PageChain,
    tickets: BTreeMap<usize, ReadTicket<u64>>,
}
impl Shell {
    fn new() -> Self {
        Self {
            owner: ReadOwner::new(1),
            chain: PageChain::default(),
            tickets: BTreeMap::new(),
        }
    }
    fn apply(&mut self, c: Command) -> &'static str {
        match c.op.as_str() {
            "begin" => {
                if let Some(ticket) = self.owner.begin(intent(&c.intent), cursor(c.input)) {
                    self.tickets.insert(self.tickets.len() + 1, ticket);
                    "started"
                } else {
                    "blocked"
                }
            }
            "page" => {
                if self.tickets.get(&c.ticket).is_some_and(|t| {
                    self.owner.accept_page(
                        t,
                        &mut self.chain,
                        c.rows.iter().map(u64::to_string),
                        cursor(c.next),
                    )
                }) {
                    "accepted"
                } else {
                    "rejected"
                }
            }
            "finish" => {
                if self
                    .tickets
                    .get(&c.ticket)
                    .is_some_and(|t| self.owner.finish(t))
                {
                    "finished"
                } else {
                    "ignored"
                }
            }
            "scope" => {
                self.owner.change_scope(c.scope);
                "changed"
            }
            "invalidate" => {
                self.owner.invalidate();
                "changed"
            }
            "dispose" => {
                self.owner.dispose();
                "disposed"
            }
            "remount" => {
                self.owner = ReadOwner::new(c.scope);
                self.chain = PageChain::default();
                "remounted"
            }
            other => panic!("unknown command {other}"),
        }
    }
    fn state(&self) -> Value {
        let flight = self.owner.flight.as_ref().map_or(0, |t| {
            *self
                .tickets
                .iter()
                .find(|(_, saved)| *saved == t)
                .unwrap()
                .0
        });
        let tickets: BTreeMap<_, _> = self
            .tickets
            .iter()
            .map(|(id, t)| {
                (
                    id.to_string(),
                    json!({"scope": t.scope, "generation": t.generation,
                "intent": intent_name(t.intent), "input": numeric(&t.cursor)}),
                )
            })
            .collect();
        json!({
            "owner": {"scope": self.owner.scope, "generation": self.owner.generation,
                "flight": flight, "disposed": self.owner.disposed},
            "chain": {"ids": self.chain.ids.iter().map(|s| s.parse::<u64>().unwrap()).collect::<Vec<_>>(),
                "cursors": self.chain.cursors.iter().map(|s| s.parse::<u64>().unwrap()).collect::<Vec<_>>(),
                "next": numeric(&self.chain.next_cursor), "pages": self.chain.pages,
                "generation": self.chain.generation},
            "tickets": tickets, "serial": self.tickets.len(),
        })
    }
}

fn replay(source: &str, counterexample: bool) -> usize {
    let fixture: Value = serde_json::from_str(source).unwrap();
    assert_eq!(fixture["quint_version"], "0.33.1");
    assert_eq!(fixture["status"], "PURE_CORE_CONFORMANCE");
    let groups = fixture[if counterexample {
        "violation"
    } else {
        "scenarios"
    }]
    .as_array()
    .unwrap();
    assert!(!groups.is_empty());
    let mut total = 0;
    for group in groups {
        let mut shell = Shell::new();
        let steps = group["steps"].as_array().unwrap();
        assert!(!steps.is_empty());
        for (index, step) in steps.iter().enumerate() {
            let before = shell.state();
            let outcome = shell.apply(serde_json::from_value(step["command"].clone()).unwrap());
            if counterexample && index + 1 == steps.len() {
                assert_eq!(step["outcome"], "accepted");
                assert_eq!(
                    outcome, "rejected",
                    "production must refuse the deliberate violation"
                );
                assert_eq!(
                    shell.state(),
                    before,
                    "refusal must preserve cursor and rows"
                );
            } else {
                assert_eq!(
                    json!(outcome),
                    step["outcome"],
                    "{} step {}",
                    group["name"],
                    index + 1
                );
                assert_eq!(
                    shell.state(),
                    step["state"]["current"],
                    "{} step {}",
                    group["name"],
                    index + 1
                );
            }
            total += 1;
        }
    }
    total
}

#[test]
fn quint_read_lifecycle_inputs_replay_through_production() {
    assert_eq!(
        replay(
            include_str!("../behavior/read-lifecycle-vectors.json"),
            false
        ),
        52
    );
}

#[test]
fn quint_read_lifecycle_counterexamples_are_refused_by_production() {
    assert_eq!(
        replay(
            include_str!("../behavior/read-lifecycle-unfenced-counterexample.json"),
            true
        ),
        4
    );
    assert_eq!(
        replay(
            include_str!("../behavior/read-lifecycle-overlap-counterexample.json"),
            true
        ),
        5
    );
}
