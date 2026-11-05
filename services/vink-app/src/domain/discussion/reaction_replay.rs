//! Replays `behavior/discussion/reaction.qnt` scenario inputs through the production reaction
//! decisions (D2T-INV-08).
//!
//! Only model INPUTS reach Rust: each `react` goes into [`decide_reaction`] and, when applied,
//! [`ReactionTally::apply`]; the generated state is used solely as an oracle afterwards.
//! Synthetic shell, labelled: comment rows, reactions and tallies live in memory, a moderator
//! applies `ModerationAction` at the comment's current version and the work author changes the
//! mode at the current generation (their authority belongs to `lifecycle.qnt`). No reaction store
//! exists yet; its locking and durability are #43. The counterexample of the former no-op rule
//! is replayed too: production must refuse it.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use uuid::Uuid;

use super::lifecycle::{CommentState, LifecycleError, Transition};
use super::moderation::ModerationAction;
use super::reaction::{
    ActorReaction, ReactionDecision, ReactionFacts, ReactionGeneration, ReactionKind,
    ReactionTally, decide_reaction, public_tally,
};
use super::settings::{CommentsMode, DiscussionSettings, ModeChange};
use crate::domain::ids::PrincipalId;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Conformance {
    format: String,
    quint_version: String,
    work_author: u128,
    writers: BTreeMap<String, u128>,
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
    mode: String,
    comments: Vec<ExpectedComment>,
    reactions: Vec<ExpectedReaction>,
    visible_tally: Vec<ExpectedTally>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    op: String,
    actor: u128,
    id: u128,
    expected: i64,
    value: String,
}

/// A model counterexample (`vink-discussion-counterexample-v1`) with reaction steps.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Counterexample {
    format: String,
    quint_version: String,
    command: Vec<String>,
    status: String,
    steps: Vec<Step>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExpectedComment {
    id: u128,
    status: String,
    tally: BTreeMap<String, u32>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExpectedReaction {
    actor: u128,
    id: u128,
    kind: String,
    generation: i64,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExpectedTally {
    id: u128,
    tally: BTreeMap<String, u32>,
}

/// The two kinds the model uses; the replay asserts the other production kinds stay at zero.
const MODELLED: [ReactionKind; 2] = [ReactionKind::Heart, ReactionKind::Moved];

struct Row {
    writer: PrincipalId,
    state: CommentState,
    tally: ReactionTally,
}

struct Shell {
    settings: DiscussionSettings,
    rows: BTreeMap<u128, Row>,
    /// (actor, comment) -> stored reaction; absent means [`ActorReaction::NONE`].
    reactions: BTreeMap<(u128, u128), ActorReaction>,
}

fn principal(id: u128) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(id))
}

fn wanted(value: &str) -> Option<ReactionKind> {
    match value {
        "none" => None,
        code => Some(ReactionKind::parse(code).expect("model kinds are production codes")),
    }
}

fn counts(tally: ReactionTally) -> BTreeMap<String, u32> {
    for kind in ReactionKind::ALL {
        if !MODELLED.contains(&kind) {
            assert_eq!(tally.count(kind), 0, "{kind:?} is outside the model");
        }
    }
    MODELLED
        .into_iter()
        .map(|kind| (kind.code().to_owned(), tally.count(kind)))
        .collect()
}

impl Shell {
    /// The model's two pending comments by their writers; nobody has reacted yet.
    fn new(writers: &BTreeMap<String, u128>) -> Self {
        Self {
            settings: DiscussionSettings::INITIAL,
            rows: writers
                .iter()
                .map(|(id, &writer)| {
                    (
                        id.parse().unwrap(),
                        Row {
                            writer: principal(writer),
                            state: CommentState::SUBMITTED,
                            tally: ReactionTally::EMPTY,
                        },
                    )
                })
                .collect(),
            reactions: BTreeMap::new(),
        }
    }

