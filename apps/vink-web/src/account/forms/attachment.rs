//! First attachment never changes the session. The only successful visual result is a
//! read of the actual binding, scoped to the same principal and (after issuance) session.

use super::{InputValue, code_value, empty_value, field_error, instant};
use crate::{
    account::{Command, Reply, Session, flow::Operation},
    contract::{CompleteEmailAttachment, EmailAttachmentStatus, StartEmailAttachment},
    session::{AuthError, Millis},
};
use leptos::prelude::*;
use vink_ui::{Button, ButtonKind, ButtonVariant, FieldVariant, TextField, TextKind};

#[derive(Clone, PartialEq, Eq)]
struct Binding {
    session_id: String,
    target: String,
}

#[derive(Clone, PartialEq, Eq)]
enum Step {
    Loading,
    Hidden,
    Missing(String),
    Address {
        session_id: String,
        focus: bool,
    },
    Code {
        binding: Binding,
        challenge_id: String,
        expires: Millis,
    },
    Verified(String),
    DifferentBinding(String),
    Unavailable(AuthError),
    Unconfirmed(Binding),
}

// Only explicit transitions may transfer focus, and never from another focused control.
fn owns_focus() -> bool {
    #[cfg(feature = "browser")]
    {
        document().active_element().is_some_and(|element| {
            element
                .closest("#recovery-email-settings")
                .ok()
                .flatten()
                .is_some()
        })
    }
    #[cfg(not(feature = "browser"))]
    false
}

