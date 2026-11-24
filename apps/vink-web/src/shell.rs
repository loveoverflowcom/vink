//! Vink's adaptive shell: its destinations, overlays and responsive policy,
//! rendered with the UI system's navigation primitives (shared/navigation.md). Independent of the
//! route's lifetime and of draft/save state: changing the frame never remounts `#noi-dung`.

use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_location;
use vink_ui::{
    AdaptiveScaffold, BottomNavigation, CloseReason, IconName, NavContext, NavDestination, NavItem,
    NavLayout, NavigationRail, OverlayLayer, WindowClass,
};

use crate::appearance::{Appearance, RAIL_BOOTSTRAP};
use crate::navigation::use_navigation_guard;
use crate::notifications::{BELL, InboxPopover, Notifications, POPOVER, RailBell};
#[cfg(feature = "hydrate")]
use crate::notifications::{INBOX_PATH, MOBILE_BELL};

// Giao diện lives only on Tôi (`/toi`); the rail carries no appearance utility (#413).
const RAIL_MODAL: &str = "navigation-modal";

/// Vink's destinations. Reader pages belong to Khám phá, the editor to Bàn viết. Order is a
/// navigation policy: it never changes the launch route `/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Destination {
    Explore,
    Desk,
    Me,
}

impl Destination {
    /// The destination a routed path belongs to, matched by whole segments like the router's
    /// routes in `lib.rs`; anything the router renders as NotFound marks none.
    pub fn for_path(path: &str) -> Option<Self> {
        match segments(path)[..] {
            [] | ["tac-pham", _] | ["tac-pham", _, "tham", _] | ["the", _] => Some(Self::Explore),
            ["ban-viet"] | ["ban-viet", _] | ["ban-viet", _, "kiem-duyet"] => Some(Self::Desk),
            ["toi"] | ["toi", "dia-chi" | "chinh-ho-so"] => Some(Self::Me),
            [name] if name.starts_with('@') => Some(Self::Explore),
            [name, "nguoi-theo-doi" | "dang-theo-doi"] if name.starts_with('@') => {
                Some(Self::Explore)
            }
            _ => None,
        }
    }

    const fn key(self) -> &'static str {
        match self {
            Self::Explore => "explore",
            Self::Desk => "desk",
            Self::Me => "me",
        }
    }

    fn nav(self) -> NavDestination {
        let (label, icon, href) = match self {
            Self::Explore => ("Khám phá", IconName::Explore, "/"),
            Self::Desk => ("Bàn viết", IconName::Edit, "/ban-viet"),
            Self::Me => ("Tôi", IconName::Person, "/toi"),
        };
        NavDestination::new(self.key(), label, icon, href)
    }
}

fn segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|part| !part.is_empty()).collect()
}

/// The open draft editor: focused work, so the rail defaults to compact and the bar hides.
fn editor_context(path: &str) -> NavContext {
    let editor = matches!(segments(path)[..], ["ban-viet", _]);
    NavContext {
        compact_rail: editor,
        hide_bar: editor,
    }
}

