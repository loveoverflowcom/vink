//! The reactive session owner of the tab and the account region of “Tôi”.
//!
//! One [`Session`] is provided by the shell. Every view that needs to know who is acting (the
//! avatar, the desk, the editor, discussion and endorsement) reads it instead of asking the
//! server on its own. Decisions are in [`crate::session`]; this module only moves answers
//! between the API and those reducers.
//!
//! Session facts stay out of SSR and browser storage. Other tabs are told only that *something*
//! changed (a `BroadcastChannel` message without data) and then ask the server themselves.

use leptos::prelude::*;
use vink_ui::{
    ActionState, Announce, Button, ButtonKind, ButtonVariant, InlineNotice, TextField, TextKind,
    Tone,
};

use crate::session::{AuthError, SessionOwner, SignInFlow, Standing, Step};

/// Logout as the person sees it. `Unconfirmed`: the server did not confirm the revocation, so
/// the session may still be valid on this device.
// Server rendering never signs anyone in or out; only the browser build drives these.
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum Exit {
    #[default]
    Idle,
    Pending,
    Unconfirmed(AuthError),
}

/// Choosing the first pen name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Naming {
    busy: bool,
    error: Option<AuthError>,
}

#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
#[derive(Clone, Copy)]
pub(crate) struct Session {
    owner: RwSignal<SessionOwner>,
    flow: RwSignal<SignInFlow>,
    exit: RwSignal<Exit>,
    naming: RwSignal<Naming>,
    /// What the server last confirmed. A memo: a check that confirms the same facts wakes nobody.
    pub(crate) standing: Memo<Standing>,
    /// The principal private requests act as.
    pub(crate) principal: Memo<Option<String>>,
    /// Scope of viewer-private page state: principal and generation. The generation advances
    /// whenever the confirmed principal changes, so private lists and replies key on this.
    pub(crate) scope: Memo<Option<String>>,
}

impl Session {
    /// Creates the tab's session owner; call once, in the shell.
    pub(crate) fn provide() -> Self {
        let owner = RwSignal::new(SessionOwner::default());
        let this = Self {
            owner,
            flow: RwSignal::new(SignInFlow::default()),
            exit: RwSignal::new(Exit::Idle),
            naming: RwSignal::new(Naming::default()),
            standing: Memo::new(move |_| owner.with(|o| o.standing().clone())),
            principal: Memo::new(move |_| owner.with(|o| o.principal().map(str::to_owned))),
            scope: Memo::new(move |_| owner.with(SessionOwner::scope)),
        };
        provide_context(this);
        #[cfg(feature = "hydrate")]
        browser::watch(this);
        this
    }

    pub(crate) fn current() -> Self {
        expect_context()
    }

    /// Asks the server again. The newest request wins; an older answer is dropped.
    pub(crate) fn refresh(self) {
        #[cfg(feature = "hydrate")]
        browser::refresh(self);
    }

    fn request_code(self, email: String) {
        #[cfg(feature = "hydrate")]
        browser::request_code(self, email);
        #[cfg(not(feature = "hydrate"))]
        let _ = email;
    }

    fn submit_code(self, code: String) {
        #[cfg(feature = "hydrate")]
        browser::submit_code(self, code);
        #[cfg(not(feature = "hydrate"))]
        let _ = code;
    }

    fn choose_pen_name(self, pen_name: String) {
        #[cfg(feature = "hydrate")]
        browser::choose_pen_name(self, pen_name);
        #[cfg(not(feature = "hydrate"))]
        let _ = pen_name;
    }

    fn sign_out(self) {
        #[cfg(feature = "hydrate")]
        browser::sign_out(self);
    }
}

