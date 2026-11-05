//! Replays `behavior/discussion/endorsement.qnt` scenario inputs through the production
//! endorsement decisions (LVE-INV-03..08, LVE-INV-12).
//!
//! Only model INPUTS reach Rust: each delivered request goes into [`decide_work_endorsement`] or
//! [`decide_comment_endorsement`]; an accepted change's delta goes through
//! [`EndorsementCount::apply`]; summaries and own views come from the production projections.
//! The generated state is used solely as an oracle afterwards.
//! Synthetic shell, labelled: requests wait in an in-memory network keyed by the model's stamp
//! (the client's version knowledge stays in the model); endorsement rows and per-anchor counts
//! live in memory; the publication pointer is set directly (no publish/withdraw command exists);
//! a moderator applies `ModerationAction` at the comment's current version and the work author
//! changes the mode at the current generation. No endorsement store exists yet: its locking and
//! durability are #73 PR3.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use uuid::Uuid;

use super::comment::{
    CommentDecision, CommentFacts, CommentIntent, CommentRequest, comment_summary,
    decide_comment_endorsement, own_comment_view,
};
use super::work::{
    WorkDecision, WorkFacts, WorkIntent, WorkRequest, decide_work_endorsement, own_work_view,
    work_summary,
};
use super::{
    CommentAnchor, Endorsement, EndorsementCount, EndorsementVersion, OwnView, POLICY, Summary,
    Transition,
};
use crate::domain::discussion::lifecycle::{
    CommentState, LifecycleError, Transition as Moderation,
};
use crate::domain::discussion::moderation::ModerationAction;
use crate::domain::discussion::settings::{CommentsMode, DiscussionSettings, ModeChange};
use crate::domain::ids::{CommentId, PrincipalId, RevisionId, WorkId};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Conformance {
    format: String,
    quint_version: String,
    policy: String,
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
    answer: i64,
    published: u128,
    mode: String,
    comments: BTreeMap<String, String>,
    rows: BTreeMap<String, Row>,
    counts: BTreeMap<String, u32>,
    summary: BTreeMap<String, Option<u32>>,
    views: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Command {
    op: String,
    stamp: i64,
    stream: u128,
    actor: u128,
    target: u128,
    expected: i64,
    desire: String,
    observed: u128,
    keep: bool,
    heard: bool,
    value: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Row {
    sel: u128,
    version: i64,
}

const ACTORS: [u128; 3] = [1, 2, 3];
const REVISIONS: [u128; 2] = [1, 2];
/// Target 0 is the work; 1 and 2 are a root comment and a flat reply to it.
const COMMENTS: [u128; 2] = [1, 2];

fn principal(id: u128) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(id))
}

fn revision(id: u128) -> RevisionId {
    RevisionId::from_uuid(Uuid::from_u128(id))
}

fn comment_id(id: u128) -> CommentId {
    CommentId::from_uuid(Uuid::from_u128(id))
}

fn work() -> WorkId {
    WorkId::from_uuid(Uuid::from_u128(100))
}

fn view_code(view: OwnView) -> &'static str {
    match view {
        OwnView::Unavailable => "unavailable",
        OwnView::NotEndorsed => "none",
        OwnView::Endorsed => "endorsed",
        OwnView::EndorsedEarlierRevision => "earlier_revision",
    }
}

fn summary_count(summary: Summary) -> Option<u32> {
    match summary {
        Summary::Count(count) => Some(count.get()),
        Summary::Unavailable => None,
    }
}

/// The answer a client receives: the version it may continue from, none for a refusal.
fn settled<A: Copy + Eq>(transition: Transition<A>) -> (&'static str, i64) {
    match transition {
        Transition::Changed { next, .. } => ("changed", next.version().get()),
        Transition::Reaffirmed { next } => ("reaffirmed", next.version().get()),
        Transition::AlreadyCurrent(current) => ("already_current", current.version().get()),
        Transition::Stale { current } => ("stale", current.version().get()),
        Transition::VersionExhausted => panic!("model versions are small"),
    }
}