#[component]
pub(crate) fn AdaptiveShell(children: Children) -> impl IntoView {
    let appearance = Appearance::provide();
    let session = crate::account::Session::provide();
    #[cfg(feature = "browser")]
    {
        crate::pages::profile::observe_changes(crate::public_reads::invalidate_author_attribution);
        crate::pages::profile::browser::provide_draft(session);
    }
    // SSR and the first browser render use the same anonymous-safe account link.
    // Only confirmed standing changes the label to Tôi.
    let guest = Memo::new(move |_| {
        use crate::session::Standing;
        matches!(
            session.standing.get(),
            Standing::Unknown | Standing::Anonymous | Standing::Unavailable
        )
    });
    let location = use_location();
    let current = Memo::new(move |_| location.pathname.with(|path| Destination::for_path(path)));
    let context = Memo::new(move |_| location.pathname.with(|path| editor_context(path)));
    let window_class = RwSignal::new(current_window_class());
    let layout = Memo::new(move |_| {
        NavLayout::resolve(window_class.get(), context.get(), appearance.rail.get())
    });
    let overlays = RwSignal::new(OverlayLayer::default());
    let rail_open = Memo::new(move |_| overlays.with(|layer| layer.is_open(RAIL_MODAL)));
    let rail_modal = NodeRef::<html::Dialog>::new();
    let inbox_route = Memo::new(move |_| {
        location
            .pathname
            .with(|path| segments(path) == ["thong-bao"])
    });
    let popover = NodeRef::<html::Dialog>::new();
    let popover_open = Memo::new(move |_| overlays.with(|layer| layer.is_open(POPOVER)));
    let large_text = RwSignal::new(false);
    Notifications::provide(
        session,
        Signal::derive(move || inbox_route.get() || popover_open.get()),
        large_text,
    );
    let text_probe = NodeRef::<html::Span>::new();
    let page_scroll = StoredValue::new(0.0);
    let close_notifications = move |reason| {
        // Resize is an exit from the overlay layout, including its keyboard target.
        let trigger = if reason == CloseReason::Resize {
            "noi-dung"
        } else {
            BELL
        };
        close_dialog(POPOVER, reason, overlays, popover, trigger, page_scroll)
    };
    let close_rail = move |reason| {
        close_dialog(
            RAIL_MODAL,
            reason,
            overlays,
            rail_modal,
            "navigation-toggle",
            page_scroll,
        )
    };
    let navigation = use_navigation_guard();
    // Back/forward and real navigations close the overlay before the editor's check runs.
    let overlay_closer =
        navigation.set_overlay(move |reason| close_notifications(reason) || close_rail(reason));
    on_cleanup(move || navigation.clear_overlay(overlay_closer));

    let open_notifications = Callback::new(move |()| {
        #[cfg(feature = "browser")]
        {
            if place_notifications(popover) && show_dialog(POPOVER, overlays, popover, page_scroll)
            {
                focus_without_scroll(document().get_element_by_id("notification-popover-title"));
                restore_scroll(page_scroll);
            }
        }
    });
    let page_only = Signal::derive(move || large_text.get() || inbox_route.get());

    #[cfg(feature = "browser")]
    {
        let listener = window_event_listener(leptos::ev::resize, move |_| {
            let class = current_window_class();
            window_class.set(class);
            if class != WindowClass::Medium {
                close_rail(CloseReason::Resize);
            }
            if class == WindowClass::Compact {
                close_notifications(CloseReason::Resize);
            } else if popover_open.get_untracked() {
                place_notifications(popover);
            }
        });
        on_cleanup(move || listener.remove());
        observe_notification_text(text_probe, large_text, close_notifications);
        Effect::new(move |_| {
            if session.notification_scope.get().is_none() {
                close_notifications(CloseReason::Dismiss);
            }
        });
        let path = use_location().pathname;
        Effect::new(move |was_inbox: Option<bool>| {
            let current = path.get();
            if was_inbox == Some(true)
                && matches!(segments(&current)[..], [] | ["ban-viet"] | ["toi"])
            {
                request_animation_frame(move || {
                    let bell = [MOBILE_BELL, BELL]
                        .into_iter()
                        .filter_map(|id| document().get_element_by_id(id))
                        .find(|bell| bell.get_bounding_client_rect().width() > 0.0)
                        .or_else(|| document().get_element_by_id("noi-dung"));
                    focus_without_scroll(bell);
                });
            }
            current.trim_end_matches('/') == INBOX_PATH
        });
    }

    let active =
        move |destination: Destination| Signal::derive(move || current.get() == Some(destination));
    let primary = move |modal: bool| {
        [Destination::Explore, Destination::Desk]
            .into_iter()
            .map(|destination| {
                let selected = Signal::derive(move || {
                    (!modal || rail_open.get()) && current.get() == Some(destination)
                });
                (destination.nav(), selected)
            })
            .collect::<Vec<_>>()
    };
    let toggle = Callback::new(move |()| {
        // Match CSS at the time of activation, including clicks between resize and its event.
        match current_window_class() {
            WindowClass::Compact => {}
            WindowClass::Medium => {
                if show_dialog(RAIL_MODAL, overlays, rail_modal, page_scroll) {
                    #[cfg(feature = "browser")]
                    {
                        focus_without_scroll(
                            document().get_element_by_id("navigation-expanded-toggle"),
                        );
                        restore_scroll(page_scroll);
                    }
                }
            }
            WindowClass::Expanded => {
                let shown = NavLayout::resolve(
                    WindowClass::Expanded,
                    context.get_untracked(),
                    appearance.rail.get_untracked(),
                );
                if let Some(next) = shown.next_preference() {
                    appearance.set_rail(Some(next));
                }
            }
        }
    });

    view! {
        <AdaptiveScaffold
            id="shell"
            main_id="noi-dung"
            skip_label="Bỏ qua tới nội dung"
            context=context
            rail=move || view! {
                <NavigationRail
                    id="navigation"
                    label="Không gian Vink"
                    nav_label="Điều hướng chính"
                    expanded=Signal::derive(move || match window_class.get() {
                        WindowClass::Medium => rail_open.get(),
                        _ => layout.get().rail_expanded(),
                    })
                    modal_controls=Signal::derive(move || (window_class.get() == WindowClass::Medium).then_some(RAIL_MODAL))
                    expand_label="Mở rộng điều hướng"
                    collapse_label="Thu gọn điều hướng"
                    on_toggle=toggle
                    destinations=primary(false)
                    utility=move || view! { <RailBell current=inbox_route.into() open=popover_open.into()
                        page_only on_open=open_notifications /> }
                    account=move || view! {
                        <AccountLink guest=guest.into() current=active(Destination::Me) />
                    }
                />
                <script inner_html=RAIL_BOOTSTRAP></script>
            }
            bar=move || view! {
                <BottomNavigation
                    id="bottom-navigation"
                    label="Điều hướng mobile"
                    destinations=primary(false)
                    trailing=move || view! { <AccountLink guest=guest.into() current=active(Destination::Me) /> }
                />
            }
            overlays=move || view! {
                <span class="vk-notif-text-probe" aria-hidden="true" node_ref=text_probe></span>
                <ShellDialog id=POPOVER class="vk-notif-popover" label="Thông báo"
                    labelledby="notification-popover-title"
                    dialog=popover on_close=Callback::new(move |reason| { close_notifications(reason); })>
                    <InboxPopover open=popover_open.into() on_close=Callback::new(move |()| { close_notifications(CloseReason::Dismiss); }) />
                </ShellDialog>
                <ShellDialog id=RAIL_MODAL class="vk-rail-modal m-0 p-0 [border:0] [max-inline-size:100%] [max-block-size:100%]" label="Điều hướng chính"
                    dialog=rail_modal on_close=Callback::new(move |reason| { close_rail(reason); })>
                    <NavigationRail
                        id="navigation-expanded"
                        label="Không gian Vink"
                        nav_label="Điều hướng chính"
                        expanded=Signal::stored(true)
                        expand_label="Mở rộng điều hướng"
                        collapse_label="Thu gọn điều hướng"
                        on_toggle=Callback::new(move |()| { close_rail(CloseReason::Dismiss); })
                        destinations=primary(true)
                        account=move || view! {
                            <AccountLink guest=guest.into()
                                current=Signal::derive(move || rail_open.get() && current.get() == Some(Destination::Me)) />
                        }
                    />
                </ShellDialog>
            }
        >
            {children()}
        </AdaptiveScaffold>
    }
}

