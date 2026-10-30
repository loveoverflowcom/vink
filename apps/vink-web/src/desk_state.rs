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
}
