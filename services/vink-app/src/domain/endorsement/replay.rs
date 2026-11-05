//! Generated Quint INPUTS call decide_endorsement, then checked count arithmetic.
//! In-memory rows/receipts and environment changes are a synthetic serialized shell, not SQL,
//! publication/auth implementations. Expected rows are only read AFTER production decisions.
use super::*;
use crate::domain::discussion::lifecycle::{ModerationVersion, Review};
use crate::domain::discussion::{reply_placement, root_placement};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    format: String,
    quint_version: String,
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
struct Step {
    command: Command,
    outcome: String,
    rows: Vec<Row>,
    counts: Vec<Count>,
    public_counts: Vec<Count>,
    receipt_count: usize,
    historical: Option<State>,
    publication: i64,
    shown: Vec<i64>,
    comments_open: bool,
    authority: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    op: String,
    actor: i64,
    id: i64,
    expected: i64,
    key: i64,
    value: bool,
}
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Row {
    actor: i64,
    id: i64,
    selected: bool,
    version: i64,
    order: i64,
}
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct State {
    selected: bool,
    version: i64,
    order: i64,
}
impl From<EndorsementState> for State {
    fn from(s: EndorsementState) -> Self {
        Self {
            selected: s.selected(),
            version: s.version().get(),
            order: s.order().get(),
        }
    }
}
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Count {
    id: i64,
    count: i64,
}
fn principal(n: i64) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(n as u128))
}
fn work() -> WorkId {
    WorkId::from_uuid(Uuid::from_u128(10))
}
fn revision(n: i64) -> RevisionId {
    RevisionId::from_uuid(Uuid::from_u128(n as u128))
}
fn target(n: i64) -> EndorsementTarget {
    match n {
        1 | 2 => EndorsementTarget::Work {
            work: work(),
            revision: revision(n),
        },
        3 | 4 => EndorsementTarget::Comment {
            work: work(),
            comment: CommentId::from_uuid(Uuid::from_u128(n as u128)),
        },
        _ => panic!("model target"),
    }
}
struct Shell {
    rows: BTreeMap<(i64, i64), StoredEndorsement>,
    receipts: BTreeMap<(i64, i64), EndorsementReceipt>,
    counts: BTreeMap<i64, EndorsementCount>,
    publication: i64,
    shown: BTreeSet<i64>,
    comments_open: bool,
    authority: bool,
}
impl Shell {
    fn new() -> Self {
        Self {
            rows: BTreeMap::new(),
            receipts: BTreeMap::new(),
            counts: (1..=4).map(|id| (id, EndorsementCount::ZERO)).collect(),
            publication: 1,
            shown: BTreeSet::from([3, 4]),
            comments_open: true,
            authority: true,
        }
    }
    fn target_facts(&self, id: i64) -> EndorsementTargetFacts {
        if id <= 2 {
            return EndorsementTargetFacts::work(
                work(),
                principal(1),
                (self.publication != 0).then(|| revision(self.publication)),
            );
        }
        let thread = ThreadId::from_uuid(Uuid::from_u128(100));
        let root = root_placement(thread, CommentId::from_uuid(Uuid::from_u128(3)));
        let placement = if id == 3 {
            root
        } else {
            reply_placement(thread, CommentId::from_uuid(Uuid::from_u128(4)), root).unwrap()
        };
        let state = if self.shown.contains(&id) {
            CommentState::restore(Review::Approved, None, ModerationVersion::FIRST)
        } else {
            CommentState::SUBMITTED
        };
        EndorsementTargetFacts::comment(
            work(),
            thread,
            placement,
            principal(if id == 3 { 2 } else { 3 }),
            state,
            self.publication != 0,
        )
        .unwrap()
    }
    fn apply(&mut self, c: &Command) -> (&'static str, Option<State>) {
        if c.op != "set" {
            match c.op.as_str() {
                "publish" => self.publication = c.id,
                "show" => {
                    if c.value {
                        self.shown.insert(c.id);
                    } else {
                        self.shown.remove(&c.id);
                    }
                }
                "mode" => self.comments_open = c.value,
                "authority" => self.authority = c.value,
                _ => panic!("model op"),
            }
            return ("facts_set", None);
        }
        let command = EndorsementCommand {
            key: EndorsementCommandKey::from_uuid(Uuid::from_u128(c.key as u128)),
            target: target(c.id),
            expected: AcceptedOrder::new(c.expected).unwrap(),
            intent: if c.value {
                EndorsementIntent::Select
            } else {
                EndorsementIntent::Remove
            },
        };
        let facts = EndorsementFacts {
            can_endorse: self.authority,
            target: Some(self.target_facts(c.id)),
            current: self.rows.get(&(c.actor, c.id)).copied(),
            receipt: self.receipts.get(&(c.actor, c.key)).copied(),
        };
        match decide_endorsement(principal(c.actor), command, facts) {
            EndorsementOutcome::Accepted(w) => {
                let count = self.counts[&c.id].apply(w.delta()).unwrap();
                self.rows.insert((c.actor, c.id), w.next());
                self.receipts.insert((c.actor, c.key), w.receipt());
                self.counts.insert(c.id, count);
                (
                    if w.delta() == EndorsementDelta::None {
                        "accepted_unchanged"
                    } else {
                        "accepted_changed"
                    },
                    None,
                )
            }
            EndorsementOutcome::Replay {
                committed,
                observed,
            } => {
                assert_eq!(observed, self.rows[&(c.actor, c.id)].state());
                ("replay", Some(committed.into()))
            }
            EndorsementOutcome::Conflict { observed } => {
                assert_eq!(
                    observed,
                    self.rows
                        .get(&(c.actor, c.id))
                        .map_or(EndorsementState::INITIAL, |r| r.state())
                );
                ("conflict", None)
            }
            EndorsementOutcome::Refused(reason) => (
                match reason {
                    EndorsementRefusal::AuthorityUnavailable => "authority_unavailable",
                    EndorsementRefusal::TargetUnavailable => "target_unavailable",
                    EndorsementRefusal::OwnContent => "own_content",
                    EndorsementRefusal::KeyReused => "key_reused",
                    other => panic!("unexpected model refusal {other:?}"),
                },
                None,
            ),
        }
    }
}

