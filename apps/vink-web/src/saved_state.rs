//! Pure Reader “Lưu” state (#388, SAV-04, D2/D11). No DOM, HTTP or storage runs here.
//!
//! One owner per (session, work): the browser adapter reads the viewer's own membership after
//! hydration and sends desired states with the generation it last confirmed. Rules:
//! - Nothing is optimistic. The shown membership changes only with an answer of the server
//!   (ACK or read), and a newer generation always wins over an older one (SAV-INV-04).
//! - Every request carries the scope it was made in. A session change (A → B, sign-out) starts
//!   a new scope: confirmed state, outcome and in-flight answers of the old one are dropped.
//! - A lost answer is uncertain: the known state stays, presses wait for “Kiểm tra lại”, and
//!   the intent is never replayed. A refused intent (409 conflict) reads the pair again.
//! - Membership never reaches SSR, the public cache or storage; it lives in this value only.

use crate::contract::{ApiError, SavedWorkState};
use vink_patterns::{SaveFacts, SaveOutcome, SaveRefusal};
use vink_ui::{TogglePhase, ToggleValue};

/// What the session owner confirmed about the viewer, as far as saving is concerned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionView {
    /// Not asked yet, or a session change is being confirmed: hold quietly.
    Unknown,
    /// The first session check failed: nothing is known, “Kiểm tra lại” asks again.
    Unavailable,
    /// No account: a press opens the Reader sign-in sheet (D2).
    Guest,
    /// Signed in without a pen name: the same sheet continues to the pen name.
    NeedsPenName,
    /// This account cannot save; the reason is shown, never hover-only.
    Blocked(&'static str),
    /// An active principal; `scope` is `principal/generation` from the session owner.
    Active { scope: String },
}

/// What a press asks the adapter to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Press {
    Ignore,
    /// Open the sign-in sheet; nothing is saved on the guest's behalf.
    SignIn,
    Send(SaveCommand),
}

/// What a finished request asks the adapter to do next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Next {
    Nothing,
    /// The intent was refused by the fence: read the pair again before any new intent.
    Read(ReadTicket),
    /// The server no longer accepts this session; the tab's session owner checks again.
    RefreshSession,
}

/// One private read, valid only for the scope and serial it was issued in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadTicket {
    scope: String,
    serial: u64,
}

/// One desired-state request (`PUT` saves, `DELETE` unsaves) with the confirmed generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveCommand {
    scope: String,
    serial: u64,
    pub desired: bool,
    pub expected_generation: i64,
    /// The principal that started the intent (`X-Vink-Expected-Principal`).
    pub principal: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Pending {
        desired: bool,
    },
    /// A 409 was answered; the pair is being read before the outcome is known.
    Conflict {
        desired: bool,
    },
    Uncertain {
        desired: bool,
    },
    Failed(SaveRefusal),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveController {
    work_id: String,
    session: SessionView,
    confirmed: Option<SavedWorkState>,
    phase: Phase,
    reading: Option<u64>,
    in_flight: Option<u64>,
    serial: u64,
    outcome: Option<SaveOutcome>,
    list_href: Option<String>,
    sign_in_hint: bool,
}

const fn refusal(desired: bool) -> SaveRefusal {
    if desired {
        SaveRefusal::Save
    } else {
        SaveRefusal::Unsave
    }
}

impl SaveController {
    pub fn new(work_id: impl Into<String>) -> Self {
        Self {
            work_id: work_id.into(),
            session: SessionView::Unknown,
            confirmed: None,
            phase: Phase::Idle,
            reading: None,
            in_flight: None,
            serial: 0,
            outcome: None,
            list_href: None,
            sign_in_hint: false,
        }
    }

    pub fn work_id(&self) -> &str {
        &self.work_id
    }

    /// `principal/generation` of the active session this state belongs to.
    pub fn scope(&self) -> Option<&str> {
        match &self.session {
            SessionView::Active { scope } => Some(scope),
            _ => None,
        }
    }

