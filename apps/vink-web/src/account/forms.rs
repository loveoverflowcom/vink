//! Form lifetimes own the inputs. Only the mounted form can display an answer;
//! the tab still reconciles any write that could have changed the browser's cookie.

use leptos::{html::Input, prelude::*};
use vink_ui::{
    ActionState, Announce, Button, ButtonKind, ButtonVariant, FieldVariant, InlineNotice,
    TextField, TextInput, TextKind, Tone,
};

use super::{Command, Reply, Session, flow::Operation};
use crate::{
    contract::{EnrollPasswordRequest, PasswordCredentials},
    session::{AuthError, Millis},
};

fn use_clock() -> ReadSignal<Millis> {
    #[cfg(feature = "hydrate")]
    {
        let (now, set_now) = signal(super::browser::now());
        if let Ok(handle) = set_interval_with_handle(
            move || set_now.set(super::browser::now()),
            std::time::Duration::from_secs(1),
        ) {
            on_cleanup(move || handle.clear());
        }
        now
    }
    #[cfg(not(feature = "hydrate"))]
    signal(0).0
}

fn instant(at: Millis) -> String {
    #[cfg(feature = "hydrate")]
    {
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(at as f64));
        format!(
            " Mã có hiệu lực đến {:02}:{:02}.",
            date.get_hours(),
            date.get_minutes()
        )
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = at;
        String::new()
    }
}

#[derive(Clone, Copy)]
struct InputValue {
    node: NodeRef<Input>,
    composing: RwSignal<bool>,
}

impl InputValue {
    fn new() -> Self {
        let this = Self {
            node: NodeRef::new(),
            composing: RwSignal::new(false),
        };
        // Drop the actual value even if a browser still holds the detached input briefly.
        #[cfg(feature = "hydrate")]
        on_cleanup(move || this.clear());
        this
    }

    // Read the native value on submit: autofill need not send an input event. Values are not
    // mirrored into reactive state or into the HTML value attribute.
    fn read(self) -> Option<String> {
        (!self.composing.get_untracked())
            .then(|| self.node.get_untracked().map(|node| node.value()))
            .flatten()
    }

    fn edit(self) -> Callback<TextInput> {
        Callback::new(move |input: TextInput| self.composing.set(input.composing))
    }

    fn clear(self) {
        if let Some(Some(node)) = self.node.try_get_untracked() {
            node.set_value("");
        }
    }

