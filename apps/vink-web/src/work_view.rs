//! Reader work-view visibility tracker and bounded retry (`#343`, `VIEW-01..06`, `VIEW-10`,
//! `OQ-3A/B`, `CHOICE-01`, `CHOICE-04`, `CHOICE-POLICY`).
//!
//! Pure timing/state machine logic lives here without DOM dependencies so boundary conditions,
//! monotonic accumulation, hidden pause/resume, cancellation on leave/revision/control/session
//! switches, bounded same-event retries, and single-source threshold overrides are verified
//! deterministically under unit tests.

use vink_work_view_policy::WorkViewDurationPolicy;

/// Exponential backoff delays (ms) for bounded in-tab retries using the same `event_id`
/// (`CHOICE-04`). Total retry span (~13 s) stays well within the `VIEW-10` expectation (~1 min)
/// and terminates on unmount, cancellation, `204`, or permanent `4xx`.
pub const VIEW_RETRY_BACKOFF_MS: [u32; 3] = [1_000, 3_000, 9_000];

/// Returns the canonical author-facing `UX-07` explanation for the configured default threshold.
pub fn ux_explanation_vi() -> String {
    explanation_for_policy(WorkViewDurationPolicy::DEFAULT)
}

/// Returns the canonical `UX-07` explanation for any [`WorkViewDurationPolicy`] value.
pub fn explanation_for_policy(policy: WorkViewDurationPolicy) -> String {
    policy.ux_explanation_vi()
}

/// First paragraph of the Desk “Cách tính lượt xem” sheet (`DESIGN_SPEC §4`), driven directly
/// by the shared [`WorkViewDurationPolicy`] so the threshold is never hardcoded a second time.
pub fn views_explainer_lead_vi(policy: WorkViewDurationPolicy) -> String {
    format!(
        "Số lần bài được mở đọc và hiển thị ít nhất {}, mỗi người tối đa một lần trong 24 giờ.",
        policy.threshold_label_vi()
    )
}

/// Lifecycle state of a single intentional public Reader open in one tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitState {
    Tracking,
    Triggered,
    Cancelled,
}

/// Settled session identity classification used to detect mid-visit authority transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityKind {
    Anonymous,
    NeedsPrincipal,
    Suspended,
    Active,
}

