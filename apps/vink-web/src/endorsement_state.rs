//! Browser command state for literary endorsement (LVE #73), not endorsement authority. The
//! App's `decide_endorsement` owns eligibility, order and replay; this reducer only decides
//! which server answer the page may show for one (session, target) and which command a press
//! sends. It never stores text, identity or a count of its own making.
//!
//! - **CHOICE-10 A.** Nothing is shown as selected before the owner confirms it: an
//!   accepted/replayed write or an own-state read. A press only marks the target pending.
//! - **Order fence.** A token is taken only when its `order` is not older than the one shown
//!   for that (session, target). An equal order is the same committed state, a greater one is
//!   newer, so a late, duplicated or reordered answer never moves the page back.
//! - **Counts** carry no order: a public read is taken for a target only if no write of that
//!   target settled after the read started. `unavailable` stays unavailable, never 0.
//! - **Lost answers.** A transport loss, timeout, `503` or unclassified answer is `Uncertain`
//!   and blocks new presses; “Kiểm tra lại” replays exactly that command (same key, order,
//!   desired state and revision) under a new attempt, never a new intent. An unread or failed
//!   own state is reconciled by an own-state read instead.
//! - **Session fence.** Every command and read carries the controller generation; a scope
//!   change (other session, work) clears all targets, so nothing from before can settle after.

use std::collections::BTreeMap;

use vink_patterns::{EndorsementBlock, EndorsementFacts};
use vink_ui::{CountFact, TogglePhase, ToggleValue};

use crate::contract::{
    ApiError, CommentDto, CommentEndorsementRequest, DiscussionPage, EndorsementSummaries,
    EndorsementToken, EndorsementWrite, OwnEndorsement, OwnEndorsements, RootView,
    WorkEndorsementRequest,
};

/// At most this many comment IDs per batch read (`MAX_BATCH_COMMENTS` of the provider).
pub const MAX_BATCH: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub work_id: String,
    /// The same opaque host marker as the discussion controller; `None` for a guest. Never a
    /// credential or an API actor: each request still resolves its actor on the server.
    pub session: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TargetKey {
    /// One published revision of the work: each revision is its own target.
    Work { revision_id: String },
    /// A root comment or a flat reply; never summed into its root or the work.
    Comment { comment_id: String },
}

impl TargetKey {
    pub fn comment(id: &str) -> Self {
        Self::Comment {
            comment_id: id.to_owned(),
        }
    }
}

/// One endorsement command as it goes on the wire. A replay keeps every field but `attempt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub target: TargetKey,
    pub command_key: String,
    pub expected_order: i64,
    pub endorsed: bool,
    generation: u64,
    /// Physical attempt: a replay gets a new one, so an older attempt cannot settle it.
    attempt: u64,
}

/// The HTTP body of a command and the comment it addresses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Work(WorkEndorsementRequest),
    Comment {
        comment_id: String,
        body: CommentEndorsementRequest,
    },
}

impl Command {
    pub fn request(&self) -> Request {
        match &self.target {
            TargetKey::Work { revision_id } => Request::Work(WorkEndorsementRequest {
                command_key: self.command_key.clone(),
                expected_order: self.expected_order,
                endorsed: self.endorsed,
                revision_id: revision_id.clone(),
            }),
            TargetKey::Comment { comment_id } => Request::Comment {
                comment_id: comment_id.clone(),
                body: CommentEndorsementRequest {
                    command_key: self.command_key.clone(),
                    expected_order: self.expected_order,
                    endorsed: self.endorsed,
                },
            },
        }
    }
}

/// A batch read started by this controller; its answer only applies within its generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadTicket {
    generation: u64,
    stamp: u64,
    pub targets: Vec<TargetKey>,
}

impl ReadTicket {
    /// The `comment_ids` of this read, in request order.
    pub fn comment_ids(&self) -> Vec<String> {
        self.targets
            .iter()
            .filter_map(|target| match target {
                TargetKey::Comment { comment_id } => Some(comment_id.clone()),
                TargetKey::Work { .. } => None,
            })
            .collect()
    }
}

/// What “Kiểm tra lại” does for one target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recovery {
    /// Resend exactly this command: the server answers `replayed` if it had committed.
    Replay(Command),
    /// Read the actor's own state of these targets.
    ReadOwn(ReadTicket),
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    /// Another generation, or not the live attempt: at most a newer token was taken from it.
    Stale,
    Settled,
    /// `409 revision_changed`: the page shows a revision that is no longer the public one.
    RevisionChanged,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Own {
    #[default]
    Unread,
    /// A temporary read failure: unknown, never Off.
    Failed,
    Known {
        token: EndorsementToken,
        /// Advisory `blocked_reason`/`can_endorse` of the last own-state read.
        block: Option<EndorsementBlock>,
    },
    /// Missing, hidden or not the current revision: no stored state is shown.
    Unavailable,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    Pending(Command),
    Uncertain(Command),
    /// A known refusal: nothing was written; a new press is a deliberate new intent.
    Failed,
}

#[derive(Clone, Debug, Default)]
struct Target {
    own: Own,
    /// Highest order taken in this generation, kept while the target is unavailable.
    shown_order: i64,
    reading: bool,
    count: CountFact,
    count_stamp: u64,
    phase: Phase,
    /// `403 self_endorsement`/`endorsement_not_permitted` learned from a write.
    refused: Option<EndorsementBlock>,
}

