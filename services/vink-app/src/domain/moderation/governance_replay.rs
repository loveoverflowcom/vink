//! M4 governance replay (ADMIN-MODERATION-20261111).
//!
//! Every step of `governance-vectors.json` (4 scenarios, 105 steps) runs through production:
//! [`ModerationDirectory`] (grants, groups, admins), [`ReviewCase`] (claims fenced by case and
//! decision versions), [`AppealBook`], [`Automation`], [`Investigation`], and the decision owners
//! [`WorkModeration`] + [`Publication`] (work revisions) and [`CommentState`] (the comment). After
//! each step the whole model state is projected from production facts and compared.
//!
//! Harness glue, stated plainly: the model's abstract `submit` of an already submitted target is
//! answered `not_needed` by the harness (republication is M1's); comment creation is not gated on
//! the work's access (M3 owns that); a comment's evaluation result goes through the shared
//! [`validate_evaluation_result`] and then the comment lifecycle, because no comment evaluator
//! consumer exists yet (`#154`), and who decided a comment is tracked by the harness (the
//! lifecycle has no provenance). The investigation scenario starts from a fixture in which
//! revision 1 of work 1 was published through production commands.
//! The five negative controls replay their prefix the same way and show production refusing
//! the mutated last step.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use super::appeal::{AppealBook, AppealVerdict, CurrentDecision, DecidedBy, Filed};
use super::authority::{
    AuthorityBasis, DirectoryFacts, GrantScope, GrantSpec, Grantee, GroupFacts,
    ModerationDirectory, ReviewCapability, ReviewTarget,
};
use super::automation::{Automation, AutomationMode};
use super::case::{
    CaseEvent, CaseOrigin, CaseRefusal, CaseState, CaseVerdict, CaseVersion, CloseReason,
    InternalNote, ReviewCase, ReviewSubject,
};
use super::investigation::{EvidenceSource, Investigation, InvestigationState, ViolationBasis};
use super::*;
use crate::domain::discussion::lifecycle::{
    CommentState, Decision as CommentDecision, Review, Transition,
};
use crate::domain::discussion::submission::PayloadDigest;
use crate::domain::ids::{AppealId, CaseId, GrantId, GroupId, InvestigationId};
use crate::domain::publication::tests::{owner, revision, save, snapshot, work};
use crate::domain::publication::{Audience, IntentGeneration, Moderation, Snapshot};
use crate::domain::revision::RevisionContent;

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
    a: u128,
    actor: u128,
    b: i64,
    op: String,
    value: String,
}

#[derive(Deserialize, Debug, PartialEq)]
struct Decision {
    by: String,
    verdict: String,
    version: i64,
}

#[derive(Deserialize, Debug, PartialEq)]
struct Job {
    state: String,
    target: u128,
}

#[derive(Deserialize, Debug, PartialEq)]
struct Case {
    basis: u128,
    claimant: u128,
    fence: i64,
    notes: usize,
    state: String,
    target: u128,
    version: i64,
}

#[derive(Deserialize, Debug, PartialEq)]
struct Appeal {
    by: String,
    decision: i64,
    notice: String,
    state: String,
    target: u128,
}