    fn principal(&self) -> Option<&str> {
        let scope = self.scope()?;
        Some(
            scope
                .split_once('/')
                .map_or(scope, |(principal, _)| principal),
        )
    }

    /// Applies the session owner's view. A different scope drops every private fact and
    /// in-flight answer of the previous one; an active scope starts its first read.
    pub fn change_session(&mut self, next: SessionView) -> Option<ReadTicket> {
        if next == self.session {
            return None;
        }
        let hint = self.sign_in_hint;
        *self = Self::new(std::mem::take(&mut self.work_id));
        // A guest who opened the sheet from “Lưu” keeps the guidance across the sign-in only.
        self.sign_in_hint =
            hint && !matches!(next, SessionView::Blocked(_) | SessionView::Unavailable);
        self.session = next;
        self.begin_read()
    }

    /// The projection `SaveAction` renders.
    pub fn facts(&self) -> SaveFacts {
        let mut facts = SaveFacts::default();
        match &self.session {
            SessionView::Unknown => facts.reading = true,
            SessionView::Unavailable => {}
            SessionView::Guest | SessionView::NeedsPenName => facts.value = ToggleValue::Off,
            SessionView::Blocked(reason) => facts.blocked = Some(reason),
            SessionView::Active { .. } => {
                facts.value = self.confirmed.map_or(ToggleValue::Unknown, |c| {
                    ToggleValue::from_selected(c.saved)
                });
                facts.reading = self.reading.is_some();
                facts.phase = match self.phase {
                    Phase::Idle => TogglePhase::Idle,
                    Phase::Pending { desired } | Phase::Conflict { desired } => {
                        TogglePhase::Pending { desired }
                    }
                    Phase::Uncertain { desired } => TogglePhase::Uncertain { desired },
                    Phase::Failed(reason) => {
                        facts.refusal = Some(reason);
                        TogglePhase::Failed
                    }
                };
                facts.outcome = self.outcome.clone();
            }
        }
        facts
    }

    /// A press toward `desired`, from the confirmed state only.
    pub fn press(&mut self, desired: bool) -> Press {
        match &self.session {
            SessionView::Guest | SessionView::NeedsPenName => return Press::SignIn,
            SessionView::Active { .. } => {}
            _ => return Press::Ignore,
        }
        let Some(confirmed) = self.confirmed else {
            return Press::Ignore;
        };
        if confirmed.saved == desired
            || self.in_flight.is_some()
            || matches!(
                self.phase,
                Phase::Pending { .. } | Phase::Conflict { .. } | Phase::Uncertain { .. }
            )
        {
            return Press::Ignore;
        }
        let (Some(scope), Some(principal)) = (
            self.scope().map(str::to_owned),
            self.principal().map(str::to_owned),
        ) else {
            return Press::Ignore;
        };
        // A background refresh never blocks a press: its answer is dropped, the ACK decides.
        self.reading = None;
        self.serial += 1;
        self.in_flight = Some(self.serial);
        self.phase = Phase::Pending { desired };
        self.outcome = None;
        self.sign_in_hint = false;
        Press::Send(SaveCommand {
            scope,
            serial: self.serial,
            desired,
            expected_generation: confirmed.generation,
            principal,
        })
    }

    /// Accepts a membership only if it is not older than the one already confirmed.
    fn confirm(&mut self, state: SavedWorkState) {
        if self
            .confirmed
            .is_none_or(|known| state.generation >= known.generation)
        {
            if self
                .confirmed
                .is_some_and(|known| known.saved != state.saved)
            {
                self.outcome = None;
            }
            self.confirmed = Some(state);
        }
    }

    fn saved_outcome(&self, saved: bool) -> SaveOutcome {
        if saved {
            SaveOutcome::Saved {
                list_href: self.list_href.clone(),
            }
        } else {
            SaveOutcome::Unsaved
        }
    }