struct Shell {
    work_author: u128,
    writers: BTreeMap<u128, u128>,
    published: Option<u128>,
    comments: BTreeMap<u128, CommentState>,
    settings: DiscussionSettings,
    /// actor -> stored work endorsement; absent means [`Endorsement::ABSENT`].
    work_rows: BTreeMap<u128, Endorsement<RevisionId>>,
    /// (actor, comment) -> stored comment endorsement.
    comment_rows: BTreeMap<(u128, u128), Endorsement<CommentAnchor>>,
    /// Per-anchor projections: revision -> count, comment -> count.
    work_counts: BTreeMap<u128, EndorsementCount>,
    comment_counts: BTreeMap<u128, EndorsementCount>,
    /// The network: issued requests by stamp.
    inflight: BTreeMap<i64, Command>,
}

impl Shell {
    fn work_facts(&self) -> Option<WorkFacts> {
        Some(WorkFacts {
            work: work(),
            author: principal(self.work_author),
            published: self.published.map(revision),
        })
    }

    fn comment_facts(&self, id: u128) -> Option<CommentFacts> {
        Some(CommentFacts {
            work: work(),
            comment: comment_id(id),
            work_public: self.published.is_some(),
            writer: principal(self.writers[&id]),
            state: self.comments[&id],
            settings: self.settings,
        })
    }

    fn work_row(&self, actor: u128) -> Endorsement<RevisionId> {
        self.work_rows
            .get(&actor)
            .copied()
            .unwrap_or(Endorsement::ABSENT)
    }

    fn comment_row(&self, actor: u128, id: u128) -> Endorsement<CommentAnchor> {
        self.comment_rows
            .get(&(actor, id))
            .copied()
            .unwrap_or(Endorsement::ABSENT)
    }

    fn view(&self, actor: u128, target: u128) -> OwnView {
        if target == 0 {
            own_work_view(work(), self.work_facts(), self.work_row(actor))
        } else {
            own_comment_view(
                work(),
                comment_id(target),
                self.comment_facts(target),
                self.comment_row(actor, target),
            )
        }
    }

