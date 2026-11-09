//! Draft-tag controller state (TAG-PR-05), kept free of DOM and transport code.
//!
//! The author's selection is local until the server confirms the exact selection that was sent:
//! a ticket remembers what it submitted, so an acknowledgement of an older selection never marks
//! a newer one saved. The server stays the authority for catalog, cardinality and ownership; the
//! limits here only shape the view (`MAX_THEMES` mirrors the contract and the server refuses
//! anyway). Recovery follows the TAG-PR-03 contract (desired state + `expected_version`, `409`
//! carrying the stored tags): a conflict whose stored tags equal what this tab asked for is its
//! own write (a lost acknowledgement); anything else is shown to the author, never overwritten.

use crate::contract::{TagRefusal, TagRefusalCode};
use crate::desk_state::Failure;

/// At most three distinct themes per work (`too_many_themes` otherwise).
pub const MAX_THEMES: usize = 3;

/// Selected tag identities: one optional form, distinct themes in first-seen order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    pub form: Option<String>,
    pub themes: Vec<String>,
}

impl Selection {
    pub fn new(form: Option<String>, themes: impl IntoIterator<Item = String>) -> Self {
        let mut distinct: Vec<String> = Vec::new();
        for theme in themes {
            if !distinct.contains(&theme) {
                distinct.push(theme);
            }
        }
        Self {
            form,
            themes: distinct,
        }
    }

    pub fn contains(&self, tag_id: &str) -> bool {
        self.form.as_deref() == Some(tag_id) || self.themes.iter().any(|t| t == tag_id)
    }
}

