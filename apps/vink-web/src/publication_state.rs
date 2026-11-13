//! Browser operation state for the author's publication controls (PUB-AUTHOR-WEB-01), kept free
//! of DOM and transport code. It is not publication authority: the App's guards decide what is
//! public, private or approved. This reducer only decides which server answer the page may show
//! and which command a press sends next.
//!
//! - **Server facts vs. operation.** What the page says about the work comes from the last read
//!   of `GET …/publication/state`. A command's receipt is history of that command, never the
//!   state: after every acknowledgement the page reads again, and the operation only counts as
//!   settled once a read that *started after* the acknowledgement is applied.
//! - **Exact revision.** A submission first needs the editor's text and tags confirmed by the
//!   server (the editor's own `DraftSession` owns saving; this module only watches its facts).
//!   The snapshot is captured at exactly the acknowledged `draft_version` and
//!   `metadata_version`, and `request-public` names exactly the captured `revision_id`. Anything
//!   typed after the author pressed the button stops the submission: nothing is captured, and
//!   nothing differing from what the author pressed on is ever sent.
//! - **Idempotency.** Every logical command has its own key. A retry of a command whose answer was
//!   lost keeps its key and payload. After a lost `request-public`/`set-private` answer the state
//!   is read first; the command is replayed only if the generation it was pinned to still stands,
//!   so a replay can never revive what another tab changed.
//! - **Pinned generation.** A command carries the generation of the state the author was looking
//!   at, never a fresher one fetched to get past a decision made elsewhere.
//! - **Fences.** Every ticket carries the controller epoch (a new session or work starts a new
//!   one) and the operation's id and physical attempt; an answer for anything else is ignored. A
//!   read is applied only if it started after the one already shown.
//! - **Withdrawal wins.** Making the work private supersedes a submission that has not been
//!   answered: its next stage never starts, and a late answer cannot change the page. A request
//!   already on the wire may still commit; the generation fence settles that, not an abort.

use crate::contract::publication::{
    Audience, AuthorPublicationState, AuthorRevision, CapturedSnapshot, Decision, Lifecycle,
    RequestPublicReceipt, SetPrivateReceipt,
};
use crate::contract::{ApiError, SnapshotField};
use crate::desk_state::{DraftSession, Failure, SaveStatus};
use crate::tag_state::{Selection, TagStatus};

/// How many times one submission asks the editor to save before it gives up. A save already in
/// flight when the author pressed may leave newer, still-matching text behind it.
const MAX_SAVES: u8 = 3;

// ---------------------------------------------------------------------------------------------
// What the editor owner says (read only)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFacts {
    /// The server acknowledged the text the editor shows, at this draft version.
    Confirmed {
        version: i64,
    },
    Unsaved,
    Saving,
    Conflict,
    Failed(Failure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagFacts {
    NotLoaded,
    Confirmed {
        version: i64,
    },
    Unsaved,
    Saving,
    /// A conflict, a refused tag or a failed save: the editor shows its own recovery.
    Blocked,
}

/// What the editor's `DraftSession` knows. `text_edits` and `selection` change with every edit,
/// so equal values mean the author typed nothing in between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorFacts {
    pub text: TextFacts,
    pub tags: TagFacts,
    pub text_edits: u64,
    pub selection: Option<Selection>,
}

impl EditorFacts {
    pub fn of(session: &DraftSession) -> Self {
        let text = match session.text.status() {
            SaveStatus::Saved { version } => TextFacts::Confirmed { version },
            SaveStatus::Unsaved => TextFacts::Unsaved,
            SaveStatus::Saving => TextFacts::Saving,
            SaveStatus::Conflict { .. } => TextFacts::Conflict,
            SaveStatus::Failed(failure) => TextFacts::Failed(failure),
        };
        let tags = match session.tags.as_ref() {
            None => TagFacts::NotLoaded,
            Some(tags) => match tags.status() {
                // A save whose outcome is unknown keeps the selection "unsaved" even when it
                // looks equal again; only a confirmed one may be captured.
                TagStatus::Saved { version } if !tags.has_unsaved() => {
                    TagFacts::Confirmed { version }
                }
                TagStatus::Saved { .. } | TagStatus::Unsaved => TagFacts::Unsaved,
                TagStatus::Saving => TagFacts::Saving,
                TagStatus::Conflict { .. } | TagStatus::Refused(_) | TagStatus::Failed(_) => {
                    TagFacts::Blocked
                }
            },
        };
        Self {
            text,
            tags,
            text_edits: session.text.edits(),
            selection: session.tags.as_ref().map(|tags| tags.current().clone()),
        }
    }

    /// Something typed or chosen has not reached the server (a banner, not a gate).
    pub fn has_unsaved(&self) -> bool {
        matches!(self.text, TextFacts::Unsaved | TextFacts::Saving)
            || matches!(self.tags, TagFacts::Unsaved | TagFacts::Saving)
    }

    fn confirmed(&self) -> Option<(i64, i64)> {
        match (self.text, self.tags) {
            (TextFacts::Confirmed { version: text }, TagFacts::Confirmed { version: tags }) => {
                Some((text, tags))
            }
            _ => None,
        }
    }
}

/// What the author pressed on: the editor's edit counters at that moment.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Mark {
    text_edits: u64,
    selection: Option<Selection>,
}

// ---------------------------------------------------------------------------------------------
// Commands, answers, steps
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureCommand {
    pub command_key: String,
    pub expected_draft_version: i64,
    pub expected_metadata_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestCommand {
    pub command_key: String,
    pub revision_id: String,
    pub expected_generation: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateCommand {
    pub command_key: String,
    pub expected_generation: i64,
}

/// The revision a submission froze. Kept for the whole operation, so a retry never freezes another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    pub revision_id: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadTicket {
    epoch: u64,
    seq: u64,
}

/// One physical attempt of one command of one operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandTicket {
    epoch: u64,
    op: u64,
    attempt: u64,
}

/// What the owner of the transport does next. Pure data: the controller never performs I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    None,
    /// The editor owner saves once (its own "Lưu"); this module watches the outcome.
    Save,
    Capture(CommandTicket, CaptureCommand),
    RequestPublic(CommandTicket, RequestCommand),
    SetPrivate(CommandTicket, PrivateCommand),
    Read(ReadTicket),
    /// The server confirmed a withdrawal: public copies in this tab are stale. Then read.
    WithdrawalCommitted(Option<ReadTicket>),
}

/// Which version or generation moved. Nothing was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    /// Another tab saved the text after this one did.
    Draft { current: i64 },
    /// Another tab saved the tags.
    Metadata { current: i64 },
    /// The work's audience intent moved elsewhere.
    Generation { current: i64 },
}

/// Why an operation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The author typed after pressing; nothing was captured.
    EditedWhileSaving,
    /// The editor could not confirm the text or tags; nothing was captured.
    SaveNotConfirmed,
    SessionEnded,
    Forbidden,
    NotFound,
    WorkUnavailable,
    /// That revision was already rejected: only a new revision can be sent.
    RevisionRejected,
    UnknownRevision,
    /// The server saw this key with another payload: a defect, not something to retry.
    KeyReused,
    Invalid,
    /// A success answer that does not describe this command.
    UnexpectedAnswer,
}

