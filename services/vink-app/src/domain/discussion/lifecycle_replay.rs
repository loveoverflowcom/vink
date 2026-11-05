//! Replays `behavior/discussion/lifecycle.qnt` scenario inputs through the production decisions.
//!
//! Only model INPUTS reach Rust: each command goes into the same pure functions the PostgreSQL
//! store calls, in the store's order (`persistence/discussion.rs`). The generated state is used
//! solely as an oracle afterwards. Synthetic shell, labelled: comment rows, receipts, the thread's
//! settings and the active grant set live in memory here, and grant/revoke mirror the operator
//! commands' idempotent outcomes; their locking and durability belong to the postgres lane.
//! Receipts carry the real [`PayloadDigest`] of the model's (work, target, body), so replay,
//! key reuse and the receipt-before-gates order run through `decide_submission` itself.
//! The COM-07 counterexample of the former no-op rule is replayed too: production must refuse it.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use uuid::Uuid;

use super::CommentPlacement;
use super::body::CommentBody;
use super::lifecycle::{
    CommentState, LifecycleError, ModerationVersion, Removal, Review, Transition,
};
use super::moderation::ModerationAction;
use super::policy::{
    Denied, authorize_mode_change, authorize_moderation, authorize_writer_removal,
};
use super::settings::{
    CommentsMode, DiscussionSettings, ModeChange, SettingsError, SettingsGeneration,
};
use super::submission::{
    PayloadDigest, Receipt, SubmissionDecision, SubmissionFacts, TargetFacts, decide_submission,
};
use super::visibility::{CommentView, view};
use crate::domain::ids::{CommentId, PrincipalId, ThreadId, WorkId};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Conformance {
    format: String,
    quint_version: String,
    work_author: u128,
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
    /// The comment a submit created or replayed.
    answer: Option<u128>,
    mode: String,
    generation: i64,
    grants: Vec<u128>,
    receipts: Vec<ExpectedReceipt>,
    comments: Vec<ExpectedComment>,
}

/// A model counterexample (`vink-discussion-counterexample-v1`) with lifecycle steps.
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
struct ExpectedReceipt {
    actor: u128,
    key: u128,
    target: Option<u128>,
    body: u8,
    comment: u128,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    op: String,
    actor: u128,
    id: u128,
    target: u128,
    expected: i64,
    value: String,
    key: u128,
    body: u8,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExpectedComment {
    id: u128,
    writer: u128,
    target: Option<u128>,
    review: String,
    removal: String,
    version: i64,
    /// What viewers 0 (anonymous), 1, 2 and 3 see.
    views: Vec<String>,
}

struct Row {
    writer: PrincipalId,
    target: Option<u128>,
    placement: CommentPlacement,
    state: CommentState,
}

struct StoredReceipt {
    receipt: Receipt,
    target: Option<u128>,
    body: u8,
}

struct Shell {
    thread: ThreadId,
    work: WorkId,
    work_author: PrincipalId,
    settings: DiscussionSettings,
    grants: BTreeSet<u128>,
    rows: BTreeMap<u128, Row>,
    /// Receipts by (actor, key), the store's (actor, operation, key) for the one operation here.
    receipts: BTreeMap<(u128, u128), StoredReceipt>,
    answer: Option<u128>,
}

/// The model's body texts 1 and 2; NFC/NFD spellings of one text share a digest.
fn body(index: u8) -> CommentBody {
    let text = match index {
        1 => "Hay qua\u{0301}",
        2 => "Cảm ơn tác giả",
        other => panic!("unknown body {other}"),
    };
    CommentBody::parse(text).unwrap()
}

fn principal(id: u128) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(id))
}

fn comment(id: u128) -> CommentId {
    CommentId::from_uuid(Uuid::from_u128(id))
}

fn denied(denied: Denied) -> &'static str {
    match denied {
        Denied::NotWorkAuthor => "not_work_author",
        Denied::NotCommentWriter => "not_comment_writer",
        Denied::Hidden => "hidden",
        Denied::NotModerator => "not_moderator",
        Denied::Recused => "recused",
    }
}

fn transition(decided: Result<Transition, LifecycleError>) -> (&'static str, Option<CommentState>) {
    match decided {
        Ok(Transition::Changed(next)) => ("changed", Some(next)),
        Ok(Transition::Unchanged(_)) => ("unchanged", None),
        Err(LifecycleError::Removed) => ("removed", None),
        Err(LifecycleError::Stale { .. }) => ("stale", None),
        Err(LifecycleError::VersionExhausted) => panic!("model versions stay small"),
    }
}

impl Shell {
    fn new(work_author: u128) -> Self {
        Self {
            thread: ThreadId::from_uuid(Uuid::from_u128(10)),
            work: WorkId::from_uuid(Uuid::from_u128(77)),
            work_author: principal(work_author),
            // A new public work starts open (owner decision 03/11/2026).
            settings: DiscussionSettings::INITIAL,
            grants: BTreeSet::new(),
            rows: BTreeMap::new(),
            receipts: BTreeMap::new(),
            answer: None,
        }
    }