#[derive(Deserialize, Debug, PartialEq)]
struct Inv {
    admin: u128,
    expires: i64,
    rev: u128,
    state: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct State {
    granted: Vec<u128>,
    members: Vec<u128>,
    now: i64,
    automation: String,
    work_public: bool,
    decisions: BTreeMap<u128, Decision>,
    jobs: BTreeMap<u128, Job>,
    cases: BTreeMap<u128, Case>,
    appeals: BTreeMap<u128, Appeal>,
    investigations: BTreeMap<u128, Inv>,
    reads: Vec<(u128, u128)>,
}

fn p(n: u128) -> PrincipalId {
    PrincipalId::from_uuid(Uuid::from_u128(n))
}
fn num(id: PrincipalId) -> u128 {
    id.as_uuid().as_u128()
}
fn gid(n: u128) -> GrantId {
    GrantId::from_uuid(Uuid::from_u128(0x6000 + n))
}
fn gnum(id: GrantId) -> u128 {
    id.as_uuid().as_u128() - 0x6000
}
const GROUP: u128 = 7;
fn group() -> GroupId {
    GroupId::from_uuid(Uuid::from_u128(GROUP))
}
fn tick(n: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::hours(n)
}
fn reason(text: &str) -> DecisionReason {
    DecisionReason::parse(text).unwrap()
}
fn comment_id() -> CommentId {
    CommentId::from_uuid(Uuid::from_u128(0xC2))
}
fn comment_digest() -> PayloadDigest {
    PayloadDigest::from_bytes(&[2; 32]).unwrap()
}
/// Work 2 belongs to moderator 2; its revision 3 is target 3.
fn snapshot2() -> Snapshot {
    Snapshot {
        work: work(2),
        revision: revision(3),
        content: RevisionContent::new(
            crate::domain::document::Title::parse("Bản của người duyệt").unwrap(),
            crate::domain::document::Document::empty(),
        )
        .unwrap(),
    }
}

/// The model's fixed grant table (`grantTable`).
fn grant_spec(g: u128) -> GrantSpec {
    let (grantee, capability, scope, expires) = match g {
        1 => (
            Grantee::Principal(p(2)),
            ReviewCapability::Works,
            GrantScope::Global,
            None,
        ),
        2 => (
            Grantee::Principal(p(3)),
            ReviewCapability::Works,
            GrantScope::Work(work(1)),
            Some(tick(3)),
        ),
        3 => (
            Grantee::Group(group()),
            ReviewCapability::Comments,
            GrantScope::Global,
            None,
        ),
        4 => (
            Grantee::Principal(p(3)),
            ReviewCapability::Works,
            GrantScope::Global,
            None,
        ),
        5 => (
            Grantee::Principal(p(2)),
            ReviewCapability::Comments,
            GrantScope::Global,
            None,
        ),
        other => panic!("no grant {other} in the model table"),
    };
    GrantSpec {
        grantee,
        capability,
        scope,
        expires_at: expires,
    }
}

/// The model's fixed investigation table (`invTable`): (admin, revision, expiry tick).
fn inv_spec(k: u128) -> (u128, u128, i64) {
    match k {
        1 => (4, 1, 3),
        2 => (1, 1, 0),
        3 => (4, 2, 0),
        4 => (2, 1, 0),
        5 => (4, 1, 0),
        other => panic!("no investigation {other}"),
    }
}

fn target(t: u128) -> ReviewTarget {
    match t {
        1 | 4 => ReviewTarget::work(work(1), owner()),
        2 => ReviewTarget::comment(work(1), p(5), owner()),
        3 => ReviewTarget::work(work(2), p(2)),
        other => panic!("no target {other}"),
    }
}

struct World {
    directory: ModerationDirectory,
    now: i64,
    automation: Automation,
    pub1: Publication,
    mod1: WorkModeration,
    pub2: Publication,
    mod2: WorkModeration,
    comment: Option<CommentState>,
    comment_jobs: Vec<EvaluationJobFacts>,
    comment_by: &'static str,
    /// Model job id -> (target, production evaluation id).
    jobs: BTreeMap<u128, (u128, EvaluationId)>,
    cases: BTreeMap<u128, ReviewCase>,
    books: BTreeMap<u128, AppealBook>,
    appeals: BTreeMap<u128, (u128, AppealId)>,
    investigations: BTreeMap<u128, Investigation>,
    reads: BTreeSet<(u128, u128)>,
    /// Investigation fixture: work 1's revision 1 was public before the scenario.
    fixture: bool,
}

impl World {
    fn new(fixture: bool) -> Self {
        let directory = ModerationDirectory::restore(DirectoryFacts {
            admins: vec![p(1), p(4)],
            suspended: vec![],
            grants: vec![],
            groups: vec![GroupFacts {
                id: group(),
                members: vec![],
            }],
        })
        .unwrap();
        let mut pub1 = Publication::new(work(1), owner());
        for n in [1, 4, 2] {
            pub1 = save(&pub1, n);
        }
        let mut mod1 = WorkModeration::new(work(1));
        if fixture {
            let submitted = mod1
                .decide_author(
                    &pub1,
                    owner(),
                    AuthorCommand::RequestPublic {
                        revision: revision(1),
                        expected: pub1.facts().generation,
                    },
                    Some(EvaluationId::from_uuid(Uuid::from_u128(0xF1))),
                )
                .unwrap();
            let ticket = submitted.ticket.clone().unwrap();
            let approved = submitted
                .moderation
                .apply_evaluator_result(
                    &submitted.publication,
                    Some(EvaluatorCapability::evaluator()),
                    &ScriptedStep::approve(&ticket).result,
                )
                .unwrap();
            assert_eq!(approved.outcome, AppliedEvaluation::Published);
            pub1 = approved.publication;
            mod1 = approved.moderation;
        }
        let pub2 = Publication::new(work(2), p(2))
            .decide_author(p(2), AuthorCommand::SaveSnapshot(snapshot2()))
            .unwrap()
            .next;
        Self {
            directory,
            now: 0,
            automation: Automation::INITIAL,
            pub1,
            mod1,
            pub2,
            mod2: WorkModeration::new(work(2)),
            comment: None,
            comment_jobs: vec![],
            comment_by: "none",
            jobs: BTreeMap::new(),
            cases: BTreeMap::new(),
            books: BTreeMap::new(),
            appeals: BTreeMap::new(),
            investigations: BTreeMap::new(),
            reads: BTreeSet::new(),
            fixture,
        }
    }

    fn at(&self) -> OffsetDateTime {
        tick(self.now)
    }

    fn work_pair(&self, t: u128) -> (&Publication, &WorkModeration) {
        if t == 3 {
            (&self.pub2, &self.mod2)
        } else {
            (&self.pub1, &self.mod1)
        }
    }

    fn rev_of(t: u128) -> RevisionId {
        match t {
            1 => revision(1),
            3 => revision(3),
            4 => revision(4),
            other => panic!("target {other} is not a work revision"),
        }
    }

    fn withdrawn(&self, t: u128) -> bool {
        matches!(t, 1 | 4)
            && self.pub1.facts().audience == Audience::Private
            && self.pub1.facts().generation > IntentGeneration::FIRST
    }