impl Target {
    /// Takes `token` unless it is older than the one shown. `block` replaces the advisory
    /// block (own-state reads); `None` keeps it (writes carry no advisory facts).
    fn take(&mut self, token: EndorsementToken, block: Option<Option<EndorsementBlock>>) -> bool {
        let valid = token.order >= 1 && (1..=token.order).contains(&token.version);
        if !valid || token.order < self.shown_order {
            return false;
        }
        let kept = match self.own {
            Own::Known { block, .. } => block,
            _ => None,
        };
        self.own = Own::Known {
            token,
            block: block.unwrap_or(kept),
        };
        self.shown_order = token.order;
        true
    }

    fn set_count(&mut self, count: CountFact, stamp: u64) {
        self.count = count;
        self.count_stamp = stamp;
    }
}

#[derive(Clone, Debug)]
pub struct EndorsementController {
    scope: Scope,
    generation: u64,
    clock: u64,
    session_invalid: bool,
    targets: BTreeMap<TargetKey, Target>,
}

impl EndorsementController {
    pub fn new(scope: Scope) -> Self {
        Self {
            scope,
            generation: 1,
            clock: 0,
            session_invalid: false,
            targets: BTreeMap::new(),
        }
    }

    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// Another session or work never sees this one's states, counts or in-flight commands,
    /// even if it changes back: every earlier command and read belongs to an old generation.
    pub fn change_scope(&mut self, scope: Scope) {
        if self.scope != scope {
            self.scope = scope;
            self.generation += 1;
            self.session_invalid = false;
            self.targets.clear();
        }
    }

    /// Commands need a host session marker the server has not rejected.
    pub fn write_ready(&self) -> bool {
        self.scope.session.is_some() && !self.session_invalid
    }

    pub fn track(&mut self, target: TargetKey) {
        self.targets.entry(target).or_default();
    }

    pub fn is_tracked(&self, target: &TargetKey) -> bool {
        self.targets.contains_key(target)
    }

    /// The view facts of one tracked target; `None` when it has no endorsement here.
    pub fn facts(&self, key: &TargetKey) -> Option<EndorsementFacts> {
        let target = self.targets.get(key)?;
        let mut facts = EndorsementFacts {
            value: ToggleValue::Unknown,
            phase: TogglePhase::Idle,
            count: target.count,
            block: None,
        };
        // A guest, or a session the server did not recognize: public count and a reason only.
        if !self.write_ready() {
            facts.block = Some(EndorsementBlock::SignedOut);
            return Some(facts);
        }
        facts.phase = match &target.phase {
            Phase::Idle => TogglePhase::Idle,
            Phase::Pending(command) => TogglePhase::Pending {
                desired: command.endorsed,
            },
            Phase::Uncertain(command) => TogglePhase::Uncertain {
                desired: command.endorsed,
            },
            Phase::Failed => TogglePhase::Failed,
        };
        match target.own {
            Own::Known { token, block } => {
                facts.value = ToggleValue::from_selected(token.selected);
                facts.block = target.refused.or(block);
            }
            Own::Unavailable => {
                facts.block = Some(target.refused.unwrap_or(EndorsementBlock::Unavailable));
            }
            Own::Unread | Own::Failed => facts.block = target.refused,
        }
        Some(facts)
    }

    /// A press with its desired state. The target turns `Pending` before this returns, so a
    /// second press on the same frame (or while the answer is outstanding) sends nothing.
    pub fn press(
        &mut self,
        key: &TargetKey,
        desired: bool,
        command_key: String,
    ) -> Option<Command> {
        if !self.write_ready() {
            return None;
        }
        let target = self.targets.get_mut(key)?;
        if target.refused.is_some()
            || matches!(target.phase, Phase::Pending(_) | Phase::Uncertain(_))
        {
            return None;
        }
        let Own::Known { token, block: None } = target.own else {
            return None;
        };
        self.clock += 1;
        let command = Command {
            target: key.clone(),
            command_key,
            expected_order: token.order,
            endorsed: desired,
            generation: self.generation,
            attempt: self.clock,
        };
        target.phase = Phase::Pending(command.clone());
        Some(command)
    }

    pub fn recover(&mut self, key: &TargetKey) -> Recovery {
        if !self.write_ready() {
            return Recovery::Nothing;
        }
        let Some(target) = self.targets.get_mut(key) else {
            return Recovery::Nothing;
        };
        if let Phase::Uncertain(command) = &target.phase {
            let mut replay = command.clone();
            self.clock += 1;
            replay.attempt = self.clock;
            target.phase = Phase::Pending(replay.clone());
            return Recovery::Replay(replay);
        }
        let readable = matches!(target.own, Own::Unread | Own::Failed)
            && matches!(target.phase, Phase::Idle | Phase::Failed);
        if readable {
            return self
                .begin_own(vec![key.clone()])
                .map_or(Recovery::Nothing, Recovery::ReadOwn);
        }
        Recovery::Nothing
    }