    /// The answer to `command`. Answers of another scope or an older request are ignored.
    pub fn complete(
        &mut self,
        command: &SaveCommand,
        result: Result<SavedWorkState, ApiError>,
    ) -> Next {
        if self.scope() != Some(command.scope.as_str()) || self.in_flight != Some(command.serial) {
            return Next::Nothing;
        }
        self.in_flight = None;
        let desired = command.desired;
        match result {
            Ok(state) => {
                self.confirm(state);
                let saved = self.confirmed.is_some_and(|c| c.saved);
                if saved == desired {
                    self.phase = Phase::Idle;
                    self.outcome = Some(self.saved_outcome(saved));
                } else {
                    self.phase = Phase::Failed(refusal(desired));
                }
                Next::Nothing
            }
            Err(ApiError::GenerationConflict { .. } | ApiError::VersionConflict { .. }) => {
                self.phase = Phase::Conflict { desired };
                self.begin_read().map_or(Next::Nothing, Next::Read)
            }
            Err(ApiError::SavedWorksLimit) => {
                self.phase = Phase::Failed(SaveRefusal::Limit);
                Next::Nothing
            }
            Err(ApiError::Unauthenticated | ApiError::Forbidden) => {
                self.phase = Phase::Failed(refusal(desired));
                Next::RefreshSession
            }
            Err(ApiError::Network | ApiError::Unavailable) => {
                self.phase = Phase::Uncertain { desired };
                Next::Nothing
            }
            Err(ApiError::Unexpected { status }) if status == 0 || status >= 500 => {
                self.phase = Phase::Uncertain { desired };
                Next::Nothing
            }
            Err(_) => {
                self.phase = Phase::Failed(refusal(desired));
                Next::Nothing
            }
        }
    }

    /// Starts a read of the pair for the active scope, unless a write is in flight.
    pub fn begin_read(&mut self) -> Option<ReadTicket> {
        let scope = self.scope()?.to_owned();
        if self.in_flight.is_some() {
            return None;
        }
        self.serial += 1;
        self.reading = Some(self.serial);
        Some(ReadTicket {
            scope,
            serial: self.serial,
        })
    }

    pub fn apply_read(
        &mut self,
        ticket: &ReadTicket,
        result: Result<SavedWorkState, ApiError>,
    ) -> Next {
        if self.scope() != Some(ticket.scope.as_str()) || self.reading != Some(ticket.serial) {
            return Next::Nothing;
        }
        self.reading = None;
        match result {
            Ok(state) => {
                self.confirm(state);
                let saved = self.confirmed.is_some_and(|c| c.saved);
                match self.phase {
                    // The intent was fenced or its answer was lost: the read is the truth now.
                    Phase::Conflict { desired } | Phase::Uncertain { desired } => {
                        if saved == desired {
                            self.phase = Phase::Idle;
                            self.outcome = Some(self.saved_outcome(saved));
                        } else {
                            self.phase = Phase::Failed(refusal(desired));
                        }
                    }
                    _ => {}
                }
                if std::mem::take(&mut self.sign_in_hint) && !saved {
                    self.outcome = Some(SaveOutcome::SignedIn);
                }
                Next::Nothing
            }
            Err(error) => {
                if let Phase::Conflict { desired } = self.phase {
                    self.phase = Phase::Uncertain { desired };
                }
                if matches!(error, ApiError::Unauthenticated | ApiError::Forbidden) {
                    Next::RefreshSession
                } else {
                    Next::Nothing
                }
            }
        }
    }

    /// “Kiểm tra lại”: read the pair when its state is uncertain or unknown. Never replays.
    pub fn recover(&mut self) -> Option<ReadTicket> {
        if self.reading.is_some()
            || !(matches!(self.phase, Phase::Uncertain { .. }) || self.confirmed.is_none())
        {
            return None;
        }
        self.begin_read()
    }

    /// A background refresh (tab shown again, history restored): only while nothing is pending.
    pub fn refresh(&mut self) -> Option<ReadTicket> {
        if self.reading.is_some()
            || !matches!(self.phase, Phase::Idle | Phase::Failed(_))
            || self.in_flight.is_some()
        {
            return None;
        }
        self.begin_read()
    }

