//! Save-state machine of the desk editor, kept free of DOM and transport code.
//!
//! "Saved" is shown only after the server confirmed the exact local edit that was sent. One save
//! is in flight at a time; a response for an older ticket is ignored, and edits made while a
//! save is in flight keep the text marked unsaved after the acknowledgement.

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

    pub fn can_save(&self) -> bool {
        self.in_flight.is_none() && self.conflict.is_none()
    }

    pub fn in_flight(&self) -> Option<SaveTicket> {
        self.in_flight
    }
}

/// Leaving an editor that holds text the server has not confirmed.
///
/// A navigation attempt becomes an intent with its own id and destination; the author stays,
/// saves and leaves, or — once saving is failing, in conflict or still waiting — discards after
/// a second, explicit confirmation. Leaving follows a save only when that intent's own save is
/// acknowledged and no newer edit exists. Staying, choosing another destination or discarding
/// replaces the intent, so a late acknowledgement can neither navigate nor reopen it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeaveGuard {
    intents: u64,
    pending: Option<LeaveIntent>,
    released: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaveIntent {
    pub id: u64,
    pub destination: String,
    pub stage: LeaveStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveStage {
    /// Stay or save-and-leave; `notice` says why an earlier attempt did not leave.
    Choosing { notice: Option<LeaveNotice> },
    /// Leaving waits for the acknowledgement of this save.
    Saving { ticket: SaveTicket },
    /// The author asked to leave without saving and has to confirm the loss.
    ConfirmingDiscard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveNotice {
    /// The save failed or conflicted; the session status says which.
    NotSaved,
    /// The save succeeded, but text typed after it started is still unsaved.
    NewerEdits,
}

impl LeaveGuard {
    /// A navigation attempt. Returns `true` when it is held for the author's choice.
    pub fn request(&mut self, session: &EditorSession, destination: &str) -> bool {
        if self.released || !session.has_unsaved_text() {
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

    /// "Lưu và rời trang". Waits for a save already in flight, otherwise starts one; returns the
    /// ticket to send when a new save started. An unresolved conflict cannot be saved from here.
    pub fn save_and_leave(&mut self, session: &mut EditorSession) -> Option<SaveTicket> {
        let intent = self.pending.as_mut()?;
        if !matches!(intent.stage, LeaveStage::Choosing { .. }) {
            return None;
        }
        if let Some(ticket) = session.in_flight() {
            intent.stage = LeaveStage::Saving { ticket };
            return None;
        }
        match session.begin_save() {
            Some(ticket) => {
                intent.stage = LeaveStage::Saving { ticket };
                Some(ticket)
            }
            None => {
                intent.stage = LeaveStage::Choosing {
                    notice: Some(LeaveNotice::NotSaved),
                };
                None
            }
        }
    }

    /// Call after `session` applied the outcome of `ticket`. Returns the destination when the
    /// author's latest text is confirmed and this intent is still the one waiting for `ticket`.
    pub fn save_settled(&mut self, session: &EditorSession, ticket: SaveTicket) -> Option<String> {
        let intent = self.pending.as_mut()?;
        if intent.stage != (LeaveStage::Saving { ticket }) {
            return None;
        }
        if !session.has_unsaved_text() {
            self.released = true;
            return self.pending.take().map(|intent| intent.destination);
        }
        let notice = match session.status() {
            SaveStatus::Failed(_) | SaveStatus::Conflict { .. } => LeaveNotice::NotSaved,
            _ => LeaveNotice::NewerEdits,
        };
        intent.stage = LeaveStage::Choosing {
            notice: Some(notice),
        };
        None
    }

    /// Leaving without saving is offered only when saving cannot settle the text now.
    pub fn can_discard(&self, session: &EditorSession) -> bool {
        self.pending.is_some()
            && matches!(
                session.status(),
                SaveStatus::Saving | SaveStatus::Failed(_) | SaveStatus::Conflict { .. }
            )
    }

    /// "Rời mà không lưu…": asks for confirmation. A save this intent waited for stops counting.
    pub fn ask_discard(&mut self, session: &EditorSession) {
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

    /// The author confirmed losing the unsaved text. Returns where to go; nothing in this
    /// editor holds a navigation afterwards.
    pub fn confirm_discard(&mut self) -> Option<String> {
        if self.pending.as_ref()?.stage != LeaveStage::ConfirmingDiscard {
            return None;
        }
        self.released = true;
        self.pending.take().map(|intent| intent.destination)
    }

    pub fn pending(&self) -> Option<&LeaveIntent> {
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
        assert_eq!(guard.save_settled(&s, ticket), Some("/ban-viet".into()));
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
            guard.save_settled(&s, ticket),
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
        assert_eq!(guard.save_settled(&s, ticket), None);
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
        assert_eq!(guard.save_settled(&s, ticket), None);
        assert_eq!(
            stage(&guard),
            Some(LeaveStage::Choosing {
                notice: Some(LeaveNotice::NewerEdits)
            })
        );
        let next = guard.save_and_leave(&mut s).unwrap();
        assert_eq!(next.expected_version, 2);
        s.save_succeeded(next, 3);
        assert_eq!(guard.save_settled(&s, next), Some("/".into()));
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
        assert_eq!(guard.save_settled(&s, first), Some("/".into()));
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
            assert_eq!(guard.save_settled(&s, ticket), None);
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
        assert_eq!(guard.save_settled(&s, ticket), None, "no second navigation");
    }
}