fn focus_after_render(step: RwSignal<Step>, id: &'static str) {
    #[cfg(feature = "browser")]
    {
        use wasm_bindgen::JsCast;
        request_animation_frame(move || {
            if step.try_get_untracked().is_none()
                || document()
                    .active_element()
                    .is_some_and(|element| element.tag_name() != "BODY")
            {
                return;
            }
            if let Some(element) = document()
                .get_element_by_id(id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
            {
                let _ = element.focus();
            }
        });
    }
    #[cfg(not(feature = "browser"))]
    let _ = (step, id);
}

// These branches never read a prior account's address into a replacement form. A successful
// POST only asks for this read; it isn't itself the binding shown to the person.
fn observed(status: Result<EmailAttachmentStatus, AuthError>, expected: Option<Binding>) -> Step {
    match (status, expected) {
        (Ok(status), Some(binding)) if status.session_id != binding.session_id => {
            Step::Unavailable(AuthError::AttachmentContextChanged)
        }
        (Ok(status), Some(binding)) => match status.verified_email {
            Some(email) if email == binding.target => Step::Verified(email),
            Some(email) => Step::DifferentBinding(email),
            None => Step::Unconfirmed(binding),
        },
        (Ok(status), None) if !status.password_account => Step::Hidden,
        (Ok(status), None) => match status.verified_email {
            Some(email) => Step::Verified(email),
            None => Step::Missing(status.session_id),
        },
        (Err(AuthError::NotOpen), None) => Step::Hidden,
        (Err(AuthError::AttachmentContextChanged), _) => {
            Step::Unavailable(AuthError::AttachmentContextChanged)
        }
        (Err(_), Some(binding)) => Step::Unconfirmed(binding),
        (Err(error), None) => Step::Unavailable(error),
    }
}

#[component]
pub(super) fn EmailSettings(epoch: u64, now: ReadSignal<Millis>) -> impl IntoView {
    let session = Session::current();
    let step = RwSignal::new(Step::Loading);
    let read = Callback::new(move |expected: Option<Binding>| {
        let resume_focus = owns_focus();
        step.set(Step::Loading);
        session.attachment_status(
            epoch,
            move || step.try_with_untracked(|s| *s == Step::Loading) == Some(true),
            move |status| {
                let next = observed(status, expected);
                let focus = match &next {
                    Step::Missing(_) => Some("email-attach-add"),
                    Step::Verified(_) => Some("email-attach-verified"),
                    Step::DifferentBinding(_) => Some("email-attach-different"),
                    Step::Unavailable(_) => Some("email-attach-retry"),
                    Step::Unconfirmed(_) => Some("email-attach-check"),
                    _ => None,
                };
                step.set(next);
                if let Some(id) = focus.filter(|_| resume_focus) {
                    focus_after_render(step, id);
                }
            },
        );
    });
    let started = StoredValue::new(false);
    Effect::new(move |_| {
        if !started.get_value() && !session.auth.with(|gate| gate.needs_check()) {
            started.set_value(true);
            read.run(None);
        }
    });
    move || {
        let state = step.get();
        if state == Step::Hidden {
            return None;
        }
        let body = match state {
            Step::Loading => view! { <p role="status" class="vk-muted">"Đang kiểm tra email…"</p> }.into_any(),
            Step::Hidden => ().into_any(),
            Step::Missing(session_id) => view! {
                <p id="email-attach-missing" class="vk-muted">"Chưa có email xác minh."</p>
                <div class="vk-account-actions">
                    <Button id="email-attach-add" variant=ButtonVariant::Tonal
                        on_press=move |_| step.set(Step::Address { session_id: session_id.clone(), focus: true })>
                        "Thêm email"
                    </Button>
                </div>
            }.into_any(),
            Step::Address { session_id, focus } => view! {
                <AddressForm epoch now session_id focus step read />
            }.into_any(),
            Step::Code { binding, challenge_id, expires } => view! {
                <CodeForm epoch now binding challenge_id expires step read />
            }.into_any(),
            Step::Verified(email) => view! {
                <p id="email-attach-verified" role="status" tabindex="-1">"Email đã xác minh: " <span>{email}</span></p>
            }.into_any(),
            Step::DifferentBinding(email) => view! {
                <p id="email-attach-different" class="vk-account-error" role="alert" tabindex="-1">
                    "Yêu cầu này chưa gắn địa chỉ bạn vừa nhập. Tài khoản đã có email xác minh khác."
                </p>
                <p>"Email đã xác minh: " <span>{email}</span></p>
            }.into_any(),
            Step::Unavailable(error) => view! {
                <p class="vk-account-error" role="alert">{error.message()}</p>
                <div class="vk-account-actions">
                    <Button id="email-attach-retry" variant=ButtonVariant::Tonal
                        on_press=move |_| read.run(None)>"Kiểm tra lại"</Button>
                </div>
            }.into_any(),
            Step::Unconfirmed(binding) => view! {
                <p id="email-attach-unconfirmed" class="vk-account-error" role="alert">
                    "Chưa xác nhận được email đã gắn. Yêu cầu trước có thể vẫn đang được xử lý."
                </p>
                <div class="vk-account-actions">
                    <Button id="email-attach-check" variant=ButtonVariant::Tonal
                        on_press=move |_| read.run(Some(binding.clone()))>"Kiểm tra trạng thái"</Button>
                    <Button id="email-attach-restart" variant=ButtonVariant::Text
                        on_press=move |_| read.run(None)>"Bắt đầu lại"</Button>
                </div>
            }.into_any(),
        };
        Some(view! {
            <section id="recovery-email-settings" class="vk-account-setting grid gap-vk-2" aria-labelledby="email-attach-heading">
                <vink_ui::Heading id="email-attach-heading" level=vink_ui::HeadingLevel::H3 role=vink_ui::TypeRole::Body
                    attr:class="flex items-center gap-vk-2">
                    <vink_ui::Icon name=vink_ui::IconName::VerifiedUser />
                    "Email xác minh"
                </vink_ui::Heading>
                {body}
            </section>
        })
    }
}

#[component]
fn AddressForm(
    epoch: u64,
    now: ReadSignal<Millis>,
    session_id: String,
    focus: bool,
    step: RwSignal<Step>,
    read: Callback<Option<Binding>>,
) -> impl IntoView {
    let session = Session::current();
    let email = InputValue::new();
    let password = InputValue::new();
    let error = RwSignal::new(None);
    let op = Operation::AttachmentChallenge;
    let state = session.action(op, now);
    let cancel_id = session_id.clone();
    email.focus_on_mount(focus);
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let (Some(address), Some(secret)) = (email.read(), password.read()) else {
            return;
        };
        if address.trim().is_empty() {
            error.set(Some(AuthError::InvalidEmail));
            return;
        }
        if secret.is_empty() {
            error.set(Some(AuthError::InvalidCredentials));
            return;
        }
        let live_session = session_id.clone();
        let input = StartEmailAttachment {
            email: address,
            password: secret,
            session_id: live_session.clone(),
        };
        session.dispatch(
            epoch,
            Command::RequestEmailAttachment(input),
            move |result| {
                if error.try_get_untracked().is_none() {
                    return;
                }
                password.clear();
                match result {
                    Ok(Reply::AttachmentChallenge {
                        id,
                        email: target,
                        expires,
                    }) => {
                        let resume_focus = owns_focus();
                        step.set(Step::Code {
                            binding: Binding {
                                session_id: live_session,
                                target,
                            },
                            challenge_id: id,
                            expires,
                        });
                        if resume_focus {
                            focus_after_render(step, "email-attach-code");
                        }
                    }
                    Err(AuthError::AlreadyAttached | AuthError::AttachmentNeedsPassword) => {
                        read.run(None)
                    }
                    Err(AuthError::AttachmentContextChanged) => {
                        step.set(Step::Unavailable(AuthError::AttachmentContextChanged))
                    }
                    Err(problem) => error.set(Some(problem)),
                    _ => error.set(Some(AuthError::Unexpected { status: 202 })),
                }
            },
        );
    };
    view! {
        <form id="email-attach-request" class="vk-account-form" method="post" novalidate on:submit=submit>
            <p class="vk-muted">"Để thêm email vào tài khoản này, hãy xác nhận bằng mật khẩu hiện tại và mã trong thư."</p>
            <TextField id="email-attach-address" label="Địa chỉ email" name="email" autocomplete="email"
                kind=TextKind::Email variant=FieldVariant::FloatingTonal required=true
                input_ref=email.node value=empty_value() on_input=email.edit()
                error=field_error(error, |e| *e == AuthError::InvalidEmail) />
            <TextField id="email-attach-password" label="Mật khẩu hiện tại" name="password"
                autocomplete="current-password" kind=TextKind::Password variant=FieldVariant::FloatingTonal
                reveal_password=true required=true input_ref=password.node value=empty_value() on_input=password.edit()
                error=Signal::derive(move || error.with(|e| matches!(e, Some(AuthError::InvalidCredentials | AuthError::InvalidPassword))
                    .then(|| "Hãy nhập đúng mật khẩu hiện tại.".to_owned()))) />
            <Problem error fields=address_field_error />
            <div class="vk-account-actions">
                <Button id="email-attach-send" variant=ButtonVariant::Filled kind=ButtonKind::Submit state=state>
                    "Xin mã xác minh"
                </Button>
                <Button id="email-attach-cancel" variant=ButtonVariant::Text
                    state=Signal::derive(move || if session.auth.with(|gate| gate.active().is_some()) {
                        vink_ui::ActionState::Disabled { reason: None }
                    } else { vink_ui::ActionState::Enabled })
                    on_press=move |_| {
                        step.set(Step::Missing(cancel_id.clone()));
                        focus_after_render(step, "email-attach-add");
                    }>"Hủy"</Button>
            </div>
        </form>
    }
}