/// Current session authority observation for cancelling a pre-threshold or retrying visit when the
/// viewer's authority changes mid-visit (`Unsettled` prevents false cancellation during initial
/// hydration discovery).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthoritySnapshot {
    Unsettled,
    Settled {
        generation: u64,
        principal_id: Option<String>,
        kind: AuthorityKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollAction {
    WaitMs(u32),
    Paused,
    Emit { work_id: String, event_id: String },
    Stopped,
}

/// Deterministic foreground-visible tracker for a single open of a published work.
///
/// Accumulates only foreground-visible (`!hidden && !prerendering`) duration using a monotonic
/// clock (`performance.now()` in the browser), emits at most once when cumulative time reaches
/// `policy.min_visible_ms()`, and never resumes once [`VisitState::Cancelled`].
#[derive(Debug, Clone)]
pub struct ViewVisitTracker {
    work_id: String,
    initial_revision_id: String,
    policy: WorkViewDurationPolicy,
    state: VisitState,
    accumulated_ms: f64,
    visible_since_ms: Option<f64>,
    baseline_authority: Option<AuthoritySnapshot>,
    emitted_event_id: Option<String>,
}

impl ViewVisitTracker {
    pub fn new(work_id: String, revision_id: String, policy: WorkViewDurationPolicy) -> Self {
        Self {
            work_id,
            initial_revision_id: revision_id,
            policy,
            state: VisitState::Tracking,
            accumulated_ms: 0.0,
            visible_since_ms: None,
            baseline_authority: None,
            emitted_event_id: None,
        }
    }

    pub fn state(&self) -> VisitState {
        self.state
    }

    pub fn is_cancelled(&self) -> bool {
        self.state == VisitState::Cancelled
    }

    pub fn emitted_event_id(&self) -> Option<&str> {
        self.emitted_event_id.as_deref()
    }

    /// Cancels countdown and any pending in-tab retry permanently for this visit.
    pub fn cancel(&mut self) {
        self.state = VisitState::Cancelled;
        self.visible_since_ms = None;
    }

    /// Advances cumulative visible time up to `now_ms` and returns what the scheduler should do.
    ///
    /// Uses a single adaptive timeout duration instead of periodic intervals/heartbeats (`AC-05`).
    pub fn poll(
        &mut self,
        now_ms: f64,
        visible_foreground: bool,
        mint_event_id: impl FnOnce() -> Option<String>,
    ) -> PollAction {
        if self.state != VisitState::Tracking {
            return PollAction::Stopped;
        }
        let now_ms = if now_ms.is_finite() && now_ms >= 0.0 {
            now_ms
        } else {
            0.0
        };
        if let Some(start) = self.visible_since_ms {
            self.accumulated_ms += (now_ms - start).max(0.0);
        }

        let threshold = f64::from(self.policy.min_visible_ms());
        if self.accumulated_ms >= threshold
            && (visible_foreground || self.visible_since_ms.is_some())
        {
            self.visible_since_ms = None;
            if let Some(event_id) = mint_event_id() {
                self.state = VisitState::Triggered;
                self.emitted_event_id = Some(event_id.clone());
                return PollAction::Emit {
                    work_id: self.work_id.clone(),
                    event_id,
                };
            }
            self.cancel();
            return PollAction::Stopped;
        }

        if visible_foreground {
            self.visible_since_ms = Some(now_ms);
            let remaining = (threshold - self.accumulated_ms).ceil();
            let delay_ms = if remaining >= 1.0 {
                remaining.min(f64::from(u32::MAX)) as u32
            } else {
                1
            };
            PollAction::WaitMs(delay_ms)
        } else {
            self.visible_since_ms = None;
            PollAction::Paused
        }
    }

    /// Cancels if the shown public scope is lost, moved to another work, or swapped to a new
    /// revision in the same tab (`E-37`, withdrawal, removal). Background refresh notices that leave
    /// `revision_id` and `is_published` intact do not interrupt tracking.
    pub fn on_scope_change(&mut self, work_id: &str, revision_id: &str, is_published: bool) {
        if !is_published || work_id != self.work_id || revision_id != self.initial_revision_id {
            self.cancel();
        }
    }

    /// Latches the first settled session authority as baseline and cancels if authority changes later.
    pub fn on_authority_update(&mut self, snapshot: AuthoritySnapshot) {
        if self.state == VisitState::Cancelled {
            return;
        }
        let AuthoritySnapshot::Settled { .. } = snapshot else {
            return;
        };
        match &self.baseline_authority {
            None => self.baseline_authority = Some(snapshot),
            Some(baseline) if baseline != &snapshot => self.cancel(),
            Some(_) => {}
        }
    }
}

/// Bounded retry state machine reusing one minted UUIDv7 `event_id` (`CHOICE-04`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPlan {
    event_id: String,
    retries_used: usize,
    finished: bool,
}

impl RetryPlan {
    pub fn new(event_id: String) -> Self {
        Self {
            event_id,
            retries_used: 0,
            finished: false,
        }
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    /// Consumes the HTTP attempt outcome and returns either the next backoff delay (`Some(ms)`) or
    /// `None` when delivery succeeded, failed permanently (`4xx`), or exhausted retries.
    pub fn next_delay_ms(&mut self, outcome: crate::api::ViewSendOutcome) -> Option<u32> {
        if self.finished {
            return None;
        }
        match outcome {
            crate::api::ViewSendOutcome::Done | crate::api::ViewSendOutcome::Stop => {
                self.finished = true;
                None
            }
            crate::api::ViewSendOutcome::Retry => {
                let delay = VIEW_RETRY_BACKOFF_MS.get(self.retries_used).copied();
                if delay.is_some() {
                    self.retries_used += 1;
                } else {
                    self.finished = true;
                }
                delay
            }
        }
    }
}

/// Encodes an RFC 9562 UUIDv7 string from Unix epoch milliseconds and 10 random bytes.
pub fn format_uuid_v7(unix_ms: u64, random: [u8; 10]) -> String {
    let mut bytes = [0_u8; 16];
    let ts = unix_ms & 0x0000_ffff_ffff_ffff;
    bytes[0] = (ts >> 40) as u8;
    bytes[1] = (ts >> 32) as u8;
    bytes[2] = (ts >> 24) as u8;
    bytes[3] = (ts >> 16) as u8;
    bytes[4] = (ts >> 8) as u8;
    bytes[5] = ts as u8;
    bytes[6..16].copy_from_slice(&random);
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}

#[cfg(feature = "browser")]
mod browser {
    use std::time::Duration;

