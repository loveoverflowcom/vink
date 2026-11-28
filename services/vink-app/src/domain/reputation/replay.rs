//! Replays the Quint reputation model (`behavior/reputation/reputation.qnt`) through the real
//! pure decision. Each model step is a command and the source facts after it; the replay feeds
//! the same facts to `entitlement::apply` and compares every exported effect. The counterexamples
//! are mutant traces that production must refuse. No SQL, clock or worker is involved.
use serde::Deserialize;
use std::collections::BTreeMap;

use super::calendar::PeriodIndex;
use super::credit::{CreditRecord, CreditState};
use super::entitlement::{Cause, EntitlementFacts, EntitlementRecord, Periods, apply};

#[derive(Deserialize)]
struct Vectors {
    status: String,
    scenarios: Vec<Scenario>,
}
#[derive(Deserialize)]
struct Counterexample {
    invariant: String,
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
    op: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct State {
    selected: Vec<u32>,
    hidden: Vec<u32>,
    public: bool,
    mature: bool,
    deleted: bool,
    week: i64,
    month: i64,
    effective: bool,
    terminal: bool,
    week_credit: Credit,
    month_credit: Credit,
    current_delta: i64,
    week_delta: i64,
    month_delta: i64,
    week_net: BTreeMap<String, i64>,
    month_net: BTreeMap<String, i64>,
}
#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
struct Credit {
    #[serde(rename = "state")]
    name: CreditName,
    pending: i64,
}
#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum CreditName {
    Unstarted,
    Unseen,
    Active,
    Withdrawn,
    Burned,
}

const START: PeriodIndex = PeriodIndex::new(1);

fn model_credit(credit: CreditRecord) -> Credit {
    let (name, pending) = match credit.state() {
        CreditState::Unstarted => (CreditName::Unstarted, 0),
        CreditState::Unseen => (CreditName::Unseen, 0),
        CreditState::Active => (CreditName::Active, 0),
        CreditState::WithdrawnPending(p) => (CreditName::Withdrawn, p.get()),
        CreditState::Burned => (CreditName::Burned, 0),
    };
    Credit { name, pending }
}

fn cause(op: &str) -> Option<Cause> {
    match op {
        "select" | "unselect" => Some(Cause::Community),
        "hide" | "unhide" | "privatize" | "publish" => Some(Cause::Neutral),
        "mature" => Some(Cause::Maturity),
        "delete" => Some(Cause::Terminal),
        "next_week" | "next_month" => None,
        other => panic!("unknown model command {other}"),
    }
}

fn facts(state: &State) -> EntitlementFacts {
    let valid = state
        .selected
        .iter()
        .filter(|anchor| !state.hidden.contains(anchor))
        .count();
    EntitlementFacts {
        selected_anchors: u32::try_from(state.selected.len()).unwrap(),
        valid_selected_anchors: u32::try_from(valid).unwrap(),
        target_public: state.public && !state.deleted,
        actor_mature: state.mature,
    }
}

/// Production effects of one scenario, step by step, in the model's own vocabulary.
#[derive(Debug, PartialEq, Eq)]
struct Observed {
    effective: bool,
    terminal: bool,
    week_credit: Credit,
    month_credit: Credit,
    current_delta: i64,
    week_delta: i64,
    month_delta: i64,
    week_net: BTreeMap<String, i64>,
    month_net: BTreeMap<String, i64>,
}