    /// The owner's current subject (with its current decision fence) for a target.
    fn subject(&self, t: u128) -> ReviewSubject {
        if t == 2 {
            ReviewSubject::Comment {
                work: work(1),
                comment: comment_id(),
                decision: self.comment.map_or(
                    crate::domain::discussion::lifecycle::ModerationVersion::FIRST,
                    |c| c.version(),
                ),
            }
        } else {
            let (publication, moderation) = self.work_pair(t);
            let rev = Self::rev_of(t);
            ReviewSubject::WorkRevision {
                work: publication.facts().work,
                revision: rev,
                digest: publication
                    .facts()
                    .revisions
                    .iter()
                    .find(|r| r.snapshot.revision == rev)
                    .unwrap()
                    .snapshot
                    .content
                    .hash()
                    .clone(),
                decision: moderation
                    .revision_decision(rev)
                    .map_or(DecisionVersion::FIRST, |d| d.version),
            }
        }
    }

    /// (verdict, by) of a target as the model names them.
    fn decision(&self, t: u128) -> (&'static str, &'static str) {
        if t == 2 {
            return match self.comment {
                None => ("none", "none"),
                Some(c) => (
                    match c.review() {
                        Review::Pending => "pending",
                        Review::Approved => "approved",
                        Review::Rejected => "rejected",
                    },
                    self.comment_by,
                ),
            };
        }
        let (publication, moderation) = self.work_pair(t);
        let rev = Self::rev_of(t);
        let verdict = match publication
            .facts()
            .revisions
            .iter()
            .find(|r| r.snapshot.revision == rev)
            .unwrap()
            .moderation
        {
            Moderation::Unsubmitted => "none",
            Moderation::Pending => "pending",
            Moderation::Approved => "approved",
            Moderation::Rejected => "rejected",
            Moderation::NeedsAdmin => "needs_human",
        };
        let by = match moderation
            .revision_decision(rev)
            .and_then(|d| d.decision_by())
        {
            None => "none",
            Some(DecisionBy::Ai) => "ai",
            Some(DecisionBy::Moderator) => "moderator",
            Some(DecisionBy::Admin) => "admin",
            other => panic!("unexpected decision_by {other:?}"),
        };
        (verdict, by)
    }

    fn decided_by(&self, t: u128) -> Option<DecidedBy> {
        if t == 2 {
            return match self.comment_by {
                "ai" => Some(DecidedBy::Evaluator),
                "moderator" => Some(DecidedBy::Moderator(p(2))),
                "admin" => Some(DecidedBy::Admin(p(4))),
                _ => None,
            };
        }
        let (_, moderation) = self.work_pair(t);
        match moderation
            .revision_decision(Self::rev_of(t))
            .and_then(|d| d.provenance.clone())
        {
            Some(DecisionProvenance::AiRejected(_) | DecisionProvenance::AiApproved) => {
                Some(DecidedBy::Evaluator)
            }
            Some(DecisionProvenance::Moderator { actor, .. }) => Some(DecidedBy::Moderator(actor)),
            Some(DecisionProvenance::Admin { actor, .. }) => Some(DecidedBy::Admin(actor)),
            _ => None,
        }
    }

    fn current_decision(&self, t: u128) -> CurrentDecision {
        CurrentDecision {
            subject: self.subject(t),
            owner: target(t).content_owner,
            decided_by: self.decided_by(t),
            adverse: self.decision(t).0 == "rejected",
            withdrawn: self.withdrawn(t),
        }
    }

    fn target_of(case: &ReviewCase) -> u128 {
        match &case.facts().subject {
            ReviewSubject::Comment { .. } => 2,
            ReviewSubject::WorkRevision { revision: r, .. } => r.as_uuid().as_u128(),
        }
    }

    fn has_open_case(&self, t: u128) -> bool {
        self.cases.values().any(|c| {
            Self::target_of(c) == t
                && matches!(
                    c.state(),
                    CaseState::Queued | CaseState::Claimed { .. } | CaseState::Escalated
                )
        })
    }

    fn open_case(&mut self, t: u128, origin: CaseOrigin) -> u128 {
        let id = self.cases.len() as u128 + 1;
        let case = ReviewCase::open(
            CaseId::from_uuid(Uuid::from_u128(0xCA00 + id)),
            self.subject(t),
            target(t),
            origin,
        );
        self.cases.insert(id, case);
        id
    }

    /// Apply a work decision result of the owner and keep the new state.
    fn keep_work(&mut self, t: u128, publication: Publication, moderation: WorkModeration) {
        if t == 3 {
            self.pub2 = publication;
            self.mod2 = moderation;
        } else {
            self.pub1 = publication;
            self.mod1 = moderation;
        }
    }

    fn decide_comment(&mut self, verdict: CaseVerdict, by: &'static str) {
        let state = self.comment.expect("comment exists");
        let decision = match verdict {
            CaseVerdict::Approve => CommentDecision::Approve,
            CaseVerdict::Reject => CommentDecision::Reject,
        };
        match state.decide(state.version(), decision) {
            Ok(Transition::Changed(next)) => self.comment = Some(next),
            other => panic!("comment decision {other:?}"),
        }
        self.comment_by = by;
    }