    fn apply(&mut self, command: &Command) -> (&'static str, i64) {
        match command.op.as_str() {
            "issue" => {
                assert!(
                    self.inflight
                        .insert(command.stamp, command.clone())
                        .is_none(),
                    "stamps are unique"
                );
                ("issued", 0)
            }
            "deliver" => {
                let request = self.inflight[&command.stamp].clone();
                assert_eq!(
                    Command {
                        op: "deliver".into(),
                        keep: command.keep,
                        heard: command.heard,
                        ..request.clone()
                    },
                    *command,
                    "the model delivered the request it issued"
                );
                if !command.keep {
                    self.inflight.remove(&command.stamp);
                }
                self.deliver(&request)
            }
            // The production read path: an own view of a visible target carries its version.
            "read" => match self.view(command.actor, command.target) {
                OwnView::Unavailable => ("read_hidden", 0),
                _ if command.target == 0 => ("read", self.work_row(command.actor).version().get()),
                _ => (
                    "read",
                    self.comment_row(command.actor, command.target)
                        .version()
                        .get(),
                ),
            },
            "publish" => {
                let next = Some(command.observed);
                let outcome = if self.published == next {
                    "noop"
                } else {
                    "published"
                };
                self.published = next;
                (outcome, 0)
            }
            "withdraw" => {
                let outcome = if self.published.is_none() {
                    "noop"
                } else {
                    "withdrawn"
                };
                self.published = None;
                (outcome, 0)
            }
            "approve" | "remove" => {
                let state = self
                    .comments
                    .get_mut(&command.target)
                    .expect("model comment");
                let action = if command.op == "approve" {
                    ModerationAction::Approve
                } else {
                    ModerationAction::Remove
                };
                match action.apply(*state, state.version()) {
                    Ok(Moderation::Changed(next)) => {
                        *state = next;
                        ("moderated", 0)
                    }
                    Ok(Moderation::Unchanged(_)) | Err(LifecycleError::Removed) => ("noop", 0),
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
                        ("mode_set", 0)
                    }
                    // Same mode at the current generation: the accepted intent advances the
                    // generation (#79), but the mode the model observes does not change.
                    Ok(ModeChange::Reaffirmed(next)) => {
                        self.settings = next;
                        ("noop", 0)
                    }
                    Ok(ModeChange::AlreadyCurrent(_)) => {
                        panic!("the current generation is always accepted")
                    }
                    Err(other) => panic!("current generation: {other:?}"),
                }
            }
            other => panic!("unknown op {other}"),
        }
    }

    fn deliver(&mut self, request: &Command) -> (&'static str, i64) {
        let expected = EndorsementVersion::new(request.expected).expect("model versions");
        let select = match request.desire.as_str() {
            "select" => true,
            "clear" => false,
            other => panic!("unknown desire {other}"),
        };
        if request.target == 0 {
            let current = self.work_row(request.actor);
            let intent = if select {
                WorkIntent::Endorse {
                    observed: revision(request.observed),
                }
            } else {
                WorkIntent::Clear
            };
            let decided = decide_work_endorsement(
                principal(request.actor),
                work(),
                WorkRequest { expected, intent },
                self.work_facts(),
                current,
            );
            let transition = match decided {
                WorkDecision::Transition(transition) => transition,
                WorkDecision::TargetUnavailable => return ("target_unavailable", 0),
                WorkDecision::OwnWork => return ("own_content", 0),
                WorkDecision::RevisionChanged { published } => {
                    assert_eq!(Some(published), self.published.map(revision));
                    return ("revision_changed", 0);
                }
            };
            if let Some(next) = transition.write() {
                self.work_rows.insert(request.actor, next);
            }
            if let Some(delta) = transition.delta() {
                // The store writes the row and both counts or nothing.
                for (&anchor, count) in &mut self.work_counts {
                    *count = count
                        .apply(revision(anchor), delta)
                        .expect("the counts follow the selections");
                }
            }
            settled(transition)
        } else {
            let id = request.target;
            let intent = if select {
                CommentIntent::Endorse
            } else {
                CommentIntent::Clear
            };
            let decided = decide_comment_endorsement(
                principal(request.actor),
                work(),
                comment_id(id),
                CommentRequest { expected, intent },
                self.comment_facts(id),
                self.comment_row(request.actor, id),
            );
            let transition = match decided {
                CommentDecision::Transition(transition) => transition,
                CommentDecision::TargetUnavailable => return ("target_unavailable", 0),
                CommentDecision::OwnComment => return ("own_content", 0),
                CommentDecision::CommentsClosed => return ("comments_closed", 0),
            };
            if let Some(next) = transition.write() {
                self.comment_rows.insert((request.actor, id), next);
            }
            if let Some(delta) = transition.delta() {
                let count = self.comment_counts.get_mut(&id).expect("model comment");
                *count = count
                    .apply(CommentAnchor, delta)
                    .expect("the count follows the selections");
            }
            settled(transition)
        }
    }

    fn comments(&self) -> BTreeMap<String, String> {
        self.comments
            .iter()
            .map(|(id, state)| {
                let status = if state.is_public() {
                    "public"
                } else if state.is_present() {
                    "pending"
                } else {
                    "removed"
                };
                (id.to_string(), status.to_owned())
            })
            .collect()
    }

    /// Stored rows other than [`Endorsement::ABSENT`]; selections in model numbering.
    fn rows(&self) -> BTreeMap<String, Row> {
        let mut rows = BTreeMap::new();
        for (&actor, row) in &self.work_rows {
            let sel = row.selection().map_or(0, |id| id.as_uuid().as_u128());
            rows.insert(
                format!("{actor}/0"),
                Row {
                    sel,
                    version: row.version().get(),
                },
            );
        }
        for (&(actor, id), row) in &self.comment_rows {
            rows.insert(
                format!("{actor}/{id}"),
                Row {
                    sel: u128::from(row.selection().is_some()),
                    version: row.version().get(),
                },
            );
        }
        rows.retain(|_, row| row.sel != 0 || row.version != 1);
        rows
    }

    fn counts(&self) -> BTreeMap<String, u32> {
        let works = self
            .work_counts
            .iter()
            .map(|(anchor, count)| (format!("0/{anchor}"), count.get()));
        let comments = self
            .comment_counts
            .iter()
            .map(|(id, count)| (format!("{id}/1"), count.get()));
        works.chain(comments).collect()
    }

    fn summary(&self) -> BTreeMap<String, Option<u32>> {
        let published = self
            .published
            .map_or(EndorsementCount::ZERO, |anchor| self.work_counts[&anchor]);
        let mut summary = BTreeMap::from([(
            "0".to_owned(),
            summary_count(work_summary(work(), self.work_facts(), published)),
        )]);
        for id in COMMENTS {
            summary.insert(
                id.to_string(),
                summary_count(comment_summary(
                    work(),
                    comment_id(id),
                    self.comment_facts(id),
                    self.comment_counts[&id],
                )),
            );
        }
        summary
    }

    fn views(&self) -> BTreeMap<String, String> {
        let mut views = BTreeMap::new();
        for target in [0, 1, 2] {
            for actor in ACTORS {
                views.insert(
                    format!("{actor}/{target}"),
                    view_code(self.view(actor, target)).to_owned(),
                );
            }
        }
        views
    }
}

