//! “Lưu” on the Reader (#388, SAV-04, D1–D5/D11): the feature adapter of `SaveAction`.
//!
//! HTTP, the session scope, the generation fence and late answers stay here and in
//! [`crate::saved_state`]. The viewer's membership is read only in the browser, after the
//! session owner confirmed who is reading, from the private `no-store` route; SSR and the
//! public read cache (READ-01/02) render the same held control for every viewer.
use leptos::prelude::*;
use vink_ui::RecoveryIntent;

use crate::saved_state::SaveController;
use crate::ui::patterns::SaveAction;

/// The save half of the Reader's “Thấm · Lưu” pair for one work.
#[component]
pub(crate) fn ReaderSave(work_id: String) -> impl IntoView {
    let controller = RwSignal::new(SaveController::new(work_id));
    let facts = Memo::new(move |_| controller.with(SaveController::facts));
    #[cfg(feature = "browser")]
    let adapter = browser::Adapter::start(controller);
    view! {
        <SaveAction
            id="save-work"
            facts
            on_set=move |desired: bool| {
                #[cfg(feature = "browser")]
                adapter.press(desired);
                #[cfg(not(feature = "browser"))]
                let _ = desired;
            }
            on_recover=move |_: RecoveryIntent| {
                #[cfg(feature = "browser")]
                adapter.recover();
            }
        />
    }
}

#[cfg(feature = "browser")]
mod browser {
    use leptos::prelude::*;

    use crate::account::Session;
    use crate::pages::discussion::{
        ReaderAuth, ReaderAuthPurpose, ReaderAuthReady, ReaderAuthSettled,
    };
    use crate::saved_state::{Next, Press, ReadTicket, SaveCommand, SaveController, SessionView};
    use crate::session::Standing;

    /// Background refreshes (tab shown again, focus) keep two tabs honest without a request
    /// on every focus change.
    const WAKE_INTERVAL_MS: f64 = 15_000.0;
    const BUTTON: &str = "save-work";

    #[derive(Clone, Copy)]
    pub(super) struct Adapter {
        controller: RwSignal<SaveController>,
        session: Option<Session>,
        auth: Option<ReaderAuth>,
        owner: StoredValue<leptos::reactive::owner::WeakOwner>,
        last_wake: StoredValue<f64>,
    }

    fn view_of(session: Session) -> SessionView {
        match session.standing.get() {
            Standing::Unknown => SessionView::Unknown,
            Standing::Unavailable => SessionView::Unavailable,
            Standing::Anonymous => SessionView::Guest,
            Standing::NeedsPrincipal => SessionView::NeedsPenName,
            Standing::Suspended => {
                SessionView::Blocked("Tài khoản đang bị tạm khóa nên chưa thể lưu bài.")
            }
            // A cookie change is being confirmed: no private answer applies meanwhile.
            Standing::Active(_) => session
                .profile_scope
                .get()
                .map_or(SessionView::Unknown, |scope| SessionView::Active { scope }),
        }
    }

    impl Adapter {
        pub(super) fn start(controller: RwSignal<SaveController>) -> Self {
            let this = Self {
                controller,
                session: use_context::<Session>(),
                auth: ReaderAuth::current(),
                owner: StoredValue::new(
                    Owner::current()
                        .expect("ReaderSave runs in a component")
                        .downgrade(),
                ),
                last_wake: StoredValue::new(crate::read_events::now()),
            };
            if let Some(session) = this.session {
                Effect::new(move |_| {
                    let next = view_of(session);
                    if let Some(Some(ticket)) = controller.try_update(|c| c.change_session(next)) {
                        this.read(ticket);
                    }
                });
            }
            crate::read_events::observe_lifecycle(move |hint| {
                let now = crate::read_events::now();
                let due = hint == crate::read_events::Hint::HistoryRestored
                    || now - this.last_wake.get_value() >= WAKE_INTERVAL_MS;
                if due && crate::read_events::visible() {
                    this.last_wake.set_value(now);
                    if let Some(Some(ticket)) = controller.try_update(SaveController::refresh) {
                        this.read(ticket);
                    }
                }
            });
            this
        }

        fn spawn(&self, task: impl std::future::Future<Output = ()> + 'static) {
            if let Some(owner) = self.owner.try_get_value().and_then(|owner| owner.upgrade()) {
                owner.with(|| leptos::task::spawn_local_scoped_with_cancellation(task));
            }
        }