    use futures_channel::oneshot;
    use leptos::prelude::*;
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};

    use super::*;
    use crate::api::record_work_view;
    use crate::contract::Publication;
    use crate::read_events;

    fn is_prerendering() -> bool {
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return false;
        };
        js_sys::Reflect::get(&doc, &JsValue::from_str("prerendering"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    fn visible_foreground() -> bool {
        read_events::visible() && !is_prerendering()
    }

    fn mint_browser_uuid_v7() -> Option<String> {
        let mut random = [0_u8; 10];
        web_sys::window()?
            .crypto()
            .ok()?
            .get_random_values_with_u8_array(&mut random)
            .ok()?;
        let now_ms = js_sys::Date::now();
        if !now_ms.is_finite() || now_ms < 0.0 {
            return None;
        }
        Some(format_uuid_v7(now_ms as u64, random))
    }

    struct Controller {
        tracker: ViewVisitTracker,
        countdown_timer: Option<TimeoutHandle>,
        retry_timer: Option<TimeoutHandle>,
        retry_cancel: Option<oneshot::Sender<()>>,
        started_retry: bool,
    }

    impl Controller {
        fn clear_countdown_timer(&mut self) {
            if let Some(handle) = self.countdown_timer.take() {
                handle.clear();
            }
        }

        fn clear_retry_timer(&mut self) {
            if let Some(handle) = self.retry_timer.take() {
                handle.clear();
            }
        }

        fn cancel(&mut self) {
            self.clear_countdown_timer();
            self.clear_retry_timer();
            self.tracker.cancel();
            if let Some(tx) = self.retry_cancel.take() {
                let _ = tx.send(());
            }
        }
    }

    fn cancel_controller(ctrl: StoredValue<Controller, LocalStorage>) {
        ctrl.try_update_value(|c| c.cancel());
    }

    fn sync_controller(ctrl: StoredValue<Controller, LocalStorage>) {
        let visible = visible_foreground();
        let Some(action) = ctrl.try_update_value(|c| {
            c.clear_countdown_timer();
            if c.started_retry || c.tracker.state() != VisitState::Tracking {
                return PollAction::Stopped;
            }
            c.tracker
                .poll(read_events::now(), visible, mint_browser_uuid_v7)
        }) else {
            return;
        };
        match action {
            PollAction::WaitMs(delay_ms) => {
                if let Ok(handle) = set_timeout_with_handle(
                    move || sync_controller(ctrl),
                    Duration::from_millis(u64::from(delay_ms)),
                ) {
                    let stored = ctrl.try_update_value(|c| {
                        if c.started_retry || c.tracker.is_cancelled() {
                            false
                        } else {
                            c.countdown_timer = Some(handle);
                            true
                        }
                    });
                    if stored != Some(true) {
                        handle.clear();
                    }
                }
            }
            PollAction::Emit { work_id, event_id } => {
                let claimed = ctrl.try_update_value(|c| {
                    if c.started_retry || c.tracker.is_cancelled() {
                        false
                    } else {
                        c.clear_countdown_timer();
                        c.started_retry = true;
                        true
                    }
                });
                if claimed != Some(true) {
                    return;
                }
                wasm_bindgen_futures::spawn_local(async move {
                    let mut plan = RetryPlan::new(event_id);
                    loop {
                        if ctrl.try_with_value(|c| c.tracker.is_cancelled()) != Some(false) {
                            break;
                        }
                        let outcome = record_work_view(&work_id, plan.event_id()).await;
                        if ctrl.try_with_value(|c| c.tracker.is_cancelled()) != Some(false) {
                            break;
                        }
                        let Some(delay_ms) = plan.next_delay_ms(outcome) else {
                            break;
                        };
                        let (tx, rx) = oneshot::channel::<()>();
                        let armed = ctrl.try_update_value(|c| {
                            if c.tracker.is_cancelled() {
                                return false;
                            }
                            c.retry_cancel = Some(tx);
                            true
                        });
                        if armed != Some(true) {
                            break;
                        }
                        let waiter = set_timeout_with_handle(
                            move || {
                                ctrl.try_update_value(|c| {
                                    if let Some(wake) = c.retry_cancel.take() {
                                        let _ = wake.send(());
                                    }
                                });
                            },
                            Duration::from_millis(u64::from(delay_ms)),
                        );
                        match waiter {
                            Ok(handle) => {
                                let stored = ctrl.try_update_value(|c| {
                                    if c.tracker.is_cancelled() {
                                        false
                                    } else {
                                        c.retry_timer = Some(handle);
                                        true
                                    }
                                });
                                if stored != Some(true) {
                                    handle.clear();
                                    break;
                                }
                                let _ = rx.await;
                                ctrl.try_update_value(|c| c.clear_retry_timer());
                            }
                            Err(_) => break,
                        }
                    }
                });
            }
            PollAction::Paused | PollAction::Stopped => {}
        }
    }

    pub(crate) fn track_publication_visit(
        initial: &Publication,
        shown: RwSignal<Publication>,
        visit_eligible: RwSignal<bool>,
    ) {
        let ctrl: StoredValue<Controller, LocalStorage> = StoredValue::new_local(Controller {
            tracker: ViewVisitTracker::new(
                initial.work_id.clone(),
                initial.revision_id.clone(),
                WorkViewDurationPolicy::DEFAULT,
            ),
            countdown_timer: None,
            retry_timer: None,
            retry_cancel: None,
            started_retry: false,
        });
        if !visit_eligible.get_untracked() {
            cancel_controller(ctrl);
            return;
        }

        if let Some(session) = use_context::<crate::account::Session>() {
            Effect::new(move |_| {
                let snapshot = session.settled_authority();
                let cancelled = ctrl.try_update_value(|c| {
                    c.tracker.on_authority_update(snapshot);
                    let dead = c.tracker.is_cancelled();
                    if dead {
                        c.cancel();
                    }
                    dead
                });
                let _ = cancelled;
            });
        }

        Effect::new(move |_| {
            let eligible = visit_eligible.get();
            let (work_id, rev_id) =
                shown.with(|pub_state| (pub_state.work_id.clone(), pub_state.revision_id.clone()));
            ctrl.try_update_value(|c| {
                c.tracker.on_scope_change(&work_id, &rev_id, eligible);
                if c.tracker.is_cancelled() {
                    c.cancel();
                }
            });
        });

        read_events::observe(move || sync_controller(ctrl));

        let prerender_listener: StoredValue<
            Option<Closure<dyn FnMut(web_sys::Event)>>,
            LocalStorage,
        > = StoredValue::new_local(None);
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| sync_controller(ctrl));
            let _ = doc.add_event_listener_with_callback(
                "prerenderingchange",
                closure.as_ref().unchecked_ref(),
            );
            prerender_listener.set_value(Some(closure));
        }

        let pagehide =
            window_event_listener(leptos::ev::pagehide, move |_| cancel_controller(ctrl));

        on_cleanup(move || {
            cancel_controller(ctrl);
            pagehide.remove();
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                prerender_listener.try_update_value(|slot| {
                    if let Some(closure) = slot.take() {
                        let _ = doc.remove_event_listener_with_callback(
                            "prerenderingchange",
                            closure.as_ref().unchecked_ref(),
                        );
                    }
                });
            }
        });