#[component]
fn AccountLink(guest: Signal<bool>, current: Signal<bool>) -> impl IntoView {
    let label = if crate::api::sign_in_open() {
        "Đăng nhập"
    } else {
        "Tài khoản"
    };
    move || {
        if guest.get() {
            view! { <NavItem destination=NavDestination::new(Destination::Me.key(), label, IconName::Person, "/toi") active=current /> }.into_any()
        } else {
            view! { <NavItem destination=Destination::Me.nav() active=current /> }.into_any()
        }
    }
}

#[component]
fn ShellDialog(
    id: &'static str,
    class: &'static str,
    label: &'static str,
    #[prop(optional)] labelledby: Option<&'static str>,
    dialog: NodeRef<html::Dialog>,
    on_close: Callback<CloseReason>,
    children: Children,
) -> impl IntoView {
    view! {
        <dialog id=id class=class aria-label=label aria-labelledby=labelledby data-testid=id node_ref=dialog
            on:keydown=move |event| trap_dialog_tab(event, dialog)
            on:cancel=move |event: leptos::ev::Event| {
                event.prevent_default();
                on_close.run(CloseReason::Escape);
            }
            on:close=move |_| {
                // Ignore a queued native close if the same dialog has already reopened.
                if dialog.get_untracked().is_none_or(|node| !node.open()) {
                    on_close.run(CloseReason::Dismiss);
                }
            }
            on:click=move |event: leptos::ev::MouseEvent| {
                #[cfg(feature = "browser")]
                if event.target() == event.current_target() && let Some(dialog) = dialog.get_untracked() {
                    let rect = dialog.get_bounding_client_rect();
                    let (x, y) = (f64::from(event.client_x()), f64::from(event.client_y()));
                    if x < rect.left() || x >= rect.right() || y < rect.top() || y >= rect.bottom() {
                        on_close.run(CloseReason::Outside);
                    }
                }
                #[cfg(not(feature = "browser"))]
                let _ = event;
            }
        >{children()}</dialog>
    }
}