    fn step(&mut self, c: &Command) -> String {
        let now = self.at();
        let actor = p(c.actor);
        match c.op.as_str() {
            "grant" => match self
                .directory
                .require_admin(actor, now)
                .map_err(authority::DirectoryError::from)
                .and_then(|seat| self.directory.grant(seat, gid(c.a), grant_spec(c.a)))
            {
                Ok((next, audit)) => {
                    assert_eq!(audit.change, authority::DirectoryChange::Granted(gid(c.a)));
                    self.directory = next;
                    "granted".into()
                }
                Err(authority::DirectoryError::Refused(r)) => r.as_str().into(),
                Err(_) => "unchanged".into(),
            },
            "revoke" => match self
                .directory
                .require_admin(actor, now)
                .map_err(authority::DirectoryError::from)
                .and_then(|seat| self.directory.revoke(seat, gid(c.a)))
            {
                Ok((next, _)) => {
                    self.directory = next;
                    "revoked".into()
                }
                Err(authority::DirectoryError::Refused(r)) => r.as_str().into(),
                Err(_) => "unchanged".into(),
            },
            "join" | "leave" => match self
                .directory
                .require_admin(actor, now)
                .map_err(authority::DirectoryError::from)
                .and_then(|seat| {
                    self.directory
                        .set_membership(seat, group(), p(c.a), c.op == "join")
                }) {
                Ok((next, _)) => {
                    self.directory = next;
                    if c.op == "join" { "joined" } else { "left" }.into()
                }
                Err(authority::DirectoryError::Refused(r)) => r.as_str().into(),
                Err(_) => "unchanged".into(),
            },
            "tick" => {
                self.now += 1;
                "ticked".into()
            }
            "submit" => self.submit(c.a),
            "ai_result" => self.ai(c.a, &c.value),
            "pause" | "resume" => {
                let mode = if c.op == "pause" {
                    AutomationMode::Paused
                } else {
                    AutomationMode::Enabled
                };
                match self.automation.set(
                    &self.directory,
                    actor,
                    self.automation.version(),
                    mode,
                    Some(reason("vận hành")),
                    now,
                ) {
                    Ok((next, audit)) => {
                        assert_eq!(audit.mode, mode);
                        self.automation = next;
                        if mode == AutomationMode::Paused {
                            self.reroute();
                            "paused".into()
                        } else {
                            "resumed".into()
                        }
                    }
                    Err(automation::AutomationError::Authority(r)) => r.as_str().into(),
                    Err(_) => "unchanged".into(),
                }
            }
            "make_private" | "make_public" => self.audience(c.op == "make_private"),
            "claim" | "release" | "note" | "escalate" | "takeover" | "revalidate" | "conclude" => {
                self.case_command(c)
            }
            "intervene" => self.intervene(actor, c.a),
            "appeal" => self.appeal(actor, c.a, c.b),
            "appeal_conclude" => self.appeal_conclude(actor, c.a, &c.value),
            "open_investigation" | "read_investigation" | "close_investigation" => {
                self.investigation(c)
            }
            other => panic!("unknown op {other}"),
        }
    }

    fn submit(&mut self, t: u128) -> String {
        if self.decision(t).0 != "none" {
            // Harness: republication of a submitted revision is M1's (publication.qnt).
            return "not_needed".into();
        }
        let paused = self.automation.mode() == AutomationMode::Paused;
        let job = self.jobs.len() as u128 + 1;
        let eval = EvaluationId::from_uuid(Uuid::from_u128(0xE000 + job));
        if t == 2 {
            self.comment = Some(CommentState::SUBMITTED);
            if paused {
                self.open_case(2, CaseOrigin::AutomationPaused);
                return "queued".into();
            }
            self.comment_jobs.push(EvaluationJobFacts {
                id: eval,
                target: EvaluationTarget::CommentRevision {
                    comment: comment_id(),
                    revision: revision(0xC2),
                    digest: comment_digest(),
                },
                policy: PolicyVersion::FIRST,
                state: JobState::Open,
            });
            self.jobs.insert(job, (2, eval));
            return "submitted_ai".into();
        }
        let author = target(t).content_owner;
        let (publication, moderation) = self.work_pair(t);
        let transition = moderation
            .decide_author_with(
                publication,
                author,
                AuthorCommand::RequestPublic {
                    revision: Self::rev_of(t),
                    expected: publication.facts().generation,
                },
                Some(eval),
                self.automation.mode(),
            )
            .unwrap();
        assert_eq!(transition.outcome, AuthorOutcome::Submitted);
        assert_eq!(transition.review, ReviewRequest::Fresh);
        let queued = transition.human_review.is_some();
        assert_eq!(transition.ticket.is_some(), !queued);
        self.keep_work(t, transition.publication, transition.moderation);
        if queued {
            self.open_case(t, CaseOrigin::AutomationPaused);
            "queued".into()
        } else {
            self.jobs.insert(job, (t, eval));
            "submitted_ai".into()
        }
    }