fn run(steps: &[Step]) -> Vec<Observed> {
    let mut record = EntitlementRecord::NEW;
    let mut week_net: BTreeMap<String, i64> = BTreeMap::new();
    let mut month_net: BTreeMap<String, i64> = BTreeMap::new();
    let mut observed = Vec::new();
    for step in steps {
        let (current_delta, week_delta, month_delta) = match cause(&step.command.op) {
            None => (0, 0, 0),
            Some(cause) => {
                let periods = Periods {
                    week: PeriodIndex::new(step.state.week),
                    month: PeriodIndex::new(step.state.month),
                    week_start: START,
                    month_start: START,
                };
                let applied = apply(record, cause, facts(&step.state), periods);
                record = applied.record;
                for (net, credit) in [
                    (&mut week_net, applied.week),
                    (&mut month_net, applied.month),
                ] {
                    if let Some(period) = credit.period
                        && credit.delta != 0
                    {
                        *net.entry(period.get().to_string()).or_insert(0) += credit.delta;
                    }
                }
                (
                    applied.current.get(),
                    applied.week.delta,
                    applied.month.delta,
                )
            }
        };
        observed.push(Observed {
            effective: record.effective(),
            terminal: record.terminal(),
            week_credit: model_credit(record.week()),
            month_credit: model_credit(record.month()),
            current_delta,
            week_delta,
            month_delta,
            week_net: week_net.clone(),
            month_net: month_net.clone(),
        });
    }
    observed
}

fn expected(step: &Step) -> Observed {
    let s = &step.state;
    Observed {
        effective: s.effective,
        terminal: s.terminal,
        week_credit: s.week_credit,
        month_credit: s.month_credit,
        current_delta: s.current_delta,
        week_delta: s.week_delta,
        month_delta: s.month_delta,
        week_net: s.week_net.clone(),
        month_net: s.month_net.clone(),
    }
}

#[test]
fn quint_reputation_inputs_replay_through_production() {
    let vectors: Vectors = serde_json::from_str(include_str!(
        "../../../behavior/reputation/reputation-vectors.json"
    ))
    .unwrap();
    assert_eq!(vectors.status, "PURE_CORE_CONFORMANCE");
    assert_eq!(
        vectors.scenarios.len(),
        5,
        "every scripted scenario is replayed"
    );
    let mut outcomes = std::collections::BTreeSet::new();
    for scenario in &vectors.scenarios {
        assert!(!scenario.steps.is_empty(), "{}", scenario.name);
        let observed = run(&scenario.steps);
        for (index, (step, seen)) in scenario.steps.iter().zip(&observed).enumerate() {
            outcomes.insert(step.outcome.as_str());
            assert_eq!(
                seen,
                &expected(step),
                "{} step {index} ({})",
                scenario.name,
                step.command.op
            );
        }
    }
    for outcome in [
        "on",
        "off",
        "off_parked",
        "retire",
        "burn",
        "neutral",
        "ignored",
        "time",
    ] {
        assert!(
            outcomes.contains(outcome),
            "no replayed step reaches {outcome}"
        );
    }
}

#[test]
fn quint_reputation_counterexamples_are_refused_by_production() {
    for (file, invariant) in [
        (
            include_str!("../../../behavior/reputation/reputation-relike-counterexample.json"),
            "onlyFirstCreditIsGrowth",
        ),
        (
            include_str!("../../../behavior/reputation/reputation-anchor-counterexample.json"),
            "currentMatchesEffective",
        ),
    ] {
        let counterexample: Counterexample = serde_json::from_str(file).unwrap();
        assert_eq!(counterexample.invariant, invariant);
        let steps = &counterexample.violation[0].steps;
        let last = steps.last().unwrap();
        let observed = run(steps);
        let seen = observed.last().unwrap();
        // The mutant's final step violates the invariant; production must not reproduce it.
        assert_ne!(
            seen,
            &expected(last),
            "{invariant}: production repeats the mutant"
        );
        match invariant {
            "onlyFirstCreditIsGrowth" => {
                assert_eq!(
                    seen.week_delta, 0,
                    "a relike after the withdrawal week earns nothing"
                );
                assert_eq!(seen.week_net.get("3"), None);
            }
            _ => {
                assert_eq!(
                    seen.current_delta, 0,
                    "a second anchor is not a second unit"
                );
                assert!(seen.effective);
            }
        }
    }
}
