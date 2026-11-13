//! Save-state machine of the desk editor, kept free of DOM and transport code.
//!
//! "Saved" is shown only after the server confirmed the exact local edit that was sent. One save
//! is in flight at a time; a response for an older ticket is ignored, and edits made while a
//! save is in flight keep the text marked unsaved after the acknowledgement.
//!
//! Text and draft tags (TAG-PR-05) are two resources with their own versions and requests.
//! [`DraftSession`] holds both; the leave guard protects whatever either part has not had
//! confirmed and leaves only once every part it waited for is confirmed.

use crate::tag_state::{TagSession, TagStatus, TagTicket};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Network,
    Unavailable,
    Unauthenticated,
    Forbidden,
    NotFound,
    Invalid,
    Unexpected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveRejection {
    Conflict { current_version: i64 },
    Failed(Failure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveStatus {
    Saved { version: i64 },
    Unsaved,
    Saving,
    Failed(Failure),
    Conflict { server_version: i64 },
}

impl SaveStatus {
    /// Stable token for the `data-status` attribute used by styles and browser checks.
    pub fn token(self) -> &'static str {
        match self {
            Self::Saved { .. } => "saved",
            Self::Unsaved => "unsaved",
            Self::Saving => "saving",
            Self::Failed(_) => "failed",
            Self::Conflict { .. } => "conflict",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveTicket {
    pub expected_version: i64,
    edit: u64,
    request: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSession {
    base_version: i64,
    edits: u64,
    saved_edits: u64,
    requests: u64,
    in_flight: Option<SaveTicket>,
    conflict: Option<i64>,
    failure: Option<Failure>,
}

impl EditorSession {
    pub fn loaded(version: i64) -> Self {
        Self {
            base_version: version,
            edits: 0,
            saved_edits: 0,
            requests: 0,
            in_flight: None,
            conflict: None,
            failure: None,
        }
    }

    pub fn edited(&mut self) {
        self.edits += 1;
    }

    /// Starts a save of the current edit, unless one is in flight or a conflict is unresolved.
    pub fn begin_save(&mut self) -> Option<SaveTicket> {
        if self.in_flight.is_some() || self.conflict.is_some() {
            return None;
        }
        self.requests += 1;
        let ticket = SaveTicket {
            expected_version: self.base_version,
            edit: self.edits,
            request: self.requests,
        };
        self.in_flight = Some(ticket);
        self.failure = None;
        Some(ticket)
    }

    pub fn save_succeeded(&mut self, ticket: SaveTicket, new_version: i64) {
        if self.in_flight != Some(ticket) {
            return;
        }
        self.in_flight = None;
        self.base_version = new_version;
        self.saved_edits = ticket.edit;
    }

    pub fn save_rejected(&mut self, ticket: SaveTicket, rejection: SaveRejection) {
        if self.in_flight != Some(ticket) {
            return;
        }
        self.in_flight = None;
        match rejection {
            SaveRejection::Conflict { current_version } => self.conflict = Some(current_version),
            SaveRejection::Failed(failure) => self.failure = Some(failure),
        }
    }

    /// The author chose to keep the local text: the next save targets the server's version.
    pub fn keep_local_text(&mut self) {
        if let Some(server_version) = self.conflict.take() {
            self.base_version = server_version;
        }
    }

    pub fn status(&self) -> SaveStatus {
        if self.in_flight.is_some() {
            SaveStatus::Saving
        } else if let Some(server_version) = self.conflict {
            SaveStatus::Conflict { server_version }
        } else if let Some(failure) = self.failure {
            SaveStatus::Failed(failure)
        } else if self.edits == self.saved_edits {
            SaveStatus::Saved {
                version: self.base_version,
            }
        } else {
            SaveStatus::Unsaved
        }
    }

    pub fn has_unsaved_text(&self) -> bool {
        self.edits != self.saved_edits || self.conflict.is_some()
    }

    /// How many edits the author made so far. A coordinator that must send "what the author
    /// pressed on" (publication) compares two readings: equal means nothing was typed between.
    pub fn edits(&self) -> u64 {
        self.edits
    }

    pub fn can_save(&self) -> bool {
        self.in_flight.is_none() && self.conflict.is_none()
    }

    pub fn in_flight(&self) -> Option<SaveTicket> {
        self.in_flight
    }
}

/// What a leave guard protects: one resource (the text) or several (text and tags).
pub trait Guarded {
    /// Identifies the saves a leave intent waits for.
    type Ticket: Copy + Eq + std::fmt::Debug;
    /// Something the server has not confirmed.
    fn has_unsaved(&self) -> bool;
    /// Starts the saves leaving needs. Returns the newly started saves to send (if any) and every
    /// save to wait for, or `None` when an unsaved part cannot be saved now (a conflict).
    fn begin_leave_save(&mut self) -> Option<(Option<Self::Ticket>, Self::Ticket)>;
    /// Whether any save of `ticket` is still waiting for its outcome.
    fn awaiting(&self, ticket: Self::Ticket) -> bool;
    /// Saving cannot settle the unsaved state now: waiting, failed, refused or in conflict.
    fn settling_blocked(&self) -> bool;
    /// The last attempt did not save (failure, refusal or conflict).
    fn not_saved(&self) -> bool;
}

impl Guarded for EditorSession {
    type Ticket = SaveTicket;

    fn has_unsaved(&self) -> bool {
        self.has_unsaved_text()
    }

    fn begin_leave_save(&mut self) -> Option<(Option<SaveTicket>, SaveTicket)> {
        if let Some(ticket) = self.in_flight() {
            return Some((None, ticket));
        }
        self.begin_save().map(|ticket| (Some(ticket), ticket))
    }

    fn awaiting(&self, ticket: SaveTicket) -> bool {
        self.in_flight == Some(ticket)
    }

    fn settling_blocked(&self) -> bool {
        matches!(
            self.status(),
            SaveStatus::Saving | SaveStatus::Failed(_) | SaveStatus::Conflict { .. }
        )
    }

    fn not_saved(&self) -> bool {
        matches!(
            self.status(),
            SaveStatus::Failed(_) | SaveStatus::Conflict { .. }
        )
    }
}

/// The editor's two resources. `tags` is `None` until the draft tags are loaded (nothing to
/// protect or save before that).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftSession {
    pub text: EditorSession,
    pub tags: Option<TagSession>,
}

/// The saves of one "Lưu" or one leave intent; a part is `None` when it is not involved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DraftTicket {
    pub text: Option<SaveTicket>,
    pub tags: Option<TagTicket>,
}

impl DraftTicket {
    pub fn is_empty(self) -> bool {
        self.text.is_none() && self.tags.is_none()
    }
}

impl DraftSession {
    pub fn loaded(version: i64) -> Self {
        Self {
            text: EditorSession::loaded(version),
            tags: None,
        }
    }

    fn tags_unsaved(&self) -> bool {
        self.tags.as_ref().is_some_and(TagSession::has_unsaved)
    }

    /// "Lưu": starts a save of every part that has unsaved changes and can be saved. With no
    /// unsaved tags the text is saved as before, even when unchanged. Returns only the newly
    /// started saves (a part already in flight is not sent twice).
    pub fn begin_save(&mut self) -> DraftTicket {
        let tags = match self.tags.as_mut() {
            Some(tags) if tags.has_unsaved() => tags.begin_save(),
            _ => None,
        };
        let tags_pending =
            tags.is_some() || self.tags.as_ref().and_then(TagSession::in_flight).is_some();
        let text = if self.text.has_unsaved_text() || !(tags_pending || self.tags_unsaved()) {
            self.text.begin_save()
        } else {
            None
        };
        DraftTicket { text, tags }
    }

    /// Whether "Lưu" can start anything now.
    pub fn can_save(&self) -> bool {
        let tags = self
            .tags
            .as_ref()
            .is_some_and(|tags| tags.has_unsaved() && tags.can_save());
        tags || self.text.can_save()
    }

    pub fn in_flight(&self) -> DraftTicket {
        DraftTicket {
            text: self.text.in_flight(),
            tags: self.tags.as_ref().and_then(TagSession::in_flight),
        }
    }

    pub fn tag_status(&self) -> Option<TagStatus> {
        self.tags.as_ref().map(TagSession::status)
    }
}

impl Guarded for DraftSession {
    type Ticket = DraftTicket;

    fn has_unsaved(&self) -> bool {
        self.text.has_unsaved_text() || self.tags_unsaved()
    }

    fn begin_leave_save(&mut self) -> Option<(Option<DraftTicket>, DraftTicket)> {
        let text_blocked = self.text.has_unsaved_text() && self.text.conflict.is_some();
        let tags_blocked = self.tags.as_ref().is_some_and(|tags| {
            tags.has_unsaved() && !tags.can_save() && tags.in_flight().is_none()
        });
        if text_blocked || tags_blocked {
            return None;
        }
        let mut started = DraftTicket::default();
        if self.text.has_unsaved_text() && self.text.in_flight().is_none() {
            started.text = self.text.begin_save();
        }
        if let Some(tags) = self.tags.as_mut()
            && tags.has_unsaved()
            && tags.in_flight().is_none()
        {
            started.tags = tags.begin_save();
        }
        let awaited = self.in_flight();
        if awaited.is_empty() {
            return None;
        }
        Some(((!started.is_empty()).then_some(started), awaited))
    }

    fn awaiting(&self, ticket: DraftTicket) -> bool {
        let now = self.in_flight();
        (ticket.text.is_some() && now.text == ticket.text)
            || (ticket.tags.is_some() && now.tags == ticket.tags)
    }

    fn settling_blocked(&self) -> bool {
        self.text.settling_blocked()
            || self.tag_status().is_some_and(|status| {
                !matches!(status, TagStatus::Saved { .. } | TagStatus::Unsaved)
            })
    }

    fn not_saved(&self) -> bool {
        self.text.not_saved()
            || self.tag_status().is_some_and(|status| {
                matches!(
                    status,
                    TagStatus::Conflict { .. } | TagStatus::Refused(_) | TagStatus::Failed(_)
                )
            })
    }
}

/// Leaving an editor that holds changes the server has not confirmed.
///
/// A navigation attempt becomes an intent with its own id and destination; the author stays,
/// saves and leaves, or — once saving is failing, in conflict or still waiting — discards after
/// a second, explicit confirmation. Leaving follows a save only when every save that intent
/// waits for is acknowledged and no newer change exists. Staying, choosing another destination
/// or discarding replaces the intent, so a late acknowledgement can neither navigate nor reopen
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaveGuard<T = SaveTicket> {
    intents: u64,
    pending: Option<LeaveIntent<T>>,
    released: bool,
}

impl<T> Default for LeaveGuard<T> {
    fn default() -> Self {
        Self {
            intents: 0,
            pending: None,
            released: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaveIntent<T = SaveTicket> {
    pub id: u64,
    pub destination: String,
    pub stage: LeaveStage<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveStage<T = SaveTicket> {
    /// Stay or save-and-leave; `notice` says why an earlier attempt did not leave.
    Choosing { notice: Option<LeaveNotice> },
    /// Leaving waits for the acknowledgement of these saves.
    Saving { ticket: T },
    /// The author asked to leave without saving and has to confirm the loss.
    ConfirmingDiscard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveNotice {
    /// The save failed, was refused or conflicted; the session status says which.
    NotSaved,
    /// The save succeeded, but changes made after it started are still unsaved.
    NewerEdits,
}

impl<T: Copy + Eq + std::fmt::Debug> LeaveGuard<T> {
    /// A navigation attempt. Returns `true` when it is held for the author's choice.
    pub fn request<S: Guarded<Ticket = T>>(&mut self, session: &S, destination: &str) -> bool {
        if self.released || !session.has_unsaved() {
            return false;
        }
        self.intents += 1;
        self.pending = Some(LeaveIntent {
            id: self.intents,
            destination: destination.to_string(),
            stage: LeaveStage::Choosing { notice: None },
        });
        true
    }

    /// "Ở lại" or Escape: the intent and any continuation of it end here.
    pub fn stay(&mut self) {
        self.pending = None;
    }

    /// "Lưu và rời trang". Waits for saves already in flight and starts the missing ones;
    /// returns the saves to send when new ones started. An unresolved conflict cannot be saved
    /// from here.
    pub fn save_and_leave<S: Guarded<Ticket = T>>(&mut self, session: &mut S) -> Option<T> {
        let intent = self.pending.as_mut()?;
        if !matches!(intent.stage, LeaveStage::Choosing { .. }) {
            return None;
        }
        match session.begin_leave_save() {
            Some((started, awaited)) => {
                intent.stage = LeaveStage::Saving { ticket: awaited };
                started
            }
            None => {
                intent.stage = LeaveStage::Choosing {
                    notice: Some(LeaveNotice::NotSaved),
                };
                None
            }
        }
    }

    /// Call after `session` applied the outcome of a save. Returns the destination when every
    /// save this intent waits for has settled and nothing unconfirmed remains.
    pub fn save_settled<S: Guarded<Ticket = T>>(&mut self, session: &S) -> Option<String> {
        let intent = self.pending.as_mut()?;
        let LeaveStage::Saving { ticket } = intent.stage else {
            return None;
        };
        if session.awaiting(ticket) {
            return None;
        }
        if !session.has_unsaved() {
            self.released = true;
            return self.pending.take().map(|intent| intent.destination);
        }
        let notice = if session.not_saved() {
            LeaveNotice::NotSaved
        } else {
            LeaveNotice::NewerEdits
        };
        intent.stage = LeaveStage::Choosing {
            notice: Some(notice),
        };
        None
    }

    /// Leaving without saving is offered only when saving cannot settle the changes now.
    pub fn can_discard<S: Guarded<Ticket = T>>(&self, session: &S) -> bool {
        self.pending.is_some() && session.settling_blocked()
    }

    /// "Rời mà không lưu…": asks for confirmation. A save this intent waited for stops counting.
    pub fn ask_discard<S: Guarded<Ticket = T>>(&mut self, session: &S) {
        if self.can_discard(session)
            && let Some(intent) = self.pending.as_mut()
        {
            intent.stage = LeaveStage::ConfirmingDiscard;
        }
    }

    /// Back from the confirmation to the choices, without leaving.
    pub fn keep_choosing(&mut self) {
        if let Some(intent) = self.pending.as_mut()
            && intent.stage == LeaveStage::ConfirmingDiscard
        {
            intent.stage = LeaveStage::Choosing { notice: None };
        }
    }

    /// The author confirmed losing the unsaved changes. Returns where to go; nothing in this
    /// editor holds a navigation afterwards.
    pub fn confirm_discard(&mut self) -> Option<String> {
        if self.pending.as_ref()?.stage != LeaveStage::ConfirmingDiscard {
            return None;
        }
        self.released = true;
        self.pending.take().map(|intent| intent.destination)
    }

    pub fn pending(&self) -> Option<&LeaveIntent<T>> {
        self.pending.as_ref()
    }

    /// True once the author chose to leave; the unload prompt and route guard stand down.
    pub fn released(&self) -> bool {
        self.released
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag_state::{Selection, TagRejection};

    #[test]
    fn saved_only_after_server_confirms_the_sent_edit() {
        let mut s = EditorSession::loaded(3);
        assert_eq!(s.status(), SaveStatus::Saved { version: 3 });
        s.edited();
        assert_eq!(s.status(), SaveStatus::Unsaved);
        let ticket = s.begin_save().unwrap();
        assert_eq!(ticket.expected_version, 3);
        assert_eq!(s.status(), SaveStatus::Saving);
        s.save_succeeded(ticket, 4);
        assert_eq!(s.status(), SaveStatus::Saved { version: 4 });
        assert!(!s.has_unsaved_text());
    }

    #[test]
    fn late_acknowledgement_does_not_mark_newer_text_saved() {
        let mut s = EditorSession::loaded(7);
        s.edited();
        let ticket = s.begin_save().unwrap();
        s.edited(); // typed while version 7 → 8 was in flight
        s.save_succeeded(ticket, 8);
        assert_eq!(s.status(), SaveStatus::Unsaved);
        assert!(s.has_unsaved_text());
        let next = s.begin_save().unwrap();
        assert_eq!(next.expected_version, 8);
    }

    #[test]
    fn one_save_in_flight_and_stale_tickets_are_ignored() {
        let mut s = EditorSession::loaded(1);
        s.edited();
        let first = s.begin_save().unwrap();
        assert_eq!(s.begin_save(), None);
        s.save_rejected(first, SaveRejection::Failed(Failure::Network));
        assert_eq!(s.status(), SaveStatus::Failed(Failure::Network));
        let second = s.begin_save().unwrap();
        // A duplicate delivery of the first outcome cannot settle the second request.
        s.save_succeeded(first, 2);
        assert_eq!(s.status(), SaveStatus::Saving);
        s.save_succeeded(second, 2);
        assert_eq!(s.status(), SaveStatus::Saved { version: 2 });
    }

    #[test]
    fn conflict_keeps_text_until_author_chooses() {
        let mut s = EditorSession::loaded(2);
        s.edited();
        let ticket = s.begin_save().unwrap();
        s.save_rejected(ticket, SaveRejection::Conflict { current_version: 5 });
        assert_eq!(s.status(), SaveStatus::Conflict { server_version: 5 });
        assert!(s.has_unsaved_text());
        assert_eq!(s.begin_save(), None, "no silent overwrite");
        s.keep_local_text();
        let overwrite = s.begin_save().unwrap();
        assert_eq!(overwrite.expected_version, 5);
        s.save_succeeded(overwrite, 6);
        assert_eq!(s.status(), SaveStatus::Saved { version: 6 });
    }

    fn dirty(version: i64) -> EditorSession {
        let mut s = EditorSession::loaded(version);
        s.edited();
        s
    }

    fn stage(guard: &LeaveGuard) -> Option<LeaveStage> {
        guard.pending().map(|intent| intent.stage)
    }

    #[test]
    fn confirmed_text_leaves_without_asking() {
        let s = EditorSession::loaded(1);
        let mut guard = LeaveGuard::default();
        assert!(!guard.request(&s, "/"));
        assert_eq!(guard.pending(), None);
    }

    #[test]
    fn save_and_leave_goes_only_after_the_latest_text_is_acknowledged() {
        let mut s = dirty(1);
        let mut guard = LeaveGuard::default();
        assert!(guard.request(&s, "/ban-viet"));
        let ticket = guard.save_and_leave(&mut s).unwrap();
        assert_eq!(stage(&guard), Some(LeaveStage::Saving { ticket }));
        s.save_succeeded(ticket, 2);
        assert_eq!(guard.save_settled(&s), Some("/ban-viet".into()));
        assert!(guard.released());
        assert!(!guard.request(&s, "/"), "nothing left to protect");
    }

    #[test]
    fn staying_while_saving_cancels_the_navigation_but_not_the_save() {
        let mut s = dirty(1);
        let mut guard = LeaveGuard::default();
        guard.request(&s, "/");
        let ticket = guard.save_and_leave(&mut s).unwrap();
        guard.stay();
        s.save_succeeded(ticket, 2);
        assert_eq!(
            guard.save_settled(&s),
            None,
            "late success must not navigate"
        );
        assert_eq!(s.status(), SaveStatus::Saved { version: 2 });
        assert!(!guard.released());
    }

    #[test]
    fn a_new_destination_replaces_the_pending_continuation() {
        let mut s = dirty(1);
        let mut guard = LeaveGuard::default();
        guard.request(&s, "/");
        let ticket = guard.save_and_leave(&mut s).unwrap();
        assert!(guard.request(&s, "/ban-viet"), "back button while saving");
        let intent = guard.pending().unwrap().clone();
        assert_eq!(intent.destination, "/ban-viet");
        assert_eq!(intent.stage, LeaveStage::Choosing { notice: None });
        s.save_succeeded(ticket, 2);
        assert_eq!(guard.save_settled(&s), None);
        assert_eq!(guard.pending().map(|i| i.id), Some(intent.id));
    }

    #[test]
    fn newer_edits_keep_the_author_in_the_editor() {
        let mut s = dirty(1);
        let mut guard = LeaveGuard::default();
        guard.request(&s, "/");
        let ticket = guard.save_and_leave(&mut s).unwrap();
        s.edited(); // e.g. a composition committed after the save started
        s.save_succeeded(ticket, 2);
        assert_eq!(guard.save_settled(&s), None);
        assert_eq!(
            stage(&guard),
            Some(LeaveStage::Choosing {
                notice: Some(LeaveNotice::NewerEdits)
            })
        );
        let next = guard.save_and_leave(&mut s).unwrap();
        assert_eq!(next.expected_version, 2);
        s.save_succeeded(next, 3);
        assert_eq!(guard.save_settled(&s), Some("/".into()));
    }

    #[test]
    fn a_save_already_in_flight_is_awaited_not_duplicated() {
        let mut s = dirty(1);
        let first = s.begin_save().unwrap();
        let mut guard = LeaveGuard::default();
        assert!(guard.request(&s, "/"), "saving text is not confirmed yet");
        assert_eq!(guard.save_and_leave(&mut s), None);
        assert_eq!(stage(&guard), Some(LeaveStage::Saving { ticket: first }));
        s.save_succeeded(first, 2);
        assert_eq!(guard.save_settled(&s), Some("/".into()));
    }

    #[test]
    fn failure_and_conflict_stay_and_offer_an_explicit_discard() {
        for rejection in [
            SaveRejection::Failed(Failure::Unavailable),
            SaveRejection::Conflict { current_version: 4 },
        ] {
            let mut s = dirty(1);
            let mut guard = LeaveGuard::default();
            guard.request(&s, "/");
            assert!(
                !guard.can_discard(&s),
                "a plain unsaved state offers saving first"
            );
            let ticket = guard.save_and_leave(&mut s).unwrap();
            s.save_rejected(ticket, rejection);
            assert_eq!(guard.save_settled(&s), None);
            assert_eq!(
                stage(&guard),
                Some(LeaveStage::Choosing {
                    notice: Some(LeaveNotice::NotSaved)
                })
            );
            assert!(s.has_unsaved_text(), "the text stays in the editor");
            assert!(guard.can_discard(&s));
            assert_eq!(
                guard.confirm_discard(),
                None,
                "discard needs a confirmation step"
            );
            guard.ask_discard(&s);
            guard.keep_choosing();
            assert_eq!(guard.confirm_discard(), None);
            guard.ask_discard(&s);
            assert_eq!(guard.confirm_discard(), Some("/".into()));
            assert!(guard.released());
        }
    }

    #[test]
    fn a_conflict_cannot_be_saved_from_the_leave_dialog() {
        let mut s = dirty(1);
        let ticket = s.begin_save().unwrap();
        s.save_rejected(ticket, SaveRejection::Conflict { current_version: 3 });
        let mut guard = LeaveGuard::default();
        assert!(guard.request(&s, "/"));
        assert_eq!(guard.save_and_leave(&mut s), None, "no silent overwrite");
        assert_eq!(
            stage(&guard),
            Some(LeaveStage::Choosing {
                notice: Some(LeaveNotice::NotSaved)
            })
        );
    }

    #[test]
    fn discarding_while_a_save_waits_ignores_its_late_acknowledgement() {
        let mut s = dirty(1);
        let mut guard = LeaveGuard::default();
        guard.request(&s, "/ban-viet");
        let ticket = guard.save_and_leave(&mut s).unwrap();
        assert!(
            guard.can_discard(&s),
            "a hanging save must not trap the author"
        );
        guard.ask_discard(&s);
        assert_eq!(guard.confirm_discard(), Some("/ban-viet".into()));
        s.save_succeeded(ticket, 2);
        assert_eq!(guard.save_settled(&s), None, "no second navigation");
    }
    fn draft(text_dirty: bool, tags_dirty: bool) -> DraftSession {
        let mut d = DraftSession::loaded(1);
        let mut tags = TagSession::loaded(1, Selection::default());
        if text_dirty {
            d.text.edited();
        }
        if tags_dirty {
            tags.add_theme("ky-uc").unwrap();
        }
        d.tags = Some(tags);
        d
    }

    fn tags_ok(d: &mut DraftSession, ticket: DraftTicket, version: i64) {
        let tags = d.tags.as_mut().unwrap();
        let sent = tags.submitted(ticket.tags.unwrap()).unwrap().clone();
        tags.save_succeeded(ticket.tags.unwrap(), version, sent);
    }

    #[test]
    fn a_tag_only_change_holds_the_navigation_and_saves_only_the_tags() {
        let mut d = draft(false, true);
        let mut guard = LeaveGuard::<DraftTicket>::default();
        assert!(
            guard.request(&d, "/ban-viet"),
            "tags alone are unsaved work"
        );
        let sent = guard.save_and_leave(&mut d).unwrap();
        assert!(sent.text.is_none() && sent.tags.is_some());
        tags_ok(&mut d, sent, 2);
        assert_eq!(guard.save_settled(&d), Some("/ban-viet".into()));
    }

    #[test]
    fn leaving_waits_for_both_text_and_tags() {
        let mut d = draft(true, true);
        let mut guard = LeaveGuard::<DraftTicket>::default();
        guard.request(&d, "/");
        let sent = guard.save_and_leave(&mut d).unwrap();
        assert!(sent.text.is_some() && sent.tags.is_some());
        d.text.save_succeeded(sent.text.unwrap(), 2);
        assert_eq!(guard.save_settled(&d), None, "the tags are still waiting");
        assert!(matches!(
            guard.pending().map(|i| i.stage),
            Some(LeaveStage::Saving { .. })
        ));
        tags_ok(&mut d, sent, 2);
        assert_eq!(guard.save_settled(&d), Some("/".into()));
    }

    #[test]
    fn restoring_the_old_tags_while_saving_still_holds_navigation() {
        let mut d = draft(false, true);
        let first = d.begin_save();
        d.tags.as_mut().unwrap().remove_theme("ky-uc");
        let mut guard = LeaveGuard::<DraftTicket>::default();
        assert!(guard.request(&d, "/"));
        assert_eq!(
            guard.save_and_leave(&mut d),
            None,
            "waits for the existing save"
        );
        assert_eq!(guard.save_settled(&d), None);
        tags_ok(&mut d, first, 2);
        assert_eq!(
            guard.save_settled(&d),
            None,
            "the restored tags still need saving"
        );

        let restore = guard.save_and_leave(&mut d).unwrap();
        assert!(restore.text.is_none());
        assert_eq!(restore.tags.unwrap().expected_version, 2);
        tags_ok(&mut d, restore, 3);
        assert_eq!(guard.save_settled(&d), Some("/".into()));
    }

    #[test]
    fn retrying_a_lost_tag_ack_after_restoring_the_old_tags_sends_only_tags() {
        let mut d = draft(false, true);
        let first = d.begin_save().tags.unwrap();
        let tags = d.tags.as_mut().unwrap();
        tags.remove_theme("ky-uc");
        tags.save_rejected(first, TagRejection::Failed(Failure::Network));
        let mut guard = LeaveGuard::<DraftTicket>::default();
        assert!(guard.request(&d, "/"));
        let retry = d.begin_save();
        assert!(retry.text.is_none());
        assert_eq!(retry.tags.unwrap().expected_version, 1);
        assert_eq!(
            d.tags.as_ref().unwrap().submitted(retry.tags.unwrap()),
            Some(&Selection::default())
        );
    }

    #[test]
    fn a_partial_save_stays_truthful_and_does_not_leave() {
        let mut d = draft(true, true);
        let mut guard = LeaveGuard::<DraftTicket>::default();
        guard.request(&d, "/");
        let sent = guard.save_and_leave(&mut d).unwrap();
        d.tags.as_mut().unwrap().save_rejected(
            sent.tags.unwrap(),
            TagRejection::Failed(Failure::Unavailable),
        );
        assert_eq!(guard.save_settled(&d), None);
        d.text.save_succeeded(sent.text.unwrap(), 2);
        assert_eq!(guard.save_settled(&d), None);
        assert_eq!(d.text.status(), SaveStatus::Saved { version: 2 });
        assert!(matches!(d.tag_status(), Some(TagStatus::Failed(_))));
        assert_eq!(
            guard.pending().map(|i| i.stage),
            Some(LeaveStage::Choosing {
                notice: Some(LeaveNotice::NotSaved)
            })
        );
        assert!(
            guard.can_discard(&d),
            "a failing tag save must not trap the author"
        );
    }

    #[test]
    fn save_sends_only_the_changed_parts_and_never_a_part_twice() {
        let mut d = draft(false, true);
        let first = d.begin_save();
        assert!(first.text.is_none() && first.tags.is_some());
        assert!(
            d.begin_save().is_empty(),
            "the tag save is already in flight"
        );
        tags_ok(&mut d, first, 2);
        // Nothing unsaved: "Lưu" saves the text, as before tags existed.
        let plain = d.begin_save();
        assert!(plain.text.is_some() && plain.tags.is_none());
        let mut text_only = DraftSession::loaded(3);
        let ticket = text_only.begin_save();
        assert!(ticket.text.is_some() && ticket.tags.is_none());
    }

    #[test]
    fn leaving_awaits_a_text_save_in_flight_and_starts_the_tag_save_it_needs() {
        let mut d = draft(true, false);
        let text = d.begin_save();
        d.tags.as_mut().unwrap().add_theme("mua").unwrap();
        let mut guard = LeaveGuard::<DraftTicket>::default();
        guard.request(&d, "/");
        let sent = guard.save_and_leave(&mut d).unwrap();
        assert!(sent.text.is_none(), "the text save is not duplicated");
        assert!(sent.tags.is_some());
        tags_ok(&mut d, sent, 2);
        assert_eq!(guard.save_settled(&d), None, "the text save is awaited");
        d.text.save_succeeded(text.text.unwrap(), 2);
        assert_eq!(guard.save_settled(&d), Some("/".into()));
    }

    #[test]
    fn a_tag_conflict_cannot_be_saved_from_the_leave_dialog() {
        let mut d = draft(true, true);
        let first = d.begin_save();
        d.tags.as_mut().unwrap().save_rejected(
            first.tags.unwrap(),
            TagRejection::Conflict {
                current_version: 3,
                stored: Selection::new(None, ["mua".to_string()]),
            },
        );
        d.text.save_succeeded(first.text.unwrap(), 2);
        let mut guard = LeaveGuard::<DraftTicket>::default();
        assert!(guard.request(&d, "/"));
        assert_eq!(guard.save_and_leave(&mut d), None, "no silent overwrite");
        assert_eq!(d.in_flight(), DraftTicket::default(), "nothing was started");
        assert!(guard.can_discard(&d));
    }

    #[test]
    fn staying_ignores_a_late_tag_acknowledgement() {
        let mut d = draft(false, true);
        let mut guard = LeaveGuard::<DraftTicket>::default();
        guard.request(&d, "/");
        let sent = guard.save_and_leave(&mut d).unwrap();
        guard.stay();
        tags_ok(&mut d, sent, 2);
        assert_eq!(guard.save_settled(&d), None);
        assert!(!guard.released());
    }

    #[test]
    fn tags_that_never_loaded_protect_nothing() {
        let mut d = DraftSession::loaded(1);
        let mut guard = LeaveGuard::<DraftTicket>::default();
        assert!(!guard.request(&d, "/"));
        d.text.edited();
        assert!(guard.request(&d, "/"));
        let sent = guard.save_and_leave(&mut d).unwrap();
        assert!(sent.tags.is_none());
    }
}