#[cfg(feature = "hydrate")]
mod browser {
    use leptos::prelude::*;
    use leptos::task::spawn_local;
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};

    use super::{Exit, Session};
    use crate::contract::{ApiError, SessionStateKind as Kind};
    use crate::session::{AuthError, Millis, Observation, SessionOwner, SignInFlow, Step};

    const CHANNEL: &str = "vink-session";

    pub(super) fn now() -> Millis {
        js_sys::Date::now() as Millis
    }

    fn instant(text: &str) -> Option<Millis> {
        let at = js_sys::Date::parse(text);
        at.is_finite().then_some(at as Millis)
    }

    pub(super) fn watch(session: Session) {
        // Another tab may have signed in or out while this one was in the background.
        let focus = window_event_listener(leptos::ev::focus, move |_| refresh(session));
        on_cleanup(move || focus.remove());
        crate::api::on_unauthenticated(move || refresh(session));
        if let Ok(channel) = web_sys::BroadcastChannel::new(CHANNEL) {
            let heard = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |_| refresh(session));
            channel.set_onmessage(Some(heard.as_ref().unchecked_ref()));
            // Lives as long as the tab, like the shell that owns the session.
            heard.forget();
            std::mem::forget(channel);
        }
        Effect::new(move |_| refresh(session));
    }

    /// Tells the other tabs to ask the server. Carries no account data.
    fn announce() {
        if let Ok(channel) = web_sys::BroadcastChannel::new(CHANNEL) {
            let _ = channel.post_message(&JsValue::from_str("changed"));
            channel.close();
        }
    }

    async fn observe() -> Observation {
        match crate::api::session_state().await {
            Ok(state) => match state.state {
                Kind::Anonymous => Observation::Anonymous,
                Kind::NeedsPrincipal => Observation::NeedsPrincipal,
                Kind::Suspended => Observation::Suspended,
                Kind::Active => match crate::api::current_account().await {
                    Ok(account) => Observation::Active(account),
                    // Signed out between the two reads.
                    Err(ApiError::Unauthenticated) => Observation::Anonymous,
                    Err(_) => Observation::Unavailable,
                },
            },
            Err(_) => Observation::Unavailable,
        }
    }

    pub(super) fn refresh(session: Session) {
        let Some(ticket) = session.owner.try_update(SessionOwner::begin_check) else {
            return;
        };
        spawn_local(async move {
            let observation = observe().await;
            let signed_in = !matches!(
                observation,
                Observation::Anonymous | Observation::Unavailable
            );
            let applied = session
                .owner
                .try_update(|owner| {
                    let before = owner.standing().clone();
                    owner.settle(ticket, observation);
                    before != *owner.standing()
                })
                .unwrap_or(false);
            if !applied {
                return;
            }
            // A session exists now (perhaps from another tab): an open code exchange is over.
            if signed_in
                && session
                    .flow
                    .with_untracked(|flow| flow.step() == Step::Code || flow.error().is_some())
            {
                session.flow.update(SignInFlow::reset);
            }
            session.exit.set(Exit::Idle);
            session.naming.update(|naming| naming.error = None);
        });
    }

    pub(super) fn request_code(session: Session, email: String) {
        let Some((attempt, email)) = session
            .flow
            .try_update(|flow| flow.request_code(&email, now()))
            .flatten()
        else {
            return;
        };
        spawn_local(async move {
            let outcome = crate::api::request_sign_in_code(email).await;
            session.flow.try_update(|flow| match outcome {
                Ok(sent) => flow.code_sent(attempt, now(), instant(&sent.expires_at)),
                Err(error) => flow.failed(attempt, error, now()),
            });
        });
    }

    pub(super) fn submit_code(session: Session, code: String) {
        let Some((attempt, code)) = session
            .flow
            .try_update(|flow| flow.submit_code(&code))
            .flatten()
        else {
            return;
        };
        spawn_local(async move {
            match crate::api::complete_sign_in(code).await {
                Ok(_) => {
                    session.flow.try_update(|flow| flow.signed_in(attempt));
                    announce();
                    refresh(session);
                }
                Err(error) => {
                    // Without an answer the session may exist all the same: ask.
                    let lost = error == AuthError::Network;
                    session
                        .flow
                        .try_update(|flow| flow.failed(attempt, error, now()));
                    if lost {
                        refresh(session);
                    }
                }
            }
        });
    }

    pub(super) fn choose_pen_name(session: Session, pen_name: String) {
        if session.naming.with_untracked(|naming| naming.busy) {
            return;
        }
        if pen_name.trim().is_empty() {
            session
                .naming
                .update(|naming| naming.error = Some(AuthError::InvalidPenName));
            return;
        }
        session.naming.update(|naming| {
            naming.busy = true;
            naming.error = None;
        });
        spawn_local(async move {
            // Provisioning is idempotent on the server: a retry after a lost answer returns the
            // principal already bound and never creates a second one.
            let outcome = crate::api::provision_principal(pen_name).await;
            session.naming.try_update(|naming| {
                naming.busy = false;
                naming.error = outcome.as_ref().err().cloned();
            });
            if !matches!(outcome, Err(AuthError::InvalidPenName)) {
                if outcome.is_ok() {
                    announce();
                }
                refresh(session);
            }
        });
    }

    pub(super) fn sign_out(session: Session) {
        if session.exit.with_untracked(|exit| *exit == Exit::Pending) {
            return;
        }
        session.exit.set(Exit::Pending);
        spawn_local(async move {
            match crate::api::sign_out().await {
                Ok(()) => {
                    // The server revoked the session: private views let go at once, and the
                    // check that follows only confirms it.
                    session.owner.try_update(|owner| {
                        let ticket = owner.begin_check();
                        owner.settle(ticket, Observation::Anonymous);
                    });
                    session.exit.try_set(Exit::Idle);
                    announce();
                    refresh(session);
                }
                Err(error) => {
                    session.exit.try_set(Exit::Unconfirmed(error));
                }
            }
        });
    }
}