#[test]
fn quint_endorsement_inputs_replay_against_the_production_decisions() {
    let fixture: Conformance = serde_json::from_str(include_str!(
        "../../../behavior/discussion/endorsement-fixtures.json"
    ))
    .unwrap();
    let tool: serde_json::Value =
        serde_json::from_str(include_str!("../../../behavior/discussion/package.json")).unwrap();
    assert_eq!(fixture.format, "vink-endorsement-conformance-v1");
    assert_eq!(
        fixture.quint_version,
        tool["dependencies"]["@informalsystems/quint"]
    );
    assert_eq!(
        fixture.policy, POLICY,
        "the model and the code name one policy"
    );
    let writers: BTreeMap<u128, u128> = fixture
        .writers
        .iter()
        .map(|(id, &writer)| (id.parse().unwrap(), writer))
        .collect();
    assert_eq!(writers.keys().copied().collect::<Vec<_>>(), COMMENTS);
    let mut outcomes = BTreeSet::new();
    for scenario in &fixture.scenarios {
        assert!(!scenario.steps.is_empty(), "{}", scenario.name);
        let mut shell = Shell {
            work_author: fixture.work_author,
            writers: writers.clone(),
            published: None,
            comments: COMMENTS
                .into_iter()
                .map(|id| (id, CommentState::SUBMITTED))
                .collect(),
            settings: DiscussionSettings::INITIAL,
            work_rows: BTreeMap::new(),
            comment_rows: BTreeMap::new(),
            work_counts: REVISIONS
                .into_iter()
                .map(|anchor| (anchor, EndorsementCount::ZERO))
                .collect(),
            comment_counts: COMMENTS
                .into_iter()
                .map(|id| (id, EndorsementCount::ZERO))
                .collect(),
            inflight: BTreeMap::new(),
        };
        for (index, step) in scenario.steps.iter().enumerate() {
            let at = format!("{} step {index}", scenario.name);
            let (outcome, answer) = shell.apply(&step.command);
            assert_eq!(outcome, step.outcome, "{at}");
            assert_eq!(answer, step.answer, "{at}");
            outcomes.insert(outcome);
            assert_eq!(shell.published.unwrap_or(0), step.published, "{at}");
            let mode = match shell.settings.mode() {
                CommentsMode::Open => "open",
                CommentsMode::Closed => "closed",
            };
            assert_eq!(mode, step.mode, "{at}");
            assert_eq!(shell.comments(), step.comments, "{at}");
            assert_eq!(shell.rows(), step.rows, "{at}");
            assert_eq!(shell.counts(), step.counts, "{at}");
            assert_eq!(shell.summary(), step.summary, "{at}");
            assert_eq!(shell.views(), step.views, "{at}");
        }
    }
    // Non-vacuous: every decision outcome and shell outcome the scenarios distinguish replayed.
    assert_eq!(outcomes.len(), 15, "{outcomes:?}");
}