        fn work_id(&self) -> String {
            self.controller.with_untracked(|c| c.work_id().to_owned())
        }

        fn read(&self, ticket: ReadTicket) {
            let this = *self;
            let work_id = this.work_id();
            this.spawn(async move {
                let result = crate::api::load_saved_work(&work_id).await;
                let next = this
                    .controller
                    .try_update(|c| c.apply_read(&ticket, result));
                this.follow(next);
            });
        }

        fn follow(&self, next: Option<Next>) {
            match next {
                Some(Next::Read(ticket)) => self.read(ticket),
                Some(Next::RefreshSession) => {
                    if let Some(session) = self.session {
                        session.refresh();
                    }
                }
                Some(Next::Nothing) | None => {}
            }
        }

        /// `ToggleAction` needs the phase to be pending before this returns.
        pub(super) fn press(&self, desired: bool) {
            match self.controller.try_update(|c| c.press(desired)) {
                Some(Press::Send(command)) => {
                    if desired {
                        self.load_list_href();
                    }
                    self.send(command);
                }
                Some(Press::SignIn) => self.sign_in(),
                Some(Press::Ignore) | None => {}
            }
        }

        fn send(&self, command: SaveCommand) {
            let this = *self;
            let work_id = this.work_id();
            this.spawn(async move {
                let result = crate::api::set_saved_work(
                    &work_id,
                    command.desired,
                    command.expected_generation,
                    &command.principal,
                )
                .await;
                let next = this.controller.try_update(|c| c.complete(&command, result));
                this.follow(next);
            });
        }

        /// D2: the Reader's sheet, back to this button, never a save on the guest's behalf.
        /// Only Lưu takes this path; a guest's Thấm keeps its blocked reason (outside #388).
        fn sign_in(&self) {
            let Some(auth) = self.auth else {
                return;
            };
            let controller = self.controller;
            controller.update(SaveController::arm_sign_in_hint);
            let ready: ReaderAuthReady = std::sync::Arc::new(move |scope: &str| {
                controller
                    .try_with(|c| c.scope() == Some(scope))
                    .unwrap_or(false)
            });
            let settled: ReaderAuthSettled = std::sync::Arc::new(move |authenticated: bool| {
                if !authenticated {
                    controller.try_update(SaveController::disarm_sign_in_hint);
                }
            });
            auth.open(BUTTON.into(), ReaderAuthPurpose::Save, ready, Some(settled));
        }

        /// “Kiểm tra lại” reads the pair (or the session); the Reconcile button disappears, so
        /// focus moves to “Lưu” instead of the document body.
        pub(super) fn recover(&self) {
            let unavailable = self
                .session
                .is_some_and(|session| session.standing.get_untracked() == Standing::Unavailable);
            if unavailable {
                if let Some(session) = self.session {
                    session.refresh();
                }
            } else if let Some(Some(ticket)) = self.controller.try_update(SaveController::recover) {
                self.read(ticket);
            }
            request_animation_frame(|| {
                use wasm_bindgen::JsCast;
                if let Some(button) = document()
                    .get_element_by_id(BUTTON)
                    .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = button.focus();
                }
            });
        }

        /// The viewer's own handle for “Mở Đã lưu”, read once per scope from `/me/profile`.
        /// The link targets `/@handle/da-luu`, the owner-only page of #389; until #389 ships it
        /// resolves to the same NotFound as any unknown URL (accepted 26/11/2026, PR #546).
        fn load_list_href(&self) {
            let Some((scope, known)) = self
                .controller
                .with_untracked(|c| c.scope().map(|s| (s.to_owned(), c.has_list_href())))
            else {
                return;
            };
            if known {
                return;
            }
            let this = *self;
            let principal = scope.split_once('/').map(|(p, _)| p.to_owned());
            this.spawn(async move {
                use crate::contract::follow::valid_canonical_handle;
                let href = crate::api::load_own_profile()
                    .await
                    .ok()
                    // The current canonical handle, also before its one-time setup: the list
                    // belongs to the principal, not to a chosen address.
                    .filter(|profile| {
                        Some(&profile.principal_id) == principal.as_ref()
                            && valid_canonical_handle(&profile.handle)
                    })
                    .map(|profile| format!("/@{}/da-luu", profile.handle));
                if href.is_some() {
                    this.controller
                        .try_update(|c| c.set_list_href(&scope, href));
                }
            });
        }
    }
}