    /// The guest asked to save from the sheet; the next read after sign-in may say so (D2).
    pub fn arm_sign_in_hint(&mut self) {
        self.sign_in_hint = true;
    }

    pub fn disarm_sign_in_hint(&mut self) {
        self.sign_in_hint = false;
    }

    /// The viewer's own Đã lưu list, once their handle is known for this scope.
    pub fn set_list_href(&mut self, scope: &str, href: Option<String>) {
        if self.scope() != Some(scope) {
            return;
        }
        self.list_href = href;
        if let Some(SaveOutcome::Saved { list_href }) = &mut self.outcome {
            list_href.clone_from(&self.list_href);
        }
    }

    pub fn has_list_href(&self) -> bool {
        self.list_href.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "11111111-1111-4111-8111-111111111111/0";
    const B: &str = "22222222-2222-4222-8222-222222222222/1";

    fn state(saved: bool, generation: i64) -> SavedWorkState {
        SavedWorkState { saved, generation }
    }

    fn active(scope: &str) -> SessionView {
        SessionView::Active {
            scope: scope.into(),
        }
    }

    fn signed_in(scope: &str, initial: SavedWorkState) -> SaveController {
        let mut c = SaveController::new("w");
        let read = c.change_session(active(scope)).expect("first read");
        assert!(c.facts().reading && c.facts().value == ToggleValue::Unknown);
        assert_eq!(c.apply_read(&read, Ok(initial)), Next::Nothing);
        c
    }

    fn send(c: &mut SaveController, desired: bool) -> SaveCommand {
        match c.press(desired) {
            Press::Send(command) => command,
            other => panic!("expected a command, got {other:?}"),
        }
    }

    #[test]
    fn ssr_and_unknown_sessions_hold_quietly_and_guests_open_the_sheet() {
        let mut c = SaveController::new("w");
        let facts = c.facts();
        assert_eq!(facts.value, ToggleValue::Unknown);
        assert!(facts.reading, "SSR/unknown: no note, no claim");
        assert_eq!(c.press(true), Press::Ignore);
        assert_eq!(c.change_session(SessionView::Guest), None);
        assert_eq!(c.facts().value, ToggleValue::Off);
        assert_eq!(c.press(true), Press::SignIn, "D2: the sheet, never a write");
        assert_eq!(c.change_session(SessionView::NeedsPenName), None);
        assert_eq!(c.press(true), Press::SignIn);
        c.change_session(SessionView::Blocked("x"));
        assert_eq!(c.facts().blocked, Some("x"));
        assert_eq!(c.press(true), Press::Ignore);
    }

    #[test]
    fn a_press_waits_for_the_ack_and_never_changes_the_state_first() {
        let mut c = signed_in(A, state(false, 1));
        let command = send(&mut c, true);
        assert_eq!(command.expected_generation, 1);
        assert_eq!(command.principal, "11111111-1111-4111-8111-111111111111");
        let facts = c.facts();
        assert_eq!(
            facts.value,
            ToggleValue::Off,
            "pending keeps the confirmed value"
        );
        assert_eq!(facts.phase, TogglePhase::Pending { desired: true });
        assert_eq!(
            c.press(true),
            Press::Ignore,
            "no second request while pending"
        );
        assert_eq!(c.press(false), Press::Ignore);
        assert_eq!(c.complete(&command, Ok(state(true, 2))), Next::Nothing);
        let facts = c.facts();
        assert_eq!(facts.value, ToggleValue::On);
        assert_eq!(facts.phase, TogglePhase::Idle);
        assert_eq!(facts.outcome, Some(SaveOutcome::Saved { list_href: None }));
        let unsave = send(&mut c, false);
        assert_eq!(unsave.expected_generation, 2);
        assert_eq!(
            c.facts().outcome,
            None,
            "a new intent clears the old outcome"
        );
        c.complete(&unsave, Ok(state(false, 3)));
        assert_eq!(c.facts().outcome, Some(SaveOutcome::Unsaved));
    }

    #[test]
    fn a_session_change_drops_private_state_and_late_answers() {
        let mut c = signed_in(A, state(true, 4));
        let command = send(&mut c, false);
        let read_b = c.change_session(active(B)).expect("B reads its own pair");
        assert_eq!(c.facts().value, ToggleValue::Unknown, "A's state is gone");
        assert_eq!(c.complete(&command, Ok(state(false, 5))), Next::Nothing);
        assert_eq!(
            c.facts().value,
            ToggleValue::Unknown,
            "A's late ACK is ignored"
        );
        c.apply_read(&read_b, Ok(state(false, 1)));
        assert_eq!(c.facts().value, ToggleValue::Off);

        // Sign-out while A's read is in flight: the answer never shows to the guest.
        let mut c = SaveController::new("w");
        let read_a = c.change_session(active(A)).unwrap();
        c.change_session(SessionView::Guest);
        c.apply_read(&read_a, Ok(state(true, 2)));
        assert_eq!(c.facts().value, ToggleValue::Off);
        assert_eq!(c.facts().outcome, None);
        // A → B → A: a new generation is a new scope, so A's old ticket stays stale.
        let mut c = SaveController::new("w");
        let old = c.change_session(active(A)).unwrap();
        c.change_session(active(B));
        c.change_session(active("11111111-1111-4111-8111-111111111111/2"));
        assert_eq!(c.apply_read(&old, Ok(state(true, 9))), Next::Nothing);
        assert_eq!(c.facts().value, ToggleValue::Unknown);
    }

    #[test]
    fn a_lost_ack_is_uncertain_until_a_read_and_is_never_replayed() {
        let mut c = signed_in(A, state(false, 1));
        let command = send(&mut c, true);
        assert_eq!(c.complete(&command, Err(ApiError::Network)), Next::Nothing);
        let facts = c.facts();
        assert_eq!(facts.phase, TogglePhase::Uncertain { desired: true });
        assert_eq!(facts.value, ToggleValue::Off);
        assert_eq!(c.press(true), Press::Ignore, "no blind retry");
        assert_eq!(c.refresh(), None, "background refresh waits for the person");
        let read = c.recover().expect("Kiểm tra lại reads");
        assert!(c.facts().reading);
        c.apply_read(&read, Ok(state(true, 2)));
        let facts = c.facts();
        assert_eq!(
            (facts.value, facts.phase),
            (ToggleValue::On, TogglePhase::Idle)
        );
        assert!(
            matches!(facts.outcome, Some(SaveOutcome::Saved { .. })),
            "it was committed"
        );

        // The read says the intent did not hold: say so, keep the real state.
        let mut c = signed_in(A, state(false, 1));
        let command = send(&mut c, true);
        c.complete(&command, Err(ApiError::Unexpected { status: 502 }));
        let read = c.recover().unwrap();
        c.apply_read(&read, Ok(state(false, 1)));
        assert_eq!(c.facts().refusal, Some(SaveRefusal::Save));
        assert_eq!(c.facts().value, ToggleValue::Off);
        // A failed reconcile stays uncertain.
        let command = send(&mut c, true);
        c.complete(&command, Err(ApiError::Unavailable));
        let read = c.recover().unwrap();
        c.apply_read(&read, Err(ApiError::Network));
        assert_eq!(c.facts().phase, TogglePhase::Uncertain { desired: true });
    }

    #[test]
    fn a_fenced_intent_reads_again_and_a_newer_state_from_another_tab_wins() {
        // Tab 1 saw Saved/2; tab 2 unsaved (3) and saved again (4). Tab 1's unsave is refused.
        let mut c = signed_in(A, state(true, 2));
        let command = send(&mut c, false);
        let Next::Read(read) = c.complete(
            &command,
            Err(ApiError::GenerationConflict {
                current_generation: 4,
            }),
        ) else {
            panic!("a conflict reads the pair again");
        };
        assert_eq!(c.facts().phase, TogglePhase::Pending { desired: false });
        assert_eq!(c.press(false), Press::Ignore);
        c.apply_read(&read, Ok(state(true, 4)));
        let facts = c.facts();
        assert_eq!(facts.value, ToggleValue::On, "the newer save is kept");
        assert_eq!(facts.refusal, Some(SaveRefusal::Unsave));
        // A background read that raced with a press cannot undo the newer ACK.
        let mut c = signed_in(A, state(false, 1));
        let stale = c.refresh().unwrap();
        let command = send(&mut c, true);
        c.complete(&command, Ok(state(true, 2)));
        assert_eq!(c.apply_read(&stale, Ok(state(false, 1))), Next::Nothing);
        assert_eq!(c.facts().value, ToggleValue::On);
        // Even an accepted read never moves the generation backwards.
        let read = c.refresh().unwrap();
        c.apply_read(&read, Ok(state(false, 1)));
        assert_eq!(c.facts().value, ToggleValue::On);
    }

    #[test]
    fn refusals_keep_the_known_state_and_name_their_reason() {
        let mut c = signed_in(A, state(false, 7));
        let command = send(&mut c, true);
        c.complete(&command, Err(ApiError::SavedWorksLimit));
        assert_eq!(c.facts().refusal, Some(SaveRefusal::Limit));
        assert_eq!(c.facts().value, ToggleValue::Off);
        let command = send(&mut c, true);
        assert_eq!(
            command.expected_generation, 7,
            "a refusal consumes no generation"
        );
        assert_eq!(
            c.complete(&command, Err(ApiError::Unauthenticated)),
            Next::RefreshSession
        );
        assert_eq!(c.facts().refusal, Some(SaveRefusal::Save));
        let command = send(&mut c, true);
        c.complete(&command, Err(ApiError::NotFound));
        assert_eq!(c.facts().refusal, Some(SaveRefusal::Save));
    }

    #[test]
    fn the_sign_in_hint_shows_once_after_a_guest_signs_in_and_nothing_is_saved() {
        let mut c = SaveController::new("w");
        c.change_session(SessionView::Guest);
        assert_eq!(c.press(true), Press::SignIn);
        c.arm_sign_in_hint();
        c.change_session(SessionView::Unknown);
        let read = c.change_session(active(B)).unwrap();
        c.apply_read(&read, Ok(state(false, 1)));
        assert_eq!(c.facts().outcome, Some(SaveOutcome::SignedIn));
        assert_eq!(c.facts().value, ToggleValue::Off, "no auto-save");
        let read = c.refresh().unwrap();
        c.apply_read(&read, Ok(state(false, 1)));
        assert_eq!(
            c.facts().outcome,
            Some(SaveOutcome::SignedIn),
            "kept until a new intent"
        );
        let command = send(&mut c, true);
        assert_eq!(c.facts().outcome, None);
        c.complete(&command, Ok(state(true, 2)));
        // Already saved before sign-in: no guidance line.
        let mut c = SaveController::new("w");
        c.change_session(SessionView::Guest);
        c.arm_sign_in_hint();
        let read = c.change_session(active(A)).unwrap();
        c.apply_read(&read, Ok(state(true, 3)));
        assert_eq!(c.facts().outcome, None);
        // Dismissed sheet: no guidance after a later sign-in.
        let mut c = SaveController::new("w");
        c.change_session(SessionView::Guest);
        c.arm_sign_in_hint();
        c.disarm_sign_in_hint();
        let read = c.change_session(active(A)).unwrap();
        c.apply_read(&read, Ok(state(false, 1)));
        assert_eq!(c.facts().outcome, None);
    }

    #[test]
    fn the_list_link_belongs_to_the_scope_that_asked() {
        let mut c = signed_in(A, state(false, 1));
        let command = send(&mut c, true);
        c.set_list_href(B, Some("/@b/da-luu".into()));
        assert!(!c.has_list_href());
        c.complete(&command, Ok(state(true, 2)));
        c.set_list_href(A, Some("/@a/da-luu".into()));
        assert_eq!(
            c.facts().outcome,
            Some(SaveOutcome::Saved {
                list_href: Some("/@a/da-luu".into())
            })
        );
    }
}
