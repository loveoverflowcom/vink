//! Vink's adaptive shell: its destinations, the Tôi utility overlay and the responsive policy,
//! rendered with the UI system's navigation primitives (shared/navigation.md). Independent of the
//! route's lifetime and of draft/save state: changing the frame never remounts `#noi-dung`.

use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_location;
use vink_ui::{
    AdaptiveScaffold, BottomNavigation, CloseReason, IconName, NavContext, NavDestination,
    NavLayout, NavUtility, NavigationRail, OverlayLayer, WindowClass,
};

use crate::appearance::{
    AccountAvatar, Appearance, AppearanceChoices, AppearanceTitle, provide_account,
};
use crate::navigation::use_navigation_guard;

/// The Tôi overlay's id in the overlay layer and the DOM.
const PROFILE: &str = "profile-menu";

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
            [] | ["tac-pham", _] | ["the", _] => Some(Self::Explore),
            ["ban-viet"] | ["ban-viet", _] => Some(Self::Desk),
            ["toi"] => Some(Self::Me),
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
    provide_account();
    let location = use_location();
    let current = Memo::new(move |_| location.pathname.with(|path| Destination::for_path(path)));
    let context = Memo::new(move |_| location.pathname.with(|path| editor_context(path)));
    // The server renders the desktop frame's labels; CSS already sizes the rail before hydration.
    let window_class = RwSignal::new(WindowClass::Expanded);
    let layout = Memo::new(move |_| {
        NavLayout::resolve(window_class.get(), context.get(), appearance.rail.get())
    });
    let overlays = RwSignal::new(OverlayLayer::default());
    let profile_open = Memo::new(move |_| overlays.with(|layer| layer.is_open(PROFILE)));
    let profile = NodeRef::<html::Dialog>::new();
    let trigger = NodeRef::<html::Button>::new();
    let page_scroll = StoredValue::new(0.0);
    let close = move |reason| close_profile(reason, overlays, profile, trigger, page_scroll);
    let navigation = use_navigation_guard();
    // Back/forward and real navigations close the overlay before the editor's check runs.
    let overlay_closer = navigation.set_overlay(close);
    on_cleanup(move || navigation.clear_overlay(overlay_closer));

    #[cfg(feature = "hydrate")]
    {
        let listener = window_event_listener(leptos::ev::resize, move |_| {
            let class = WindowClass::from_css_px(viewport_width());
            window_class.set(class);
            if class == WindowClass::Compact {
                close(CloseReason::Resize);
            }
        });
        on_cleanup(move || listener.remove());
        Effect::new(move |_| window_class.set(WindowClass::from_css_px(viewport_width())));
    }

    let active =
        move |destination: Destination| Signal::derive(move || current.get() == Some(destination));
    let primary = move || {
        [Destination::Explore, Destination::Desk]
            .into_iter()
            .map(|destination| (destination.nav(), active(destination)))
            .collect::<Vec<_>>()
    };
    let open_profile = Callback::new(move |()| {
        #[cfg(feature = "hydrate")]
        if let Some(dialog) = profile.get_untracked()
            && overlays
                .try_update(|layer| layer.open(PROFILE))
                .is_some_and(|opened| opened.is_ok())
        {
            page_scroll.set_value(window().scroll_y().unwrap_or_default());
            if dialog.show_modal().is_ok() {
                // Focus the selected appearance choice, not an unrelated action.
                use wasm_bindgen::JsCast;
                if let Ok(Some(input)) = dialog.query_selector("input:checked")
                    && let Ok(input) = input.dyn_into::<web_sys::HtmlElement>()
                {
                    let _ = input.focus();
                }
                restore_scroll(page_scroll);
            } else {
                overlays.update(|layer| {
                    layer.close(PROFILE, CloseReason::Dismiss);
                });
            }
        }
    });
    let toggle = Callback::new(move |()| {
        // A click can arrive before the pending resize event updates the reactive layout.
        // Toggle the rail the person actually sees now.
        #[cfg(feature = "hydrate")]
        let shown = NavLayout::resolve(
            WindowClass::from_css_px(viewport_width()),
            context.get_untracked(),
            appearance.rail.get_untracked(),
        );
        #[cfg(not(feature = "hydrate"))]
        let shown = layout.get_untracked();
        if let Some(next) = shown.next_preference() {
            appearance.set_rail(Some(next));
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
                    expanded=Signal::derive(move || layout.get().rail_expanded())
                    expand_label="Mở rộng điều hướng"
                    collapse_label="Thu gọn điều hướng"
                    on_toggle=toggle
                    destinations=primary()
                    account=move || view! {
                        <NavUtility
                            id="profile-trigger"
                            label="Tôi"
                            icon=IconName::Person
                            leading=move || view! { <AccountAvatar navigation=true /> }
                            controls=PROFILE
                            expanded=profile_open
                            current=active(Destination::Me)
                            on_open=open_profile
                            node_ref=trigger
                        />
                    }
                />
            }
            bar=move || view! {
                <BottomNavigation
                    id="bottom-navigation"
                    label="Điều hướng mobile"
                    destinations=primary()
                    trailing=move || view! {
                        <vink_ui::NavItem destination=Destination::Me.nav() active=active(Destination::Me)
                            leading=move || view! { <AccountAvatar navigation=true /> } />
                    }
                />
            }
            overlays=move || view! {
                <dialog
                    id=PROFILE
                    class="vk-profile-menu"
                    aria-labelledby="profile-title"
                    data-testid="profile-menu"
                    node_ref=profile
                    on:keydown=move |event| trap_profile_tab(event, profile)
                    on:cancel=move |event: leptos::ev::Event| { event.prevent_default(); close(CloseReason::Escape); }
                    // A close the shell did not ask for still releases the overlay layer. The
                    // event is queued, so one from an earlier close can arrive after a reopen:
                    // only a dialog that is closed now releases it.
                    on:close=move |_| {
                        if profile.get_untracked().is_none_or(|dialog| !dialog.open()) {
                            overlays.update(|layer| { layer.close(PROFILE, CloseReason::Dismiss); });
                        }
                    }
                    on:click=move |event: leptos::ev::MouseEvent| {
                        #[cfg(feature = "hydrate")]
                        // Keyboard activation of a radio dispatches a click with (0, 0).
                        // Only the dialog/backdrop itself can be an outside click; bubbled
                        // control clicks must not dismiss the utility or steal their focus.
                        if event.target() == event.current_target()
                            && let Some(dialog) = profile.get_untracked() {
                            let rect = dialog.get_bounding_client_rect();
                            let (x, y) = (f64::from(event.client_x()), f64::from(event.client_y()));
                            if x < rect.left() || x > rect.right() || y < rect.top() || y > rect.bottom() {
                                close(CloseReason::Outside);
                            }
                        }
                        #[cfg(not(feature = "hydrate"))]
                        let _ = event;
                    }
                >
                    <header class="vk-profile-heading">
                        <AppearanceTitle id="profile-title" />
                        <vink_ui::IconButton icon=IconName::Close label="Đóng Giao diện" id="profile-close"
                            on_press=Callback::new(move |()| { close(CloseReason::Dismiss); }) />
                    </header>
                    <AppearanceChoices prefix="overlay" />
                    <Show when=move || current.get() != Some(Destination::Me)>
                        <vink_ui::Button variant=vink_ui::ButtonVariant::Text id="open-me" on_press=move |_| { close(CloseReason::Navigate); navigation.navigate("/toi"); }>
                            "Mở trang Tôi"
                            <span data-appearance-icon="forward" aria-hidden="true"></span>
                        </vink_ui::Button>
                    </Show>
                </dialog>
            }
        >
            {children()}
        </AdaptiveScaffold>
    }
}