    fn apply(&mut self, command: &Command) -> &'static str {
        self.answer = None;
        let actor = principal(command.actor);
        match command.op.as_str() {
            "submit" => self.submit(command),
            "set_mode" => {
                if let Err(refused) = authorize_mode_change(actor, self.work_author) {
                    return denied(refused);
                }
                let mode = match command.value.as_str() {
                    "open" => CommentsMode::Open,
                    "closed" => CommentsMode::Closed,
                    other => panic!("unknown mode {other}"),
                };
                let expected = SettingsGeneration::new(command.expected).unwrap();
                match self.settings.change_mode(expected, mode) {
                    Ok(decided) => {
                        // The store writes exactly what the decision says to write.
                        if let Some(next) = decided.write() {
                            self.settings = next;
                        }
                        match decided {
                            ModeChange::Changed(_) => "changed",
                            ModeChange::Reaffirmed(_) => "reaffirmed",
                            ModeChange::AlreadyCurrent(_) => "already_current",
                        }
                    }
                    Err(SettingsError::Stale { .. }) => "stale",
                    Err(SettingsError::GenerationExhausted) => panic!("small generations"),
                }
            }
            "remove_own" => {
                let row = self
                    .rows
                    .get_mut(&command.id)
                    .expect("model guards existence");
                // The work is public throughout, so the store's view is the viewer projection.
                let actor_view = view(row.writer, row.state, Some(actor));
                if let Err(refused) = authorize_writer_removal(actor, row.writer, actor_view) {
                    return denied(refused);
                }
                let (outcome, next) = transition(row.state.remove_by_author());
                row.state = next.unwrap_or(row.state);
                outcome
            }
            "moderate" => {
                // The store checks the grant before the comment lookup; the same function decides.
                let holds_grant = self.grants.contains(&command.actor);
                let row = self
                    .rows
                    .get_mut(&command.id)
                    .expect("model guards existence");
                if let Err(refused) =
                    authorize_moderation(actor, holds_grant, row.writer, self.work_author)
                {
                    return denied(refused);
                }
                let action = match command.value.as_str() {
                    "approve" => ModerationAction::Approve,
                    "reject" => ModerationAction::Reject,
                    "remove" => ModerationAction::Remove,
                    other => panic!("unknown action {other}"),
                };
                let expected = ModerationVersion::new(command.expected).unwrap();
                let (outcome, next) = transition(action.apply(row.state, expected));
                row.state = next.unwrap_or(row.state);
                outcome
            }
            "grant" if self.grants.insert(command.actor) => "granted",
            "grant" => "already_active",
            "revoke" if self.grants.remove(&command.actor) => "revoked",
            "revoke" => "not_active",
            other => panic!("unknown op {other}"),
        }
    }

    fn submit(&mut self, command: &Command) -> &'static str {
        let requested = (command.target != 0).then(|| comment(command.target));
        let target = self.rows.get(&command.target).map(|row| TargetFacts {
            placement: row.placement,
            state: row.state,
        });
        let key = (command.actor, command.key);
        let facts = SubmissionFacts {
            receipt: self.receipts.get(&key).map(|stored| stored.receipt),
            work_public: true,
            settings: self.settings,
            target,
        };
        let digest = PayloadDigest::of_submission(self.work, requested, &body(command.body));
        match decide_submission(self.thread, comment(command.id), digest, requested, facts) {
            SubmissionDecision::Insert(placement) => {
                assert_eq!(placement.id(), comment(command.id));
                // The store commits the receipt together with the comment, never alone.
                self.receipts.insert(
                    key,
                    StoredReceipt {
                        receipt: Receipt {
                            digest,
                            comment_id: placement.id(),
                        },
                        target: requested.map(|_| command.target),
                        body: command.body,
                    },
                );
                self.answer = Some(command.id);
                self.rows.insert(
                    command.id,
                    Row {
                        writer: principal(command.actor),
                        target: requested.map(|_| command.target),
                        placement,
                        state: CommentState::SUBMITTED,
                    },
                );
                "created"
            }
            SubmissionDecision::Replay(id) => {
                self.answer = Some(id.as_uuid().as_u128());
                "replayed"
            }
            SubmissionDecision::KeyReused => "key_reused",
            SubmissionDecision::CommentsClosed => "closed",
            SubmissionDecision::TargetUnavailable => "target_unavailable",
            other => panic!("unexpected submission decision {other:?}"),
        }
    }

    fn observed_receipts(&self) -> Vec<ExpectedReceipt> {
        self.receipts
            .iter()
            .map(|(&(actor, key), stored)| ExpectedReceipt {
                actor,
                key,
                target: stored.target,
                body: stored.body,
                comment: stored.receipt.comment_id.as_uuid().as_u128(),
            })
            .collect()
    }

    fn observed(&self) -> Vec<ExpectedComment> {
        self.rows
            .iter()
            .map(|(&id, row)| ExpectedComment {
                id,
                writer: row.writer.as_uuid().as_u128(),
                target: row.target,
                review: match row.state.review() {
                    Review::Pending => "pending",
                    Review::Approved => "approved",
                    Review::Rejected => "rejected",
                }
                .into(),
                removal: match row.state.removal() {
                    None => "none",
                    Some(Removal::ByAuthor) => "author",
                    Some(Removal::ByModeration) => "moderation",
                }
                .into(),
                version: row.state.version().get(),
                views: [None, Some(1), Some(2), Some(3)]
                    .into_iter()
                    .map(|viewer| {
                        match view(row.writer, row.state, viewer.map(principal)) {
                            CommentView::Public => "public",
                            CommentView::OwnPending => "own_pending",
                            CommentView::OwnRejected => "own_rejected",
                            CommentView::Unavailable => "unavailable",
                        }
                        .into()
                    })
                    .collect(),
            })
            .collect()
    }
}