fn summary(standing: &Standing) -> String {
    match standing {
        Standing::Unknown => "Đang kiểm tra phiên đăng nhập…".into(),
        Standing::Anonymous => "Bạn chưa đăng nhập.".into(),
        Standing::NeedsPrincipal => "Đã xác nhận email. Hãy đặt bút danh để bắt đầu.".into(),
        Standing::Suspended => "Tài khoản đang bị tạm khóa trên Vink.".into(),
        Standing::Active(account) => account.display_name.clone(),
        Standing::Unavailable => "Chưa kiểm tra được phiên đăng nhập.".into(),
    }
}

fn standing_id(standing: &Standing) -> &'static str {
    match standing {
        Standing::Unknown => "unknown",
        Standing::Anonymous => "anonymous",
        Standing::NeedsPrincipal => "needs_principal",
        Standing::Suspended => "suspended",
        Standing::Active(_) => "active",
        Standing::Unavailable => "unavailable",
    }
}

fn exit_message(error: &AuthError) -> &'static str {
    match error {
        AuthError::Unavailable | AuthError::Network => {
            "Chưa xác nhận được việc đăng xuất. Phiên có thể vẫn còn hiệu lực trên thiết bị này; hãy thử lại."
        }
        _ => "Đăng xuất chưa hoàn tất. Hãy thử lại.",
    }
}

/// "10:10" in the person's own time zone, for "the code is valid until".
fn clock(at: crate::session::Millis) -> Option<String> {
    #[cfg(feature = "hydrate")]
    {
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(at as f64));
        Some(format!("{:02}:{:02}", date.get_hours(), date.get_minutes()))
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = at;
        None
    }
}

/// Seconds tick while a resend wait is shown; nothing runs otherwise.
fn use_clock() -> ReadSignal<crate::session::Millis> {
    #[cfg(feature = "hydrate")]
    {
        let (now, set_now) = signal(browser::now());
        if let Ok(handle) = set_interval_with_handle(
            move || set_now.set(browser::now()),
            std::time::Duration::from_secs(1),
        ) {
            on_cleanup(move || handle.clear());
        }
        now
    }
    #[cfg(not(feature = "hydrate"))]
    {
        signal(0).0
    }
}