    pub fn complete(
        &mut self,
        command: &Command,
        result: Result<EndorsementWrite, ApiError>,
    ) -> Completion {
        if command.generation != self.generation {
            return Completion::Stale;
        }
        if result == Err(ApiError::Unauthenticated) {
            self.session_invalid = true;
        }
        self.clock += 1;
        let stamp = self.clock;
        let work_id = self.scope.work_id.clone();
        let Some(target) = self.targets.get_mut(&command.target) else {
            return Completion::Stale;
        };
        let answered = |write: &EndorsementWrite| answers(write, &work_id, &command.target);
        // A token is a server fact at some point: take a newer one even from an older attempt.
        match &result {
            Ok(write) if answered(write) => {
                if target.take(write.endorsement, None) {
                    target.set_count(count(&write.summary.status, write.summary.count), stamp);
                }
            }
            Err(ApiError::EndorsementConflict { observed }) => {
                target.take(*observed, None);
            }
            _ => {}
        }
        if target.phase != Phase::Pending(command.clone()) {
            return Completion::Stale;
        }
        let mut completion = Completion::Settled;
        target.phase = match result {
            Ok(write) if answered(&write) => Phase::Idle,
            // Shown already; the old intent is never resent with the new order.
            Err(ApiError::EndorsementConflict { observed })
                if observed.selected == command.endorsed =>
            {
                Phase::Idle
            }
            Err(ApiError::EndorsementConflict { .. }) => Phase::Failed,
            Err(ApiError::RevisionChanged { .. }) => {
                target.own = Own::Unavailable;
                target.set_count(CountFact::Unavailable, stamp);
                completion = Completion::RevisionChanged;
                Phase::Idle
            }
            Err(ApiError::SelfEndorsement) => {
                target.refused = Some(EndorsementBlock::OwnContent);
                Phase::Idle
            }
            Err(ApiError::EndorsementNotPermitted) => {
                target.refused = Some(EndorsementBlock::NotPermitted);
                Phase::Idle
            }
            Err(ApiError::NotFound) => {
                target.own = Own::Unavailable;
                target.set_count(CountFact::Unavailable, stamp);
                Phase::Idle
            }
            Err(ApiError::Unauthenticated) => Phase::Idle,
            Err(ApiError::KeyReused | ApiError::Invalid { .. } | ApiError::Forbidden) => {
                Phase::Failed
            }
            // Lost, timed out, `503`, `500` or an answer that does not name this target.
            Ok(_) | Err(_) => Phase::Uncertain(command.clone()),
        };
        completion
    }

    /// Starts a public summary read of `targets` (tracking them).
    pub fn begin_summaries(&mut self, targets: Vec<TargetKey>) -> ReadTicket {
        for target in &targets {
            self.track(target.clone());
        }
        self.clock += 1;
        ReadTicket {
            generation: self.generation,
            stamp: self.clock,
            targets,
        }
    }

    /// Counts of a server-rendered read, applied to `targets` only. Unlike a browser read it
    /// never supersedes other targets, so SSR and hydration agree whatever order their
    /// resources resolved in; a newer revision is detected by the browser afterwards.
    pub fn seed(&mut self, targets: Vec<TargetKey>, summaries: Option<&EndorsementSummaries>) {
        let ticket = self.begin_summaries(targets);
        self.apply_counts(&ticket, summaries);
    }

    /// `None`: the read failed. A count never seen becomes unavailable; a known one is kept
    /// as the last success. Returns true when the summaries name a newer public revision than
    /// a tracked work target (the page shows a superseded revision).
    pub fn apply_summaries(
        &mut self,
        ticket: &ReadTicket,
        summaries: Option<&EndorsementSummaries>,
    ) -> bool {
        if ticket.generation != self.generation || !self.apply_counts(ticket, summaries) {
            return false;
        }
        let current = summaries
            .map(|s| s.work.revision_id.clone())
            .unwrap_or_default();
        self.supersede(&current, ticket.stamp)
    }

    /// Returns whether `summaries` were usable for this scope.
    fn apply_counts(
        &mut self,
        ticket: &ReadTicket,
        summaries: Option<&EndorsementSummaries>,
    ) -> bool {
        if ticket.generation != self.generation {
            return false;
        }
        let Some(summaries) = summaries.filter(|s| s.work_id == self.scope.work_id) else {
            for key in &ticket.targets {
                if let Some(target) = self.targets.get_mut(key)
                    && target.count == CountFact::Hidden
                    && ticket.stamp > target.count_stamp
                {
                    target.set_count(CountFact::Unavailable, ticket.stamp);
                }
            }
            return false;
        };
        for key in &ticket.targets {
            let Some(target) = self.targets.get_mut(key) else {
                continue;
            };
            if ticket.stamp <= target.count_stamp {
                continue;
            }
            let fact = match key {
                TargetKey::Work { revision_id } if *revision_id == summaries.work.revision_id => {
                    count(&summaries.work.status, summaries.work.count)
                }
                TargetKey::Work { .. } => CountFact::Unavailable,
                TargetKey::Comment { comment_id } => summaries
                    .comments
                    .iter()
                    .find(|c| c.comment_id == *comment_id)
                    .map_or(CountFact::Unavailable, |c| count(&c.status, c.count)),
            };
            target.set_count(fact, ticket.stamp);
        }
        true
    }

    /// Starts an own-state read of the targets not already being read; `None` for a guest or
    /// when nothing is left to read.
    pub fn begin_own(&mut self, targets: Vec<TargetKey>) -> Option<ReadTicket> {
        if !self.write_ready() {
            return None;
        }
        let mut wanted = Vec::new();
        for key in targets {
            let target = self.targets.entry(key.clone()).or_default();
            if !target.reading {
                target.reading = true;
                wanted.push(key);
            }
        }
        if wanted.is_empty() {
            return None;
        }
        self.clock += 1;
        Some(ReadTicket {
            generation: self.generation,
            stamp: self.clock,
            targets: wanted,
        })
    }