#[test]
fn quint_lifecycle_inputs_replay_against_the_production_decisions() {
    let fixture: Conformance = serde_json::from_str(include_str!(
        "../../../behavior/discussion/lifecycle-fixtures.json"
    ))
    .unwrap();
    let tool: serde_json::Value =
        serde_json::from_str(include_str!("../../../behavior/discussion/package.json")).unwrap();
    assert_eq!(fixture.format, "vink-discussion-lifecycle-conformance-v1");
    assert_eq!(
        fixture.quint_version,
        tool["dependencies"]["@informalsystems/quint"]
    );
    let mut outcomes = BTreeSet::new();
    for scenario in &fixture.scenarios {
        assert!(!scenario.steps.is_empty(), "{}", scenario.name);
        let mut shell = Shell::new(fixture.work_author);
        for (index, step) in scenario.steps.iter().enumerate() {
            let at = format!("{} step {index}", scenario.name);
            let outcome = shell.apply(&step.command);
            assert_eq!(outcome, step.outcome, "{at}");
            assert_eq!(shell.answer, step.answer, "{at}");
            outcomes.insert(outcome);
            let mode = match shell.settings.mode() {
                CommentsMode::Open => "open",
                CommentsMode::Closed => "closed",
            };
            assert_eq!(mode, step.mode, "{at}");
            assert_eq!(shell.settings.generation().get(), step.generation, "{at}");
            assert_eq!(
                shell.grants.iter().copied().collect::<Vec<_>>(),
                step.grants,
                "{at}"
            );
            assert_eq!(shell.observed_receipts(), step.receipts, "{at}");
            assert_eq!(shell.observed(), step.comments, "{at}");
        }
    }
    // Non-vacuous: every production refusal and outcome the model distinguishes was replayed.
    assert_eq!(outcomes.len(), 20, "{outcomes:?}");
}

/// COM-07 "tab cũ không đè lựa chọn mới": the counterexample of the former rule, where asking for
/// the current mode was a no-op at any generation. Its inputs through production must not
/// reproduce the violation: tab B's request for the current mode at generation 2 is accepted and
/// fences it, so tab A's delayed reopen at generation 2 is stale and comments stay closed.
#[test]
fn the_no_op_mode_counterexample_is_refused_by_production() {
    let counterexample: Counterexample = serde_json::from_str(include_str!(
        "../../../behavior/discussion/mode-noop-fence-counterexample.json"
    ))
    .unwrap();
    let tool: serde_json::Value =
        serde_json::from_str(include_str!("../../../behavior/discussion/package.json")).unwrap();
    assert_eq!(counterexample.format, "vink-discussion-counterexample-v1");
    assert_eq!(
        counterexample.quint_version,
        tool["dependencies"]["@informalsystems/quint"]
    );
    assert_eq!(counterexample.status, "violation");
    assert!(
        counterexample
            .command
            .iter()
            .any(|arg| arg == "--step=mutatedNoopModeStep")
    );
    let model: Vec<_> = counterexample
        .steps
        .iter()
        .map(|step| step.outcome.as_str())
        .collect();
    assert_eq!(
        model,
        ["changed", "already_current", "changed"],
        "the mutated model reopens comments"
    );
    let mut shell = Shell::new(1);
    let production: Vec<_> = counterexample
        .steps
        .iter()
        .map(|step| shell.apply(&step.command))
        .collect();
    assert_eq!(production, ["changed", "reaffirmed", "stale"]);
    assert_eq!(
        shell.settings,
        DiscussionSettings::restore(CommentsMode::Closed, SettingsGeneration::new(3).unwrap())
    );
}