fn pending(busy: bool, status: &'static str) -> ActionState {
    if busy {
        ActionState::pending(status)
    } else {
        ActionState::Enabled
    }
}

/// The account region of “Tôi”: who is signed in, and the way in or out.
#[component]
pub(crate) fn AccountPanel(id: &'static str) -> impl IntoView {
    let session = Session::current();
    let standing = session.standing;
    let fixture = crate::api::local_discussion_fixture_scope().is_some();
    let open = crate::api::sign_in_open();
    view! {
        <section id=id class="vk-account" aria-label="Tài khoản">
            <div class="vk-account-status">
                <crate::appearance::AccountAvatar />
                <div>
                    <vink_ui::Heading level=vink_ui::HeadingLevel::H2 role=vink_ui::TypeRole::Title>"Tài khoản"</vink_ui::Heading>
                    <vink_ui::Text tone=vink_ui::TextTone::Muted>
                        {move || standing.with(summary)}
                    </vink_ui::Text>
                </div>
                <svg class="vk-account-illustration" viewBox="0 0 224 82" aria-hidden="true" focusable="false">
                    <path fill="var(--vk-primary-container)" opacity=".72" d="M9 42c1-8 8-14 16-15 7-11 24-12 31 0 9 0 15 5 15 12 0 5-4 9-11 9H19c-6 0-11-2-10-6ZM155 34c1-6 6-9 12-9 5-10 19-10 24-2 8 0 14 5 14 10 0 6-5 9-11 9h-30c-6 0-10-3-9-8ZM175 69c2-7 9-12 16-11 5-8 17-9 23-2 6 1 10 5 10 10s-5 10-10 10h-33c-5 0-7-3-6-7Z" />
                    <path fill="var(--vk-primary)" opacity=".22" d="M26 75l32-43c23-3 39 2 51 15 16-11 33-12 51-3l-23 31H26Z" />
                    <path fill="none" stroke="var(--vk-primary-container)" stroke-width="3" stroke-linecap="round" d="M86 75l23-28m-79 27c19-6 37-5 56 1 17-5 32-5 49 0" />
                    <path fill="var(--vk-primary)" d="M119 53c4-18 15-34 34-45 4-2 12-5 14-3 2 2-2 9-4 12-11 17-24 29-44 36Z" />
                    <path fill="none" stroke="var(--vk-primary-container)" stroke-width="1.6" stroke-linecap="round" d="M119 52c13-17 25-30 41-42" />
                    <path fill="none" stroke="var(--vk-primary)" stroke-width="2.2" stroke-linecap="round" d="m121 50-8 11" />
                </svg>
            </div>
            <div id="account-session" class="vk-account-session" data-standing=move || standing.with(standing_id)>
                {move || match standing.get() {
                    Standing::Unknown => ().into_any(),
                    Standing::Unavailable => view! {
                        <Button id="session-retry" variant=ButtonVariant::Tonal on_press=move |_| session.refresh()>"Kiểm tra lại"</Button>
                    }.into_any(),
                    Standing::Anonymous if !open => view! {
                        <p id="sign-in-closed" class="vk-muted">"Vink chưa mở đăng nhập. Bạn vẫn có thể đọc các tác phẩm công khai."</p>
                    }.into_any(),
                    Standing::Anonymous => view! { <SignIn /> }.into_any(),
                    Standing::NeedsPrincipal => view! { <PenName /> }.into_any(),
                    Standing::Suspended => view! {
                        <p class="vk-muted">"Bạn vẫn có thể đọc các tác phẩm công khai. Hãy liên hệ Vink nếu cần được xem xét lại."</p>
                        <SignOut />
                    }.into_any(),
                    Standing::Active(_) if fixture => view! {
                        <p class="vk-muted">"Định danh thử trên thiết bị này · Không phải tài khoản thật."</p>
                    }.into_any(),
                    Standing::Active(_) => view! {
                        <div class="vk-account-actions">
                            <vink_ui::Link href="/ban-viet" variant=vink_ui::LinkVariant::Standalone>"Mở Bàn viết"</vink_ui::Link>
                        </div>
                        <SignOut />
                    }.into_any(),
                }}
            </div>
            <noscript><p class="vk-muted">"Đăng nhập cần JavaScript. Bạn vẫn có thể đọc các tác phẩm công khai."</p></noscript>
        </section>
    }
}