    /// Returns true when the read names a newer public revision than a tracked work target.
    pub fn apply_own(
        &mut self,
        ticket: &ReadTicket,
        result: Result<OwnEndorsements, ApiError>,
    ) -> bool {
        if ticket.generation != self.generation {
            return false;
        }
        for key in &ticket.targets {
            if let Some(target) = self.targets.get_mut(key) {
                target.reading = false;
            }
        }
        match result {
            Ok(own) if own.work_id == self.scope.work_id => {
                for key in &ticket.targets {
                    let entry = match key {
                        TargetKey::Work { revision_id } if *revision_id == own.revision_id => {
                            Some(&own.work)
                        }
                        TargetKey::Work { .. } => None,
                        TargetKey::Comment { comment_id } => own
                            .comments
                            .iter()
                            .find(|c| c.comment_id == *comment_id)
                            .map(|c| &c.own),
                    };
                    let Some(target) = self.targets.get_mut(key) else {
                        continue;
                    };
                    match entry.and_then(own_token) {
                        Some((token, block)) => {
                            target.take(token, Some(block));
                        }
                        None => target.own = Own::Unavailable,
                    }
                }
                self.supersede(&own.revision_id, ticket.stamp)
            }
            Err(ApiError::Unauthenticated) => {
                self.session_invalid = true;
                false
            }
            Err(ApiError::NotFound) => {
                for key in &ticket.targets {
                    if let Some(target) = self.targets.get_mut(key) {
                        target.own = Own::Unavailable;
                    }
                }
                false
            }
            // Temporary: the last known state stays; an unread one becomes unknown, never Off.
            Ok(_) | Err(_) => {
                for key in &ticket.targets {
                    if let Some(target) = self.targets.get_mut(key)
                        && target.own == Own::Unread
                    {
                        target.own = Own::Failed;
                    }
                }
                false
            }
        }
    }

    /// A tracked work revision that is not `current` is no longer public: no count, no state.
    fn supersede(&mut self, current: &str, stamp: u64) -> bool {
        let mut superseded = false;
        for (key, target) in &mut self.targets {
            if let TargetKey::Work { revision_id } = key
                && revision_id != current
            {
                superseded = true;
                target.own = Own::Unavailable;
                if stamp > target.count_stamp {
                    target.set_count(CountFact::Unavailable, stamp);
                }
            }
        }
        superseded
    }
}

fn answers(write: &EndorsementWrite, work_id: &str, target: &TargetKey) -> bool {
    let named = match target {
        TargetKey::Work { revision_id } => {
            write.revision_id.as_deref() == Some(revision_id) && write.comment_id.is_none()
        }
        TargetKey::Comment { comment_id } => {
            write.comment_id.as_deref() == Some(comment_id) && write.revision_id.is_none()
        }
    };
    named && write.work_id == work_id && matches!(write.outcome.as_str(), "accepted" | "replayed")
}

fn count(status: &str, count: Option<i64>) -> CountFact {
    match (status, count) {
        ("available", Some(n)) => CountFact::from_i64(n),
        _ => CountFact::Unavailable,
    }
}

/// `None` for an unavailable or malformed entry: no stored token is ever shown for it.
fn own_token(own: &OwnEndorsement) -> Option<(EndorsementToken, Option<EndorsementBlock>)> {
    let selected = match own.state.as_str() {
        "selected" => true,
        "not_selected" => false,
        _ => return None,
    };
    let token = EndorsementToken {
        selected,
        version: own.version?,
        order: own.order?,
    };
    let block = match own.blocked_reason.as_deref() {
        Some("own_content") => Some(EndorsementBlock::OwnContent),
        Some("not_permitted") => Some(EndorsementBlock::NotPermitted),
        // An unknown reason or an unexplained refusal fails closed.
        Some(_) => Some(EndorsementBlock::Unavailable),
        None if !own.can_endorse => Some(EndorsementBlock::Unavailable),
        None => None,
    };
    Some((token, block))
}

/// Endorsement targets of the public comments on a page, in page order: roots that are shown
/// (not anchors) and their preview replies. Pending/rejected own comments have none.
pub fn page_targets(page: &DiscussionPage) -> Vec<TargetKey> {
    let mut comments = Vec::new();
    for thread in &page.roots {
        if let RootView::Comment { comment } = &thread.root {
            comments.push(comment);
        }
        comments.extend(thread.preview.iter());
    }
    public_targets(comments)
}

pub fn public_targets<'a>(comments: impl IntoIterator<Item = &'a CommentDto>) -> Vec<TargetKey> {
    let mut targets: Vec<TargetKey> = Vec::new();
    for comment in comments {
        let key = TargetKey::comment(&comment.comment_id);
        if comment.visibility == "public" && !targets.contains(&key) {
            targets.push(key);
        }
    }
    targets
}