#[test]
fn quint_endorsement_inputs_replay_against_production() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../behavior/discussion/endorsement-fixtures.json"
    ))
    .unwrap();
    assert_eq!(fixture.format, "vink-endorsement-conformance-v1");
    assert_eq!(fixture.quint_version, "0.33.1");
    assert_eq!(fixture.scenarios.len(), 1);
    let mut outcomes = BTreeSet::new();
    for scenario in fixture.scenarios {
        assert_eq!(scenario.name, "ordering_anchor_privacy");
        assert_eq!(scenario.steps.len(), 33);
        let mut shell = Shell::new();
        for (i, step) in scenario.steps.into_iter().enumerate() {
            let context = format!("{} step {}", scenario.name, i + 1);
            let (outcome, history) = shell.apply(&step.command);
            outcomes.insert(outcome);
            assert_eq!(outcome, step.outcome, "{context}");
            assert_eq!(history, step.historical, "{context}");
            let rows: Vec<_> = shell
                .rows
                .iter()
                .map(|(&(actor, id), record)| {
                    let s = record.state();
                    Row {
                        actor,
                        id,
                        selected: s.selected(),
                        version: s.version().get(),
                        order: s.order().get(),
                    }
                })
                .collect();
            assert_eq!(rows, step.rows, "{context}");
            let counts: Vec<_> = shell
                .counts
                .iter()
                .map(|(&id, n)| Count { id, count: n.get() })
                .collect();
            assert_eq!(counts, step.counts, "{context}");
            let public: Vec<_> = shell
                .counts
                .iter()
                .filter_map(|(&id, &n)| {
                    public_count(target(id), Some(shell.target_facts(id)), n)
                        .map(|n| Count { id, count: n.get() })
                })
                .collect();
            assert_eq!(public, step.public_counts, "{context}");
            assert_eq!(shell.receipts.len(), step.receipt_count, "{context}");
            assert_eq!(shell.publication, step.publication, "{context}");
            assert_eq!(
                shell.shown.iter().copied().collect::<Vec<_>>(),
                step.shown,
                "{context}"
            );
            assert_eq!(shell.comments_open, step.comments_open, "{context}");
            assert_eq!(shell.authority, step.authority, "{context}");
        }
    }
    assert_eq!(
        outcomes.len(),
        9,
        "no scenario or outcome may disappear silently"
    );
}