fn address_field_error(error: &AuthError) -> bool {
    matches!(
        error,
        AuthError::InvalidEmail | AuthError::InvalidCredentials | AuthError::InvalidPassword
    )
}

fn code_field_error(error: &AuthError) -> bool {
    matches!(error, AuthError::InvalidCode | AuthError::CodeRejected)
}

#[component]
fn Problem(error: RwSignal<Option<AuthError>>, fields: fn(&AuthError) -> bool) -> impl IntoView {
    move || {
        error.get().map(|error| {
            let class = if fields(&error) {
                "vk-ui-visually-hidden"
            } else {
                "vk-account-error"
            };
            let message = match error {
                AuthError::InvalidPassword | AuthError::InvalidCredentials => {
                    "Hãy nhập đúng mật khẩu hiện tại.".into()
                }
                AuthError::NotOpen => "Vink chưa hỗ trợ thêm email trên hệ thống này.".into(),
                AuthError::Network | AuthError::Unavailable => {
                    "Chưa nhận được phản hồi từ máy chủ. Hãy kiểm tra kết nối rồi thử lại.".into()
                }
                _ => error.message(),
            };
            view! { <p id="email-attach-problem" class=class role="alert">{message}</p> }
        })
    }
}

#[component]
fn CodeForm(
    epoch: u64,
    now: ReadSignal<Millis>,
    binding: Binding,
    challenge_id: String,
    expires: Millis,
    step: RwSignal<Step>,
    read: Callback<Option<Binding>>,
) -> impl IntoView {
    let session = Session::current();
    let code = InputValue::new();
    let error = RwSignal::new(None);
    let state = session.action(Operation::AttachmentComplete, now);
    let blocked = Signal::derive(move || {
        now.get() >= expires
            || error.with(|e| {
                matches!(
                    e,
                    Some(AuthError::ChallengeClosed | AuthError::AttachmentEmailUnavailable)
                )
            })
    });
    let submit_state = Signal::derive(move || {
        let current = state.get();
        if blocked.get()
            && session.auth.with(|gate| gate.active()) != Some(Operation::AttachmentComplete)
        {
            vink_ui::ActionState::Disabled { reason: None }
        } else {
            current
        }
    });
    let target = binding.target.clone();
    view! {
        <form id="email-attach-code-form" class="vk-account-form" method="post" novalidate
            on:submit=move |event| {
                event.prevent_default();
                if blocked.get_untracked() { return; }
                let Some(digits) = code_value(code, error) else { return; };
                let proof = CompleteEmailAttachment { code: digits, challenge_id: challenge_id.clone() };
                let binding = binding.clone();
                session.dispatch(epoch, Command::CompleteEmailAttachment(proof), move |result| {
                    if error.try_get_untracked().is_none() { return; }
                    match result {
                        Ok(Reply::Done) | Err(AuthError::AlreadyAttached) => {
                            code.clear();
                            read.run(Some(binding));
                        }
                        Err(problem) if problem.uncertain() => {
                            code.clear();
                            read.run(Some(binding));
                        }
                        Err(AuthError::AttachmentContextChanged) => {
                            let resume_focus = owns_focus();
                            step.set(Step::Unavailable(AuthError::AttachmentContextChanged));
                            if resume_focus { focus_after_render(step, "email-attach-retry"); }
                        }
                        Err(problem) => error.set(Some(problem)),
                        _ => read.run(Some(binding)),
                    }
                });
            }>
            <p id="email-attach-requested" role="status">
                "Đã ghi nhận yêu cầu xác minh cho " <span>{target}</span> "." {instant(expires)}
            </p>
            <TextField id="email-attach-code" label="Mã 6 chữ số" name="code" autocomplete="one-time-code"
                variant=FieldVariant::FloatingTonal required=true input_ref=code.node value=empty_value()
                on_input=code.edit() error=field_error(error, code_field_error) />
            <Problem error fields=code_field_error />
            <Show when=move || (now.get() >= expires) && error.with(Option::is_none)>
                <p class="vk-account-error" role="alert">"Mã đã hết hạn. Hãy bắt đầu lại để xin mã mới."</p>
            </Show>
            <div class="vk-account-actions">
                <Button id="email-attach-confirm" variant=ButtonVariant::Filled kind=ButtonKind::Submit state=submit_state>
                    "Xác minh và gắn email"
                </Button>
                <Button id="email-attach-new-code" variant=ButtonVariant::Text
                    state=Signal::derive(move || if session.auth.with(|gate| gate.active().is_some()) {
                        vink_ui::ActionState::Disabled { reason: None }
                    } else { vink_ui::ActionState::Enabled })
                    on_press=move |_| read.run(None)>"Bắt đầu lại"</Button>
            </div>
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> Binding {
        Binding {
            session_id: "session".into(),
            target: "banviet@example.org".into(),
        }
    }

    fn observation(session: &str, email: Option<&str>) -> EmailAttachmentStatus {
        EmailAttachmentStatus {
            session_id: session.into(),
            principal_id: "principal".into(),
            password_account: true,
            verified_email: email.map(str::to_owned),
        }
    }

    #[test]
    fn only_matching_durable_binding_confirms_attempt() {
        assert!(matches!(
            observed(
                Ok(observation("session", Some("banviet@example.org"))),
                Some(binding())
            ),
            Step::Verified(_)
        ));
        assert!(matches!(
            observed(
                Ok(observation("session", Some("other@example.org"))),
                Some(binding())
            ),
            Step::DifferentBinding(_)
        ));
        assert!(matches!(
            observed(Ok(observation("session", None)), Some(binding())),
            Step::Unconfirmed(_)
        ));
        assert!(matches!(
            observed(Err(AuthError::Network), Some(binding())),
            Step::Unconfirmed(_)
        ));
        assert!(matches!(
            observed(
                Ok(observation("new-session", Some("banviet@example.org"))),
                Some(binding())
            ),
            Step::Unavailable(AuthError::AttachmentContextChanged)
        ));
    }
}