/// A random version-4 UUID for `command_key` from 16 random bytes.
pub fn uuid_v4(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{
        CommentEndorsementSummary, CommentOwnEndorsement, EndorsementSummary,
        WorkEndorsementSummary,
    };

    const WORK: &str = "33333333-3333-4333-8333-333333333333";
    const REV: &str = "44444444-4444-4444-8444-444444444444";

    fn work() -> TargetKey {
        TargetKey::Work {
            revision_id: REV.into(),
        }
    }

    fn scope(session: Option<&str>) -> Scope {
        Scope {
            work_id: WORK.into(),
            session: session.map(str::to_owned),
        }
    }

    fn token(selected: bool, version: i64, order: i64) -> EndorsementToken {
        EndorsementToken {
            selected,
            version,
            order,
        }
    }

    fn own_entry(state: &str, version: i64, order: i64) -> OwnEndorsement {
        OwnEndorsement {
            state: state.into(),
            version: Some(version),
            order: Some(order),
            can_endorse: true,
            blocked_reason: None,
        }
    }

    fn own(work: OwnEndorsement, comments: Vec<(&str, OwnEndorsement)>) -> OwnEndorsements {
        OwnEndorsements {
            work_id: WORK.into(),
            revision_id: REV.into(),
            work,
            comments: comments
                .into_iter()
                .map(|(id, own)| CommentOwnEndorsement {
                    comment_id: id.into(),
                    own,
                })
                .collect(),
        }
    }

    fn summaries(work: i64, comments: Vec<(&str, Option<i64>)>) -> EndorsementSummaries {
        EndorsementSummaries {
            work_id: WORK.into(),
            work: WorkEndorsementSummary {
                status: "available".into(),
                count: Some(work),
                revision_id: REV.into(),
            },
            comments: comments
                .into_iter()
                .map(|(id, count)| CommentEndorsementSummary {
                    comment_id: id.into(),
                    status: if count.is_some() {
                        "available"
                    } else {
                        "unavailable"
                    }
                    .into(),
                    count,
                })
                .collect(),
        }
    }

    fn write(
        target: &TargetKey,
        outcome: &str,
        current: EndorsementToken,
        count: i64,
    ) -> EndorsementWrite {
        let (revision_id, comment_id) = match target {
            TargetKey::Work { revision_id } => (Some(revision_id.clone()), None),
            TargetKey::Comment { comment_id } => (None, Some(comment_id.clone())),
        };
        EndorsementWrite {
            work_id: WORK.into(),
            revision_id,
            comment_id,
            outcome: outcome.into(),
            endorsement: current,
            committed: None,
            summary: EndorsementSummary {
                status: "available".into(),
                count: Some(count),
            },
        }
    }

    /// A signed-in controller that has read Off@1 for the work and comment `c1`, with counts.
    fn ready() -> EndorsementController {
        let mut c = EndorsementController::new(scope(Some("fixture-a")));
        let targets = vec![work(), TargetKey::comment("c1")];
        let read = c.begin_summaries(targets.clone());
        c.apply_summaries(&read, Some(&summaries(12, vec![("c1", Some(3))])));
        let ticket = c.begin_own(targets).unwrap();
        c.apply_own(
            &ticket,
            Ok(own(
                own_entry("not_selected", 1, 1),
                vec![("c1", own_entry("not_selected", 1, 1))],
            )),
        );
        c
    }

    fn facts(c: &EndorsementController, key: &TargetKey) -> EndorsementFacts {
        c.facts(key).unwrap()
    }

    #[test]
    fn lve_web_double_tap_sends_one_command_and_confirms_only_after_the_ack() {
        let mut c = ready();
        let first = c.press(&work(), true, "k1".into()).unwrap();
        assert_eq!(first.expected_order, 1);
        assert!(
            c.press(&work(), true, "k2".into()).is_none(),
            "second tap while pending"
        );
        assert!(
            c.press(&work(), false, "k3".into()).is_none(),
            "opposite tap while pending"
        );
        let pending = facts(&c, &work());
        assert_eq!(
            pending.value,
            ToggleValue::Off,
            "CHOICE-10 A: no optimistic On"
        );
        assert_eq!(pending.phase, TogglePhase::Pending { desired: true });
        assert_eq!(pending.count, CountFact::Known(12), "no optimistic count");
        let done = c.complete(
            &first,
            Ok(write(&work(), "accepted", token(true, 2, 2), 13)),
        );
        assert_eq!(done, Completion::Settled);
        let shown = facts(&c, &work());
        assert_eq!(
            (shown.value, shown.phase, shown.count),
            (ToggleValue::On, TogglePhase::Idle, CountFact::Known(13))
        );
        assert_eq!(
            c.complete(
                &first,
                Ok(write(&work(), "accepted", token(true, 2, 2), 13))
            ),
            Completion::Stale,
            "a duplicated ACK settles nothing twice"
        );
        let next = c.press(&work(), false, "k4".into()).unwrap();
        assert_eq!((next.expected_order, next.endorsed), (2, false));
        assert_eq!(
            facts(&c, &TargetKey::comment("c1")),
            EndorsementFacts {
                value: ToggleValue::Off,
                phase: TogglePhase::Idle,
                count: CountFact::Known(3),
                block: None
            },
            "each target is independent: nothing summed into the comment"
        );
    }

    #[test]
    fn lve_web_late_and_stale_answers_never_move_the_page_back() {
        let mut c = ready();
        let on = c.press(&work(), true, "on".into()).unwrap();
        // An own-state read that started before the press returns after it: same order, kept.
        let early = c.begin_own(vec![work()]).unwrap();
        c.complete(&on, Ok(write(&work(), "accepted", token(true, 2, 2), 13)));
        let off = c.press(&work(), false, "off".into()).unwrap();
        c.complete(&off, Ok(write(&work(), "accepted", token(false, 3, 3), 12)));
        c.apply_own(&early, Ok(own(own_entry("selected", 2, 2), vec![])));
        assert_eq!(
            facts(&c, &work()).value,
            ToggleValue::Off,
            "late read (order 2) after order 3"
        );
        // A late summary that started before the write settled cannot replace its count.
        let stale_count = c.begin_summaries(vec![work()]);
        let again = c.press(&work(), true, "again".into()).unwrap();
        c.complete(
            &again,
            Ok(write(&work(), "accepted", token(true, 4, 4), 13)),
        );
        c.apply_summaries(&stale_count, Some(&summaries(12, vec![])));
        assert_eq!(facts(&c, &work()).count, CountFact::Known(13));
        // A late ACK of the first command (order 2) arrives last: ignored.
        assert_eq!(
            c.complete(&on, Ok(write(&work(), "accepted", token(true, 2, 2), 13))),
            Completion::Stale
        );
        let shown = facts(&c, &work());
        assert_eq!(
            (shown.value, shown.phase),
            (ToggleValue::On, TogglePhase::Idle)
        );
        // A fresh summary started after the write may update the count.
        let fresh = c.begin_summaries(vec![work()]);
        c.apply_summaries(&fresh, Some(&summaries(20, vec![])));
        assert_eq!(facts(&c, &work()).count, CountFact::Known(20));
    }

    #[test]
    fn lve_web_lost_ack_is_uncertain_until_an_exact_replay_reconciles_it() {
        for lost in [
            ApiError::Network,
            ApiError::Unavailable,
            ApiError::Unexpected { status: 500 },
        ] {
            let mut c = ready();
            let key = TargetKey::comment("c1");
            let sent = c.press(&key, true, "lost".into()).unwrap();
            assert_eq!(c.complete(&sent, Err(lost.clone())), Completion::Settled);
            let uncertain = facts(&c, &key);
            assert_eq!(
                uncertain.phase,
                TogglePhase::Uncertain { desired: true },
                "{lost:?}"
            );
            assert_eq!(
                uncertain.value,
                ToggleValue::Off,
                "a timeout never proves a rollback nor a commit"
            );
            assert!(
                c.press(&key, true, "blind".into()).is_none(),
                "no new intent while uncertain"
            );
            let Recovery::Replay(replay) = c.recover(&key) else {
                panic!("uncertain reconciles by replay");
            };
            assert_eq!(
                replay.request(),
                sent.request(),
                "same key, order, desired state"
            );
            assert_ne!(replay, sent, "a new physical attempt");
            assert_eq!(
                facts(&c, &key).phase,
                TogglePhase::Pending { desired: true }
            );
            // The first attempt's answer arriving now cannot settle the replay.
            assert_eq!(c.complete(&sent, Err(ApiError::Network)), Completion::Stale);
            assert_eq!(
                facts(&c, &key).phase,
                TogglePhase::Pending { desired: true }
            );
            let mut replayed = write(&key, "replayed", token(true, 2, 2), 4);
            replayed.committed = Some(token(true, 2, 2));
            assert_eq!(c.complete(&replay, Ok(replayed)), Completion::Settled);
            let shown = facts(&c, &key);
            assert_eq!(
                (shown.value, shown.phase, shown.count),
                (ToggleValue::On, TogglePhase::Idle, CountFact::Known(4))
            );
        }
    }

    #[test]
    fn lve_web_replay_shows_the_current_state_not_the_committed_history() {
        let mut c = ready();
        let key = TargetKey::comment("c1");
        let sent = c.press(&key, true, "k".into()).unwrap();
        c.complete(&sent, Err(ApiError::Network));
        let Recovery::Replay(replay) = c.recover(&key) else {
            panic!()
        };
        // Committed ON@2, then another device removed it (OFF@3).
        let mut answer = write(&key, "replayed", token(false, 3, 3), 3);
        answer.committed = Some(token(true, 2, 2));
        c.complete(&replay, Ok(answer));
        assert_eq!(facts(&c, &key).value, ToggleValue::Off);
    }

    #[test]
    fn lve_web_unread_or_failed_own_state_reconciles_by_reading_never_as_off() {
        let mut c = EndorsementController::new(scope(Some("fixture-a")));
        let read = c.begin_summaries(vec![work()]);
        c.apply_summaries(&read, None);
        assert_eq!(
            facts(&c, &work()).count,
            CountFact::Unavailable,
            "a failed first read is –, not 0"
        );
        assert_eq!(facts(&c, &work()).value, ToggleValue::Unknown);
        assert!(
            c.press(&work(), true, "k".into()).is_none(),
            "unknown is never pressed as Off"
        );
        let ticket = c.begin_own(vec![work()]).unwrap();
        assert!(
            c.begin_own(vec![work()]).is_none(),
            "one read at a time per target"
        );
        assert_eq!(c.recover(&work()), Recovery::Nothing);
        c.apply_own(&ticket, Err(ApiError::Unavailable));
        assert_eq!(facts(&c, &work()).value, ToggleValue::Unknown);
        let Recovery::ReadOwn(retry) = c.recover(&work()) else {
            panic!("unknown reconciles by an own-state read");
        };
        assert_eq!(retry.targets, vec![work()]);
        c.apply_own(&retry, Ok(own(own_entry("selected", 2, 5), vec![])));
        assert_eq!(facts(&c, &work()).value, ToggleValue::On);
    }

    #[test]
    fn lve_web_conflict_shows_the_observed_state_and_never_resends_the_old_intent() {
        let mut c = ready();
        let sent = c.press(&work(), true, "k".into()).unwrap();
        // Another tab selected and removed meanwhile: OFF@3.
        let done = c.complete(
            &sent,
            Err(ApiError::EndorsementConflict {
                observed: token(false, 3, 3),
            }),
        );
        assert_eq!(done, Completion::Settled);
        let shown = facts(&c, &work());
        assert_eq!(
            (shown.value, shown.phase),
            (ToggleValue::Off, TogglePhase::Failed)
        );
        assert_eq!(
            c.recover(&work()),
            Recovery::Nothing,
            "no replay of a refused command"
        );
        let next = c.press(&work(), true, "new-intent".into()).unwrap();
        assert_eq!(
            (next.expected_order, next.command_key.as_str()),
            (3, "new-intent")
        );
        // A conflict whose observed state already matches the intent settles quietly.
        let mut c = ready();
        let sent = c.press(&work(), true, "k".into()).unwrap();
        c.complete(
            &sent,
            Err(ApiError::EndorsementConflict {
                observed: token(true, 2, 2),
            }),
        );
        let shown = facts(&c, &work());
        assert_eq!(
            (shown.value, shown.phase),
            (ToggleValue::On, TogglePhase::Idle)
        );
    }

    #[test]
    fn lve_web_session_change_fences_commands_reads_and_states_even_after_return() {
        let mut c = ready();
        let original = c.scope().clone();
        let sent = c.press(&work(), true, "a".into()).unwrap();
        let read = c.begin_own(vec![TargetKey::comment("c1")]).unwrap();
        let counts = c.begin_summaries(vec![work()]);
        c.change_scope(scope(Some("fixture-b")));
        assert!(c.facts(&work()).is_none(), "another session starts empty");
        c.track(work());
        assert_eq!(
            c.complete(&sent, Ok(write(&work(), "accepted", token(true, 2, 2), 13))),
            Completion::Stale
        );
        assert!(!c.apply_own(
            &read,
            Ok(own(
                own_entry("selected", 2, 2),
                vec![("c1", own_entry("selected", 2, 2))]
            ))
        ));
        c.apply_summaries(&counts, Some(&summaries(99, vec![])));
        let b = facts(&c, &work());
        assert_eq!(
            (b.value, b.count),
            (ToggleValue::Unknown, CountFact::Hidden),
            "nothing of A reaches B"
        );
        c.change_scope(original);
        c.track(work());
        assert_eq!(
            c.complete(&sent, Ok(write(&work(), "accepted", token(true, 2, 2), 13))),
            Completion::Stale,
            "returning to A does not revive its old command"
        );
        assert_eq!(facts(&c, &work()).value, ToggleValue::Unknown);
        // A guest sees public counts and a text reason; nothing is sent or read for them.
        c.change_scope(scope(None));
        let read = c.begin_summaries(vec![work()]);
        c.apply_summaries(&read, Some(&summaries(12, vec![])));
        assert_eq!(
            facts(&c, &work()),
            EndorsementFacts {
                value: ToggleValue::Unknown,
                phase: TogglePhase::Idle,
                count: CountFact::Known(12),
                block: Some(EndorsementBlock::SignedOut)
            }
        );
        assert!(c.press(&work(), true, "guest".into()).is_none());
        assert!(c.begin_own(vec![work()]).is_none());
    }

    #[test]
    fn lve_web_refusals_map_to_reasons_and_unknown_answers_stay_uncertain() {
        let comment = TargetKey::comment("c1");
        let cases: [(ApiError, Option<EndorsementBlock>, TogglePhase); 7] = [
            (
                ApiError::SelfEndorsement,
                Some(EndorsementBlock::OwnContent),
                TogglePhase::Idle,
            ),
            (
                ApiError::EndorsementNotPermitted,
                Some(EndorsementBlock::NotPermitted),
                TogglePhase::Idle,
            ),
            (
                ApiError::NotFound,
                Some(EndorsementBlock::Unavailable),
                TogglePhase::Idle,
            ),
            (ApiError::KeyReused, None, TogglePhase::Failed),
            (
                ApiError::Invalid {
                    field: Some("command_key".into()),
                },
                None,
                TogglePhase::Failed,
            ),
            (ApiError::Forbidden, None, TogglePhase::Failed),
            (
                ApiError::VersionConflict { current_version: 4 },
                None,
                TogglePhase::Uncertain { desired: true },
            ),
        ];
        for (error, block, phase) in cases {
            let mut c = ready();
            let sent = c.press(&comment, true, "k".into()).unwrap();
            c.complete(&sent, Err(error.clone()));
            let shown = facts(&c, &comment);
            assert_eq!((shown.block, shown.phase), (block, phase), "{error:?}");
            if block.is_some() {
                assert!(
                    c.press(&comment, true, "again".into()).is_none(),
                    "{error:?} blocks"
                );
            }
        }
        let mut c = ready();
        let sent = c.press(&comment, true, "k".into()).unwrap();
        c.complete(&sent, Err(ApiError::NotFound));
        assert_eq!(
            facts(&c, &comment).count,
            CountFact::Unavailable,
            "hidden target: –, never 0"
        );
        // An ACK naming another target is not this command's outcome.
        let mut c = ready();
        let sent = c.press(&comment, true, "k".into()).unwrap();
        c.complete(
            &sent,
            Ok(write(
                &TargetKey::comment("other"),
                "accepted",
                token(true, 2, 2),
                1,
            )),
        );
        let shown = facts(&c, &comment);
        assert_eq!(
            (shown.value, shown.phase),
            (ToggleValue::Off, TogglePhase::Uncertain { desired: true })
        );
    }

    #[test]
    fn lve_web_401_turns_every_target_into_a_signed_out_reason() {
        let mut c = ready();
        let a = c.press(&work(), true, "a".into()).unwrap();
        let b = c
            .press(&TargetKey::comment("c1"), true, "b".into())
            .unwrap();
        c.complete(&a, Err(ApiError::Unauthenticated));
        assert!(!c.write_ready());
        for key in [work(), TargetKey::comment("c1")] {
            let shown = facts(&c, &key);
            assert_eq!(
                (shown.value, shown.block),
                (ToggleValue::Unknown, Some(EndorsementBlock::SignedOut))
            );
        }
        assert_eq!(
            c.complete(&b, Err(ApiError::Unauthenticated)),
            Completion::Settled
        );
        assert_eq!(c.recover(&work()), Recovery::Nothing);
        assert!(c.press(&work(), true, "c".into()).is_none());
    }

    #[test]
    fn lve_web_revision_change_supersedes_the_shown_revision_without_a_count() {
        let mut c = ready();
        let sent = c.press(&work(), true, "k".into()).unwrap();
        let done = c.complete(
            &sent,
            Err(ApiError::RevisionChanged {
                current_revision_id: "newer".into(),
            }),
        );
        assert_eq!(done, Completion::RevisionChanged);
        let shown = facts(&c, &work());
        assert_eq!(
            (shown.value, shown.count, shown.block),
            (
                ToggleValue::Unknown,
                CountFact::Unavailable,
                Some(EndorsementBlock::Unavailable)
            )
        );
        // A read naming a newer public revision also supersedes the page's revision.
        let mut c = ready();
        let read = c.begin_summaries(vec![work()]);
        let mut newer = summaries(2, vec![]);
        newer.work.revision_id = "newer".into();
        assert!(c.apply_summaries(&read, Some(&newer)));
        assert_eq!(
            facts(&c, &work()).count,
            CountFact::Unavailable,
            "another revision's count is not shown"
        );
        let ticket = c.begin_own(vec![work()]).unwrap();
        let mut current = own(own_entry("selected", 2, 2), vec![]);
        current.revision_id = "newer".into();
        assert!(c.apply_own(&ticket, Ok(current)));
        assert_eq!(facts(&c, &work()).value, ToggleValue::Unknown);
        // A server-rendered seed of comments never touches the work target, whatever order the
        // SSR resources resolved in; only a browser read supersedes it.
        let mut c = EndorsementController::new(scope(None));
        c.seed(vec![work()], Some(&summaries(5, vec![])));
        c.seed(vec![TargetKey::comment("c1")], Some(&newer));
        assert_eq!(facts(&c, &work()).count, CountFact::Known(5));
    }

    #[test]
    fn lve_web_own_state_advisories_and_unavailable_comments_are_explicit() {
        let mut c = EndorsementController::new(scope(Some("fixture-a")));
        let targets = vec![
            work(),
            TargetKey::comment("mine"),
            TargetKey::comment("hidden"),
        ];
        let read = c.begin_summaries(targets.clone());
        c.apply_summaries(
            &read,
            Some(&summaries(1, vec![("mine", Some(0)), ("hidden", None)])),
        );
        let ticket = c.begin_own(targets).unwrap();
        let mut author = own_entry("not_selected", 1, 1);
        author.can_endorse = false;
        author.blocked_reason = Some("own_content".into());
        let unavailable = OwnEndorsement {
            state: "unavailable".into(),
            version: None,
            order: None,
            can_endorse: false,
            blocked_reason: None,
        };
        c.apply_own(
            &ticket,
            Ok(own(
                author.clone(),
                vec![("mine", author), ("hidden", unavailable)],
            )),
        );
        assert_eq!(facts(&c, &work()).block, Some(EndorsementBlock::OwnContent));
        assert!(
            c.press(&work(), true, "self".into()).is_none(),
            "advisory own_content blocks presses"
        );
        let hidden = facts(&c, &TargetKey::comment("hidden"));
        assert_eq!(
            (hidden.value, hidden.count, hidden.block),
            (
                ToggleValue::Unknown,
                CountFact::Unavailable,
                Some(EndorsementBlock::Unavailable)
            )
        );
        assert_eq!(
            facts(&c, &TargetKey::comment("mine")).count,
            CountFact::Known(0),
            "a known zero is a real zero"
        );
    }

    #[test]
    fn lve_web_requests_carry_the_target_and_the_shown_order() {
        let mut c = ready();
        let sent = c
            .press(&work(), true, "0192f3a1-0000-4000-8000-00000000c0de".into())
            .unwrap();
        assert_eq!(
            sent.request(),
            Request::Work(WorkEndorsementRequest {
                command_key: "0192f3a1-0000-4000-8000-00000000c0de".into(),
                expected_order: 1,
                endorsed: true,
                revision_id: REV.into(),
            })
        );
        let reply = c
            .press(&TargetKey::comment("c1"), false, "k".into())
            .unwrap();
        assert!(
            matches!(reply.request(), Request::Comment { comment_id, body } if comment_id == "c1" && !body.endorsed && body.expected_order == 1)
        );
        let key = uuid_v4([0xff; 16]);
        assert_eq!(key.len(), 36);
        assert_eq!(&key[14..15], "4");
        assert!(matches!(&key[19..20], "8" | "9" | "a" | "b"), "{key}");
    }

    #[test]
    fn lve_web_page_targets_cover_public_roots_and_replies_only() {
        let page: DiscussionPage = serde_json::from_str(include_str!(
            "../../../services/vink-app/contracts/http/v1/fixtures/discussion.get.json"
        ))
        .unwrap();
        let targets = page_targets(&page);
        let mut expected = Vec::new();
        for thread in &page.roots {
            if let RootView::Comment { comment } = &thread.root
                && comment.visibility == "public"
            {
                expected.push(TargetKey::comment(&comment.comment_id));
            }
            for reply in thread.preview.iter().filter(|r| r.visibility == "public") {
                expected.push(TargetKey::comment(&reply.comment_id));
            }
        }
        assert_eq!(
            targets, expected,
            "page order, anchors and own pending/rejected excluded"
        );
        assert!(
            page.roots
                .iter()
                .any(|t| matches!(t.root, RootView::Anchor { .. }))
        );
        assert!(
            targets.len()
                < page
                    .roots
                    .iter()
                    .map(|t| t.preview.len() + 1)
                    .sum::<usize>()
        );
    }
}