    fn focus_on_mount(self, enabled: bool) {
        #[cfg(feature = "hydrate")]
        if enabled {
            let pending = StoredValue::new(true);
            Effect::new(move |_| {
                if let Some(node) = self.node.get()
                    && pending.get_value()
                {
                    pending.set_value(false);
                    let _ = node.focus();
                }
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = enabled;
    }
}

fn empty_value() -> Signal<String> {
    Signal::stored(String::new())
}

fn field_error(
    error: RwSignal<Option<AuthError>>,
    matches: impl Fn(&AuthError) -> bool + Send + Sync + 'static,
) -> Signal<Option<String>> {
    Signal::derive(move || {
        error.with(|e| {
            e.as_ref()
                .filter(|error| matches(error))
                .map(AuthError::message)
        })
    })
}

fn action_error(operation: Operation, error: &AuthError) -> String {
    match error {
        AuthError::NotOpen => operation.unavailable().into(),
        AuthError::Forbidden
            if matches!(
                operation,
                Operation::Enrollment | Operation::EnrollmentChallenge
            ) =>
        {
            "Tài khoản chưa thể thêm mật khẩu bằng cách này. Cần phiên hợp lệ và email đã xác minh."
                .into()
        }
        _ => error.message(),
    }
}

fn password_field_error(error: &AuthError) -> bool {
    matches!(
        error,
        AuthError::InvalidUsername
            | AuthError::UsernameTaken
            | AuthError::InvalidPassword
            | AuthError::InvalidCode
            | AuthError::CodeRejected
    )
}

#[component]
fn FormError(
    error: RwSignal<Option<AuthError>>,
    operation: Operation,
    #[prop(optional)] password_fields: bool,
    #[prop(optional)] waiting: Signal<Option<u32>>,
) -> impl IntoView {
    move || {
        error.get().and_then(|error| {
            if matches!(error, AuthError::RateLimited { retry_after_seconds: Some(_) }) {
                return waiting.get().map(|_| view! {
                    <p class="vk-ui-visually-hidden" role="alert">"Có quá nhiều yêu cầu."</p>
                }.into_any());
            }
            if password_fields && password_field_error(&error) {
                // Announce validation once, with visible guidance attached to its field.
                Some(view! { <p class="vk-ui-visually-hidden" role="alert">{error.message()}</p> }.into_any())
            } else {
                Some(view! { <p class="vk-account-error" role="alert">{action_error(operation, &error)}</p> }.into_any())
            }
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Login,
    Register,
    Email,
}

#[component]
pub(super) fn GuestAuth(epoch: u64) -> impl IntoView {
    let mode = RwSignal::new((Mode::Login, false));
    let now = use_clock();
    let switch = Callback::new(move |next| mode.set((next, true)));
    move || match mode.get() {
        (Mode::Login, focus) => {
            view! { <GuestPassword epoch now focus switch mode=Mode::Login /> }.into_any()
        }
        (Mode::Register, focus) => {
            view! { <GuestPassword epoch now focus switch mode=Mode::Register /> }.into_any()
        }
        (Mode::Email, focus) => view! { <EmailSignIn epoch now focus switch /> }.into_any(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PasswordUse {
    Login,
    Register,
    Enroll,
}

#[component]
fn PasswordFields(
    purpose: PasswordUse,
    username: InputValue,
    password: InputValue,
    error: RwSignal<Option<AuthError>>,
) -> impl IntoView {
    let (user_id, password_id, autocomplete) = match purpose {
        PasswordUse::Login => (
            "password-login-username",
            "password-login-password",
            "current-password",
        ),
        PasswordUse::Register => (
            "password-register-username",
            "password-register-password",
            "new-password",
        ),
        PasswordUse::Enroll => (
            "password-enroll-username",
            "password-enroll-password",
            "new-password",
        ),
    };
    let help = (purpose != PasswordUse::Login).then_some(
        "3–30 ký tự: chữ cái không dấu, số, dấu _ hoặc dấu chấm. Không phân biệt hoa thường. Tên đăng nhập khác bút danh."
    );
    let password_help = (purpose != PasswordUse::Login).then_some(
        "10–128 ký tự sau khi chuẩn hóa Unicode, tối đa 4.096 byte nhập. Không bắt buộc chữ hoa, số hay ký hiệu."
    );
    let mut user = vink_ui::TextFieldProps::builder()
        .id(user_id)
        .name("username")
        .label("Tên đăng nhập")
        .autocomplete("username")
        .variant(FieldVariant::FloatingTonal)
        .required(true)
        .input_ref(username.node)
        .value(empty_value())
        .on_input(username.edit())
        .error(field_error(error, |e| {
            matches!(e, AuthError::InvalidUsername | AuthError::UsernameTaken)
        }))
        .build();
    user.help = help.map(str::to_owned);
    let mut secret = vink_ui::TextFieldProps::builder()
        .id(password_id)
        .name("password")
        .label("Mật khẩu")
        .autocomplete(autocomplete)
        .kind(TextKind::Password)
        .variant(FieldVariant::FloatingTonal)
        .reveal_password(true)
        .required(true)
        .input_ref(password.node)
        .value(empty_value())
        .on_input(password.edit())
        .error(field_error(error, |e| *e == AuthError::InvalidPassword))
        .build();
    secret.help = password_help.map(str::to_owned);
    view! { {TextField(user)} {TextField(secret)} }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GuestResult {
    Ready,
    CheckSession,
    Uncertain,
}

#[component]
fn GuestPassword(
    epoch: u64,
    now: ReadSignal<Millis>,
    focus: bool,
    switch: Callback<Mode>,
    mode: Mode,
) -> impl IntoView {
    let session = Session::current();
    let (operation, purpose, title, form_id, submit_id) = match mode {
        Mode::Register => (
            Operation::PasswordRegister,
            PasswordUse::Register,
            "Tạo tài khoản",
            "password-register",
            "password-register-submit",
        ),
        _ => (
            Operation::PasswordLogin,
            PasswordUse::Login,
            "Đăng nhập",
            "password-login",
            "password-login-submit",
        ),
    };
    let is_register = mode == Mode::Register;
    let username = InputValue::new();
    let password = InputValue::new();
    username.focus_on_mount(focus);
    let error = RwSignal::new(None);
    let result = RwSignal::new(GuestResult::Ready);
    let state = session.action(operation, now);
    let can_submit = move || result.get() == GuestResult::Ready;
    view! {
        <form id=form_id class="vk-account-form" method="post" novalidate
            on:submit=move |event| {
                event.prevent_default();
                if result.get_untracked() != GuestResult::Ready { return; }
                let (Some(user), Some(secret)) = (username.read(), password.read()) else { return; };
                // Use server validation for registration and login, including legacy passwords.
                let input = PasswordCredentials { username: user, password: secret };
                let command = if is_register { Command::Register(input) } else { Command::Login(input) };
                session.dispatch(epoch, command, move |outcome| {
                    if result.try_get_untracked().is_none() { return; }
                    let next = match outcome {
                        Ok(_) => {
                            username.clear();
                            password.clear();
                            GuestResult::CheckSession
                        }
                        Err(AuthError::AlreadySignedIn) => {
                            username.clear();
                            password.clear();
                            GuestResult::CheckSession
                        }
                        Err(error_value) if error_value.uncertain() => GuestResult::Uncertain,
                        Err(error_value) => {
                            error.set(Some(error_value));
                            GuestResult::Ready
                        }
                    };
                    if next != GuestResult::Ready { error.set(None); }
                    result.set(next);
                });
            }>
            <PasswordFields purpose username password error />
            <FormError error operation password_fields=true waiting=session.retry_wait(operation, now) />
            {move || match result.get() {
                GuestResult::Ready => ().into_any(),
                GuestResult::CheckSession => view! {
                    <p role="status">"Hãy kiểm tra phiên đăng nhập để tiếp tục."</p>
                }.into_any(),
                GuestResult::Uncertain if is_register => view! {
                    <p class="vk-account-error" role="alert">
                        "Chưa xác nhận được việc tạo tài khoản. Tài khoản có thể đã được tạo. Hãy kiểm tra phiên hoặc thử đăng nhập bằng tên bạn vừa chọn."
                    </p>
                }.into_any(),
                GuestResult::Uncertain => view! {
                    <p class="vk-account-error" role="alert">"Chưa xác nhận được việc đăng nhập. Hãy kiểm tra phiên trước khi thử lại."</p>
                }.into_any(),
            }}
            <div class="vk-account-actions">
                <Button id=submit_id variant=ButtonVariant::Filled kind=ButtonKind::Submit
                    state=Signal::derive(move || if can_submit() { state.get() } else { ActionState::Disabled { reason: None } })>{title}</Button>
                {move || (result.get() != GuestResult::Ready).then(|| view! {
                    <Button id="password-check-session" variant=ButtonVariant::Tonal
                        state=Signal::derive(move || if session.checking.get() {
                            ActionState::pending("Đang kiểm tra phiên…")
                        } else { ActionState::Enabled })
                        on_press=move |_| session.refresh()>"Kiểm tra phiên"</Button>
                })}
                {move || (!is_register && result.get() != GuestResult::Ready).then(|| view! {
                    <Button id="password-login-retry" variant=ButtonVariant::Text
                        state=Signal::derive(move || if session.checking.get() || session.auth.with(|g| g.needs_check()) {
                            ActionState::Disabled { reason: None }
                        } else { ActionState::Enabled })
                        on_press=move |_| result.set(GuestResult::Ready)>"Thử đăng nhập lại"</Button>
                })}
            </div>
        </form>
        <div class="vk-account-actions vk-auth-alternatives">
            {if is_register {
                view! { <Button id="use-password" variant=ButtonVariant::Text
                    on_press=move |_| switch.run(Mode::Login)>"Quay lại đăng nhập"</Button> }.into_any()
            } else {
                view! { <Button id="use-registration" variant=ButtonVariant::Text
                    on_press=move |_| switch.run(Mode::Register)>"Tạo tài khoản"</Button> }.into_any()
            }}
            <Button id="use-email-code" variant=ButtonVariant::Text
                on_press=move |_| switch.run(Mode::Email)>"Dùng mã email"</Button>
        </div>
    }
}

#[derive(Clone, Copy)]
struct Challenge {
    expires: Millis,
}

fn code_value(input: InputValue, error: RwSignal<Option<AuthError>>) -> Option<String> {
    let code = input
        .read()?
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    if code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()) {
        Some(code)
    } else {
        error.set(Some(AuthError::InvalidCode));
        None
    }
}

#[component]
fn EmailSignIn(
    epoch: u64,
    now: ReadSignal<Millis>,
    focus: bool,
    switch: Callback<Mode>,
) -> impl IntoView {
    let session = Session::current();
    let challenge = RwSignal::new(None::<Challenge>);
    let address = RwSignal::new(String::new());
    let error = RwSignal::new(None);
    let response_uncertain = RwSignal::new(false);
    let email = InputValue::new();
    let code = InputValue::new();
    email.focus_on_mount(focus);
    let entered_code = Memo::new(move |_| challenge.with(Option::is_some));
    let request = move |value: String| {
        let value = value.trim().to_owned();
        if !value.contains('@') {
            error.set(Some(AuthError::InvalidEmail));
            return;
        }
        session.dispatch(
            epoch,
            Command::RequestEmailCode(value.clone()),
            move |outcome| {
                if challenge.try_get_untracked().is_none() {
                    return;
                }
                match outcome {
                    Ok(Reply::Challenge(expires)) => {
                        address.set(value);
                        code.clear();
                        challenge.set(Some(Challenge { expires }));
                        error.set(None);
                        response_uncertain.set(false);
                    }
                    Err(failure) => {
                        if failure.uncertain() {
                            // A resend may have replaced the challenge cookie even without an ACK.
                            code.clear();
                            challenge.set(None);
                        }
                        if !entered_code.get_untracked() {
                            address.set(value);
                        }
                        error.set(Some(failure));
                    }
                    _ => {}
                }
            },
        );
    };
    let send_state = session.action(Operation::EmailChallenge, now);
    let complete_state = session.action(Operation::EmailCode, now);
    view! {
        {move || if entered_code.get() {
            view! {
                <form id="sign-in-code" class="vk-account-form" method="post" novalidate
                    on:submit=move |event| {
                        event.prevent_default();
                        if response_uncertain.get_untracked() { return; }
                        let Some(value) = code_value(code, error) else { return; };
                        session.dispatch(epoch, Command::EmailCode(value), move |outcome| {
                            if challenge.try_get_untracked().is_none() { return; }
                            match outcome {
                                Ok(_) => {
                                    code.clear();
                                    address.set(String::new());
                                    error.set(None);
                                    response_uncertain.set(true);
                                }
                                Err(failure) => {
                                    if matches!(failure, AuthError::ChallengeClosed | AuthError::AccountDisabled | AuthError::Suspended) {
                                        code.clear();
                                        challenge.set(None);

                                    }
                                    response_uncertain.set(failure.uncertain());
                                    error.set(Some(failure));
                                }
                            }
                        });
                    }>
                    <p id="sign-in-sent" role="status">
                        "Đã ghi nhận yêu cầu gửi mã tới " <strong>{address}</strong> "."
                        {move || challenge.with(|c| c.map(|value| instant(value.expires)))}
                        " Nếu chưa thấy thư, hãy xem cả mục thư rác."
                    </p>
                    <TextField id="sign-in-digits" label="Mã 6 chữ số" name="code"
                        autocomplete="one-time-code" required=true variant=FieldVariant::FloatingTonal
                        input_ref=code.node value=empty_value() on_input=code.edit()
                        error=field_error(error, |e| matches!(e, AuthError::InvalidCode | AuthError::CodeRejected)) />
                    {move || error.get().map(|e| {
                        let field = matches!(e, AuthError::InvalidCode | AuthError::CodeRejected);
                        view! { <p class=if field { "vk-ui-visually-hidden" } else { "vk-account-error" } role="alert">
                            {action_error(Operation::EmailCode, &e)}
                        </p> }
                    })}
                    <div class="vk-account-actions">
                        <Button id="sign-in-complete" variant=ButtonVariant::Filled kind=ButtonKind::Submit
                            state=Signal::derive(move || if response_uncertain.get() {
                                ActionState::Disabled { reason: None }
                            } else { complete_state.get() })>"Đăng nhập"</Button>
                        <Button id="sign-in-resend" variant=ButtonVariant::Text state=send_state
                            on_press=move |_| request(address.get_untracked())>"Gửi lại mã"</Button>
                        <Button id="sign-in-change" variant=ButtonVariant::Text
                            state=Signal::derive(move || if session.auth.with(|g| g.active().is_some()) {
                                ActionState::Disabled { reason: None }
                            } else { ActionState::Enabled })
                            on_press=move |_| {
                                code.clear();
                                challenge.set(None);
                                error.set(None);
                                response_uncertain.set(false);
                            }>"Dùng email khác"</Button>
                    </div>
                </form>
            }.into_any()
        } else {
            // NodeRefs belong to the whole email flow, and can be assigned a new element
            // each time the address step opens. Move focus only for an explicit mode switch.
            let first_value = Signal::stored(address.get_untracked());
            view! {
                <form id="sign-in-email" class="vk-account-form" method="post" novalidate
                    on:submit=move |event| {
                        event.prevent_default();
                        if let Some(value) = email.read() { request(value); }
                    }>
                    <p class="vk-muted">"Nhập email để nhận mã đăng nhập gồm 6 chữ số."</p>
                    <TextField id="sign-in-address" label="Email" name="email" kind=TextKind::Email
                        autocomplete="email" required=true max_length=254 variant=FieldVariant::FloatingTonal
                        input_ref=email.node value=first_value on_input=email.edit()
                        error=field_error(error, |e| *e == AuthError::InvalidEmail) />
                    {move || error.get().map(|e| {
                        view! { <p class=if e == AuthError::InvalidEmail { "vk-ui-visually-hidden" } else { "vk-account-error" } role="alert">
                            {action_error(Operation::EmailChallenge, &e)}
                        </p> }
                    })}
                    <div class="vk-account-actions">
                        <Button id="sign-in-send" variant=ButtonVariant::Filled kind=ButtonKind::Submit
                            state=send_state>"Gửi mã đăng nhập"</Button>
                    </div>
                </form>
            }.into_any()
        }}
        <div class="vk-account-actions vk-auth-alternatives">
            <Button id="use-password" variant=ButtonVariant::Text
                on_press=move |_| switch.run(Mode::Login)>"Dùng mật khẩu"</Button>
        </div>
    }
}

#[derive(Clone, PartialEq, Eq)]
enum Credential {
    Loading,
    Reconciling,
    Missing,
    Present,
    Unavailable(AuthError),
    Uncertain(Option<AuthError>),
}

#[component]
fn CredentialSettings(epoch: u64, now: ReadSignal<Millis>) -> impl IntoView {
    let session = Session::current();
    let status = RwSignal::new(Credential::Loading);
    let challenge = RwSignal::new(None::<Challenge>);
    let read = move |uncertain| {
        status.set(if uncertain {
            Credential::Reconciling
        } else {
            Credential::Loading
        });
        session.enrollment_status(
            epoch,
            move || status.try_get_untracked().is_some(),
            move |outcome| {
                let next = match (outcome, uncertain) {
                    (Ok(true), _) => Credential::Present,
                    (Ok(false), true) => Credential::Uncertain(None),
                    (Ok(false), false) => Credential::Missing,
                    (Err(error), true) => Credential::Uncertain(Some(error)),
                    (Err(error), false) => Credential::Unavailable(error),
                };
                status.try_set(next);
            },
        );
    };
    let started = StoredValue::new(false);
    Effect::new(move |_| {
        if !started.get_value() {
            started.set_value(true);
            read(false);
        }
    });
    let problem = RwSignal::new(None);
    let finished = Callback::new(move |outcome| {
        challenge.set(None);
        problem.set(None);
        match outcome {
            Ok(Reply::Done) | Err(AuthError::AlreadyEnrolled) => read(false),
            Err(error) if AuthError::uncertain(&error) => read(true),
            Err(AuthError::ChallengeClosed) => problem.set(Some(AuthError::ChallengeClosed)),
            _ => {}
        }
    });
    let step = Memo::new(move |_| challenge.with(Option::is_some));
    let send = Callback::new(move |()| {
        session.dispatch(epoch, Command::RequestEnrollmentCode, move |outcome| {
            if challenge.try_get_untracked().is_none() {
                return;
            }
            match outcome {
                Ok(Reply::Challenge(expires)) => {
                    challenge.set(Some(Challenge { expires }));
                    problem.set(None);
                }
                Err(AuthError::AlreadyEnrolled) => finished.run(Err(AuthError::AlreadyEnrolled)),
                Err(error) => {
                    if error.uncertain() {
                        challenge.set(None);
                    }
                    problem.set(Some(error));
                }
                _ => {}
            }
        });
    });
    let send_state = session.action(Operation::EnrollmentChallenge, now);
    view! {
        <div class="vk-account-form" id="password-credential-status">
            {move || match status.get() {
                Credential::Loading | Credential::Reconciling => view! { <p role="status">"Đang kiểm tra cách đăng nhập…"</p> }.into_any(),
                Credential::Present => view! {
                    <p role="status">"Tài khoản đã có thông tin đăng nhập bằng mật khẩu."</p>
                }.into_any(),
                Credential::Unavailable(error) => view! {
                    <p class="vk-account-error" role="alert">{action_error(Operation::Enrollment, &error)}</p>
                    {(error != AuthError::NotOpen).then(|| view! {
                        <div class="vk-account-actions">
                            <Button id="password-enroll-retry" variant=ButtonVariant::Tonal on_press=move |_| read(false)>"Kiểm tra lại"</Button>
                        </div>
                    })}
                }.into_any(),
                Credential::Uncertain(error) => view! {
                    <p class="vk-account-error" role="alert">
                        "Chưa xác nhận được việc thêm mật khẩu. Có thể yêu cầu trước vẫn đang được xử lý. Hãy kiểm tra trạng thái hoặc xin mã mới để bắt đầu lại."
                    </p>
                    {error.map(|error| view! { <p>{action_error(Operation::Enrollment, &error)}</p> })}
                    <div class="vk-account-actions">
                        <Button id="password-enroll-check" variant=ButtonVariant::Tonal on_press=move |_| read(true)>"Kiểm tra trạng thái"</Button>
                        <Button id="password-enroll-restart" variant=ButtonVariant::Text
                            on_press=move |_| {
                                problem.set(None);
                                read(false);
                            }>"Bắt đầu lại"</Button>
                    </div>
                }.into_any(),
                Credential::Missing => view! {
                    {move || if let Some(proof) = challenge.get() {
                        view! { <EnrollmentForm epoch now expires=proof.expires finished /> }.into_any()
                    } else {
                        view! { <p class="vk-muted">"Vink sẽ gửi mã xác minh tới email đã xác minh của tài khoản này."</p> }.into_any()
                    }}
                    <FormError error=problem operation=Operation::EnrollmentChallenge waiting=session.retry_wait(Operation::EnrollmentChallenge, now) />
                    <div class="vk-account-actions">
                        <Button id="password-enroll-send" variant=ButtonVariant::Tonal state=send_state
                            on_press=move |_| send.run(())>{move || if step.get() { "Gửi lại mã" } else { "Xin mã xác minh" }}</Button>
                    </div>
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn EnrollmentForm(
    epoch: u64,
    now: ReadSignal<Millis>,
    expires: Millis,
    finished: Callback<Result<Reply, AuthError>>,
) -> impl IntoView {
    let session = Session::current();
    let code = InputValue::new();
    let username = InputValue::new();
    let password = InputValue::new();
    let error = RwSignal::new(None);
    let operation = Operation::Enrollment;
    let state = session.action(operation, now);
    view! {
        <form id="password-enroll" class="vk-account-form" method="post" novalidate
            on:submit=move |event| {
                event.prevent_default();
                let (Some(user), Some(secret)) = (username.read(), password.read()) else { return; };
                let Some(digits) = code_value(code, error) else { return; };
                let proof = EnrollPasswordRequest { username: user, password: secret, code: digits };
                session.dispatch(epoch, Command::Enroll(proof), move |outcome| {
                    if error.try_get_untracked().is_none() { return; }
                    match outcome {
                        Err(failure) if !failure.uncertain() && !matches!(failure, AuthError::AlreadyEnrolled | AuthError::ChallengeClosed) =>
                            error.set(Some(failure)),
                        _ => {
                            code.clear();
                            username.clear();
                            password.clear();
                            finished.run(outcome);
                        }
                    }
                });
            }>
            <p id="password-enroll-sent" role="status">
                "Đã ghi nhận yêu cầu gửi mã xác minh."
                {instant(expires)}
            </p>
            <TextField id="password-enroll-code" label="Mã 6 chữ số" name="code" autocomplete="one-time-code"
                variant=FieldVariant::FloatingTonal required=true
                input_ref=code.node value=empty_value() on_input=code.edit()
                error=field_error(error, |e| matches!(e, AuthError::InvalidCode | AuthError::CodeRejected)) />
            <PasswordFields purpose=PasswordUse::Enroll username password error />
            <FormError error operation password_fields=true waiting=session.retry_wait(operation, now) />
            <div class="vk-account-actions">
                <Button id="password-enroll-submit" variant=ButtonVariant::Filled kind=ButtonKind::Submit
                    state=state>"Thêm mật khẩu"</Button>
            </div>
        </form>
    }
}

#[component]
pub(super) fn SignedIn(epoch: u64) -> impl IntoView {
    let session = Session::current();
    let opened = RwSignal::new(false);
    let now = use_clock();
    view! {
        <div class="vk-account-actions">
            <vink_ui::Link href="/ban-viet" variant=vink_ui::LinkVariant::Standalone>"Mở Bàn viết"</vink_ui::Link>
        </div>
        <div class="vk-account-form">
            <div class="vk-account-actions">
                <Button id="password-credential" variant=ButtonVariant::Text expanded=Signal::derive(move || opened.get())
                    state=Signal::derive(move || if !opened.get() && session.auth.with(|g| g.needs_check() || g.active().is_some()) {
                        ActionState::Disabled { reason: None }
                    } else { ActionState::Enabled })
                    on_press=move |_| opened.update(|open| *open = !*open)>"Tên đăng nhập và mật khẩu"</Button>
            </div>
            {move || opened.get().then(|| view! { <CredentialSettings epoch now /> })}
        </div>
        <SignOut epoch before=move |_| opened.set(false) />
    }
}

#[component]
pub(super) fn PenName(epoch: u64) -> impl IntoView {
    let session = Session::current();
    let now = use_clock();
    let pen_name = InputValue::new();
    let error = RwSignal::new(None);
    let state = session.action(Operation::PenName, now);
    view! {
        <form id="pen-name-form" class="vk-account-form" method="post" novalidate
            on:submit=move |event| {
                event.prevent_default();
                let Some(value) = pen_name.read() else { return; };
                if value.trim().is_empty() {
                    error.set(Some(AuthError::InvalidPenName));
                    return;
                }
                session.dispatch(epoch, Command::PenName(value), move |result| {
                    if result.is_ok() { pen_name.clear(); }
                    error.try_set(result.err());
                });
            }>
            <TextField id="pen-name" label="Bút danh" name="pen_name" autocomplete="nickname"
                required=true max_length=80 variant=FieldVariant::FloatingTonal
                help="Tên hiển thị cùng bài viết và nhận xét của bạn. Bút danh không dùng để đăng nhập."
                input_ref=pen_name.node value=empty_value() on_input=pen_name.edit()
                error=field_error(error, |e| *e == AuthError::InvalidPenName) />
            {move || error.get().filter(|e| *e != AuthError::InvalidPenName).map(|e| view! {
                <p class="vk-account-error" role="alert">{action_error(Operation::PenName, &e)}</p>
            })}
            <div class="vk-account-actions">
                <Button id="pen-name-save" variant=ButtonVariant::Filled kind=ButtonKind::Submit state=state>"Lưu bút danh"</Button>
            </div>
        </form>
        <SignOut epoch />
    }
}

#[component]
pub(super) fn SignOut(
    epoch: u64,
    #[prop(optional, into)] before: Option<Callback<()>>,
) -> impl IntoView {
    let session = Session::current();
    let error = RwSignal::new(None);
    let now = use_clock();
    let state = session.action(Operation::SignOut, now);
    view! {
        <div class="vk-account-actions">
            <Button id="sign-out" variant=ButtonVariant::Tonal state=state
                on_press=move |_| {
                    if let Some(before) = before { before.run(()); }
                    session.dispatch(epoch, Command::SignOut, move |outcome| { error.try_set(outcome.err()); });
                }>
                "Đăng xuất"
            </Button>
        </div>
        {move || error.get().map(|_| view! {
            <div id="sign-out-unconfirmed" class="vk-account-form">
                <InlineNotice tone=Tone::Error announce=Announce::Assertive
                    title="Chưa đăng xuất xong."
                    message="Chưa xác nhận được việc đăng xuất. Phiên có thể vẫn còn hiệu lực trên thiết bị này; hãy thử lại." />
            </div>
        })}
    }
}