/// Why a local edit was not applied; the server would refuse it as well.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditRefused {
    AlreadySelected,
    ThemesFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TagTicket {
    pub expected_version: i64,
    request: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagRejection {
    /// `409 version_conflict` with the stored tags.
    Conflict {
        current_version: i64,
        stored: Selection,
    },
    Refused(TagRefusal),
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagStatus {
    Saved {
        version: i64,
    },
    Unsaved,
    Saving,
    /// Another save won; the author chooses between the stored tags and their own.
    Conflict {
        server_version: i64,
        stored: Selection,
    },
    Refused(TagRefusal),
    Failed(Failure),
}

impl TagStatus {
    /// Stable token for `data-status` (styles and browser checks).
    pub fn token(&self) -> &'static str {
        match self {
            Self::Saved { .. } => "saved",
            Self::Unsaved => "unsaved",
            Self::Saving => "saving",
            Self::Conflict { .. } => "conflict",
            Self::Refused(_) => "refused",
            Self::Failed(_) => "failed",
        }
    }
}

/// A missing or ambiguous answer: the save may have committed.
pub fn uncertain(failure: Failure) -> bool {
    matches!(
        failure,
        Failure::Network | Failure::Unavailable | Failure::Unexpected
    )
}

/// Selections remembered while their outcome is unknown; enough for repeated retries.
const UNCERTAIN_KEPT: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagSession {
    base_version: i64,
    saved: Selection,
    current: Selection,
    requests: u64,
    in_flight: Option<(TagTicket, Selection)>,
    /// Sent selections whose outcome is unknown since the last confirmed version.
    unconfirmed: Vec<Selection>,
    conflict: Option<(i64, Selection)>,
    refusal: Option<TagRefusal>,
    failure: Option<Failure>,
}

impl TagSession {
    pub fn loaded(version: i64, selection: Selection) -> Self {
        Self {
            base_version: version,
            saved: selection.clone(),
            current: selection,
            requests: 0,
            in_flight: None,
            unconfirmed: Vec::new(),
            conflict: None,
            refusal: None,
            failure: None,
        }
    }

    pub fn current(&self) -> &Selection {
        &self.current
    }

    pub fn saved(&self) -> &Selection {
        &self.saved
    }

    pub fn choose_form(&mut self, form: Option<String>) {
        if self.current.form != form {
            self.current.form = form;
            self.edited();
        }
    }

    pub fn add_theme(&mut self, tag_id: &str) -> Result<(), EditRefused> {
        if self.current.themes.iter().any(|t| t == tag_id) {
            return Err(EditRefused::AlreadySelected);
        }
        if self.current.themes.len() >= MAX_THEMES {
            return Err(EditRefused::ThemesFull);
        }
        self.current.themes.push(tag_id.to_string());
        self.edited();
        Ok(())
    }

    pub fn remove_theme(&mut self, tag_id: &str) {
        let before = self.current.themes.len();
        self.current.themes.retain(|t| t != tag_id);
        if self.current.themes.len() != before {
            self.edited();
        }
    }

    /// Replaces a theme in place (its position is kept), e.g. a retired tag by its successor.
    pub fn replace_theme(&mut self, old: &str, new: &str) {
        if self.current.themes.iter().any(|t| t == new) {
            self.remove_theme(old);
            return;
        }
        if let Some(slot) = self.current.themes.iter_mut().find(|t| *t == old) {
            *slot = new.to_string();
            self.edited();
        }
    }

    /// Back to the last confirmed tags. A save in flight still settles normally.
    pub fn revert(&mut self) {
        if self.current != self.saved {
            self.current = self.saved.clone();
            self.edited();
        }
    }

    /// A refusal names a tag; once that tag is gone (or the themes fit again) it no longer
    /// describes the selection. Failures stay until the next save, like text failures.
    fn edited(&mut self) {
        let stale = match &self.refusal {
            Some(refusal) => match (refusal.code, refusal.tag_id.as_deref()) {
                (TagRefusalCode::TooManyThemes, _) => self.current.themes.len() <= MAX_THEMES,
                (_, Some(tag_id)) => !self.current.contains(tag_id),
                (_, None) => true,
            },
            None => false,
        };
        if stale {
            self.refusal = None;
        }
    }

    /// Starts a save of the current selection, unless one is in flight or a conflict is open.
    pub fn begin_save(&mut self) -> Option<TagTicket> {
        if !self.can_save() {
            return None;
        }
        self.requests += 1;
        let ticket = TagTicket {
            expected_version: self.base_version,
            request: self.requests,
        };
        self.in_flight = Some((ticket, self.current.clone()));
        self.refusal = None;
        self.failure = None;
        Some(ticket)
    }

    /// The exact selection a ticket carries; the request body is built from it.
    pub fn submitted(&self, ticket: TagTicket) -> Option<&Selection> {
        self.in_flight
            .as_ref()
            .filter(|(t, _)| *t == ticket)
            .map(|(_, s)| s)
    }

    /// `saved` is what the server answered it committed.
    pub fn save_succeeded(&mut self, ticket: TagTicket, version: i64, saved: Selection) {
        if self.in_flight.as_ref().map(|(t, _)| *t) != Some(ticket) {
            return;
        }
        self.in_flight = None;
        self.adopt(version, saved);
    }

    pub fn save_rejected(&mut self, ticket: TagTicket, rejection: TagRejection) {
        let Some((sent, submitted)) = self.in_flight.take() else {
            return;
        };
        if sent != ticket {
            self.in_flight = Some((sent, submitted));
            return;
        }
        match rejection {
            TagRejection::Conflict {
                current_version,
                stored,
            } => {
                if stored == submitted || self.unconfirmed.contains(&stored) {
                    // The server holds a selection this tab asked for: its version is ours to
                    // continue from. A newer local choice stays unsaved, it is not resent here.
                    self.adopt(current_version, stored);
                } else {
                    self.conflict = Some((current_version, stored));
                }
            }
            TagRejection::Refused(refusal) => self.refusal = Some(refusal),
            TagRejection::Failed(failure) => {
                if uncertain(failure) {
                    self.unconfirmed.push(submitted);
                    if self.unconfirmed.len() > UNCERTAIN_KEPT {
                        self.unconfirmed.remove(0);
                    }
                }
                self.failure = Some(failure);
            }
        }
    }

    fn adopt(&mut self, version: i64, saved: Selection) {
        self.base_version = version;
        self.saved = saved;
        self.unconfirmed.clear();
        self.conflict = None;
    }

    /// "Dùng thẻ đang lưu": the stored tags replace the local choice.
    pub fn use_stored(&mut self) {
        if let Some((version, stored)) = self.conflict.take() {
            self.current = stored.clone();
            self.adopt(version, stored);
            self.edited();
        }
    }

    /// "Giữ lựa chọn của tôi": the next save targets the stored version, by the author's choice.
    pub fn keep_mine(&mut self) {
        if let Some((version, stored)) = self.conflict.take() {
            self.adopt(version, stored);
        }
    }

    pub fn status(&self) -> TagStatus {
        if self.in_flight.is_some() {
            TagStatus::Saving
        } else if let Some((server_version, stored)) = &self.conflict {
            TagStatus::Conflict {
                server_version: *server_version,
                stored: stored.clone(),
            }
        } else if let Some(refusal) = &self.refusal {
            TagStatus::Refused(refusal.clone())
        } else if let Some(failure) = self.failure {
            TagStatus::Failed(failure)
        } else if self.current == self.saved {
            TagStatus::Saved {
                version: self.base_version,
            }
        } else {
            TagStatus::Unsaved
        }
    }

    /// Tags the server has not confirmed: a different local choice or an open conflict.
    pub fn has_unsaved(&self) -> bool {
        self.current != self.saved || self.conflict.is_some()
    }

    pub fn can_save(&self) -> bool {
        self.in_flight.is_none() && self.conflict.is_none()
    }

    pub fn in_flight(&self) -> Option<TagTicket> {
        self.in_flight.as_ref().map(|(t, _)| *t)
    }

    /// The tag a refusal points at, so the view can mark that chip.
    pub fn refused_tag(&self) -> Option<&TagRefusal> {
        self.refusal.as_ref()
    }
}

/// Suggestions for the raw query typed in the theme field.
///
/// The raw text is kept as typed, through IME composition; a request starts only for a settled,
/// non-blank value. Every request has a generation and the answer must also echo the same query,
/// so an older answer can never replace the suggestions of newer input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryState<T = String> {
    raw: String,
    generation: u64,
    phase: QueryPhase<T>,
}

