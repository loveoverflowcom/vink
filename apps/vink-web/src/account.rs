//! The reactive session owner of the tab and the account region of “Tôi”.
//! The shell provides one owner; the views and forms render its server-confirmed facts.

#[cfg(feature = "browser")]
mod browser;
mod flow;
mod forms;

use leptos::prelude::*;
use vink_ui::{ActionState, Button, ButtonVariant};

use crate::contract::{
    CompleteEmailAttachment, EmailAttachmentStatus, EnrollPasswordRequest, PasswordCredentials,
    StartEmailAttachment,
};
use crate::session::{AuthError, Millis, SessionOwner, Standing};

use flow::{AuthGate, Operation};
#[cfg(feature = "browser")]
pub(crate) use forms::DeletionStage;
use forms::{AuthHeading, GuestAuth, PenName, SignOut, SignedInSettings};

// Auth inputs exist only in the mounted form and the request that consumes them.
#[cfg_attr(not(feature = "browser"), allow(dead_code))]
enum Command {
    Login(PasswordCredentials),
    Register(PasswordCredentials),
    RequestEmailCode(String),
    EmailCode(String),
    RequestEnrollmentCode,
    Enroll(EnrollPasswordRequest),
    RequestEmailAttachment(StartEmailAttachment),
    CompleteEmailAttachment(CompleteEmailAttachment),
    PenName(String),
    StartDeletion(String),
    ConfirmDeletion(serde_json::Value),
    DeletionStatus(String),
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
            Self::RequestEmailAttachment(_) => Operation::AttachmentChallenge,
            Self::CompleteEmailAttachment(_) => Operation::AttachmentComplete,
            Self::PenName(_) => Operation::PenName,
            Self::StartDeletion(_) => Operation::DeletionChallenge,
            Self::ConfirmDeletion(_) => Operation::ConfirmDeletion,
            Self::DeletionStatus(_) => Operation::DeletionStatus,
            Self::SignOut => Operation::SignOut,
        }
    }
}

#[cfg_attr(not(feature = "browser"), allow(dead_code))]
enum Reply {
    Done,
    Challenge(Millis),
    DeletionChallenge(String),
    Deletion(forms::DeletionStage),
    AttachmentChallenge {
        id: String,
        email: String,
        expires: Millis,
    },
}

#[cfg_attr(not(feature = "browser"), allow(dead_code))]
#[derive(Clone, Copy)]
pub(crate) struct Session {
    owner: RwSignal<SessionOwner>,
    auth: RwSignal<AuthGate>,
    checking: RwSignal<bool>,
    #[cfg(feature = "browser")]
    cookie_epoch: RwSignal<u64>,
    epoch: Memo<u64>,
    pub(crate) standing: Memo<Standing>,
    pub(crate) principal: Memo<Option<String>>,
    /// Scope of viewer-private state. Its format remains principal/generation.
    pub(crate) scope: Memo<Option<String>>,
    pub(crate) profile_scope: Memo<Option<String>>,
    pub(crate) notification_scope: Memo<Option<String>>,
}

impl Session {
    pub(crate) fn provide() -> Self {
        let owner = RwSignal::new(SessionOwner::default());
        let auth = RwSignal::new(AuthGate::default());
        let this = Self {
            owner,
            auth,
            checking: RwSignal::new(false),
            #[cfg(feature = "browser")]
            cookie_epoch: RwSignal::new(0),
            epoch: Memo::new(move |_| auth.with(AuthGate::epoch)),
            standing: Memo::new(move |_| owner.with(|o| o.standing().clone())),
            principal: Memo::new(move |_| owner.with(|o| o.principal().map(str::to_owned))),
            scope: Memo::new(move |_| owner.with(SessionOwner::scope)),
            profile_scope: Memo::new(move |_| owner.with(SessionOwner::profile_scope)),
            notification_scope: Memo::new(move |_| owner.with(SessionOwner::notification_scope)),
        };
        provide_context(this);
        #[cfg(feature = "browser")]
        browser::watch(this);
        this
    }

    pub(crate) fn current() -> Self {
        expect_context()
    }

    pub(crate) fn refresh(self) {
        #[cfg(feature = "browser")]
        browser::refresh(self);
    }

    #[cfg(feature = "browser")]
    pub(crate) fn fence_profile(self) {
        self.suspend_private_scope();
        self.refresh();
    }

    #[cfg(feature = "browser")]
    pub(crate) fn suspend_private_scope(self) {
        self.owner.update(SessionOwner::expect_session_change);
    }

    pub(crate) fn private_scope_fenced(self) -> bool {
        self.owner.with(SessionOwner::private_scope_fenced)
    }

    pub(crate) fn checking(self) -> bool {
        self.checking.get()
    }