    fn apply(&mut self, command: &Command) -> &'static str {
        match command.op.as_str() {
            "react" => self.react(command),
            "approve" | "remove" => {
                let row = self.rows.get_mut(&command.id).expect("model comment");
                let action = if command.op == "approve" {
                    ModerationAction::Approve
                } else {
                    ModerationAction::Remove
                };
                match action.apply(row.state, row.state.version()) {
                    Ok(Transition::Changed(next)) => {
                        row.state = next;
                        "moderated"
                    }
                    Ok(Transition::Unchanged(_)) | Err(LifecycleError::Removed) => "noop",
                    Err(other) => panic!("synthetic moderation at the current version: {other:?}"),
                }
            }
            "set_mode" => {
                let mode = match command.value.as_str() {
                    "open" => CommentsMode::Open,
                    "closed" => CommentsMode::Closed,
                    other => panic!("unknown mode {other}"),
                };
                match self.settings.change_mode(self.settings.generation(), mode) {
                    Ok(ModeChange::Changed(next)) => {
                        self.settings = next;
                        "mode_set"
                    }
                    Ok(ModeChange::Reaffirmed(next)) => {
                        self.settings = next;
                        "mode_reaffirmed"
                    }
                    other => panic!("current generation: {other:?}"),
                }
            }
            other => panic!("unknown op {other}"),
        }
    }

    fn react(&mut self, command: &Command) -> &'static str {
        let key = (command.actor, command.id);
        let row = self.rows.get_mut(&command.id).expect("model comment");
        let facts = ReactionFacts {
            work_public: true,
            settings: self.settings,
            target: Some((row.writer, row.state)),
            current: self
                .reactions
                .get(&key)
                .copied()
                .unwrap_or(ActorReaction::NONE),
        };
        let expected = ReactionGeneration::new(command.expected).expect("model generations");
        match decide_reaction(
            principal(command.actor),
            expected,
            wanted(&command.value),
            facts,
        ) {
            ReactionDecision::Apply { next, delta } => {
                // The store writes both or neither; a refused tally would roll back the change.
                row.tally = row
                    .tally
                    .apply(delta)
                    .expect("the tally follows the reactions");
                self.reactions.insert(key, next);
                "changed"
            }
            ReactionDecision::Reaffirm { next } => {
                // Only the generation advances; the tally is untouched.
                self.reactions.insert(key, next);
                "reaffirmed"
            }
            ReactionDecision::AlreadyCurrent(_) => "already_current",
            ReactionDecision::Stale { .. } => "stale",
            ReactionDecision::TargetUnavailable => "target_unavailable",
            ReactionDecision::OwnComment => "own_comment",
            ReactionDecision::CommentsClosed => "closed",
            other => panic!("unexpected reaction decision {other:?}"),
        }
    }

    fn comments(&self) -> Vec<ExpectedComment> {
        self.rows
            .iter()
            .map(|(&id, row)| ExpectedComment {
                id,
                status: if row.state.is_public() {
                    "public"
                } else if row.state.is_present() {
                    "pending"
                } else {
                    "removed"
                }
                .into(),
                tally: counts(row.tally),
            })
            .collect()
    }

    fn reactions(&self, actors: &[u128]) -> Vec<ExpectedReaction> {
        let mut all = Vec::new();
        for &id in self.rows.keys() {
            for &actor in actors {
                let reaction = self
                    .reactions
                    .get(&(actor, id))
                    .copied()
                    .unwrap_or(ActorReaction::NONE);
                all.push(ExpectedReaction {
                    actor,
                    id,
                    kind: reaction.kind().map_or("none", ReactionKind::code).into(),
                    generation: reaction.generation().get(),
                });
            }
        }
        all
    }

    fn visible(&self) -> Vec<ExpectedTally> {
        self.rows
            .iter()
            .filter_map(|(&id, row)| {
                public_tally(row.state, row.tally).map(|tally| ExpectedTally {
                    id,
                    tally: counts(tally),
                })
            })
            .collect()
    }
}

#[test]
fn quint_reaction_inputs_replay_against_the_production_decisions() {
    let fixture: Conformance = serde_json::from_str(include_str!(
        "../../../behavior/discussion/reaction-fixtures.json"
    ))
    .unwrap();
    let tool: serde_json::Value =
        serde_json::from_str(include_str!("../../../behavior/discussion/package.json")).unwrap();
    assert_eq!(fixture.format, "vink-discussion-reaction-conformance-v1");
    assert_eq!(
        fixture.quint_version,
        tool["dependencies"]["@informalsystems/quint"]
    );
    assert_eq!(fixture.work_author, 1);
    let actors = [1, 2, 3];
    let mut outcomes = BTreeSet::new();
    for scenario in &fixture.scenarios {
        assert!(!scenario.steps.is_empty(), "{}", scenario.name);
        let mut shell = Shell::new(&fixture.writers);
        for (index, step) in scenario.steps.iter().enumerate() {
            let at = format!("{} step {index}", scenario.name);
            let outcome = shell.apply(&step.command);
            assert_eq!(outcome, step.outcome, "{at}");
            outcomes.insert(outcome);
            let mode = match shell.settings.mode() {
                CommentsMode::Open => "open",
                CommentsMode::Closed => "closed",
            };
            assert_eq!(mode, step.mode, "{at}");
            assert_eq!(shell.comments(), step.comments, "{at}");
            assert_eq!(shell.reactions(&actors), step.reactions, "{at}");
            assert_eq!(shell.visible(), step.visible_tally, "{at}");
        }
    }
    // Non-vacuous: every reaction refusal and outcome the model distinguishes was replayed.
    assert_eq!(outcomes.len(), 10, "{outcomes:?}");
}

/// The counterexample of the former rule, where asking for the held reaction was a no-op at any
/// generation. Its inputs through production must not reproduce the violation: tab B's request
/// for no reaction at generation 1 is accepted and fences it, so tab A's delayed Heart at
/// generation 1 is stale and no reaction is counted.
#[test]
fn the_no_op_reaction_counterexample_is_refused_by_production() {
    let fixture: Conformance = serde_json::from_str(include_str!(
        "../../../behavior/discussion/reaction-fixtures.json"
    ))
    .unwrap();
    let counterexample: Counterexample = serde_json::from_str(include_str!(
        "../../../behavior/discussion/reaction-noop-fence-counterexample.json"
    ))
    .unwrap();
    assert_eq!(counterexample.format, "vink-discussion-counterexample-v1");
    assert_eq!(counterexample.quint_version, fixture.quint_version);
    assert_eq!(counterexample.status, "violation");
    assert!(
        counterexample
            .command
            .iter()
            .any(|arg| arg == "--step=mutatedNoopStep")
    );
    let model: Vec<_> = counterexample
        .steps
        .iter()
        .map(|step| step.outcome.as_str())
        .collect();
    assert_eq!(
        model,
        ["moderated", "already_current", "changed"],
        "the mutated model applies the older Heart"
    );
    let mut shell = Shell::new(&fixture.writers);
    let production: Vec<_> = counterexample
        .steps
        .iter()
        .map(|step| shell.apply(&step.command))
        .collect();
    assert_eq!(production, ["moderated", "reaffirmed", "stale"]);
    assert_eq!(
        shell.reactions.get(&(3, 1)),
        Some(&ActorReaction::restore(
            None,
            ReactionGeneration::new(2).unwrap()
        ))
    );
    assert_eq!(shell.rows[&1].tally, ReactionTally::EMPTY);
}