    fn ai(&mut self, job: u128, value: &str) -> String {
        let Some(&(t, eval)) = self.jobs.get(&job) else {
            return "unknown_job".into();
        };
        let outcome = match value {
            "approve" => EvaluatorOutcome::Approve,
            "reject" => EvaluatorOutcome::Reject(PolicyReasonCode::SafetyPolicy),
            "uncertain" => EvaluatorOutcome::Uncertain,
            other => panic!("ai value {other}"),
        };
        let capability = Some(EvaluatorCapability::under(self.automation.mode()));
        if t == 2 {
            let result = EvaluatorResult {
                evaluation_id: eval,
                target: EvaluationTarget::CommentRevision {
                    comment: comment_id(),
                    revision: revision(0xC2),
                    digest: comment_digest(),
                },
                policy: PolicyVersion::FIRST,
                outcome,
            };
            let state = self.comment.expect("comment exists");
            let human = matches!(self.comment_by, "moderator" | "admin");
            let validated = validate_evaluation_result(
                capability,
                Some(EvaluationSubject::Comment(comment_id())),
                &self.comment_jobs,
                PolicyVersion::FIRST,
                &result,
                |rev| {
                    Some(TargetRevisionFacts {
                        target: EvaluationTarget::CommentRevision {
                            comment: comment_id(),
                            revision: rev,
                            digest: comment_digest(),
                        },
                        target_active: state.is_present(),
                        overridden_by_admin: human,
                        candidate_pending: state.review() == Review::Pending,
                    })
                },
            );
            return match validated {
                Err(refusal) => refusal.as_str().into(),
                Ok(_) => {
                    let verdict = match outcome {
                        EvaluatorOutcome::Approve => CaseVerdict::Approve,
                        EvaluatorOutcome::Reject(_) => CaseVerdict::Reject,
                        other => panic!("no comment needs-admin state for {other:?}"),
                    };
                    self.comment_jobs
                        .iter_mut()
                        .find(|j| j.id == eval)
                        .unwrap()
                        .state = JobState::Applied;
                    self.decide_comment(verdict, "ai");
                    "applied".into()
                }
            };
        }
        let (publication, moderation) = self.work_pair(t);
        let job_facts = moderation
            .facts()
            .jobs
            .iter()
            .find(|j| j.id == eval)
            .unwrap()
            .clone();
        let result = EvaluatorResult {
            evaluation_id: eval,
            target: job_facts.target,
            policy: job_facts.policy,
            outcome,
        };
        match moderation.apply_evaluator_result(publication, capability, &result) {
            Ok(transition) => {
                let needs_human =
                    matches!(transition.outcome, AppliedEvaluation::NeedsAdmin { .. });
                self.keep_work(t, transition.publication, transition.moderation);
                if needs_human {
                    if !self.has_open_case(t) {
                        self.open_case(t, CaseOrigin::EvaluatorUncertain);
                    }
                    "needs_human".into()
                } else {
                    "applied".into()
                }
            }
            Err(ModerationError::Refused(refusal)) => refusal.as_str().into(),
            other => panic!("unexpected evaluator result {other:?}"),
        }
    }

    fn reroute(&mut self) {
        let mut waiting = BTreeSet::new();
        let (next1, revs1) = self.mod1.reroute_to_humans();
        let (next2, revs2) = self.mod2.reroute_to_humans();
        self.mod1 = next1;
        self.mod2 = next2;
        for r in revs1.into_iter().chain(revs2) {
            waiting.insert(r.as_uuid().as_u128());
        }
        let mut comment_waiting = false;
        for job in &mut self.comment_jobs {
            if job.state == JobState::Open {
                job.state = JobState::Rerouted;
                comment_waiting = true;
            }
        }
        if comment_waiting && self.comment.is_some_and(|c| c.review() == Review::Pending) {
            waiting.insert(2);
        }
        for t in waiting {
            if !self.has_open_case(t) {
                self.open_case(t, CaseOrigin::AutomationPaused);
            }
        }
    }

    fn audience(&mut self, private: bool) -> String {
        let command = if private {
            AuthorCommand::SetPrivate {
                expected: self.pub1.facts().generation,
            }
        } else {
            AuthorCommand::RequestPublic {
                revision: revision(1),
                expected: self.pub1.facts().generation,
            }
        };
        let transition = self
            .mod1
            .decide_author_with(&self.pub1, owner(), command, None, self.automation.mode())
            .unwrap();
        let outcome = transition.outcome;
        let withdrawn = transition.withdrawn_reviews;
        self.pub1 = transition.publication;
        self.mod1 = transition.moderation;
        if private {
            assert_eq!(outcome, AuthorOutcome::MadePrivate);
            assert!(withdrawn);
            // The caller closes the work's cases and appeals (comment cases go on).
            for case in self.cases.values_mut() {
                if matches!(case.facts().subject, ReviewSubject::WorkRevision { work: w, .. } if w == work(1))
                    && let Ok(t) = case.close(CloseReason::Withdrawn)
                {
                    *case = t.next;
                }
            }
            for t in [1, 4] {
                if let Some(book) = self.books.get_mut(&t) {
                    *book = book.close_open(CloseReason::Withdrawn);
                }
            }
            "made_private".into()
        } else {
            assert_eq!(outcome, AuthorOutcome::Published);
            "made_public".into()
        }
    }

    fn intervene(&mut self, actor: PrincipalId, t: u128) -> String {
        if let Err(r) = self.directory.resolve(actor, &target(t), self.at()) {
            return r.as_str().into();
        }
        if self.decision(t).0 == "none" {
            return "not_submitted".into();
        }
        if self.withdrawn(t) {
            return "case_closed".into();
        }
        if self.has_open_case(t) {
            return "case_open".into();
        }
        let id = self.open_case(t, CaseOrigin::ReviewerIntervention);
        let case = &self.cases[&id];
        let claimed = case
            .claim(&self.directory, actor, case.version(), self.at())
            .unwrap();
        assert_eq!(claimed.event, CaseEvent::Claimed);
        self.cases.insert(id, claimed.next);
        "intervened".into()
    }