impl<T> Default for QueryState<T> {
    fn default() -> Self {
        Self {
            raw: String::new(),
            generation: 0,
            phase: QueryPhase::Idle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryPhase<T = String> {
    /// Nothing to look up: the field is blank.
    Idle,
    /// The typed text is still composing; the last settled answer is not shown for it.
    Composing,
    Loading,
    Ready(Vec<T>),
    /// The server refused the query (`422`), e.g. no letter or digit.
    Invalid,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRequest {
    pub generation: u64,
    pub q: String,
}

impl<T> QueryState<T> {
    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn phase(&self) -> &QueryPhase<T> {
        &self.phase
    }

    /// One edit of the field. Returns the request to send, if any.
    pub fn input(&mut self, raw: &str, composing: bool) -> Option<QueryRequest> {
        self.raw = raw.to_string();
        self.generation += 1;
        if composing {
            self.phase = QueryPhase::Composing;
            return None;
        }
        let q = raw.trim();
        if q.is_empty() {
            self.phase = QueryPhase::Idle;
            return None;
        }
        self.phase = QueryPhase::Loading;
        Some(QueryRequest {
            generation: self.generation,
            q: q.to_string(),
        })
    }

    /// The field was cleared after a tag was added from it.
    pub fn clear(&mut self) {
        self.raw.clear();
        self.generation += 1;
        self.phase = QueryPhase::Idle;
    }

    /// Applies an answer only when it belongs to the latest request and echoes its query.
    /// `Ok` holds the echoed query and the suggestions in server order.
    pub fn settle(
        &mut self,
        request: &QueryRequest,
        answer: Result<(String, Vec<T>), QueryFailure>,
    ) -> bool {
        if request.generation != self.generation {
            return false;
        }
        self.phase = match answer {
            Ok((echo, ids)) if echo == request.q => QueryPhase::Ready(ids),
            Ok(_) => return false,
            Err(QueryFailure::Invalid) => QueryPhase::Invalid,
            Err(QueryFailure::Unavailable) => QueryPhase::Unavailable,
        };
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryFailure {
    Invalid,
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(form: Option<&str>, themes: &[&str]) -> Selection {
        Selection::new(
            form.map(str::to_string),
            themes.iter().map(|t| t.to_string()),
        )
    }

    fn refusal(code: TagRefusalCode, tag: Option<&str>) -> TagRefusal {
        TagRefusal {
            code,
            field: Some("themes".into()),
            tag_id: tag.map(str::to_string),
            successor_id: None,
        }
    }

    #[test]
    fn a_metadata_only_change_is_unsaved_until_the_exact_selection_is_acknowledged() {
        let mut s = TagSession::loaded(4, sel(Some("tan-van"), &["ky-uc"]));
        assert_eq!(s.status(), TagStatus::Saved { version: 4 });
        assert!(!s.has_unsaved());
        s.add_theme("gia-dinh").unwrap();
        assert_eq!(s.status(), TagStatus::Unsaved);
        assert!(s.has_unsaved(), "tag-only edits are protected on leave");
        let ticket = s.begin_save().unwrap();
        assert_eq!(ticket.expected_version, 4);
        assert_eq!(
            s.submitted(ticket),
            Some(&sel(Some("tan-van"), &["ky-uc", "gia-dinh"]))
        );
        s.save_succeeded(ticket, 5, sel(Some("tan-van"), &["ky-uc", "gia-dinh"]));
        assert_eq!(s.status(), TagStatus::Saved { version: 5 });
        assert!(!s.has_unsaved());
    }

    #[test]
    fn choosing_back_the_saved_tags_is_not_a_change() {
        let mut s = TagSession::loaded(1, sel(None, &["ky-uc"]));
        s.remove_theme("ky-uc");
        assert!(s.has_unsaved());
        s.add_theme("ky-uc").unwrap();
        assert!(!s.has_unsaved());
        s.choose_form(Some("tho".into()));
        s.revert();
        assert_eq!(s.current(), &sel(None, &["ky-uc"]));
        assert_eq!(s.status(), TagStatus::Saved { version: 1 });
    }

    #[test]
    fn a_late_acknowledgement_does_not_mark_a_newer_selection_saved() {
        let mut s = TagSession::loaded(7, Selection::default());
        s.add_theme("mua").unwrap();
        let ticket = s.begin_save().unwrap();
        s.add_theme("ky-uc").unwrap(); // chosen while version 7 → 8 was in flight
        s.save_succeeded(ticket, 8, sel(None, &["mua"]));
        assert_eq!(s.status(), TagStatus::Unsaved);
        assert_eq!(s.saved(), &sel(None, &["mua"]));
        let next = s.begin_save().unwrap();
        assert_eq!(next.expected_version, 8);
        assert_eq!(s.submitted(next), Some(&sel(None, &["mua", "ky-uc"])));
    }

    #[test]
    fn one_save_in_flight_and_stale_or_duplicate_outcomes_are_ignored() {
        let mut s = TagSession::loaded(1, Selection::default());
        s.choose_form(Some("tho".into()));
        let first = s.begin_save().unwrap();
        assert_eq!(s.begin_save(), None);
        s.save_rejected(first, TagRejection::Failed(Failure::Unavailable));
        assert_eq!(s.status(), TagStatus::Failed(Failure::Unavailable));
        let second = s.begin_save().unwrap();
        // A duplicate delivery of the first outcome cannot settle the second request.
        s.save_succeeded(first, 2, sel(Some("tho"), &[]));
        s.save_rejected(first, TagRejection::Failed(Failure::Network));
        assert_eq!(s.status(), TagStatus::Saving);
        s.save_succeeded(second, 2, sel(Some("tho"), &[]));
        assert_eq!(s.status(), TagStatus::Saved { version: 2 });
    }

    #[test]
    fn a_lost_acknowledgement_is_reconciled_by_the_stored_tags_not_by_a_blind_resend() {
        // Contract TEACH 4: v3 committed as v4, the answer was lost; the retry keeps v3.
        let mut s = TagSession::loaded(3, Selection::default());
        s.add_theme("ky-uc").unwrap();
        let lost = s.begin_save().unwrap();
        s.save_rejected(lost, TagRejection::Failed(Failure::Network));
        assert!(s.has_unsaved(), "uncertain is not saved");
        let retry = s.begin_save().unwrap();
        assert_eq!(retry.expected_version, 3, "never refreshed silently");
        s.save_rejected(
            retry,
            TagRejection::Conflict {
                current_version: 4,
                stored: sel(None, &["ky-uc"]),
            },
        );
        assert_eq!(s.status(), TagStatus::Saved { version: 4 });
    }

    #[test]
    fn a_lost_write_followed_by_a_newer_choice_keeps_the_newer_choice_unsaved() {
        let mut s = TagSession::loaded(3, Selection::default());
        s.add_theme("ky-uc").unwrap();
        let lost = s.begin_save().unwrap();
        s.save_rejected(lost, TagRejection::Failed(Failure::Unexpected));
        s.add_theme("mua").unwrap();
        let retry = s.begin_save().unwrap();
        // The stored tags are this tab's earlier (unconfirmed) choice.
        s.save_rejected(
            retry,
            TagRejection::Conflict {
                current_version: 4,
                stored: sel(None, &["ky-uc"]),
            },
        );
        assert_eq!(s.status(), TagStatus::Unsaved, "not resent automatically");
        assert_eq!(s.saved(), &sel(None, &["ky-uc"]));
        assert_eq!(s.begin_save().unwrap().expected_version, 4);
    }

    #[test]
    fn another_tabs_tags_are_shown_and_never_overwritten_without_a_choice() {
        let mut s = TagSession::loaded(2, Selection::default());
        s.add_theme("ky-uc").unwrap();
        let ticket = s.begin_save().unwrap();
        let stored = sel(None, &["mua"]);
        s.save_rejected(
            ticket,
            TagRejection::Conflict {
                current_version: 3,
                stored: stored.clone(),
            },
        );
        assert_eq!(
            s.status(),
            TagStatus::Conflict {
                server_version: 3,
                stored: stored.clone()
            }
        );
        assert!(s.has_unsaved());
        assert_eq!(s.begin_save(), None, "no silent overwrite");
        // Choosing while in conflict keeps the conflict open.
        s.add_theme("gia-dinh").unwrap();
        assert!(matches!(s.status(), TagStatus::Conflict { .. }));

        let mut adopt = s.clone();
        adopt.use_stored();
        assert_eq!(adopt.current(), &stored);
        assert_eq!(adopt.status(), TagStatus::Saved { version: 3 });

        s.keep_mine();
        assert_eq!(s.status(), TagStatus::Unsaved);
        let overwrite = s.begin_save().unwrap();
        assert_eq!(overwrite.expected_version, 3);
        assert_eq!(
            s.submitted(overwrite),
            Some(&sel(None, &["ky-uc", "gia-dinh"]))
        );
    }

    #[test]
    fn a_definitive_refusal_is_not_remembered_as_a_possible_write() {
        let mut s = TagSession::loaded(2, Selection::default());
        s.add_theme("ky-uc").unwrap();
        let ticket = s.begin_save().unwrap();
        s.save_rejected(ticket, TagRejection::Failed(Failure::Unauthenticated));
        s.remove_theme("ky-uc");
        s.add_theme("mua").unwrap();
        let retry = s.begin_save().unwrap();
        s.save_rejected(
            retry,
            TagRejection::Conflict {
                current_version: 3,
                stored: sel(None, &["ky-uc"]),
            },
        );
        assert!(
            matches!(s.status(), TagStatus::Conflict { .. }),
            "a 401 wrote nothing; equal tags came from elsewhere"
        );
    }

    #[test]
    fn a_retired_or_unknown_tag_keeps_the_selection_until_the_author_acts() {
        let mut s = TagSession::loaded(1, sel(None, &["ky-uc"]));
        s.add_theme("do-thi").unwrap();
        let ticket = s.begin_save().unwrap();
        let mut retired = refusal(TagRefusalCode::Retired, Some("do-thi"));
        retired.successor_id = Some("mua".into());
        s.save_rejected(ticket, TagRejection::Refused(retired.clone()));
        assert_eq!(s.status(), TagStatus::Refused(retired));
        assert!(s.current().contains("do-thi"), "never removed silently");
        // Another edit that keeps the refused tag keeps the explanation.
        s.choose_form(Some("tho".into()));
        assert!(matches!(s.status(), TagStatus::Refused(_)));
        s.replace_theme("do-thi", "mua");
        assert_eq!(s.current().themes, vec!["ky-uc".to_string(), "mua".into()]);
        assert_eq!(s.status(), TagStatus::Unsaved);

        let ticket = s.begin_save().unwrap();
        s.save_rejected(
            ticket,
            TagRejection::Refused(refusal(TagRefusalCode::UnknownTag, Some("mua"))),
        );
        s.remove_theme("mua");
        assert!(s.refused_tag().is_none(), "the refused tag is gone");
        assert_eq!(
            s.status(),
            TagStatus::Unsaved,
            "the form change is still unsaved"
        );
    }

    #[test]
    fn too_many_themes_clears_once_the_themes_fit() {
        let mut s = TagSession::loaded(1, Selection::default());
        let ticket = s.begin_save().unwrap();
        s.save_rejected(
            ticket,
            TagRejection::Refused(refusal(TagRefusalCode::TooManyThemes, None)),
        );
        assert!(matches!(s.status(), TagStatus::Refused(_)));
        s.choose_form(Some("tho".into()));
        assert_eq!(s.status(), TagStatus::Unsaved);
    }

    #[test]
    fn local_limits_mirror_the_contract_and_the_server_still_decides() {
        let mut s = TagSession::loaded(1, Selection::default());
        for theme in ["a", "b", "c"] {
            s.add_theme(theme).unwrap();
        }
        assert_eq!(s.add_theme("d"), Err(EditRefused::ThemesFull));
        assert_eq!(s.add_theme("a"), Err(EditRefused::AlreadySelected));
        s.replace_theme("b", "c");
        assert_eq!(s.current().themes, vec!["a".to_string(), "c".into()]);
        assert_eq!(
            Selection::new(None, ["x".to_string(), "y".into(), "x".into()]).themes,
            vec!["x".to_string(), "y".into()]
        );
    }

    #[test]
    fn composition_keeps_the_raw_query_and_requests_only_settled_text() {
        let mut q = QueryState::<String>::default();
        assert_eq!(q.input("h", true), None);
        assert_eq!(q.input("hô", true), None);
        assert_eq!(q.phase(), &QueryPhase::Composing);
        assert_eq!(q.raw(), "hô");
        let request = q.input("hồi", false).unwrap();
        assert_eq!(request.q, "hồi");
        assert_eq!(q.phase(), &QueryPhase::Loading);
        assert_eq!(q.input("   ", false), None);
        assert_eq!(q.phase(), &QueryPhase::Idle);
        assert_eq!(q.raw(), "   ", "raw text is never rewritten");
    }

    #[test]
    fn an_older_answer_never_replaces_newer_suggestions() {
        let mut q = QueryState::<String>::default();
        let old = q.input("h", false).unwrap();
        let new = q.input("hồi", false).unwrap();
        assert!(!q.settle(&old, Ok(("h".into(), vec!["hoi-hoa".into()]))));
        assert_eq!(q.phase(), &QueryPhase::Loading);
        // An answer that echoes another query is not this request's answer.
        assert!(!q.settle(&new, Ok(("h".into(), vec!["x".into()]))));
        assert!(q.settle(&new, Ok(("hồi".into(), vec!["ky-uc".into()]))));
        assert_eq!(q.phase(), &QueryPhase::Ready(vec!["ky-uc".into()]));
        let again = q.input("hồi ", false).unwrap();
        q.clear();
        assert!(!q.settle(&again, Err(QueryFailure::Unavailable)));
        assert_eq!(q.phase(), &QueryPhase::Idle);
        let bad = q.input("!!", false).unwrap();
        assert!(q.settle(&bad, Err(QueryFailure::Invalid)));
        assert_eq!(q.phase(), &QueryPhase::Invalid);
    }
}