#[component]
fn SignIn() -> impl IntoView {
    let session = Session::current();
    let flow = session.flow;
    let step = Memo::new(move |_| flow.with(SignInFlow::step));
    let busy = Memo::new(move |_| flow.with(SignInFlow::busy));
    let error = Memo::new(move |_| flow.with(|f| f.error().map(AuthError::message)));
    let email = RwSignal::new(flow.with_untracked(|f| f.email().to_owned()));
    let code = RwSignal::new(String::new());
    let now = use_clock();
    let wait = Memo::new(move |_| flow.with(|f| f.resend_wait(now.get())));
    let resend_state = Signal::derive(move || match wait.get() {
        _ if busy.get() => ActionState::Disabled { reason: None },
        Some(seconds) => ActionState::disabled(format!(
            "Có thể gửi lại sau {}.",
            crate::session::wait_text(seconds)
        )),
        None => ActionState::Enabled,
    });
    view! {
        {move || match step.get() {
            Step::Email => view! {
                <form id="sign-in-email" class="vk-account-form" novalidate
                    on:submit=move |event| {
                        event.prevent_default();
                        session.request_code(email.get_untracked());
                    }>
                    <p class="vk-muted">"Nhập email để nhận mã đăng nhập gồm 6 chữ số. Vink không dùng mật khẩu."</p>
                    <TextField id="sign-in-address" label="Email" name="email" kind=TextKind::Email
                        autocomplete="email" required=true max_length=254
                        value=email error=Signal::derive(move || error.get())
                        on_input=move |input: vink_ui::TextInput| email.set(input.value) />
                    <div class="vk-account-actions">
                        <Button id="sign-in-send" variant=ButtonVariant::Filled kind=ButtonKind::Submit
                            state=Signal::derive(move || match wait.get() {
                                _ if busy.get() => ActionState::pending("Đang gửi mã…"),
                                Some(seconds) => ActionState::disabled(format!(
                                    "Có thể xin mã sau {}.",
                                    crate::session::wait_text(seconds)
                                )),
                                None => ActionState::Enabled,
                            })>"Gửi mã đăng nhập"</Button>
                    </div>
                </form>
            }.into_any(),
            Step::Code => view! {
                <form id="sign-in-code" class="vk-account-form" novalidate
                    on:submit=move |event| {
                        event.prevent_default();
                        session.submit_code(code.get_untracked());
                    }>
                    <p id="sign-in-sent" role="status">
                        "Đã gửi mã tới " <strong>{move || flow.with(|f| f.email().to_owned())}</strong> "."
                        {move || flow.with(SignInFlow::expires_at).and_then(clock).map(|at| format!(" Mã có hiệu lực đến {at}."))}
                        " Nếu chưa thấy thư, hãy xem cả mục thư rác."
                    </p>
                    <TextField id="sign-in-digits" label="Mã 6 chữ số" name="code"
                        autocomplete="one-time-code" required=true max_length=12
                        value=code error=Signal::derive(move || error.get())
                        on_input=move |input: vink_ui::TextInput| code.set(input.value) />
                    <div class="vk-account-actions">
                        <Button id="sign-in-complete" variant=ButtonVariant::Filled kind=ButtonKind::Submit
                            state=Signal::derive(move || pending(busy.get(), "Đang kiểm tra mã…"))>"Đăng nhập"</Button>
                        <Button id="sign-in-resend" variant=ButtonVariant::Text state=resend_state
                            on_press=move |_| {
                                code.set(String::new());
                                session.request_code(flow.with_untracked(|f| f.email().to_owned()));
                            }>"Gửi lại mã"</Button>
                        <Button id="sign-in-change" variant=ButtonVariant::Text
                            state=Signal::derive(move || if busy.get() { ActionState::Disabled { reason: None } } else { ActionState::Enabled })
                            on_press=move |_| {
                                code.set(String::new());
                                flow.update(SignInFlow::change_email);
                            }>"Dùng email khác"</Button>
                    </div>
                </form>
            }.into_any(),
        }}
    }
}

