//! Web entry points for Notifications. Private data starts only in the browser.
use leptos::prelude::*;
use vink_ui::{BadgeState, Icon, IconName, NavUtility, NotificationBadge};

use crate::account::Session;
use crate::session::Standing;

#[cfg(feature = "browser")]
mod browser;
#[cfg_attr(not(any(feature = "browser", test)), allow(dead_code))]
mod contract;
mod destinations;
mod inbox;
#[cfg_attr(not(any(feature = "browser", test)), allow(dead_code))]
mod state;

pub(crate) use destinations::{CommentDecisionPage, EndorsersPage, EndorsersTrigger};
pub(crate) use inbox::{InboxPage, InboxPopover};

pub(crate) const INBOX_PATH: &str = "/thong-bao";
pub(crate) const BELL: &str = "notification-bell";
pub(crate) const MOBILE_BELL: &str = "notification-bell-mobile";
pub(crate) const POPOVER: &str = "notification-popover";

#[derive(Clone, Copy, PartialEq, Eq)]
enum PageAccess {
    Pending,
    SessionUnavailable,
    Anonymous,
    UnavailableToAccount,
    Available,
}

impl PageAccess {
    fn from_session(session: Session) -> Self {
        if session.notification_scope.with(Option::is_some) {
            return Self::Available;
        }
        // A failed recheck retains the previous standing. The fence owns whether it
        // can still be used, and checking tells us whether another answer is pending.
        let standing = session.standing.get();
        if session.private_scope_fenced() || standing == Standing::Unavailable {
            return if session.checking() {
                Self::Pending
            } else {
                Self::SessionUnavailable
            };
        }
        match standing {
            Standing::Unknown => Self::Pending,
            Standing::Anonymous => Self::Anonymous,
            _ => Self::UnavailableToAccount,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Notifications {
    session: Session,
    page_access: Memo<PageAccess>,
    count: RwSignal<BadgeState>,
    count_failed: RwSignal<bool>,
    offline: RwSignal<bool>,
    large_text: RwSignal<bool>,
    #[cfg(feature = "browser")]
    arrived_from_app: RwSignal<bool>,
    inbox: RwSignal<state::Inbox>,
    #[cfg(feature = "browser")]
    view_generation: RwSignal<u64>,
    #[cfg(feature = "browser")]
    poll: StoredValue<browser::Poll>,
}

impl Notifications {
    pub(crate) fn provide(
        session: Session,
        open: Signal<bool>,
        large_text: RwSignal<bool>,
    ) -> Self {
        let this = Self {
            session,
            page_access: Memo::new(move |_| PageAccess::from_session(session)),
            count: RwSignal::new(BadgeState::Hidden),
            count_failed: RwSignal::new(false),
            offline: RwSignal::new(false),
            large_text,
            #[cfg(feature = "browser")]
            arrived_from_app: RwSignal::new(false),
            inbox: RwSignal::new(state::Inbox::default()),
            #[cfg(feature = "browser")]
            view_generation: RwSignal::new(0),
            #[cfg(feature = "browser")]
            poll: StoredValue::new(browser::Poll::default()),
        };
        provide_context(this);
        #[cfg(feature = "browser")]
        {
            this.connect(open);
            let path = leptos_router::hooks::use_location().pathname;
            Effect::new(move |previous: Option<String>| {
                let current = path.get();
                if previous.as_deref().is_some_and(|path| path != current) {
                    this.arrived_from_app.set(true);
                }
                current
            });
        }
        #[cfg(not(feature = "browser"))]
        let _ = open;
        this
    }

    fn available(self) -> bool {
        self.session.notification_scope.with(Option::is_some)
    }

    pub(crate) fn current() -> Self {
        expect_context()
    }
}

#[component]
pub(crate) fn RailBell(
    current: Signal<bool>,
    open: Signal<bool>,
    page_only: Signal<bool>,
    on_open: Callback<()>,
) -> impl IntoView {
    let notifications = Notifications::current();
    let name = Signal::derive(move || notifications.count.get().trigger_name("Thông báo"));
    let badge = move || view! { <NotificationBadge state=notifications.count /> };
    move || {
        if !notifications.available() {
            return ().into_any();
        }
        if page_only.get() {
            view! { <NavUtility id=BELL label="Thông báo" icon=IconName::NotificationsNone
            active_icon=IconName::Notifications expanded=open current on_open
            controls=POPOVER href=INBOX_PATH badge accessible_label=name /> }
            .into_any()
        } else {
            view! { <NavUtility id=BELL label="Thông báo" icon=IconName::NotificationsNone
            active_icon=IconName::Notifications expanded=open current on_open
            controls=POPOVER badge accessible_label=name /> }
            .into_any()
        }
    }
}

#[component]
pub(crate) fn MobileBell() -> impl IntoView {
    let notifications = Notifications::current();
    view! {
        <Show when=move || notifications.available()>
            <a id=MOBILE_BELL class="vk-notif-mobile-bell" href=INBOX_PATH
                data-large-text=move || notifications.large_text.get().to_string()
                aria-label=move || notifications.count.get().trigger_name("Thông báo")
                on:keydown=move |event: leptos::ev::KeyboardEvent| {
                    if event.key() == " " && !event.repeat() {
                        event.prevent_default();
                        #[cfg(feature = "browser")]
                        {
                            use leptos::wasm_bindgen::JsCast;
                            if let Some(link) = event.current_target().and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok()) {
                                link.click();
                            }
                        }
                    }
                }>
                <Icon name=IconName::NotificationsNone />
                <NotificationBadge state=notifications.count />
            </a>
        </Show>
    }
}

#[cfg(feature = "browser")]
pub(super) fn back_or_explore(navigation: crate::navigation::NavigationGuard, from_app: bool) {
    let can_go_back = from_app
        || document().referrer().starts_with(&format!(
            "{}/",
            window().location().origin().unwrap_or_default()
        ));
    if can_go_back
        && window()
            .history()
            .is_ok_and(|history| history.back().is_ok())
    {
        return;
    }
    navigation.navigate("/");
}