    fn case_command(&mut self, c: &Command) -> String {
        let now = self.at();
        let actor = p(c.actor);
        let Some(case) = self.cases.get(&c.a).cloned() else {
            return "unknown_case".into();
        };
        let expected = CaseVersion::new(c.b).unwrap_or(CaseVersion::FIRST);
        let t = Self::target_of(&case);
        let result = match c.op.as_str() {
            "claim" => case.claim(&self.directory, actor, expected, now),
            "release" => case.release(actor, expected),
            "note" => case.note(
                &self.directory,
                actor,
                expected,
                InternalNote::parse("ghi chú nội bộ").unwrap(),
                now,
            ),
            "escalate" => case.escalate(actor, expected),
            "takeover" => case.take_over(
                &self.directory,
                actor,
                expected,
                Some(&reason("tiếp quản")),
                now,
            ),
            "revalidate" => case.revalidate(&self.directory, now),
            "conclude" => {
                let verdict = if c.value == "approve" {
                    CaseVerdict::Approve
                } else {
                    CaseVerdict::Reject
                };
                let current = self.subject(t);
                let appeal_open = self.books.get(&t).is_some_and(|b| b.open_on(&current));
                match case.conclude(
                    &self.directory,
                    actor,
                    expected,
                    &current,
                    verdict,
                    Some(reason("kết luận của người duyệt")),
                    appeal_open,
                    now,
                ) {
                    Ok((transition, conclusion)) => {
                        if t == 2 {
                            let by = if conclusion.authority.is_admin() {
                                "admin"
                            } else {
                                "moderator"
                            };
                            self.decide_comment(verdict, by);
                        } else {
                            let (publication, moderation) = self.work_pair(t);
                            let applied = moderation
                                .apply_case_conclusion(publication, &conclusion)
                                .unwrap();
                            self.keep_work(t, applied.publication, applied.moderation);
                        }
                        Ok(transition)
                    }
                    Err(refusal) => Err(refusal),
                }
            }
            other => panic!("case op {other}"),
        };
        match result {
            Ok(transition) => {
                let changed = transition.next.version() != case.version();
                let outcome = match (c.op.as_str(), transition.event) {
                    ("revalidate", CaseEvent::AuthorityKept) if !changed => "authority_kept",
                    ("revalidate", CaseEvent::AuthorityKept) => "basis_changed",
                    (_, event) => event.as_str(),
                };
                self.cases.insert(c.a, transition.next);
                outcome.into()
            }
            Err(CaseRefusal::NotClaimant) if c.op == "revalidate" => "not_claimant".into(),
            Err(refusal) => refusal.as_str().into(),
        }
    }

    fn appeal(&mut self, actor: PrincipalId, t: u128, seen: i64) -> String {
        let current = self.current_decision(t);
        let mut seen_subject = current.subject.clone();
        match &mut seen_subject {
            ReviewSubject::WorkRevision { decision, .. } => {
                *decision = DecisionVersion::new(seen).unwrap();
            }
            ReviewSubject::Comment { decision, .. } => {
                *decision =
                    crate::domain::discussion::lifecycle::ModerationVersion::new(seen).unwrap();
            }
        }
        let next_model = self.appeals.len() as u128 + 1;
        let id = AppealId::from_uuid(Uuid::from_u128(0xA000 + next_model));
        let book = self.books.get(&t).cloned().unwrap_or_default();
        match book.file(actor, &current, &seen_subject, id) {
            Ok((next, Filed::Recorded(recorded))) => {
                self.books.insert(t, next);
                self.appeals.insert(next_model, (t, recorded));
                "appeal_recorded".into()
            }
            Ok((_, Filed::Existing(_))) => "appeal_replayed".into(),
            Err(refusal) => refusal.as_str().into(),
        }
    }

    fn appeal_conclude(&mut self, actor: PrincipalId, k: u128, value: &str) -> String {
        let verdict = if value == "uphold" {
            AppealVerdict::Uphold
        } else {
            AppealVerdict::Reverse
        };
        let (t, id) = self
            .appeals
            .get(&k)
            .copied()
            .unwrap_or((1, AppealId::from_uuid(Uuid::from_u128(0xAFFF))));
        let current = self.current_decision(t);
        let book = self.books.get(&t).cloned().unwrap_or_default();
        match book.conclude(
            &self.directory,
            actor,
            id,
            &current,
            verdict,
            Some(reason("kết luận phản đối")),
            self.at(),
        ) {
            Ok((next, conclusion)) => {
                self.books.insert(t, next);
                if t == 2 {
                    let now_verdict = match (verdict, self.decision(2).0) {
                        (AppealVerdict::Uphold, "approved")
                        | (AppealVerdict::Reverse, "rejected") => CaseVerdict::Approve,
                        _ => CaseVerdict::Reject,
                    };
                    self.decide_comment(now_verdict, "admin");
                } else {
                    let (publication, moderation) = self.work_pair(t);
                    let applied = moderation
                        .apply_appeal_conclusion(publication, &conclusion)
                        .unwrap();
                    self.keep_work(t, applied.publication, applied.moderation);
                }
                if verdict == AppealVerdict::Uphold {
                    "upheld"
                } else {
                    "reversed"
                }
                .into()
            }
            Err(refusal) => refusal.as_str().into(),
        }
    }