/// Keep keyboard traversal inside the utility even when a browser would send an edge Tab to
/// its own chrome. Unchecked radios are one native tab stop with the checked option.
fn trap_profile_tab(event: leptos::ev::KeyboardEvent, profile: NodeRef<html::Dialog>) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if event.key() != "Tab" {
            return;
        }
        let Some(dialog) = profile.get_untracked() else {
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
        let target = if event.shift_key() && first.is_same_node(Some(&active)) {
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
    #[cfg(not(feature = "hydrate"))]
    let _ = (event, profile);
}

/// Closes the Tôi overlay if it is the active one and returns whether it did. Focus follows the
/// overlay contract; this shell's `#noi-dung` outlives routes, so it is also where a starting
/// navigation leaves focus.
fn close_profile(
    reason: CloseReason,
    overlays: RwSignal<OverlayLayer>,
    profile: NodeRef<html::Dialog>,
    trigger: NodeRef<html::Button>,
    page_scroll: StoredValue<f64>,
) -> bool {
    let closed = overlays
        .try_update(|layer| layer.close(PROFILE, reason))
        .flatten();
    let Some(closed) = closed else {
        return false;
    };
    if let Some(dialog) = profile.get_untracked() {
        dialog.close();
    }
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        let visible = trigger.get_untracked().filter(|trigger| {
            trigger.is_connected() && trigger.get_bounding_client_rect().width() > 0.0
        });
        let options = web_sys::FocusOptions::new();
        options.set_prevent_scroll(true);
        match (closed.reason.focus_return(visible.is_some()), visible) {
            (vink_ui::FocusReturn::Trigger, Some(trigger)) => {
                let _ = trigger.focus_with_options(&options);
            }
            _ => {
                if let Some(main) = document().get_element_by_id("noi-dung")
                    && let Ok(main) = main.dyn_into::<web_sys::HtmlElement>()
                {
                    let _ = main.focus_with_options(&options);
                }
            }
        }
        restore_scroll(page_scroll);
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (closed, trigger, page_scroll);
    true
}

#[cfg(feature = "hydrate")]
fn restore_scroll(scroll: StoredValue<f64>) {
    window().scroll_to_with_x_and_y(window().scroll_x().unwrap_or_default(), scroll.get_value());
}

#[cfg(feature = "hydrate")]
fn viewport_width() -> f64 {
    window()
        .inner_width()
        .ok()
        .and_then(|width| width.as_f64())
        .unwrap_or_default()
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
            ("/toi", Some(Destination::Me)),
            // Prefixes and deeper paths are NotFound, not a lit tab.
            ("/ban-vietnam", None),
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
        for path in ["/", "/ban-viet", "/toi", "/tac-pham/a", "/ban-vietnam/x"] {
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