/// Keep keyboard traversal inside the dialog even when a browser would send an edge Tab to
/// its own chrome. Unchecked radios are one native tab stop with the checked option.
fn trap_dialog_tab(event: leptos::ev::KeyboardEvent, dialog: NodeRef<html::Dialog>) {
    #[cfg(feature = "browser")]
    {
        use wasm_bindgen::JsCast;
        if event.key() != "Tab" {
            return;
        }
        let Some(dialog) = dialog.get_untracked() else {
            return;
        };
        let Ok(stops) =
            dialog.query_selector_all("button:not(:disabled), input:checked, a[href], summary")
        else {
            return;
        };
        let (Some(first), Some(last), Some(active)) = (
            stops.item(0),
            stops.item(stops.length().saturating_sub(1)),
            document().active_element(),
        ) else {
            return;
        };
        let target = if event.shift_key()
            && (first.is_same_node(Some(&active)) || active.id() == "notification-popover-title")
        {
            Some(last)
        } else if !event.shift_key() && last.is_same_node(Some(&active)) {
            Some(first)
        } else {
            None
        };
        if let Some(target) = target.and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok()) {
            event.prevent_default();
            let _ = target.focus();
        }
    }
    #[cfg(not(feature = "browser"))]
    let _ = (event, dialog);
}

fn show_dialog(
    id: &'static str,
    overlays: RwSignal<OverlayLayer>,
    dialog: NodeRef<html::Dialog>,
    page_scroll: StoredValue<f64>,
) -> bool {
    #[cfg(feature = "browser")]
    {
        if let Some(dialog) = dialog.get_untracked().filter(|node| !node.open())
            && document()
                .query_selector("dialog:modal")
                .is_ok_and(|modal| modal.is_none())
            && overlays
                .try_update(|layer| layer.open(id))
                .is_some_and(|result| result.is_ok())
        {
            page_scroll.set_value(window().scroll_y().unwrap_or_default());
            if dialog.show_modal().is_ok() {
                return true;
            }
            overlays.update(|layer| {
                layer.close(id, CloseReason::Dismiss);
            });
        }
    }
    #[cfg(not(feature = "browser"))]
    let _ = (id, overlays, dialog, page_scroll);
    false
}

fn close_dialog(
    id: &'static str,
    reason: CloseReason,
    overlays: RwSignal<OverlayLayer>,
    dialog: NodeRef<html::Dialog>,
    trigger: &'static str,
    page_scroll: StoredValue<f64>,
) -> bool {
    let closed = overlays
        .try_update(|layer| layer.close(id, reason))
        .flatten();
    let Some(closed) = closed else {
        return false;
    };
    if let Some(dialog) = dialog.get_untracked() {
        dialog.close();
    }
    #[cfg(feature = "browser")]
    {
        let trigger = document()
            .get_element_by_id(trigger)
            .filter(|node| node.get_bounding_client_rect().width() > 0.0);
        let target = match closed.reason.focus_return(trigger.is_some()) {
            vink_ui::FocusReturn::Trigger => trigger,
            vink_ui::FocusReturn::Main | vink_ui::FocusReturn::Leave => {
                document().get_element_by_id("noi-dung")
            }
        };
        focus_without_scroll(target);
        restore_scroll(page_scroll);
    }
    #[cfg(not(feature = "browser"))]
    let _ = (closed, trigger, page_scroll);
    true
}

#[cfg(feature = "browser")]
pub(crate) fn focus_without_scroll(target: Option<web_sys::Element>) {
    use wasm_bindgen::JsCast;
    if let Some(target) = target.and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok()) {
        let options = web_sys::FocusOptions::new();
        options.set_prevent_scroll(true);
        let _ = target.focus_with_options(&options);
    }
}

#[cfg(feature = "browser")]
fn restore_scroll(scroll: StoredValue<f64>) {
    window().scroll_to_with_x_and_y(window().scroll_x().unwrap_or_default(), scroll.get_value());
}