    fn investigation(&mut self, c: &Command) -> String {
        let now = self.at();
        match c.op.as_str() {
            "open_investigation" => {
                if self.investigations.contains_key(&c.a) {
                    return "unchanged".into();
                }
                let (admin, rev, expires) = inv_spec(c.a);
                match Investigation::open(
                    &self.directory,
                    p(admin),
                    &self.pub1,
                    InvestigationId::from_uuid(Uuid::from_u128(0x1000 + c.a)),
                    revision(rev),
                    ViolationBasis::parse("nghi vi phạm chính sách cộng đồng").unwrap(),
                    EvidenceSource::parse("báo cáo vận hành").unwrap(),
                    (expires != 0).then(|| tick(expires)),
                    now,
                ) {
                    Ok(file) => {
                        self.investigations.insert(c.a, file);
                        "investigation_opened".into()
                    }
                    Err(refusal) => refusal.as_str().into(),
                }
            }
            "read_investigation" => {
                let Some(file) = self.investigations.get(&c.a) else {
                    return "unknown_investigation".into();
                };
                match file.read(
                    &self.directory,
                    p(c.actor),
                    &self.pub1,
                    revision(c.b as u128),
                    now,
                ) {
                    Ok(read) => {
                        // The audit row comes with the exact snapshot of the file's revision.
                        assert_eq!(read.snapshot, &snapshot(c.b as u128));
                        assert_eq!(read.audit.revision, revision(c.b as u128));
                        self.reads.insert((c.a, c.b as u128));
                        "read_audited".into()
                    }
                    Err(refusal) => refusal.as_str().into(),
                }
            }
            _ => {
                let Some(file) = self.investigations.get(&c.a) else {
                    return "unknown_investigation".into();
                };
                match file.close(
                    &self.directory,
                    p(c.actor),
                    investigation::Finding::NoViolation,
                    ViolationBasis::parse("không đủ căn cứ").unwrap(),
                    now,
                ) {
                    Ok(next) => {
                        self.investigations.insert(c.a, next);
                        "investigation_closed".into()
                    }
                    Err(refusal) => refusal.as_str().into(),
                }
            }
        }
    }