        sync_controller(ctrl);
    }
}

#[cfg(feature = "browser")]
pub(crate) use browser::track_publication_visit;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ViewSendOutcome;
    use vink_work_view_policy::WORK_VIEW_MIN_VISIBLE_MS;

    fn mint() -> Option<String> {
        Some("0193a000-0000-7000-8000-000000000042".into())
    }

    fn tracker(policy: WorkViewDurationPolicy) -> ViewVisitTracker {
        ViewVisitTracker::new("work-1".into(), "rev-1".into(), policy)
    }

    #[test]
    fn threshold_boundary_needs_exact_configured_duration_and_emits_once() {
        let mut t = tracker(WorkViewDurationPolicy::DEFAULT);
        assert_eq!(
            t.poll(100.0, true, mint),
            PollAction::WaitMs(WORK_VIEW_MIN_VISIBLE_MS)
        );
        assert_eq!(
            t.poll(
                100.0 + f64::from(WORK_VIEW_MIN_VISIBLE_MS) - 1.0,
                true,
                mint
            ),
            PollAction::WaitMs(1)
        );
        assert_eq!(t.state(), VisitState::Tracking);

        assert_eq!(
            t.poll(100.0 + f64::from(WORK_VIEW_MIN_VISIBLE_MS), true, mint),
            PollAction::Emit {
                work_id: "work-1".into(),
                event_id: "0193a000-0000-7000-8000-000000000042".into(),
            }
        );
        assert_eq!(t.state(), VisitState::Triggered);
        // Staying visible longer never creates another event.
        assert_eq!(t.poll(60_000.0, true, mint), PollAction::Stopped);
    }

    #[test]
    fn hidden_tab_pauses_and_resumes_cumulative_foreground_duration() {
        let mut t = tracker(WorkViewDurationPolicy::DEFAULT);
        assert_eq!(
            t.poll(1_000.0, true, mint),
            PollAction::WaitMs(WORK_VIEW_MIN_VISIBLE_MS)
        );
        // Hidden at 3_000ms cumulative = 3_000ms -> paused with 2_000ms left.
        assert_eq!(t.poll(4_000.0, false, mint), PollAction::Paused);
        // 10_000ms passes in hidden state -> must not count.
        assert_eq!(t.poll(14_000.0, false, mint), PollAction::Paused);
        // Back to foreground -> needs remaining 2_000ms.
        let half = WORK_VIEW_MIN_VISIBLE_MS - 3_000;
        assert_eq!(t.poll(14_000.0, true, mint), PollAction::WaitMs(half));
        assert_eq!(t.poll(15_999.0, true, mint), PollAction::WaitMs(1));
        assert_eq!(
            t.poll(16_000.0, true, mint),
            PollAction::Emit {
                work_id: "work-1".into(),
                event_id: "0193a000-0000-7000-8000-000000000042".into(),
            }
        );
    }

    #[test]
    fn background_opened_tab_waits_for_foreground_visibility() {
        let mut t = tracker(WorkViewDurationPolicy::DEFAULT);
        assert_eq!(t.poll(500.0, false, mint), PollAction::Paused);
        assert_eq!(t.poll(60_000.0, false, mint), PollAction::Paused);
        assert_eq!(
            t.poll(60_000.0, true, mint),
            PollAction::WaitMs(WORK_VIEW_MIN_VISIBLE_MS)
        );
        assert_eq!(
            t.poll(
                60_000.0 + f64::from(WORK_VIEW_MIN_VISIBLE_MS) - 1.0,
                true,
                mint
            ),
            PollAction::WaitMs(1)
        );
        assert!(matches!(
            t.poll(60_000.0 + f64::from(WORK_VIEW_MIN_VISIBLE_MS), true, mint),
            PollAction::Emit { .. }
        ));
    }

    #[test]
    fn scope_and_session_authority_changes_cancel_cleanly_while_baseline_and_refresh_keep_tracking()
    {
        let mut t = tracker(WorkViewDurationPolicy::DEFAULT);
        assert_eq!(
            t.poll(0.0, true, mint),
            PollAction::WaitMs(WORK_VIEW_MIN_VISIBLE_MS)
        );
        // Hydration settling initial session from Unsettled -> Anonymous is baseline discovery, not change.
        t.on_authority_update(AuthoritySnapshot::Unsettled);
        t.on_authority_update(AuthoritySnapshot::Settled {
            generation: 0,
            principal_id: None,
            kind: AuthorityKind::Anonymous,
        });
        // Same scope + same revision (e.g. background update notice without applying r2) keeps tracking.
        t.on_scope_change("work-1", "rev-1", true);
        assert_eq!(t.state(), VisitState::Tracking);

        // Signing in / switching authority mid-countdown cancels immediately.
        t.on_authority_update(AuthoritySnapshot::Settled {
            generation: 1,
            principal_id: Some("p-1".into()),
            kind: AuthorityKind::Active,
        });
        assert!(t.is_cancelled());
        assert_eq!(t.poll(10_000.0, true, mint), PollAction::Stopped);

        // Replacing revision ("Xem bản mới", E-37) cancels pre-threshold tracker cleanly.
        let mut rev_switch = tracker(WorkViewDurationPolicy::DEFAULT);
        assert_eq!(
            rev_switch.poll(0.0, true, mint),
            PollAction::WaitMs(WORK_VIEW_MIN_VISIBLE_MS)
        );
        rev_switch.on_scope_change("work-1", "rev-2", true);
        assert!(rev_switch.is_cancelled());

        // Losing public status (Hidden / ReviewWithdrawn) cancels cleanly.
        let mut withdrawn = tracker(WorkViewDurationPolicy::DEFAULT);
        assert_eq!(
            withdrawn.poll(0.0, true, mint),
            PollAction::WaitMs(WORK_VIEW_MIN_VISIBLE_MS)
        );
        withdrawn.on_scope_change("work-1", "rev-1", false);
        assert!(withdrawn.is_cancelled());
    }

    #[test]
    fn non_default_policy_override_controls_both_eligibility_and_ux_explanation() {
        let custom = WorkViewDurationPolicy::from_millis(1_200).unwrap();
        let mut t = tracker(custom);
        assert_eq!(t.poll(0.0, true, mint), PollAction::WaitMs(1_200));
        assert_eq!(t.poll(1_199.0, true, mint), PollAction::WaitMs(1));
        assert_eq!(t.state(), VisitState::Tracking);
        assert!(matches!(
            t.poll(1_200.0, true, mint),
            PollAction::Emit { .. }
        ));

        let custom_copy = explanation_for_policy(custom);
        assert!(custom_copy.contains("1,2 giây"), "{custom_copy}");
        assert!(!custom_copy.contains("5 giây"), "{custom_copy}");
        assert!(ux_explanation_vi().contains("5 giây"));

        let custom_lead = views_explainer_lead_vi(custom);
        assert!(custom_lead.contains("ít nhất 1,2 giây"), "{custom_lead}");
        assert!(!custom_lead.contains("5 giây"), "{custom_lead}");
        assert!(
            views_explainer_lead_vi(WorkViewDurationPolicy::DEFAULT).contains("ít nhất 5 giây")
        );
    }

    #[test]
    fn retry_plan_reuses_event_id_bounds_attempts_and_stops_on_done_or_4xx() {
        let mut retries = RetryPlan::new("0193a000-0000-7000-8000-000000000099".into());
        assert_eq!(retries.event_id(), "0193a000-0000-7000-8000-000000000099");
        assert_eq!(retries.next_delay_ms(ViewSendOutcome::Retry), Some(1_000));
        assert_eq!(retries.next_delay_ms(ViewSendOutcome::Retry), Some(3_000));
        assert_eq!(retries.next_delay_ms(ViewSendOutcome::Retry), Some(9_000));
        assert_eq!(retries.next_delay_ms(ViewSendOutcome::Retry), None);
        assert_eq!(retries.event_id(), "0193a000-0000-7000-8000-000000000099");

        let mut permanent = RetryPlan::new("0193a000-0000-7000-8000-000000000098".into());
        assert_eq!(permanent.next_delay_ms(ViewSendOutcome::Stop), None);
        assert_eq!(permanent.next_delay_ms(ViewSendOutcome::Retry), None);

        let mut ok = RetryPlan::new("0193a000-0000-7000-8000-000000000097".into());
        assert_eq!(ok.next_delay_ms(ViewSendOutcome::Done), None);
    }

    #[test]
    fn formats_rfc_9562_uuid_v7_with_correct_version_and_variant() {
        let encoded = format_uuid_v7(
            1_764_000_000_123,
            [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22],
        );
        assert_eq!(encoded.len(), 36);
        assert_eq!(encoded.chars().nth(14), Some('7'));
        assert!(matches!(
            encoded.chars().nth(19),
            Some('8' | '9' | 'a' | 'b')
        ));
    }
}
