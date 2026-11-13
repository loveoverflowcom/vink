//! The reactive session owner of the tab and the account region of “Tôi”.
//! The shell provides one owner; the views and forms render its server-confirmed facts.

#[cfg(feature = "hydrate")]
mod browser;
mod flow;
mod forms;

use leptos::prelude::*;
use vink_ui::{ActionState, Button, ButtonVariant};

use crate::contract::{EnrollPasswordRequest, PasswordCredentials};
use crate::session::{AuthError, Millis, SessionOwner, Standing};

use flow::{AuthGate, Operation};
use forms::{GuestAuth, PenName, SignOut, SignedIn};

// Auth inputs exist only in the mounted form and the request that consumes them.
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
enum Command {
    Login(PasswordCredentials),
    Register(PasswordCredentials),
    RequestEmailCode(String),
    EmailCode(String),
    RequestEnrollmentCode,
    Enroll(EnrollPasswordRequest),
    PenName(String),
    SignOut,
}

impl Command {
    fn operation(&self) -> Operation {
        match self {
            Self::Login(_) => Operation::PasswordLogin,
            Self::Register(_) => Operation::PasswordRegister,
            Self::RequestEmailCode(_) => Operation::EmailChallenge,
            Self::EmailCode(_) => Operation::EmailCode,
            Self::RequestEnrollmentCode => Operation::EnrollmentChallenge,
            Self::Enroll(_) => Operation::Enrollment,
            Self::PenName(_) => Operation::PenName,
            Self::SignOut => Operation::SignOut,
        }
    }
}

#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
enum Reply {
    Done,
    Challenge(Millis),
}

#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
#[derive(Clone, Copy)]
pub(crate) struct Session {
    owner: RwSignal<SessionOwner>,
    auth: RwSignal<AuthGate>,
    checking: RwSignal<bool>,
    epoch: Memo<u64>,
    pub(crate) standing: Memo<Standing>,
    pub(crate) principal: Memo<Option<String>>,
    /// Scope of viewer-private state. Its format remains principal/generation.
    pub(crate) scope: Memo<Option<String>>,
}

impl Session {
    pub(crate) fn provide() -> Self {
        let owner = RwSignal::new(SessionOwner::default());
        let auth = RwSignal::new(AuthGate::default());
        let this = Self {
            owner,
            auth,
            checking: RwSignal::new(false),
            epoch: Memo::new(move |_| auth.with(AuthGate::epoch)),
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

    pub(crate) fn refresh(self) {
        #[cfg(feature = "hydrate")]
        browser::refresh(self);
    }

    fn dispatch(
        self,
        epoch: u64,
        command: Command,
        complete: impl FnOnce(Result<Reply, AuthError>) + 'static,
    ) {
        #[cfg(feature = "hydrate")]
        browser::dispatch(self, epoch, command, complete);
        #[cfg(not(feature = "hydrate"))]
        let _ = (epoch, command.operation(), complete);
    }

    fn enrollment_status(
        self,
        epoch: u64,
        live: impl Fn() -> bool + 'static,
        complete: impl FnOnce(Result<bool, AuthError>) + 'static,
    ) {
        #[cfg(feature = "hydrate")]
        browser::enrollment_status(self, epoch, live, complete);
        #[cfg(not(feature = "hydrate"))]
        let _ = (epoch, live, complete);
    }

    fn retry_wait(self, operation: Operation, now: ReadSignal<Millis>) -> Signal<Option<u32>> {
        Signal::derive(move || self.auth.with(|gate| gate.wait(operation, now.get())))
    }

    fn action(self, operation: Operation, now: ReadSignal<Millis>) -> Signal<ActionState> {
        Signal::derive(move || {
            self.auth.with(|gate| {
                if gate.active() == Some(operation) {
                    ActionState::pending(operation.status())
                } else if gate.active().is_some() || gate.needs_check() {
                    ActionState::Disabled { reason: None }
                } else if let Some(seconds) = gate.wait(operation, now.get()) {
                    let verb = if matches!(
                        operation,
                        Operation::EmailChallenge | Operation::EnrollmentChallenge
                    ) {
                        "gửi lại"
                    } else {
                        "thử lại"
                    };
                    ActionState::disabled(format!(
                        "Có thể {verb} sau {}.",
                        crate::session::wait_text(seconds)
                    ))
                } else {
                    ActionState::Enabled
                }
            })
        })
    }
}

fn summary(standing: &Standing) -> String {
    match standing {
        Standing::Unknown => "Đang kiểm tra phiên đăng nhập…".into(),
        Standing::Anonymous => "Bạn chưa đăng nhập.".into(),
        Standing::NeedsPrincipal => "Hãy đặt bút danh để bắt đầu.".into(),
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

/// The account region of “Tôi”: who is signed in, and the way in or out.
#[component]
pub(crate) fn AccountPanel(id: &'static str) -> impl IntoView {
    let session = Session::current();
    let standing = session.standing;
    let fixture = crate::api::local_discussion_fixture_scope().is_some();
    let open = crate::api::sign_in_open();
    let lifetime = session.epoch;
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
                {move || {
                    // Rebuild sensitive forms on every auth change, even if two sessions have
                    // the same standing or principal. Quiet focus checks leave the DOM alone.
                    let epoch = lifetime.get();
                    match standing.get() {
                    Standing::Unknown => ().into_any(),
                    Standing::Unavailable => view! {
                        <Button id="session-retry" variant=ButtonVariant::Tonal on_press=move |_| session.refresh()>"Kiểm tra lại"</Button>
                    }.into_any(),
                    Standing::Anonymous if !open => view! {
                        <p id="sign-in-closed" class="vk-muted">"Vink chưa mở đăng nhập. Bạn vẫn có thể đọc các tác phẩm công khai."</p>
                    }.into_any(),
                    Standing::Anonymous => view! { <GuestAuth epoch /> }.into_any(),
                    Standing::NeedsPrincipal => view! { <PenName epoch /> }.into_any(),
                    Standing::Suspended => view! {
                        <p class="vk-muted">"Bạn vẫn có thể đọc các tác phẩm công khai. Hãy liên hệ Vink nếu cần được xem xét lại."</p>
                        <SignOut epoch />
                    }.into_any(),
                    Standing::Active(_) if fixture => view! {
                        <p class="vk-muted">"Định danh thử trên thiết bị này · Không phải tài khoản thật."</p>
                    }.into_any(),
                    Standing::Active(_) => view! { <SignedIn epoch /> }.into_any(),
                    }
                }}
                {move || (standing.get() != Standing::Unavailable
                    && session.auth.with(AuthGate::needs_check) && !session.checking.get())
                    .then(|| view! {
                        <Button id="auth-session-retry" variant=ButtonVariant::Tonal
                            on_press=move |_| session.refresh()>"Kiểm tra lại phiên"</Button>
                    })}
            </div>
            <noscript><p class="vk-muted">"Đăng nhập cần JavaScript. Bạn vẫn có thể đọc các tác phẩm công khai."</p></noscript>
        </section>
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
    }
}