#[component]
fn PenName() -> impl IntoView {
    let session = Session::current();
    let naming = session.naming;
    let pen_name = RwSignal::new(String::new());
    let composing = RwSignal::new(false);
    view! {
        <form id="pen-name-form" class="vk-account-form" novalidate
            on:submit=move |event| {
                event.prevent_default();
                // Enter that only ends an IME composition must not submit a half-typed name.
                if !composing.get_untracked() {
                    session.choose_pen_name(pen_name.get_untracked());
                }
            }>
            <TextField id="pen-name" label="Bút danh" name="pen_name" autocomplete="nickname"
                required=true max_length=80
                help="Tên hiển thị cùng bài viết và nhận xét của bạn. Bút danh không dùng để đăng nhập."
                value=pen_name
                error=Signal::derive(move || naming.with(|n| n.error.as_ref().map(AuthError::message)))
                on_input=move |input: vink_ui::TextInput| {
                    composing.set(input.composing);
                    pen_name.set(input.value);
                } />
            <div class="vk-account-actions">
                <Button id="pen-name-save" variant=ButtonVariant::Filled kind=ButtonKind::Submit
                    state=Signal::derive(move || pending(naming.with(|n| n.busy), "Đang lưu bút danh…"))>"Lưu bút danh"</Button>
            </div>
        </form>
        <SignOut />
    }
}

#[component]
fn SignOut() -> impl IntoView {
    let session = Session::current();
    let exit = session.exit;
    view! {
        <div class="vk-account-actions">
            <Button id="sign-out" variant=ButtonVariant::Tonal
                state=Signal::derive(move || pending(exit.with(|e| *e == Exit::Pending), "Đang đăng xuất…"))
                on_press=move |_| session.sign_out()>"Đăng xuất"</Button>
        </div>
        {move || match exit.get() {
            Exit::Unconfirmed(error) => view! {
                <div id="sign-out-unconfirmed">
                    <InlineNotice tone=Tone::Error announce=Announce::Assertive
                        title="Chưa đăng xuất xong." message=exit_message(&error) />
                </div>
            }.into_any(),
            _ => ().into_any(),
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::CurrentAccount;

    #[test]
    fn account_summary_names_every_standing_without_inventing_an_identity() {
        let active = Standing::Active(CurrentAccount {
            principal_id: "p".into(),
            display_name: "An Nhiên".into(),
        });
        assert_eq!(summary(&active), "An Nhiên");
        for (standing, id) in [
            (Standing::Unknown, "unknown"),
            (Standing::Anonymous, "anonymous"),
            (Standing::NeedsPrincipal, "needs_principal"),
            (Standing::Suspended, "suspended"),
            (Standing::Unavailable, "unavailable"),
            (active, "active"),
        ] {
            assert_eq!(standing_id(&standing), id);
            assert!(!summary(&standing).is_empty());
        }
        // A logout the server did not confirm is never reported as done.
        assert!(exit_message(&AuthError::Unavailable).contains("vẫn còn hiệu lực"));
        assert!(exit_message(&AuthError::Network).contains("vẫn còn hiệu lực"));
    }
}