    /// A real auth action or a cross-tab session signal invalidates visits immediately,
    /// even while the follow-up session read is pending.
    #[cfg(feature = "browser")]
    fn note_cookie_change(self) {
        self.cookie_epoch.update(|epoch| *epoch += 1);
        self.suspend_private_scope();
    }

    #[cfg(feature = "browser")]
    pub(crate) fn cookie_epoch(self) -> RwSignal<u64> {
        self.cookie_epoch
    }

    #[cfg(feature = "browser")]
    pub(crate) fn settled_authority(self) -> crate::work_view::AuthoritySnapshot {
        use crate::session::Standing;
        use crate::work_view::{AuthorityKind, AuthoritySnapshot};

        self.owner.with(|owner| match owner.standing() {
            Standing::Unknown | Standing::Unavailable => AuthoritySnapshot::Unsettled,
            Standing::Anonymous => AuthoritySnapshot::Settled {
                generation: owner.generation(),
                principal_id: None,
                kind: AuthorityKind::Anonymous,
            },
            Standing::NeedsPrincipal => AuthoritySnapshot::Settled {
                generation: owner.generation(),
                principal_id: None,
                kind: AuthorityKind::NeedsPrincipal,
            },
            Standing::Suspended => AuthoritySnapshot::Settled {
                generation: owner.generation(),
                principal_id: None,
                kind: AuthorityKind::Suspended,
            },
            Standing::Active(account) => AuthoritySnapshot::Settled {
                generation: owner.generation(),
                principal_id: Some(account.principal_id.clone()),
                kind: AuthorityKind::Active,
            },
        })
    }

    fn dispatch(
        self,
        epoch: u64,
        command: Command,
        complete: impl FnOnce(Result<Reply, AuthError>) + 'static,
    ) {
        #[cfg(feature = "browser")]
        browser::dispatch(self, epoch, command, complete);
        #[cfg(not(feature = "browser"))]
        let _ = (epoch, command.operation(), complete);
    }

    fn enrollment_status(
        self,
        epoch: u64,
        live: impl Fn() -> bool + 'static,
        complete: impl FnOnce(Result<bool, AuthError>) + 'static,
    ) {
        #[cfg(feature = "browser")]
        browser::enrollment_status(self, epoch, live, complete);
        #[cfg(not(feature = "browser"))]
        let _ = (epoch, live, complete);
    }

    fn attachment_status(
        self,
        epoch: u64,
        live: impl Fn() -> bool + 'static,
        complete: impl FnOnce(Result<EmailAttachmentStatus, AuthError>) + 'static,
    ) {
        #[cfg(feature = "browser")]
        browser::attachment_status(self, epoch, live, complete);
        #[cfg(not(feature = "browser"))]
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

pub(crate) fn standing_id(standing: &Standing) -> &'static str {
    match standing {
        Standing::Unknown => "unknown",
        Standing::Anonymous => "anonymous",
        Standing::NeedsPrincipal => "needs_principal",
        Standing::Suspended => "suspended",
        Standing::Active(_) => "active",
        Standing::Unavailable => "unavailable",
    }
}

#[component]
fn SessionRetry() -> impl IntoView {
    let session = Session::current();
    move || {
        (session.standing.get() != Standing::Unavailable
            && session.auth.with(AuthGate::needs_check)
            && !session.checking.get())
        .then(|| {
            view! {
                <Button id="auth-session-retry" variant=ButtonVariant::Tonal
                    on_press=move |_| session.refresh()>"Kiểm tra lại phiên"</Button>
            }
        })
    }
}

/// Only presentation varies by host; AuthGate and all input owners are shared. The Reader
/// names why it asks (“Đăng nhập để nhận xét” / “Đăng nhập để lưu bài”) and may add a lead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReaderPresentation {
    pub(crate) title: &'static str,
    pub(crate) lead: Option<&'static str>,
}

/// Mount once in the current host. The host supplies layout and post-auth navigation, never
/// standing or credentials. The existing form owners are disposed on a mode/epoch change.
#[component]
pub(crate) fn AuthEntry(#[prop(optional)] reader: Option<ReaderPresentation>) -> impl IntoView {
    if let Some(presentation) = reader {
        provide_context(presentation);
    }
    let session = Session::current();
    let open = crate::api::sign_in_open();
    move || {
        let epoch = session.epoch.get();
        let body = match session.standing.get() {
            Standing::Anonymous if !open => view! {
                <AuthHeading title="Đăng nhập" />
                <p id="sign-in-closed" class="vk-muted">"Vink chưa mở đăng nhập. Bạn vẫn có thể đọc các tác phẩm công khai."</p>
            }.into_any(),
            Standing::Anonymous => view! { <GuestAuth epoch /> }.into_any(),
            Standing::NeedsPrincipal => {
                let focus = forms::focus_within("#password-login, #password-register, #sign-in-code");
                view! { <PenName epoch focus /> }.into_any()
            },
            _ => return None,
        };
        Some(
            view! { <div class="vk-auth-entry grid [min-inline-size:0] wrap-anywhere">{body}<SessionRetry /></div> },
        )
    }
}