/// Why a press cannot start anything now (the reason shown beside the disabled action).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    /// The state has not been read yet, or no longer can be.
    NotRead,
    /// The server answered something this build does not understand.
    Unreadable,
    /// The work is deleted or removed.
    NotActive,
    /// Another operation is waiting for its answer.
    Busy,
    TagsNotLoaded,
    TextConflict,
    TagsBlocked,
    SaveFailing,
    /// `set-private` is only offered for a work that is public or waiting to be.
    NotPublic,
    /// The generation the page shows predates the conflict: read again first.
    NotFresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Done {
    Requested,
    MadePrivate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadFailure {
    SessionEnded,
    Forbidden,
    NotFound,
    /// Network, timeout, `503` or an unclassified answer: the last facts may still be shown.
    Unreachable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UncertainStage {
    Capturing,
    Requesting,
    Making,
}

/// The client operation phases. They say what this tab is doing, never a moderation verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Reading,
    /// The state could not be read and nothing is shown.
    Unavailable(ReadFailure),
    /// The state was read, but contains a value this build does not understand.
    Unreadable,
    Ready,
    Saving,
    Capturing,
    RequestingPublic,
    MakingPrivate,
    Uncertain(UncertainStage),
    Conflict(Conflict),
    Failed(Refusal),
    /// The server acknowledged the command, but the read that states its result has not
    /// succeeded (yet). Nothing is waiting for an answer; the page only has to read again.
    Acknowledged(Done),
    Reconciled(Done),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Stage {
    /// Waiting for the editor owner to confirm the text and tags.
    Saving,
    Capturing(CaptureCommand),
    Requesting(RequestCommand),
    Making(PrivateCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Running,
    /// The answer was lost; the command may have committed.
    Uncertain,
    /// Reading the state to settle an uncertain command.
    Reconciling,
    Conflict(Conflict),
    Stopped(Refusal),
    /// Acknowledged by a receipt; waiting for a read that started afterwards.
    Settling(Done),
    Done(Done),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Operation {
    id: u64,
    stage: Stage,
    status: Status,
    /// Physical attempt of the current command; a replay gets a new one.
    attempt: u64,
    /// The generation of the state the author acted on.
    pinned_generation: i64,
    mark: Option<Mark>,
    saves: u8,
    captured: Option<Captured>,
    /// Reads that started after this sequence may settle the operation.
    after_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub work_id: String,
    /// The tab's session owner marker (`None` for no session). Never a credential.
    pub session: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// The controller
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Controller {
    scope: Scope,
    epoch: u64,
    entropy: [u8; 16],
    keys: u32,
    facts: Option<AuthorPublicationState>,
    read_error: Option<ReadFailure>,
    read_seq: u64,
    applied_seq: u64,
    reading: Option<ReadTicket>,
    reread: bool,
    ops: u64,
    op: Option<Operation>,
}

impl Controller {
    /// `entropy` is 16 random bytes from the browser (`None`: no command can be made).
    pub fn new(scope: Scope, entropy: [u8; 16]) -> Self {
        Self {
            scope,
            epoch: 0,
            entropy,
            keys: 0,
            facts: None,
            read_error: None,
            read_seq: 0,
            applied_seq: 0,
            reading: None,
            reread: false,
            ops: 0,
            op: None,
        }
    }

    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// Another session or work: nothing of the earlier one is shown, kept or answered.
    pub fn change_scope(&mut self, scope: Scope) {
        self.scope = scope;
        self.epoch += 1;
        self.facts = None;
        self.read_error = None;
        self.reading = None;
        self.reread = false;
        self.op = None;
    }

    pub fn facts(&self) -> Option<&AuthorPublicationState> {
        self.facts.as_ref()
    }

    pub fn read_failure(&self) -> Option<ReadFailure> {
        self.read_error
    }

    pub fn reading(&self) -> bool {
        self.reading.is_some()
    }

    fn next_key(&mut self) -> String {
        self.keys += 1;
        let mut bytes = self.entropy;
        let tail = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]])
            .wrapping_add(self.keys);
        bytes[12..].copy_from_slice(&tail.to_be_bytes());
        crate::endorsement_state::uuid_v4(bytes)
    }

    // ---- reads -------------------------------------------------------------------------------

    /// Starts a read unless one is in flight (then it is asked for again when that one ends).
    pub fn begin_read(&mut self) -> Option<ReadTicket> {
        if self.reading.is_some() {
            self.reread = true;
            return None;
        }
        self.read_seq += 1;
        let ticket = ReadTicket {
            epoch: self.epoch,
            seq: self.read_seq,
        };
        self.reading = Some(ticket);
        Some(ticket)
    }

    fn read_step(&mut self) -> Step {
        self.begin_read().map_or(Step::None, Step::Read)
    }

    pub fn read_finished(
        &mut self,
        ticket: ReadTicket,
        result: Result<AuthorPublicationState, ApiError>,
    ) -> Step {
        if ticket.epoch != self.epoch || self.reading != Some(ticket) {
            return Step::None;
        }
        self.reading = None;
        let step = match result {
            Ok(state) => {
                let ours = state.work_id == self.scope.work_id;
                if ours && ticket.seq > self.applied_seq {
                    self.applied_seq = ticket.seq;
                    self.facts = Some(state);
                    self.read_error = None;
                    self.after_read(ticket)
                } else if !ours {
                    // An answer about another work is never shown as this one's.
                    self.facts = None;
                    self.read_error = Some(ReadFailure::Unreachable);
                    self.read_lost()
                } else {
                    Step::None
                }
            }
            Err(error) => {
                let failure = match error {
                    ApiError::Unauthenticated => ReadFailure::SessionEnded,
                    ApiError::Forbidden => ReadFailure::Forbidden,
                    ApiError::NotFound => ReadFailure::NotFound,
                    _ => ReadFailure::Unreachable,
                };
                // A temporary outage keeps the last facts; a loss of authority clears them, so
                // stale private content never bypasses a denial.
                if failure != ReadFailure::Unreachable {
                    self.facts = None;
                }
                self.read_error = Some(failure);
                self.read_lost()
            }
        };
        if matches!(step, Step::None) && self.reread {
            self.reread = false;
            return self.read_step();
        }
        step
    }

    /// A read that settles an uncertain command failed: the author checks again.
    fn read_lost(&mut self) -> Step {
        if let Some(op) = self.op.as_mut()
            && op.status == Status::Reconciling
        {
            op.status = Status::Uncertain;
        }
        Step::None
    }

    /// A read was applied: it may settle or reconcile the operation that waits for one.
    fn after_read(&mut self, ticket: ReadTicket) -> Step {
        let Some(op) = self.op.as_ref() else {
            return Step::None;
        };
        if ticket.seq <= op.after_seq {
            return Step::None;
        }
        match op.status {
            Status::Settling(done) => {
                if let Some(op) = self.op.as_mut() {
                    op.status = Status::Done(done);
                }
                Step::None
            }
            Status::Reconciling => self.reconcile(),
            _ => Step::None,
        }
    }

    /// Settles an uncertain `request-public`/`set-private` from a read that started after the
    /// author asked to check. The command is replayed only while the generation it was pinned to
    /// still stands.
    fn reconcile(&mut self) -> Step {
        let (Some(facts), Some(op)) = (self.facts.as_ref(), self.op.as_ref()) else {
            return Step::None;
        };
        let (generation, audience) = (facts.generation, facts.audience);
        let intent = facts.intent.as_ref().map(|i| i.revision_id.clone());
        let pinned = op.pinned_generation;
        match op.stage.clone() {
            Stage::Requesting(command) => {
                if generation == pinned {
                    return self.replay_request(command);
                }
                let committed = generation > pinned
                    && audience == Audience::Public
                    && intent.as_deref() == Some(command.revision_id.as_str());
                self.reconciled(committed, Done::Requested, generation)
            }
            Stage::Making(command) => {
                if generation == pinned {
                    return self.replay_private(command);
                }
                let committed = generation > pinned && audience == Audience::Private;
                self.reconciled(committed, Done::MadePrivate, generation)
            }
            _ => Step::None,
        }
    }

    fn reconciled(&mut self, committed: bool, done: Done, generation: i64) -> Step {
        if let Some(op) = self.op.as_mut() {
            op.status = if committed {
                Status::Done(done)
            } else {
                // Someone else moved the work first. Never replayed, never overwritten.
                Status::Conflict(Conflict::Generation {
                    current: generation,
                })
            };
        }
        if committed && done == Done::MadePrivate {
            return Step::WithdrawalCommitted(None);
        }
        Step::None
    }

    // ---- operation access ---------------------------------------------------------------------

    pub fn phase(&self) -> Phase {
        let Some(op) = self.op.as_ref() else {
            return match (self.facts.as_ref(), self.read_error) {
                (Some(_), _) if self.unreadable() => Phase::Unreadable,
                (Some(_), _) => Phase::Ready,
                (None, Some(failure)) => Phase::Unavailable(failure),
                (None, None) => Phase::Reading,
            };
        };
        let in_flight = match op.stage {
            Stage::Saving => Phase::Saving,
            Stage::Capturing(_) => Phase::Capturing,
            Stage::Requesting(_) => Phase::RequestingPublic,
            Stage::Making(_) => Phase::MakingPrivate,
        };
        let uncertain = match op.stage {
            Stage::Making(_) => UncertainStage::Making,
            Stage::Requesting(_) => UncertainStage::Requesting,
            _ => UncertainStage::Capturing,
        };
        match op.status {
            Status::Running => in_flight,
            // Acknowledged: only the confirming read is missing. While one is on its way the
            // command is still being settled; after it failed there is nothing left to wait for.
            Status::Settling(_) if self.reading.is_some() => in_flight,
            Status::Settling(done) => Phase::Acknowledged(done),
            Status::Uncertain | Status::Reconciling => Phase::Uncertain(uncertain),
            Status::Conflict(conflict) => Phase::Conflict(conflict),
            Status::Stopped(refusal) => Phase::Failed(refusal),
            Status::Done(done) => Phase::Reconciled(done),
        }
    }

    /// A command is waiting for its answer, or for the read that confirms it.
    /// A submission is waiting for the editor owner to confirm the text and tags.
    pub fn waiting_for_editor(&self) -> bool {
        self.op
            .as_ref()
            .is_some_and(|op| op.stage == Stage::Saving && op.status == Status::Running)
    }

    pub fn working(&self) -> bool {
        let reading = self.reading.is_some();
        self.op.as_ref().is_some_and(|op| match op.status {
            Status::Running | Status::Reconciling => true,
            Status::Settling(_) => reading,
            _ => false,
        })
    }

    /// A lost answer is being checked right now.
    pub fn checking(&self) -> bool {
        self.op
            .as_ref()
            .is_some_and(|op| op.status == Status::Reconciling)
    }

    pub fn captured(&self) -> Option<&Captured> {
        self.op.as_ref().and_then(|op| op.captured.as_ref())
    }

    /// The operation's reconcile action is "Kiểm tra lại".
    pub fn can_retry(&self) -> bool {
        self.op
            .as_ref()
            .is_some_and(|op| op.status == Status::Uncertain)
    }

    /// Whether a conflict-stopped submission has a captured revision the author may send again
    /// against the current state (explicitly; never automatically).
    pub fn can_resume_captured(&self) -> bool {
        self.resume_target().is_ok()
    }

    fn resume_target(&self) -> Result<(String, i64), Blocked> {
        let op = self.op.as_ref().ok_or(Blocked::Busy)?;
        let (Status::Conflict(Conflict::Generation { .. }), Some(captured)) =
            (op.status, op.captured.as_ref())
        else {
            return Err(Blocked::Busy);
        };
        if self.applied_seq <= op.after_seq {
            return Err(Blocked::NotFresh);
        }
        let facts = self.facts.as_ref().ok_or(Blocked::NotRead)?;
        self.guard_facts(facts)?;
        Ok((captured.revision_id.clone(), facts.generation))
    }

    fn unreadable(&self) -> bool {
        self.facts.as_ref().is_some_and(|facts| {
            facts.audience == Audience::Unknown
                || facts.lifecycle == Lifecycle::Unknown
                || facts
                    .revisions
                    .iter()
                    .any(|revision| revision.moderation == Decision::Unknown)
        })
    }

    fn guard_facts(&self, facts: &AuthorPublicationState) -> Result<(), Blocked> {
        if self.unreadable() {
            return Err(Blocked::Unreadable);
        }
        if facts.lifecycle != Lifecycle::Active {
            return Err(Blocked::NotActive);
        }
        Ok(())
    }

    /// Clears a finished operation (its notice). A running one stays.
    pub fn dismiss(&mut self) {
        if self.op.as_ref().is_some_and(|op| {
            matches!(
                op.status,
                Status::Conflict(_) | Status::Stopped(_) | Status::Done(_)
            )
        }) {
            self.op = None;
        }
    }

    fn blocks_new_submission(&self) -> bool {
        self.op.as_ref().is_some_and(|op| {
            !matches!(
                op.status,
                Status::Conflict(_) | Status::Stopped(_) | Status::Done(_)
            )
        })
    }

    /// Withdrawal may replace anything except a withdrawal already waiting for its answer.
    fn blocks_withdrawal(&self) -> bool {
        self.op.as_ref().is_some_and(|op| {
            matches!(op.stage, Stage::Making(_))
                && !matches!(
                    op.status,
                    Status::Conflict(_) | Status::Stopped(_) | Status::Done(_)
                )
        })
    }

    /// Why "Gửi duyệt" cannot start now; `Ok` when a press would start the submission.
    pub fn can_submit(&self, editor: &EditorFacts) -> Result<(), Blocked> {
        let Some(facts) = self.facts.as_ref() else {
            return Err(Blocked::NotRead);
        };
        self.guard_facts(facts)?;
        if self.blocks_new_submission() {
            return Err(Blocked::Busy);
        }
        match (editor.text, editor.tags) {
            (TextFacts::Conflict, _) => Err(Blocked::TextConflict),
            (TextFacts::Failed(_), _) => Err(Blocked::SaveFailing),
            (_, TagFacts::NotLoaded) => Err(Blocked::TagsNotLoaded),
            (_, TagFacts::Blocked) => Err(Blocked::TagsBlocked),
            _ => Ok(()),
        }
    }

    pub fn can_withdraw(&self) -> Result<(), Blocked> {
        let Some(facts) = self.facts.as_ref() else {
            return Err(Blocked::NotRead);
        };
        self.guard_facts(facts)?;
        if facts.audience != Audience::Public {
            return Err(Blocked::NotPublic);
        }
        if self.blocks_withdrawal() {
            return Err(Blocked::Busy);
        }
        Ok(())
    }

    // ---- submit ------------------------------------------------------------------------------

    /// The author pressed "Gửi duyệt" / "Lưu và gửi duyệt". Pins the generation on screen.
    pub fn start_submit(&mut self, editor: &EditorFacts) -> Result<Step, Blocked> {
        self.can_submit(editor)?;
        let pinned = self.facts.as_ref().map_or(0, |facts| facts.generation);
        self.ops += 1;
        let mut op = Operation {
            id: self.ops,
            stage: Stage::Saving,
            status: Status::Running,
            attempt: 0,
            pinned_generation: pinned,
            mark: Some(Mark {
                text_edits: editor.text_edits,
                selection: editor.selection.clone(),
            }),
            saves: 0,
            captured: None,
            after_seq: self.read_seq,
        };
        // Already confirmed and unchanged: nothing to save, capture exactly what is confirmed.
        if let Some((text, tags)) = editor.confirmed() {
            let command = self.capture_command(text, tags);
            op.stage = Stage::Capturing(command.clone());
            op.attempt = 1;
            let ticket = self.ticket(&op);
            self.op = Some(op);
            return Ok(Step::Capture(ticket, command));
        }
        self.op = Some(op);
        Ok(self.editor_changed(editor))
    }

    fn capture_command(&mut self, text: i64, tags: i64) -> CaptureCommand {
        CaptureCommand {
            command_key: self.next_key(),
            expected_draft_version: text,
            expected_metadata_version: tags,
        }
    }

    fn ticket(&self, op: &Operation) -> CommandTicket {
        CommandTicket {
            epoch: self.epoch,
            op: op.id,
            attempt: op.attempt,
        }
    }

    /// The editor's facts changed. While a submission waits for the editor, this is the only
    /// place it moves on: to the capture of exactly the acknowledged versions, to a stop, or to
    /// one more save.
    pub fn editor_changed(&mut self, editor: &EditorFacts) -> Step {
        let Some(op) = self.op.as_ref() else {
            return Step::None;
        };
        if op.stage != Stage::Saving || op.status != Status::Running {
            return Step::None;
        }
        let mark = op.mark.clone();
        if mark.is_some_and(|mark| {
            mark.text_edits != editor.text_edits || mark.selection != editor.selection
        }) {
            // Text or tags changed after the press: the saved version is no longer the one the
            // author asked to send. Nothing is captured.
            return self.stop(Refusal::EditedWhileSaving);
        }
        if let Some((text, tags)) = editor.confirmed() {
            let command = self.capture_command(text, tags);
            let Some(op) = self.op.as_mut() else {
                return Step::None;
            };
            op.attempt += 1;
            op.stage = Stage::Capturing(command.clone());
            let ticket = self.ticket(self.op.as_ref().expect("just set"));
            return Step::Capture(ticket, command);
        }
        match (editor.text, editor.tags) {
            (TextFacts::Conflict | TextFacts::Failed(_), _) | (_, TagFacts::Blocked) => {
                self.stop(Refusal::SaveNotConfirmed)
            }
            (TextFacts::Saving, _) | (_, TagFacts::Saving) => Step::None,
            _ => {
                // Unsaved, nothing in flight (the first save has not started, or an older one
                // ended before this text): the editor saves again, a bounded number of times.
                let Some(op) = self.op.as_mut() else {
                    return Step::None;
                };
                if op.saves >= MAX_SAVES || matches!(editor.tags, TagFacts::NotLoaded) {
                    return self.stop(Refusal::SaveNotConfirmed);
                }
                op.saves += 1;
                Step::Save
            }
        }
    }

    fn stop(&mut self, refusal: Refusal) -> Step {
        if let Some(op) = self.op.as_mut() {
            op.status = Status::Stopped(refusal);
        }
        Step::None
    }

    pub fn capture_finished(
        &mut self,
        ticket: CommandTicket,
        result: Result<CapturedSnapshot, ApiError>,
    ) -> Step {
        let Some(Stage::Capturing(command)) = self.live(ticket, Status::Running) else {
            return Step::None;
        };
        match result {
            Ok(captured) => {
                if captured.revision_id.is_empty()
                    || captured.draft_version != command.expected_draft_version
                    || captured.metadata_version != command.expected_metadata_version
                {
                    // It does not describe the snapshot that was asked for: nothing is sent on.
                    return self.stop(Refusal::UnexpectedAnswer);
                }
                let request = RequestCommand {
                    command_key: self.next_key(),
                    revision_id: captured.revision_id.clone(),
                    expected_generation: self.op.as_ref().map_or(0, |op| op.pinned_generation),
                };
                let Some(op) = self.op.as_mut() else {
                    return Step::None;
                };
                op.captured = Some(Captured {
                    revision_id: captured.revision_id,
                    content_hash: captured.content_hash,
                });
                op.attempt += 1;
                op.stage = Stage::Requesting(request.clone());
                let ticket = self.ticket(self.op.as_ref().expect("just set"));
                Step::RequestPublic(ticket, request)
            }
            Err(error) => self.answer_failed(error),
        }
    }

    pub fn request_finished(
        &mut self,
        ticket: CommandTicket,
        result: Result<RequestPublicReceipt, ApiError>,
    ) -> Step {
        let Some(Stage::Requesting(command)) = self.live(ticket, Status::Running) else {
            return Step::None;
        };
        match result {
            Ok(receipt) if receipt.revision_id == command.revision_id => {
                // A replay may omit the ticket: that is still an answer. The receipt is history;
                // what the page shows follows a read that starts now.
                self.acknowledged(Done::Requested);
                self.read_step()
            }
            Ok(_) => self.stop(Refusal::UnexpectedAnswer),
            Err(error) => self.answer_failed(error),
        }
    }

    /// Looks up the stage of the live attempt: same epoch, operation, attempt and status.
    fn live(&self, ticket: CommandTicket, status: Status) -> Option<Stage> {
        let op = self.op.as_ref()?;
        (ticket.epoch == self.epoch
            && ticket.op == op.id
            && ticket.attempt == op.attempt
            && op.status == status)
            .then(|| op.stage.clone())
    }

    fn acknowledged(&mut self, done: Done) {
        let seq = self.read_seq;
        if let Some(op) = self.op.as_mut() {
            op.after_seq = seq;
            op.status = Status::Settling(done);
        }
    }

    /// A refusal, a conflict or a lost answer of the live command.
    fn answer_failed(&mut self, error: ApiError) -> Step {
        let seq = self.read_seq;
        let answer = classify(&error);
        let Some(op) = self.op.as_mut() else {
            return Step::None;
        };
        match answer {
            Answer::Lost => {
                op.status = Status::Uncertain;
                Step::None
            }
            Answer::Conflict(conflict) => {
                op.status = Status::Conflict(conflict);
                op.after_seq = seq;
                self.read_step()
            }
            Answer::Refused(refusal) => {
                op.status = Status::Stopped(refusal);
                op.after_seq = seq;
                // A read explains a refusal about the work; it cannot help a lost session.
                let reads = !matches!(
                    refusal,
                    Refusal::SessionEnded
                        | Refusal::Forbidden
                        | Refusal::NotFound
                        | Refusal::Invalid
                );
                if reads { self.read_step() } else { Step::None }
            }
        }
    }

    /// "Kiểm tra lại": settles a lost answer. A lost capture replays the same command (same key,
    /// same versions: the server returns the first revision). A lost `request-public` or
    /// `set-private` reads the state first.
    pub fn retry(&mut self) -> Step {
        let Some(op) = self.op.as_mut() else {
            return Step::None;
        };
        if op.status != Status::Uncertain {
            return Step::None;
        }
        match op.stage.clone() {
            Stage::Capturing(command) => {
                op.status = Status::Running;
                op.attempt += 1;
                let ticket = self.ticket(self.op.as_ref().expect("present"));
                Step::Capture(ticket, command)
            }
            Stage::Requesting(_) | Stage::Making(_) => {
                op.status = Status::Reconciling;
                op.after_seq = self.read_seq;
                match self.begin_read() {
                    Some(ticket) => Step::Read(ticket),
                    // A read is already in flight; it was asked again and settles this.
                    None => Step::None,
                }
            }
            Stage::Saving => Step::None,
        }
    }

    fn replay_request(&mut self, command: RequestCommand) -> Step {
        let Some(op) = self.op.as_mut() else {
            return Step::None;
        };
        op.status = Status::Running;
        op.attempt += 1;
        let ticket = self.ticket(self.op.as_ref().expect("present"));
        Step::RequestPublic(ticket, command)
    }

    fn replay_private(&mut self, command: PrivateCommand) -> Step {
        let Some(op) = self.op.as_mut() else {
            return Step::None;
        };
        op.status = Status::Running;
        op.attempt += 1;
        let ticket = self.ticket(self.op.as_ref().expect("present"));
        Step::SetPrivate(ticket, command)
    }

    /// After a generation conflict, the author may send the revision already captured against the
    /// state now shown: a new `request-public` command, never a new snapshot.
    pub fn resume_captured(&mut self) -> Result<Step, Blocked> {
        let (revision_id, generation) = self.resume_target()?;
        let command = RequestCommand {
            command_key: self.next_key(),
            revision_id,
            expected_generation: generation,
        };
        let Some(op) = self.op.as_mut() else {
            return Err(Blocked::Busy);
        };
        op.pinned_generation = generation;
        op.attempt += 1;
        op.status = Status::Running;
        op.stage = Stage::Requesting(command.clone());
        let ticket = self.ticket(self.op.as_ref().expect("present"));
        Ok(Step::RequestPublic(ticket, command))
    }

    // ---- withdrawal --------------------------------------------------------------------------

    /// The author pressed "Chuyển riêng tư". It never waits for the editor and never captures
    /// anything. A submission that has not been answered is superseded: its next stage never
    /// starts and a late answer changes nothing.
    pub fn start_private(&mut self) -> Result<Step, Blocked> {
        self.can_withdraw()?;
        let pinned = self.facts.as_ref().map_or(0, |facts| facts.generation);
        let command = PrivateCommand {
            command_key: self.next_key(),
            expected_generation: pinned,
        };
        self.ops += 1;
        let op = Operation {
            id: self.ops,
            stage: Stage::Making(command.clone()),
            status: Status::Running,
            attempt: 1,
            pinned_generation: pinned,
            mark: None,
            saves: 0,
            captured: None,
            after_seq: self.read_seq,
        };
        let ticket = self.ticket(&op);
        self.op = Some(op);
        Ok(Step::SetPrivate(ticket, command))
    }

    pub fn private_finished(
        &mut self,
        ticket: CommandTicket,
        result: Result<SetPrivateReceipt, ApiError>,
    ) -> Step {
        let Some(Stage::Making(_)) = self.live(ticket, Status::Running) else {
            return Step::None;
        };
        match result {
            Ok(_) => {
                self.acknowledged(Done::MadePrivate);
                // Shared public copies of this work are stale from now on, whatever the read says.
                Step::WithdrawalCommitted(self.begin_read())
            }
            Err(error) => self.answer_failed(error),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    /// The command may or may not have committed.
    Lost,
    Conflict(Conflict),
    Refused(Refusal),
}

/// What an error means for a publication command. Anything not understood is a lost answer: it
/// is reconciled by reading, never taken for success and never for a definite rejection.
fn classify(error: &ApiError) -> Answer {
    match error {
        ApiError::Unauthenticated => Answer::Refused(Refusal::SessionEnded),
        ApiError::Forbidden => Answer::Refused(Refusal::Forbidden),
        ApiError::NotFound => Answer::Refused(Refusal::NotFound),
        ApiError::SnapshotConflict {
            field: SnapshotField::Draft,
            current_version,
        } => Answer::Conflict(Conflict::Draft {
            current: *current_version,
        }),
        ApiError::SnapshotConflict {
            field: SnapshotField::Metadata,
            current_version,
        } => Answer::Conflict(Conflict::Metadata {
            current: *current_version,
        }),
        ApiError::GenerationConflict { current_generation } => {
            Answer::Conflict(Conflict::Generation {
                current: *current_generation,
            })
        }
        ApiError::RevisionRejected => Answer::Refused(Refusal::RevisionRejected),
        ApiError::UnknownRevision => Answer::Refused(Refusal::UnknownRevision),
        ApiError::WorkUnavailable => Answer::Refused(Refusal::WorkUnavailable),
        ApiError::KeyReused => Answer::Refused(Refusal::KeyReused),
        ApiError::Invalid { .. } => Answer::Refused(Refusal::Invalid),
        ApiError::Network | ApiError::Unavailable | ApiError::Unexpected { .. } => Answer::Lost,
        // Discussion, endorsement and tag answers never come from these routes.
        ApiError::VersionConflict { .. }
        | ApiError::CommentsClosed
        | ApiError::TargetUnavailable
        | ApiError::CommentRemoved
        | ApiError::SelfEndorsement
        | ApiError::EndorsementNotPermitted
        | ApiError::EndorsementConflict { .. }
        | ApiError::RevisionChanged { .. }
        | ApiError::TagConflict { .. }
        | ApiError::TagRefused(_) => Answer::Lost,
    }
}

// ---------------------------------------------------------------------------------------------
// What the page says about the work (from server facts only)
// ---------------------------------------------------------------------------------------------

/// One revision as the page names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionLine {
    pub revision_id: String,
    pub title: String,
    pub decision: Decision,
    pub reason: Option<String>,
    pub created_at: String,
    pub can_appeal: bool,
}

impl RevisionLine {
    fn of(revision: &AuthorRevision) -> Self {
        Self {
            revision_id: revision.revision_id.clone(),
            title: revision.title.clone(),
            decision: revision.moderation,
            reason: revision.decision_reason.clone(),
            created_at: revision.created_at.clone(),
            can_appeal: revision.can_appeal,
        }
    }
}

/// The four things the page keeps apart: what readers see now, what was asked to be public, how
/// that revision was decided, and snapshots nobody asked to publish. "Đang công khai" comes only
/// from the public pointer: neither the audience, an HTTP `2xx` nor an approval says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub audience: Audience,
    /// The revision readers read now.
    pub public: Option<RevisionLine>,
    /// The revision the author asked to publish, when it is not the one readers see.
    pub requested: Option<RevisionLine>,
    /// Captured revisions that were never submitted (for example, the page closed in between).
    pub unsent: usize,
}

impl Summary {
    pub fn of(facts: &AuthorPublicationState) -> Self {
        let line = |id: &str| {
            facts
                .revisions
                .iter()
                .find(|revision| revision.revision_id == id)
                .map(RevisionLine::of)
        };
        let pointer = facts.pointer.as_ref().map(|p| p.revision_id.as_str());
        let public = pointer.map(|id| {
            line(id).unwrap_or_else(|| RevisionLine {
                revision_id: id.to_string(),
                title: String::new(),
                decision: Decision::Approved,
                reason: None,
                created_at: String::new(),
                can_appeal: false,
            })
        });
        let intent = facts.intent.as_ref().map(|i| i.revision_id.as_str());
        let requested = intent.filter(|id| Some(*id) != pointer).and_then(line);
        let unsent = facts
            .revisions
            .iter()
            .filter(|revision| {
                revision.moderation == Decision::Unsubmitted
                    && Some(revision.revision_id.as_str()) != intent
            })
            .count();
        Self {
            audience: facts.audience,
            public,
            requested,
            unsent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::publication::{OpenTicket, PublicPointer, PublishIntent};
    use crate::tag_state::{Selection, TagSession};

    const WORK: &str = "33333333-3333-4333-8333-333333333333";
    const R1: &str = "0192f3a1-0000-7000-8000-0000000000b1";
    const R2: &str = "0192f3a1-0000-7000-8000-0000000000b2";

    fn controller() -> Controller {
        Controller::new(
            Scope {
                work_id: WORK.into(),
                session: Some("session-a".into()),
            },
            [7; 16],
        )
    }

    fn revision(id: &str, moderation: Decision) -> AuthorRevision {
        AuthorRevision {
            can_appeal: false,
            decision_reason: None,
            revision_id: id.into(),
            title: format!("Bản {id}"),
            content_hash: "h".repeat(64),
            moderation,
            decision_version: Some(1),
            metadata_snapshot_id: None,
            created_at: "2026-11-13T08:00:00Z".into(),
        }
    }

    fn state(generation: i64, audience: Audience) -> AuthorPublicationState {
        AuthorPublicationState {
            work_id: WORK.into(),
            audience,
            lifecycle: Lifecycle::Active,
            generation,
            intent: None,
            pointer: None,
            revisions: vec![],
            open_ticket: None,
        }
    }

    /// A controller that has read `state` (so it can act on its generation).
    fn read(mut c: Controller, state: AuthorPublicationState) -> Controller {
        let ticket = c.begin_read().expect("no read in flight");
        assert_eq!(c.read_finished(ticket, Ok(state)), Step::None);
        c
    }

    fn clean() -> EditorFacts {
        EditorFacts {
            text: TextFacts::Confirmed { version: 7 },
            tags: TagFacts::Confirmed { version: 5 },
            text_edits: 4,
            selection: Some(Selection::new(None, [])),
        }
    }

    fn dirty() -> EditorFacts {
        EditorFacts {
            text: TextFacts::Unsaved,
            tags: TagFacts::Confirmed { version: 5 },
            text_edits: 9,
            selection: Some(Selection::new(None, [])),
        }
    }

    fn captured(version: i64, tags: i64) -> CapturedSnapshot {
        CapturedSnapshot {
            outcome: "saved".into(),
            revision_id: R2.into(),
            content_hash: "h".repeat(64),
            metadata_snapshot_id: Some("m".into()),
            draft_version: version,
            metadata_version: tags,
            replayed: false,
        }
    }

    fn receipt(replayed: bool, ticket: bool) -> RequestPublicReceipt {
        RequestPublicReceipt {
            outcome: "submitted".into(),
            revision_id: R2.into(),
            generation: 4,
            ticket_id: ticket.then(|| "ticket".into()),
            replayed,
        }
    }

    fn capture_of(step: Step) -> (CommandTicket, CaptureCommand) {
        match step {
            Step::Capture(ticket, command) => (ticket, command),
            other => panic!("expected a capture, got {other:?}"),
        }
    }

    fn request_of(step: Step) -> (CommandTicket, RequestCommand) {
        match step {
            Step::RequestPublic(ticket, command) => (ticket, command),
            other => panic!("expected a request-public, got {other:?}"),
        }
    }

    fn private_of(step: Step) -> (CommandTicket, PrivateCommand) {
        match step {
            Step::SetPrivate(ticket, command) => (ticket, command),
            other => panic!("expected a set-private, got {other:?}"),
        }
    }

    fn read_of(step: Step) -> ReadTicket {
        match step {
            Step::Read(ticket) => ticket,
            other => panic!("expected a read, got {other:?}"),
        }
    }

    // ---- the happy path -------------------------------------------------------------------

    #[test]
    fn a_confirmed_editor_is_captured_at_exactly_its_versions_and_the_revision_is_the_one_sent() {
        let mut c = read(controller(), state(3, Audience::Private));
        let (ticket, capture) = capture_of(c.start_submit(&clean()).unwrap());
        assert_eq!(
            (
                capture.expected_draft_version,
                capture.expected_metadata_version
            ),
            (7, 5)
        );
        assert_eq!(c.phase(), Phase::Capturing);
        let (ticket, request) = request_of(c.capture_finished(ticket, Ok(captured(7, 5))));
        // The revision is the captured one and the generation is the one the author looked at.
        assert_eq!(
            (request.revision_id.as_str(), request.expected_generation),
            (R2, 3)
        );
        assert_ne!(request.command_key, capture.command_key);
        assert_eq!(c.phase(), Phase::RequestingPublic);
        // The receipt is history: the page still waits for a read that starts afterwards.
        let after = read_of(c.request_finished(ticket, Ok(receipt(false, true))));
        assert_eq!(c.phase(), Phase::RequestingPublic);
        assert_eq!(
            c.facts().map(|f| f.generation),
            Some(3),
            "a receipt is not the state"
        );
        let mut next = state(4, Audience::Public);
        next.intent = Some(PublishIntent {
            revision_id: R2.into(),
            generation: 4,
        });
        next.revisions = vec![revision(R2, Decision::Pending)];
        assert_eq!(c.read_finished(after, Ok(next)), Step::None);
        assert_eq!(c.phase(), Phase::Reconciled(Done::Requested));
        assert_eq!(c.facts().map(|f| f.generation), Some(4));
        c.dismiss();
        assert_eq!(c.phase(), Phase::Ready);
    }

    #[test]
    fn a_replayed_request_without_a_ticket_is_an_answer_not_an_error() {
        let mut c = read(controller(), state(3, Audience::Private));
        let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
        let (t, _) = request_of(c.capture_finished(t, Ok(captured(7, 5))));
        let read = read_of(c.request_finished(t, Ok(receipt(true, false))));
        let mut next = state(4, Audience::Public);
        next.intent = Some(PublishIntent {
            revision_id: R2.into(),
            generation: 4,
        });
        c.read_finished(read, Ok(next));
        assert_eq!(c.phase(), Phase::Reconciled(Done::Requested));
    }

    #[test]
    fn every_command_has_its_own_key_and_a_press_never_reuses_an_earlier_one() {
        let mut c = read(controller(), state(1, Audience::Private));
        let (t, first) = capture_of(c.start_submit(&clean()).unwrap());
        let (_, request) = request_of(c.capture_finished(t, Ok(captured(7, 5))));
        c.change_scope(c.scope().clone());
        let mut c = read(c, state(1, Audience::Private));
        let (_, second) = capture_of(c.start_submit(&clean()).unwrap());
        let keys = [
            &first.command_key,
            &request.command_key,
            &second.command_key,
        ];
        assert!(keys[0] != keys[1] && keys[1] != keys[2] && keys[0] != keys[2]);
        for key in keys {
            assert_eq!(key.len(), 36, "{key}");
            assert_eq!(key.as_bytes()[14], b'4', "{key} is a version-4 UUID");
        }
    }

    // ---- save first ------------------------------------------------------------------------

    #[test]
    fn unsaved_text_asks_the_editor_to_save_and_captures_only_what_it_acknowledges() {
        let mut c = read(controller(), state(3, Audience::Private));
        assert_eq!(c.start_submit(&dirty()).unwrap(), Step::Save);
        assert_eq!(c.phase(), Phase::Saving);
        // The editor is saving: wait. No snapshot yet.
        let saving = EditorFacts {
            text: TextFacts::Saving,
            ..dirty()
        };
        assert_eq!(c.editor_changed(&saving), Step::None);
        // Acknowledged at version 8: that exact version is captured, never the earlier 7.
        let saved = EditorFacts {
            text: TextFacts::Confirmed { version: 8 },
            ..dirty()
        };
        let (_, capture) = capture_of(c.editor_changed(&saved));
        assert_eq!(capture.expected_draft_version, 8);
        // The editor changing again later does not start another capture.
        assert_eq!(c.editor_changed(&saved), Step::None);
    }

    #[test]
    fn typing_while_the_save_is_in_flight_stops_the_submission_without_a_snapshot() {
        let mut c = read(controller(), state(3, Audience::Private));
        assert_eq!(c.start_submit(&dirty()).unwrap(), Step::Save);
        let typed_more = EditorFacts {
            text: TextFacts::Confirmed { version: 8 },
            text_edits: 10,
            ..dirty()
        };
        // The saved text is clean again, but it is not what the author pressed on.
        assert_eq!(c.editor_changed(&typed_more), Step::None);
        assert_eq!(c.phase(), Phase::Failed(Refusal::EditedWhileSaving));
        assert!(c.captured().is_none());
    }

    #[test]
    fn a_tag_chosen_while_the_save_is_in_flight_stops_it_as_well() {
        let mut c = read(controller(), state(3, Audience::Private));
        assert_eq!(c.start_submit(&dirty()).unwrap(), Step::Save);
        let changed = EditorFacts {
            text: TextFacts::Confirmed { version: 8 },
            selection: Some(Selection::new(Some("form".into()), [])),
            ..dirty()
        };
        assert_eq!(c.editor_changed(&changed), Step::None);
        assert_eq!(c.phase(), Phase::Failed(Refusal::EditedWhileSaving));
    }

    #[test]
    fn a_save_that_fails_or_conflicts_stops_without_capturing() {
        for text in [TextFacts::Failed(Failure::Network), TextFacts::Conflict] {
            let mut c = read(controller(), state(3, Audience::Private));
            assert_eq!(c.start_submit(&dirty()).unwrap(), Step::Save);
            let ended = EditorFacts { text, ..dirty() };
            assert_eq!(c.editor_changed(&ended), Step::None);
            assert_eq!(c.phase(), Phase::Failed(Refusal::SaveNotConfirmed));
        }
        let mut c = read(controller(), state(3, Audience::Private));
        assert_eq!(c.start_submit(&dirty()).unwrap(), Step::Save);
        let blocked = EditorFacts {
            tags: TagFacts::Blocked,
            ..dirty()
        };
        c.editor_changed(&blocked);
        assert_eq!(c.phase(), Phase::Failed(Refusal::SaveNotConfirmed));
    }

    #[test]
    fn an_older_save_ending_before_this_text_asks_for_one_more_save_but_not_forever() {
        let mut c = read(controller(), state(3, Audience::Private));
        assert_eq!(c.start_submit(&dirty()).unwrap(), Step::Save);
        // An earlier save finished, leaving the pressed-on text unsaved: save again, up to a bound.
        let mut saves = 1;
        while c.editor_changed(&dirty()) == Step::Save {
            saves += 1;
            assert!(saves <= MAX_SAVES as usize + 1, "unbounded saves");
        }
        assert_eq!(saves, MAX_SAVES as usize);
        assert_eq!(c.phase(), Phase::Failed(Refusal::SaveNotConfirmed));
    }

    #[test]
    fn what_blocks_a_submission_is_named_before_anything_starts() {
        let c = controller();
        assert_eq!(c.can_submit(&clean()), Err(Blocked::NotRead));
        let c = read(controller(), state(1, Audience::Private));
        let with = |text, tags| EditorFacts {
            text,
            tags,
            ..clean()
        };
        assert_eq!(c.can_submit(&clean()), Ok(()));
        assert_eq!(
            c.can_submit(&with(
                TextFacts::Confirmed { version: 1 },
                TagFacts::NotLoaded
            )),
            Err(Blocked::TagsNotLoaded)
        );
        assert_eq!(
            c.can_submit(&with(
                TextFacts::Conflict,
                TagFacts::Confirmed { version: 1 }
            )),
            Err(Blocked::TextConflict)
        );
        assert_eq!(
            c.can_submit(&with(
                TextFacts::Failed(Failure::Network),
                TagFacts::Confirmed { version: 1 }
            )),
            Err(Blocked::SaveFailing)
        );
        assert_eq!(
            c.can_submit(&with(TextFacts::Unsaved, TagFacts::Blocked)),
            Err(Blocked::TagsBlocked)
        );
        // Text still unsaved is fine: the editor saves it first.
        assert_eq!(
            c.can_submit(&with(TextFacts::Unsaved, TagFacts::Unsaved)),
            Ok(())
        );
    }

    #[test]
    fn a_double_press_cannot_start_a_second_operation() {
        let mut c = read(controller(), state(3, Audience::Private));
        capture_of(c.start_submit(&clean()).unwrap());
        assert_eq!(c.start_submit(&clean()), Err(Blocked::Busy));
        assert_eq!(c.start_private(), Err(Blocked::NotPublic));
        let mut c = read(controller(), state(3, Audience::Private));
        assert_eq!(c.start_submit(&dirty()).unwrap(), Step::Save);
        assert_eq!(c.start_submit(&dirty()), Err(Blocked::Busy));
    }

    // ---- uncertainty ------------------------------------------------------------------------

    #[test]
    fn a_lost_capture_answer_is_replayed_with_the_same_key_and_versions() {
        let mut c = read(controller(), state(3, Audience::Private));
        let (first, command) = capture_of(c.start_submit(&clean()).unwrap());
        assert_eq!(
            c.capture_finished(first, Err(ApiError::Network)),
            Step::None
        );
        assert_eq!(c.phase(), Phase::Uncertain(UncertainStage::Capturing));
        // The author cannot start something new on top of it.
        assert_eq!(c.start_submit(&clean()), Err(Blocked::Busy));
        let (second, replay) = capture_of(c.retry());
        assert_eq!(
            replay, command,
            "same key and payload: never a new snapshot"
        );
        assert_ne!(first, second);
        // The old attempt cannot answer for the new one.
        assert_eq!(c.capture_finished(first, Ok(captured(7, 5))), Step::None);
        assert_eq!(c.phase(), Phase::Capturing);
        let mut again = captured(7, 5);
        again.replayed = true;
        request_of(c.capture_finished(second, Ok(again)));
    }

    #[test]
    fn every_unreadable_answer_is_uncertain_never_success_or_rejection() {
        for error in [
            ApiError::Network,
            ApiError::Unavailable,
            ApiError::Unexpected { status: 502 },
            ApiError::Unexpected { status: 200 },
            ApiError::CommentsClosed,
            ApiError::VersionConflict { current_version: 3 },
        ] {
            let mut c = read(controller(), state(3, Audience::Private));
            let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
            c.capture_finished(t, Err(error.clone()));
            assert_eq!(
                c.phase(),
                Phase::Uncertain(UncertainStage::Capturing),
                "{error:?}"
            );
            assert!(c.can_retry());
        }
    }

    #[test]
    fn a_success_that_does_not_describe_the_snapshot_asked_for_sends_nothing_on() {
        let mut c = read(controller(), state(3, Audience::Private));
        let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
        assert_eq!(c.capture_finished(t, Ok(captured(6, 5))), Step::None);
        assert_eq!(c.phase(), Phase::Failed(Refusal::UnexpectedAnswer));
        assert!(c.captured().is_none());
    }

    #[test]
    fn a_lost_request_is_read_first_and_replayed_only_if_the_pinned_generation_still_stands() {
        let mut c = read(controller(), state(3, Audience::Private));
        let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
        let (t, request) = request_of(c.capture_finished(t, Ok(captured(7, 5))));
        c.request_finished(t, Err(ApiError::Network));
        assert_eq!(c.phase(), Phase::Uncertain(UncertainStage::Requesting));
        // The check reads the state; the server still shows generation 3: nothing committed.
        let ticket = read_of(c.retry());
        assert!(c.checking());
        let (replay_ticket, replay) =
            request_of(c.read_finished(ticket, Ok(state(3, Audience::Private))));
        assert_eq!(replay, request, "original key and original payload");
        // The captured revision is untouched and no second capture was made.
        assert_eq!(c.captured().map(|r| r.revision_id.as_str()), Some(R2));
        let after = read_of(c.request_finished(replay_ticket, Ok(receipt(true, false))));
        c.read_finished(after, Ok(state(4, Audience::Public)));
        assert_eq!(c.phase(), Phase::Reconciled(Done::Requested));
    }

    #[test]
    fn a_lost_request_that_did_commit_is_settled_from_the_state_without_a_replay() {
        let mut c = read(controller(), state(3, Audience::Private));
        let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
        let (t, _) = request_of(c.capture_finished(t, Ok(captured(7, 5))));
        c.request_finished(t, Err(ApiError::Unavailable));
        let ticket = read_of(c.retry());
        let mut committed = state(4, Audience::Public);
        committed.intent = Some(PublishIntent {
            revision_id: R2.into(),
            generation: 4,
        });
        assert_eq!(c.read_finished(ticket, Ok(committed)), Step::None);
        assert_eq!(c.phase(), Phase::Reconciled(Done::Requested));
    }

    #[test]
    fn a_lost_request_when_another_tab_moved_the_work_is_a_conflict_never_a_replay() {
        let mut c = read(controller(), state(3, Audience::Private));
        let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
        let (t, _) = request_of(c.capture_finished(t, Ok(captured(7, 5))));
        c.request_finished(t, Err(ApiError::Network));
        let ticket = read_of(c.retry());
        // Another tab moved the generation and the intent is not ours.
        let mut elsewhere = state(5, Audience::Public);
        elsewhere.intent = Some(PublishIntent {
            revision_id: R1.into(),
            generation: 5,
        });
        assert_eq!(c.read_finished(ticket, Ok(elsewhere)), Step::None);
        assert_eq!(
            c.phase(),
            Phase::Conflict(Conflict::Generation { current: 5 })
        );
    }

    #[test]
    fn a_check_whose_read_fails_stays_uncertain_and_can_be_repeated() {
        let mut c = read(controller(), state(3, Audience::Private));
        let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
        let (t, _) = request_of(c.capture_finished(t, Ok(captured(7, 5))));
        c.request_finished(t, Err(ApiError::Network));
        let ticket = read_of(c.retry());
        c.read_finished(ticket, Err(ApiError::Network));
        assert_eq!(c.phase(), Phase::Uncertain(UncertainStage::Requesting));
        assert!(c.can_retry());
        assert_eq!(
            c.facts().map(|f| f.generation),
            Some(3),
            "the last facts stay"
        );
    }

    // ---- conflicts and refusals -------------------------------------------------------------

    #[test]
    fn each_conflict_names_the_version_that_moved_and_reads_the_state_again() {
        for (error, expected) in [
            (
                ApiError::SnapshotConflict {
                    field: SnapshotField::Draft,
                    current_version: 8,
                },
                Conflict::Draft { current: 8 },
            ),
            (
                ApiError::SnapshotConflict {
                    field: SnapshotField::Metadata,
                    current_version: 6,
                },
                Conflict::Metadata { current: 6 },
            ),
        ] {
            let mut c = read(controller(), state(3, Audience::Private));
            let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
            let step = c.capture_finished(t, Err(error));
            read_of(step);
            assert_eq!(c.phase(), Phase::Conflict(expected));
            assert!(c.captured().is_none(), "nothing was captured");
        }
        let mut c = read(controller(), state(3, Audience::Private));
        let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
        let (t, _) = request_of(c.capture_finished(t, Ok(captured(7, 5))));
        let step = c.request_finished(
            t,
            Err(ApiError::GenerationConflict {
                current_generation: 5,
            }),
        );
        read_of(step);
        assert_eq!(
            c.phase(),
            Phase::Conflict(Conflict::Generation { current: 5 })
        );
    }

    #[test]
    fn after_a_generation_conflict_the_captured_revision_can_be_sent_again_only_on_a_fresh_read() {
        let mut c = read(controller(), state(3, Audience::Private));
        let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
        let (t, first) = request_of(c.capture_finished(t, Ok(captured(7, 5))));
        let ticket = read_of(c.request_finished(
            t,
            Err(ApiError::GenerationConflict {
                current_generation: 5,
            }),
        ));
        // The state shown is still the old one: sending now would pin a generation nobody saw.
        assert_eq!(c.resume_captured(), Err(Blocked::NotFresh));
        c.read_finished(ticket, Ok(state(5, Audience::Private)));
        assert!(c.can_resume_captured());
        let (_, again) = request_of(c.resume_captured().unwrap());
        assert_eq!(
            again.revision_id, first.revision_id,
            "the same captured revision"
        );
        assert_eq!(again.expected_generation, 5, "the generation now shown");
        assert_ne!(
            again.command_key, first.command_key,
            "a new logical command"
        );
    }

    #[test]
    fn refusals_stop_the_operation_and_never_pretend_to_be_lost_answers() {
        for (error, expected, reads) in [
            (ApiError::Unauthenticated, Refusal::SessionEnded, false),
            (ApiError::Forbidden, Refusal::Forbidden, false),
            (ApiError::NotFound, Refusal::NotFound, false),
            (ApiError::Invalid { field: None }, Refusal::Invalid, false),
            (ApiError::RevisionRejected, Refusal::RevisionRejected, true),
            (ApiError::UnknownRevision, Refusal::UnknownRevision, true),
            (ApiError::WorkUnavailable, Refusal::WorkUnavailable, true),
            (ApiError::KeyReused, Refusal::KeyReused, true),
        ] {
            let mut c = read(controller(), state(3, Audience::Private));
            let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
            let (t, _) = request_of(c.capture_finished(t, Ok(captured(7, 5))));
            let step = c.request_finished(t, Err(error.clone()));
            assert_eq!(matches!(step, Step::Read(_)), reads, "{error:?}");
            assert_eq!(c.phase(), Phase::Failed(expected), "{error:?}");
            assert!(!c.can_retry());
        }
    }

    // ---- withdrawal ---------------------------------------------------------------------------

    fn public_state(generation: i64) -> AuthorPublicationState {
        let mut s = state(generation, Audience::Public);
        s.pointer = Some(PublicPointer {
            revision_id: R1.into(),
            intent_generation: 2,
        });
        s.intent = Some(PublishIntent {
            revision_id: R1.into(),
            generation: 2,
        });
        s.revisions = vec![revision(R1, Decision::Approved)];
        s
    }

    #[test]
    fn making_private_needs_neither_a_save_nor_a_snapshot_and_pins_the_generation_on_screen() {
        let mut c = read(controller(), public_state(4));
        assert_eq!(c.can_withdraw(), Ok(()));
        let (t, command) = private_of(c.start_private().unwrap());
        assert_eq!(command.expected_generation, 4);
        assert_eq!(c.phase(), Phase::MakingPrivate);
        let Step::WithdrawalCommitted(Some(after)) = c.private_finished(
            t,
            Ok(SetPrivateReceipt {
                outcome: "made_private".into(),
                generation: 5,
                replayed: false,
            }),
        ) else {
            panic!("a withdrawal purges the public copies and reads");
        };
        // Still not "private" until a read that started afterwards says so.
        assert_eq!(c.phase(), Phase::MakingPrivate);
        assert_eq!(c.facts().map(|f| f.audience), Some(Audience::Public));
        c.read_finished(after, Ok(state(5, Audience::Private)));
        assert_eq!(c.phase(), Phase::Reconciled(Done::MadePrivate));
        assert_eq!(c.facts().map(|f| f.audience), Some(Audience::Private));
    }

    #[test]
    fn a_private_press_supersedes_a_submission_and_its_late_answers_change_nothing() {
        let mut c = read(controller(), public_state(4));
        let (capture_ticket, _) = capture_of(c.start_submit(&clean()).unwrap());
        // The capture is on the wire; the author withdraws.
        let (private_ticket, _) = private_of(c.start_private().unwrap());
        // The superseded capture answers late: nothing starts, nothing changes.
        assert_eq!(
            c.capture_finished(capture_ticket, Ok(captured(7, 5))),
            Step::None
        );
        assert_eq!(c.phase(), Phase::MakingPrivate);
        assert!(c.captured().is_none());
        // And its failure cannot overwrite the withdrawal either.
        assert_eq!(
            c.capture_finished(capture_ticket, Err(ApiError::Network)),
            Step::None
        );
        assert_eq!(c.phase(), Phase::MakingPrivate);
        let step = c.private_finished(
            private_ticket,
            Err(ApiError::GenerationConflict {
                current_generation: 5,
            }),
        );
        read_of(step);
        assert_eq!(
            c.phase(),
            Phase::Conflict(Conflict::Generation { current: 5 })
        );
    }

    #[test]
    fn a_request_that_committed_first_makes_the_withdrawal_conflict_and_it_is_confirmed_again() {
        let mut c = read(controller(), public_state(4));
        let (t, _) = private_of(c.start_private().unwrap());
        let ticket = read_of(c.private_finished(
            t,
            Err(ApiError::GenerationConflict {
                current_generation: 5,
            }),
        ));
        c.read_finished(ticket, Ok(public_state(5)));
        // The author sees the new state and confirms the withdrawal again, on the new generation.
        c.dismiss();
        let (_, command) = private_of(c.start_private().unwrap());
        assert_eq!(command.expected_generation, 5);
    }

    #[test]
    fn a_lost_withdrawal_answer_is_read_then_replayed_with_its_own_key_or_settled() {
        let mut c = read(controller(), public_state(4));
        let (t, command) = private_of(c.start_private().unwrap());
        c.private_finished(t, Err(ApiError::Network));
        assert_eq!(c.phase(), Phase::Uncertain(UncertainStage::Making));
        // A pending withdrawal cannot be replaced by another one.
        assert_eq!(c.start_private(), Err(Blocked::Busy));
        let ticket = read_of(c.retry());
        let (t, replay) = private_of(c.read_finished(ticket, Ok(public_state(4))));
        assert_eq!(replay, command);
        c.private_finished(t, Err(ApiError::Network));
        // This time the work shows private at the next generation: settled without a replay.
        let ticket = read_of(c.retry());
        let step = c.read_finished(ticket, Ok(state(5, Audience::Private)));
        assert_eq!(step, Step::WithdrawalCommitted(None));
        assert_eq!(c.phase(), Phase::Reconciled(Done::MadePrivate));
    }

    #[test]
    fn a_lost_withdrawal_answer_when_the_work_is_still_public_elsewhere_is_a_conflict() {
        let mut c = read(controller(), public_state(4));
        let (t, _) = private_of(c.start_private().unwrap());
        c.private_finished(t, Err(ApiError::Network));
        let ticket = read_of(c.retry());
        // Another tab republished: generation moved and the work is public again.
        assert_eq!(c.read_finished(ticket, Ok(public_state(6))), Step::None);
        assert_eq!(
            c.phase(),
            Phase::Conflict(Conflict::Generation { current: 6 })
        );
    }

    #[test]
    fn a_private_work_offers_no_withdrawal_and_a_missing_state_offers_nothing() {
        let c = read(controller(), state(1, Audience::Private));
        assert_eq!(c.can_withdraw(), Err(Blocked::NotPublic));
        assert_eq!(controller().can_withdraw(), Err(Blocked::NotRead));
    }

    // ---- acknowledged, but the read that confirms it failed ------------------------------------

    /// The server acknowledged the command; only the read that states the result is missing. The
    /// page must not keep saying it is still sending, and must offer the read again.
    #[test]
    fn an_acknowledged_command_whose_confirming_read_failed_says_so_and_can_be_read_again() {
        for request in [true, false] {
            let mut c = read(controller(), public_state(4));
            let after = if request {
                let (t, _) = capture_of(c.start_submit(&clean()).unwrap());
                let (t, _) = request_of(c.capture_finished(t, Ok(captured(7, 5))));
                read_of(c.request_finished(t, Ok(receipt(false, true))))
            } else {
                let (t, _) = private_of(c.start_private().unwrap());
                let Step::WithdrawalCommitted(Some(after)) = c.private_finished(
                    t,
                    Ok(SetPrivateReceipt {
                        outcome: "made_private".into(),
                        generation: 5,
                        replayed: false,
                    }),
                ) else {
                    panic!("a withdrawal reads");
                };
                after
            };
            let done = if request {
                Done::Requested
            } else {
                Done::MadePrivate
            };
            // While the confirming read is on its way the command is still being settled.
            assert!(c.working());
            c.read_finished(after, Err(ApiError::Network));
            // The command is acknowledged; nothing is pending any more, and it is not "done" either.
            assert_eq!(c.phase(), Phase::Acknowledged(done));
            assert!(!c.working(), "nothing is waiting for an answer");
            assert_eq!(c.can_submit(&clean()), Err(Blocked::Busy));
            assert_eq!(
                c.facts().map(|f| f.generation),
                Some(4),
                "the old facts are not rewritten"
            );
            // Reading again settles it from the state.
            let again = c.begin_read().expect("no read in flight");
            let mut next = state(5, Audience::Private);
            if request {
                next = state(5, Audience::Public);
                next.intent = Some(PublishIntent {
                    revision_id: R2.into(),
                    generation: 5,
                });
            }
            c.read_finished(again, Ok(next));
            assert_eq!(c.phase(), Phase::Reconciled(done));
        }
    }

    // ---- reads, scope, unknown values ---------------------------------------------------------

    #[test]
    fn a_new_session_or_work_leaves_nothing_of_the_old_one_to_answer() {
        let mut c = read(controller(), state(3, Audience::Private));
        let (capture_ticket, _) = capture_of(c.start_submit(&clean()).unwrap());
        let reading = c.begin_read().unwrap();
        c.change_scope(Scope {
            work_id: WORK.into(),
            session: Some("session-b".into()),
        });
        assert_eq!(c.phase(), Phase::Reading);
        assert!(c.facts().is_none() && !c.working());
        // Answers that belong to the earlier session are ignored whatever they say.
        assert_eq!(
            c.capture_finished(capture_ticket, Ok(captured(7, 5))),
            Step::None
        );
        assert_eq!(
            c.read_finished(reading, Ok(state(9, Audience::Public))),
            Step::None
        );
        assert!(
            c.facts().is_none(),
            "the earlier session's facts never appear"
        );
        assert_eq!(c.phase(), Phase::Reading);
        // The new session reads for itself.
        let c = read(c, state(1, Audience::Private));
        assert_eq!(c.facts().map(|f| f.generation), Some(1));
    }

    #[test]
    fn an_answer_about_another_work_is_never_shown_as_this_ones() {
        let mut c = controller();
        let ticket = c.begin_read().unwrap();
        let mut other = state(1, Audience::Public);
        other.work_id = "44444444-4444-4444-8444-444444444444".into();
        c.read_finished(ticket, Ok(other));
        assert!(c.facts().is_none());
        assert_eq!(c.phase(), Phase::Unavailable(ReadFailure::Unreachable));
    }

    #[test]
    fn reads_are_single_flight_and_a_newer_request_is_not_lost() {
        let mut c = controller();
        let first = c.begin_read().unwrap();
        assert!(c.begin_read().is_none(), "no overlapping reads");
        // When the first ends, the read asked for meanwhile starts.
        let next = read_of(c.read_finished(first, Ok(state(1, Audience::Private))));
        assert!(c.reading());
        c.read_finished(next, Ok(state(2, Audience::Private)));
        assert_eq!(c.facts().map(|f| f.generation), Some(2));
        assert!(!c.reading());
    }

    #[test]
    fn a_read_for_a_superseded_ticket_or_an_older_start_is_not_applied() {
        let mut c = controller();
        let first = c.begin_read().unwrap();
        c.read_finished(first, Ok(state(2, Audience::Private)));
        // The same ticket again (a duplicate delivery) changes nothing.
        assert_eq!(
            c.read_finished(first, Ok(state(1, Audience::Private))),
            Step::None
        );
        assert_eq!(c.facts().map(|f| f.generation), Some(2));
    }

    #[test]
    fn a_failed_read_keeps_the_last_facts_unless_authority_was_lost() {
        let mut c = read(controller(), state(3, Audience::Private));
        let ticket = c.begin_read().unwrap();
        c.read_finished(ticket, Err(ApiError::Network));
        assert_eq!(c.facts().map(|f| f.generation), Some(3));
        assert_eq!(c.read_failure(), Some(ReadFailure::Unreachable));
        for (error, expected) in [
            (ApiError::Unauthenticated, ReadFailure::SessionEnded),
            (ApiError::Forbidden, ReadFailure::Forbidden),
            (ApiError::NotFound, ReadFailure::NotFound),
        ] {
            let mut c = read(controller(), state(3, Audience::Private));
            let ticket = c.begin_read().unwrap();
            c.read_finished(ticket, Err(error));
            assert!(
                c.facts().is_none(),
                "stale private facts must not outlive a denial"
            );
            assert_eq!(c.phase(), Phase::Unavailable(expected));
        }
    }

    #[test]
    fn a_value_this_build_does_not_understand_blocks_every_command_and_says_so() {
        for mutate in [
            (|s: &mut AuthorPublicationState| s.audience = Audience::Unknown) as fn(&mut _),
            |s| s.lifecycle = Lifecycle::Unknown,
            |s| s.revisions = vec![revision(R1, Decision::Unknown)],
        ] {
            let mut s = public_state(4);
            mutate(&mut s);
            let c = read(controller(), s);
            assert_eq!(c.phase(), Phase::Unreadable);
            assert_eq!(c.can_submit(&clean()), Err(Blocked::Unreadable));
            assert_eq!(c.can_withdraw(), Err(Blocked::Unreadable));
        }
        let mut s = public_state(4);
        s.lifecycle = Lifecycle::AuthorDeleted;
        let c = read(controller(), s);
        assert_eq!(c.can_submit(&clean()), Err(Blocked::NotActive));
    }

    // ---- editor facts -------------------------------------------------------------------------

    #[test]
    fn editor_facts_confirm_only_what_the_server_acknowledged() {
        let mut session = DraftSession::loaded(7);
        assert_eq!(EditorFacts::of(&session).tags, TagFacts::NotLoaded);
        session.tags = Some(TagSession::loaded(5, Selection::new(None, [])));
        let facts = EditorFacts::of(&session);
        assert_eq!(facts.confirmed(), Some((7, 5)));
        session.text.edited();
        let typed = EditorFacts::of(&session);
        assert_eq!(typed.text, TextFacts::Unsaved);
        assert_ne!(
            typed.text_edits, facts.text_edits,
            "every edit moves the counter"
        );
        assert!(typed.has_unsaved());
        // Choosing a tag changes the selection the mark compares.
        session
            .tags
            .as_mut()
            .unwrap()
            .choose_form(Some("a-form".into()));
        assert_ne!(EditorFacts::of(&session).selection, typed.selection);
        assert_eq!(EditorFacts::of(&session).tags, TagFacts::Unsaved);
    }

    // ---- what the page says --------------------------------------------------------------------

    #[test]
    fn the_summary_keeps_the_public_revision_apart_from_the_requested_one() {
        let mut s = public_state(4);
        s.intent = Some(PublishIntent {
            revision_id: R2.into(),
            generation: 4,
        });
        s.revisions = vec![
            revision(R2, Decision::Pending),
            revision(R1, Decision::Approved),
        ];
        s.open_ticket = Some(OpenTicket {
            evaluation_id: "e".into(),
            revision_id: R2.into(),
            policy_version: 1,
        });
        let summary = Summary::of(&s);
        // r1 is what readers read, whatever r2 is doing.
        assert_eq!(
            summary.public.as_ref().map(|r| r.revision_id.as_str()),
            Some(R1)
        );
        assert_eq!(
            summary.requested.as_ref().map(|r| r.decision),
            Some(Decision::Pending)
        );
        assert_eq!(summary.unsent, 0);
        // r2 rejected: still r1 for readers.
        s.revisions[0].moderation = Decision::Rejected;
        let summary = Summary::of(&s);
        assert_eq!(
            summary.public.as_ref().map(|r| r.revision_id.as_str()),
            Some(R1)
        );
        assert_eq!(
            summary.requested.as_ref().map(|r| r.decision),
            Some(Decision::Rejected)
        );
    }

    #[test]
    fn approved_is_not_public_and_public_audience_is_not_a_public_revision() {
        // Approved, intent public, but no pointer: nobody reads it (for example, the work was
        // made private after approval and made public again, waiting).
        let mut s = state(6, Audience::Public);
        s.intent = Some(PublishIntent {
            revision_id: R1.into(),
            generation: 6,
        });
        s.revisions = vec![revision(R1, Decision::Approved)];
        let summary = Summary::of(&s);
        assert!(summary.public.is_none(), "no pointer, nothing is public");
        assert_eq!(
            summary.requested.as_ref().map(|r| r.decision),
            Some(Decision::Approved)
        );
        // The pointer revision is the same as the intent: nothing is "requested" on top of it.
        let summary = Summary::of(&public_state(4));
        assert!(summary.public.is_some() && summary.requested.is_none());
    }

    #[test]
    fn captured_but_never_submitted_revisions_are_counted_not_guessed_about() {
        let mut s = state(2, Audience::Private);
        s.revisions = vec![
            revision(R2, Decision::Unsubmitted),
            revision(R1, Decision::Unsubmitted),
        ];
        assert_eq!(Summary::of(&s).unsent, 2);
        s.audience = Audience::Public;
        s.intent = Some(PublishIntent {
            revision_id: R2.into(),
            generation: 2,
        });
        s.revisions[0].moderation = Decision::Pending;
        assert_eq!(Summary::of(&s).unsent, 1);
    }
}