#[cfg(feature = "hydrate")]
fn place_notifications(dialog: NodeRef<html::Dialog>) -> bool {
    let Some(anchor) = document().get_element_by_id(BELL) else {
        return false;
    };
    let Some(rail) = document().get_element_by_id("navigation") else {
        return false;
    };
    let Some(dialog) = dialog.get_untracked() else {
        return false;
    };
    let top = anchor.get_bounding_client_rect().top().max(12.0);
    let left = rail.get_bounding_client_rect().right() + 8.0;
    let style = web_sys::HtmlElement::style(&dialog);
    let _ = style.set_property("--vk-notif-anchor-top", &format!("{top}px"));
    let _ = style.set_property("--vk-notif-anchor-left", &format!("{left}px"));
    true
}

#[cfg(feature = "hydrate")]
fn observe_notification_text(
    probe: NodeRef<html::Span>,
    large: RwSignal<bool>,
    close: impl Fn(CloseReason) -> bool + Send + Sync + 'static,
) {
    use wasm_bindgen::{JsCast, closure::Closure};
    let observer = StoredValue::new_local(None);
    let close = Callback::new(move |reason| {
        close(reason);
    });
    Effect::new(move |_| {
        let Some(node) = probe.get() else {
            return;
        };
        let measure = move || {
            let is_large = node.get_bounding_client_rect().width() >= 24.0;
            if is_large {
                close.run(CloseReason::Resize);
            }
            large.set(is_large);
        };
        measure();
        let callback = Closure::<dyn FnMut()>::new(measure);
        if let Ok(resize) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
            if let Some(node) = probe.get_untracked() {
                resize.observe(&node);
            }
            observer.set_value(Some((resize, callback)));
        }
    });
    on_cleanup(move || {
        observer.try_update_value(|value| {
            if let Some((resize, _)) = value.take() {
                resize.disconnect();
            }
        });
    });
}

fn current_window_class() -> WindowClass {
    #[cfg(feature = "browser")]
    {
        let (medium, expanded) = WindowClass::CSS_PX;
        let at_least = |width| {
            window()
                .match_media(&format!("(min-width: {width}px)"))
                .ok()
                .flatten()
                .is_some_and(|media| media.matches())
        };
        if at_least(expanded) {
            WindowClass::Expanded
        } else if at_least(medium) {
            WindowClass::Medium
        } else {
            WindowClass::Compact
        }
    }
    #[cfg(not(feature = "browser"))]
    WindowClass::Expanded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_belong_to_explore_editors_to_desk_and_unknown_routes_to_none() {
        for (path, expected) in [
            ("/", Some(Destination::Explore)),
            (
                "/tac-pham/33333333-3333-4333-8333-333333333333",
                Some(Destination::Explore),
            ),
            ("/the/ký-ức", Some(Destination::Explore)),
            ("/the/a/b", None),
            ("/ban-viet", Some(Destination::Desk)),
            ("/ban-viet/", Some(Destination::Desk)),
            ("/ban-viet/abc", Some(Destination::Desk)),
            ("/ban-viet/abc/kiem-duyet", Some(Destination::Desk)),
            ("/toi", Some(Destination::Me)),
            // Prefixes and deeper paths are NotFound, not a lit tab.
            ("/ban-vietnam", None),
            ("/ban-viet/abc/kiem-duyet/khac", None),
            ("/kiem-duyet", None),
            ("/kiem-duyet/ca/abc", None),
            ("/toi/khac", None),
            ("/tac-pham", None),
            ("/tac-pham/a/b", None),
            ("/kham-pha", None),
        ] {
            assert_eq!(Destination::for_path(path), expected, "{path}");
        }
    }

    #[test]
    fn only_the_open_editor_is_focused_work() {
        assert_eq!(
            editor_context("/ban-viet/abc"),
            NavContext {
                compact_rail: true,
                hide_bar: true
            }
        );
        for path in [
            "/",
            "/ban-viet",
            "/toi",
            "/tac-pham/a",
            "/ban-vietnam/x",
            "/ban-viet/abc/kiem-duyet",
        ] {
            assert_eq!(editor_context(path), NavContext::default(), "{path}");
        }
    }

    #[test]
    fn destinations_keep_the_locked_order_and_real_hrefs() {
        let hrefs: Vec<_> = [Destination::Explore, Destination::Desk, Destination::Me]
            .into_iter()
            .map(|d| (d.nav().label, d.nav().href))
            .collect();
        assert_eq!(
            hrefs,
            [
                ("Khám phá".to_string(), "/".to_string()),
                ("Bàn viết".to_string(), "/ban-viet".to_string()),
                ("Tôi".to_string(), "/toi".to_string()),
            ]
        );
    }
}