    /// Compare every model field with the production facts.
    fn project(&self, expected: &State, at: &str) {
        let granted: Vec<u128> = self
            .directory
            .facts()
            .grants
            .iter()
            .filter(|g| !g.revoked)
            .map(|g| gnum(g.id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(granted, expected.granted, "{at}: granted");
        let members: Vec<u128> = self.directory.facts().groups[0]
            .members
            .iter()
            .map(|m| num(*m))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(members, expected.members, "{at}: members");
        assert_eq!(self.now, expected.now, "{at}: now");
        assert_eq!(
            self.automation.mode().as_str(),
            expected.automation,
            "{at}: automation"
        );
        let work_public = self.pub1.facts().audience == Audience::Public
            || self.pub1.facts().generation == IntentGeneration::FIRST;
        assert_eq!(work_public, expected.work_public, "{at}: workPublic");
        for (&t, d) in &expected.decisions {
            if self.fixture && t == 1 {
                continue; // the investigation fixture published revision 1 before the scenario
            }
            let (verdict, by) = self.decision(t);
            let version = match self.subject(t) {
                ReviewSubject::WorkRevision { decision, .. } => decision.get(),
                ReviewSubject::Comment { decision, .. } => decision.get(),
            };
            assert_eq!(
                (verdict, by, version),
                (d.verdict.as_str(), d.by.as_str(), d.version),
                "{at}: decision of target {t}"
            );
        }
        assert_eq!(
            self.jobs.keys().copied().collect::<Vec<_>>(),
            expected.jobs.keys().copied().collect::<Vec<_>>(),
            "{at}: jobs"
        );
        for (job, (t, eval)) in &self.jobs {
            let state = if *t == 2 {
                self.comment_jobs
                    .iter()
                    .find(|j| j.id == *eval)
                    .unwrap()
                    .state
            } else {
                self.work_pair(*t)
                    .1
                    .facts()
                    .jobs
                    .iter()
                    .find(|j| j.id == *eval)
                    .unwrap()
                    .state
            };
            assert_eq!(
                Job {
                    state: state.as_str().into(),
                    target: *t
                },
                expected.jobs[job],
                "{at}: job {job}"
            );
        }
        assert_eq!(self.cases.len(), expected.cases.len(), "{at}: cases");
        for (k, case) in &self.cases {
            let (claimant, basis) = match case.state() {
                CaseState::Claimed { by, basis } | CaseState::Concluded { by, basis, .. } => (
                    num(by),
                    match basis {
                        AuthorityBasis::Admin => 0,
                        AuthorityBasis::Grant(g) => gnum(g),
                    },
                ),
                _ => (0, 0),
            };
            let actual = Case {
                basis,
                claimant,
                fence: case.facts().subject.fence(),
                notes: case.facts().notes.len(),
                state: case.state().as_str().into(),
                target: Self::target_of(case),
                version: case.version().get(),
            };
            assert_eq!(actual, expected.cases[k], "{at}: case {k}");
        }
        assert_eq!(self.appeals.len(), expected.appeals.len(), "{at}: appeals");
        for (k, (t, id)) in &self.appeals {
            let facts = self.books[t].get(*id).unwrap();
            let actual = Appeal {
                by: match facts.decided_by {
                    DecidedBy::Evaluator => "ai",
                    DecidedBy::Moderator(_) => "moderator",
                    DecidedBy::Admin(_) => "admin",
                }
                .into(),
                decision: facts.subject.fence(),
                notice: "not_sent".into(),
                state: facts.state.as_str().into(),
                target: *t,
            };
            assert_eq!(facts.notice, appeal::Notice::NotSent);
            assert_eq!(actual, expected.appeals[k], "{at}: appeal {k}");
        }
        assert_eq!(self.investigations.len(), expected.investigations.len());
        for (k, file) in &self.investigations {
            let facts = file.facts();
            let actual = Inv {
                admin: num(facts.admin),
                expires: facts
                    .expires_at
                    .map_or(0, |at| (at - OffsetDateTime::UNIX_EPOCH).whole_hours()),
                rev: facts.revision.as_uuid().as_u128(),
                state: match facts.state {
                    InvestigationState::Open => "open",
                    InvestigationState::Closed { .. } => "closed",
                }
                .into(),
            };
            assert_eq!(
                actual, expected.investigations[k],
                "{at}: investigation {k}"
            );
        }
        assert_eq!(
            self.reads.iter().copied().collect::<Vec<_>>(),
            expected.reads,
            "{at}: reads"
        );
        // Every production aggregate restores from its own facts.
        assert_eq!(
            WorkModeration::restore(&self.pub1, self.mod1.facts().clone()),
            Ok(self.mod1.clone())
        );
        assert_eq!(
            Publication::restore(self.pub1.facts().clone()),
            Ok(self.pub1.clone())
        );
    }
}

fn fixture_for(name: &str) -> bool {
    matches!(name, "investigation" | "mutatedReadStep")
}

#[test]
fn quint_governance_scenarios_replay_through_production() {
    let vectors: Vectors = serde_json::from_str(include_str!(
        "../../../behavior/publication/governance-vectors.json"
    ))
    .unwrap();
    let mut counts = BTreeMap::new();
    for scenario in &vectors.scenarios {
        let mut world = World::new(fixture_for(&scenario.name));
        for (index, step) in scenario.steps.iter().enumerate() {
            let at = format!("{} step {} {}", scenario.name, index + 1, step.command.op);
            let outcome = world.step(&step.command);
            assert_eq!(outcome, step.outcome, "{at}");
            world.project(&step.state, &at);
        }
        counts.insert(scenario.name.as_str(), scenario.steps.len());
    }
    // Exact selection: a reduced or empty replay fails instead of passing.
    assert_eq!(
        counts,
        BTreeMap::from([
            ("appeals", 31),
            ("automation_and_late_results", 16),
            ("grants_and_claims", 39),
            ("investigation", 19),
        ])
    );
}

/// Replays the prefix lawfully, then shows production refusing the mutated last step.
fn refuse_counterexample(json: &str, refusal: &str) {
    let vectors: Counterexample = serde_json::from_str(json).unwrap();
    let scenario = &vectors.violation[0];
    let mut world = World::new(fixture_for(&scenario.name));
    let (last, prefix) = scenario.steps.split_last().unwrap();
    for (index, step) in prefix.iter().enumerate() {
        let at = format!("{} step {}", scenario.name, index + 1);
        assert_eq!(world.step(&step.command), step.outcome, "{at}");
        world.project(&step.state, &at);
    }
    let before = (
        world.cases.len(),
        world.appeals.len(),
        world.reads.clone(),
        world.decision(1),
        world.cases.get(&1).map(|c| c.version()),
    );
    let outcome = world.step(&last.command);
    assert_ne!(
        outcome, last.outcome,
        "production must refuse the mutated step"
    );
    assert_eq!(outcome, refusal);
    let after = (
        world.cases.len(),
        world.appeals.len(),
        world.reads.clone(),
        world.decision(1),
        world.cases.get(&1).map(|c| c.version()),
    );
    assert_eq!(before, after, "a refusal writes nothing");
}

#[test]
fn the_governance_counterexamples_are_refused_by_production() {
    refuse_counterexample(
        include_str!("../../../behavior/publication/governance-lost-authority-counterexample.json"),
        "not_authorized",
    );
    refuse_counterexample(
        include_str!("../../../behavior/publication/governance-stolen-claim-counterexample.json"),
        "stale_case",
    );
    refuse_counterexample(
        include_str!(
            "../../../behavior/publication/governance-duplicate-appeal-counterexample.json"
        ),
        "appeal_replayed",
    );
    refuse_counterexample(
        include_str!("../../../behavior/publication/governance-paused-result-counterexample.json"),
        "automation_paused",
    );
    refuse_counterexample(
        include_str!("../../../behavior/publication/governance-wrong-read-counterexample.json"),
        "wrong_revision",
    );
}