/// The identity, session entry and settings on “Tôi”.
#[component]
pub(crate) fn AccountHub(children: Children) -> impl IntoView {
    let session = Session::current();
    let standing = session.standing;
    let fixture = crate::api::local_discussion_fixture_scope().is_some();
    let entry = move || {
        matches!(
            standing.get(),
            Standing::Anonymous | Standing::NeedsPrincipal
        )
    };
    let lifetime = session.epoch;
    view! {
        <div id="page-account" class="vk-account grid gap-vk-4 min-w-0">
            <Show when=move || !entry() fallback=|| view! {
                <h1 id="me-title" class="vk-ui-visually-hidden" tabindex="-1">"Tài khoản"</h1>
            }>
                <div class="vk-account-hero grid gap-vk-3 p-vk-4 rounded-vk-xl bg-vk-surface-container-low">
                    <div class="vk-account-status grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-vk-3">
                        <crate::appearance::AccountAvatar />
                        <div class="min-w-0 wrap-anywhere">
                            <vink_ui::Heading id="me-title" level=vink_ui::HeadingLevel::H1
                                role=vink_ui::TypeRole::Title attr:tabindex="-1">
                                {move || standing.with(|current| match current {
                                    Standing::Active(account) => account.display_name.clone(),
                                    _ => "Tài khoản".into(),
                                })}
                            </vink_ui::Heading>
                            <Show when=move || !matches!(standing.get(), Standing::Active(_))>
                                <vink_ui::Text tone=vink_ui::TextTone::Muted>
                                    {move || standing.with(summary)}
                                </vink_ui::Text>
                            </Show>
                        </div>
                        <crate::notifications::MobileBell />
                    </div>
                    <Show when=move || session.profile_scope.with(Option::is_some)>
                        <crate::pages::profile::ProfileActions />
                    </Show>
                </div>
            </Show>
            {children()}
            <div id="account-session" class="vk-account-session" data-standing=move || standing.with(standing_id)>
                <AuthEntry />
                {move || {
                    // Rebuild sensitive forms on every auth change, even if two sessions have
                    // the same standing or principal. Quiet focus checks leave the DOM alone.
                    let epoch = lifetime.get();
                    match standing.get() {
                    Standing::Unknown => ().into_any(),
                    Standing::Unavailable => view! {
                        <Button id="session-retry" variant=ButtonVariant::Tonal on_press=move |_| session.refresh()>"Kiểm tra lại"</Button>
                    }.into_any(),
                    Standing::Anonymous => ().into_any(),
                    // Signing out abandons onboarding; it is a page utility, not a form action.
                    Standing::NeedsPrincipal => view! {
                        <div class="vk-auth-exit"><SignOut epoch /></div>
                    }.into_any(),
                    Standing::Suspended => view! {
                        <p class="vk-muted">"Bạn vẫn có thể đọc các tác phẩm công khai. Hãy liên hệ Vink nếu cần được xem xét lại."</p>
                        <SignOut epoch />
                    }.into_any(),
                    Standing::Active(_) => view! {
                        <SignedInSettings epoch fixture />
                    }.into_any(),
                    }
                }}
                {(!fixture).then(|| view! { <forms::AccountDeletion /> })}
                <Show when=move || !entry()><SessionRetry /></Show>
                <Show when=entry>
                    // Let the browser scroll *and focus* the target instead of the router only scrolling it.
                    <a id="auth-appearance" class=ButtonVariant::Text.classes()
                        class=("min-[760px]:absolute", true)
                        rel="external" href="#page-appearance">
                        <span data-appearance-icon="palette" aria-hidden="true"></span>
                        <span class="vk-ui-button__label">"Giao diện"</span>
                    </a>
                </Show>
            </div>
            <noscript><p class="vk-muted">"Đăng nhập cần JavaScript. Bạn vẫn có thể đọc các tác phẩm công khai."</p></noscript>
        </div>
    }
}

#[component]
pub(crate) fn AccountAppearance() -> impl IntoView {
    view! {
        <section id="page-appearance" tabindex="-1" class="vk-appearance-card" aria-labelledby="page-appearance-title">
            <crate::appearance::AppearanceTitle id="page-appearance-title" />
            <crate::appearance::AppearanceChoices prefix="page" />
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
            notification_scope: None,
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
