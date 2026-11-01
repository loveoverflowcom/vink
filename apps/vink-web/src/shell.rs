//! Adaptive navigation, independent of the route's lifetime and draft/save state.

use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_location;

use crate::appearance::{AccountStatus, Appearance, Rail, ThemePicker};
use crate::navigation::use_navigation_guard;

#[derive(Clone, Copy)]
enum IconKind {
    Explore,
    Write,
    Person,
    Rail,
}

#[component]
fn Icon(kind: IconKind) -> impl IntoView {
    let path = match kind {
        IconKind::Explore => "m16 8-3 5-5 3 3-5 5-3ZM12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Z",
        IconKind::Write => "m15 5 4 4M4 20l4-1L20 7a2.8 2.8 0 0 0-4-4L4 15v5Z",
        IconKind::Person => "M20 21v-2a7 7 0 0 0-14 0v2M13 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z",
        IconKind::Rail => "M9 3v18M3 3h18v18H3V3Zm10 6 3 3-3 3",
    };
    view! {
        <svg class="vk-icon" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d=path />
        </svg>
    }
}

#[component]
fn Brand(#[prop(optional)] mobile: bool) -> impl IntoView {
    view! {
        <a class=if mobile { "vk-brand vk-mobile-brand" } else { "vk-brand vk-rail-brand" } href="/" aria-label="Vink — trang chủ">
            <span class="vk-brand__mark" aria-hidden="true"></span>
            <span class="vk-brand__name">"Vink"</span>
        </a>
    }
}

#[component]
fn PrimaryLinks() -> impl IntoView {
    let location = use_location();
    view! {
        <a href="/" aria-current=move || {
            let path = location.pathname.get();
            (path == "/" || path.starts_with("/tac-pham/")).then_some("page")
        }>
            <Icon kind=IconKind::Explore /><span>"Khám phá"</span>
        </a>
        <a href="/ban-viet" aria-current=move || location.pathname.get().starts_with("/ban-viet").then_some("page")>
            <Icon kind=IconKind::Write /><span>"Bàn viết"</span>
        </a>
    }
}

#[component]
pub(crate) fn AdaptiveShell(children: Children) -> impl IntoView {
    let appearance = Appearance::provide();
    let location = use_location();
    let editor = Memo::new(move |_| location.pathname.get().starts_with("/ban-viet/"));
    let desktop = RwSignal::new(true);
    let expanded = move || {
        appearance.rail.get().map_or_else(
            || desktop.get() && !editor.get(),
            |rail| rail == Rail::Expanded,
        )
    };
    let profile = NodeRef::<html::Dialog>::new();
    let trigger = NodeRef::<html::Button>::new();
    let bottom = NodeRef::<html::Nav>::new();
    let open = RwSignal::new(false);
    let page_scroll = StoredValue::new(0.0);
    let tooltip = RwSignal::new(false);
    let close = move || close_profile(profile, trigger, open, page_scroll);
    let navigation = use_navigation_guard();

    #[cfg(feature = "hydrate")]
    {
        let listener = window_event_listener(leptos::ev::resize, move |_| {
            desktop.set(viewport_width() >= 1100.0);
            if viewport_width() < 760.0 && open.get_untracked() {
                close();
            }
        });
        let escape = window_event_listener(leptos::ev::keydown, move |event| {
            if event.key() == "Escape" {
                tooltip.set(false);
            }
        });
        on_cleanup(move || {
            listener.remove();
            escape.remove();
        });
        Effect::new(move |_| {
            desktop.set(viewport_width() >= 1100.0);
            observe_bottom_bar(bottom);
        });
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (desktop, bottom);

    view! {
        <div id="shell" class="vk-shell" data-editor=move || editor.get().to_string()>
            <a class="vk-skip-link" href="#noi-dung">"Bỏ qua tới nội dung"</a>
            <aside id="navigation" class="vk-rail" aria-label="Không gian Vink">
                <Brand />
                <div class="vk-rail-toggle">
                    <button
                        type="button"
                        class="vk-icon-button"
                        data-testid="rail-toggle"
                        aria-label=move || if expanded() { "Thu gọn điều hướng" } else { "Mở rộng điều hướng" }
                        aria-expanded=move || expanded().to_string()
                        aria-controls="navigation"
                        aria-describedby=move || tooltip.get().then_some("rail-tooltip")
                        on:click=move |_| {
                            // A click can arrive before the pending resize event updates the
                            // reactive label. Toggle the width the person actually sees now.
                            #[cfg(feature = "hydrate")]
                            let expanded_now = appearance.rail.get_untracked().map_or_else(
                                || viewport_width() >= 1100.0 && !editor.get_untracked(),
                                |rail| rail == Rail::Expanded,
                            );
                            #[cfg(not(feature = "hydrate"))]
                            let expanded_now = expanded();
                            appearance.set_rail(if expanded_now { Rail::Compact } else { Rail::Expanded });
                            tooltip.set(false);
                        }
                        on:mouseenter=move |_| tooltip.set(true)
                        on:mouseleave=move |_| tooltip.set(false)
                        on:focus=move |_| tooltip.set(true)
                        on:blur=move |_| tooltip.set(false)
                    >
                        <Icon kind=IconKind::Rail />
                    </button>
                </div>
                <nav id="nav-primary" class="vk-nav" aria-label="Điều hướng chính"><PrimaryLinks /></nav>
                <div id="account-bottom" class="vk-account-bottom">
                    <button
                        class="vk-account-trigger"
                        type="button"
                        data-testid="profile-trigger"
                        node_ref=trigger
                        aria-haspopup="dialog"
                        aria-controls="profile-menu"
                        aria-expanded=move || open.get().to_string()
                        aria-current=move || (location.pathname.get() == "/toi").then_some("page")
                        on:click=move |_| {
                            #[cfg(feature = "hydrate")]
                            if let Some(dialog) = profile.get_untracked() {
                                page_scroll.set_value(window().scroll_y().unwrap_or_default());
                                if dialog.show_modal().is_ok() {
                                    open.set(true);
                                    // Focus the selected appearance choice, not an unrelated action.
                                    use wasm_bindgen::JsCast;
                                    if let Ok(Some(input)) = dialog.query_selector("input:checked")
                                        && let Ok(input) = input.dyn_into::<web_sys::HtmlElement>()
                                    {
                                        let _ = input.focus();
                                    }
                                    restore_scroll(page_scroll);
                                }
                            }
                        }
                    ><Icon kind=IconKind::Person /><span>"Tôi"</span></button>
                </div>
            </aside>
            <div class="vk-shell-content">
                <header class="vk-mobile-header"><Brand mobile=true /></header>
                {children()}
            </div>
            <nav id="bottom-navigation" class="vk-bottom-nav" aria-label="Điều hướng mobile" node_ref=bottom>
                <PrimaryLinks />
                <a href="/toi" aria-current=move || (location.pathname.get() == "/toi").then_some("page")>
                    <Icon kind=IconKind::Person /><span>"Tôi"</span>
                </a>
            </nav>
            // Outside the rail's scrolling/container-query boundary, so it cannot be clipped.
            <span id="rail-tooltip" role="tooltip" class="vk-tooltip" hidden=move || !tooltip.get()>
                {move || if expanded() { "Thu gọn điều hướng" } else { "Mở rộng điều hướng" }}
            </span>
            <dialog
                id="profile-menu"
                class="vk-profile-menu"
                aria-labelledby="profile-title"
                data-testid="profile-menu"
                node_ref=profile
                on:keydown=move |event| trap_profile_tab(event, profile)
                on:cancel=move |event: leptos::ev::Event| { event.prevent_default(); close(); }
                on:close=move |_| open.set(false)
                on:click=move |event: leptos::ev::MouseEvent| {
                    #[cfg(feature = "hydrate")]
                    if let Some(dialog) = profile.get_untracked() {
                        let rect = dialog.get_bounding_client_rect();
                        let (x, y) = (f64::from(event.client_x()), f64::from(event.client_y()));
                        if x < rect.left() || x > rect.right() || y < rect.top() || y > rect.bottom() {
                            close();
                        }
                    }
                    #[cfg(not(feature = "hydrate"))]
                    let _ = event;
                }
            >
                <header class="vk-profile-heading">
                    <h2 id="profile-title">"Tôi"</h2>
                    <button type="button" class="vk-button vk-button--text" aria-label="Đóng Tôi" data-testid="profile-close" on:click=move |_| close()>
                        "Đóng"
                    </button>
                </header>
                <AccountStatus />
                <ThemePicker group="overlay-theme" />
                <button type="button" class="vk-button vk-button--tonal" data-testid="open-me" on:click=move |_| { close(); navigation.navigate("/toi"); }>
                    "Mở trang Tôi"
                </button>
            </dialog>
        </div>
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
        let Ok(stops) = dialog.query_selector_all("button:not(:disabled), input:checked, a[href]")
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

fn close_profile(
    profile: NodeRef<html::Dialog>,
    trigger: NodeRef<html::Button>,
    open: RwSignal<bool>,
    page_scroll: StoredValue<f64>,
) {
    if let Some(dialog) = profile.get_untracked() {
        dialog.close();
    }
    open.set(false);
    #[cfg(feature = "hydrate")]
    {
        if let Some(trigger) = trigger.get_untracked()
            && trigger.is_connected()
            && trigger.get_bounding_client_rect().width() > 0.0
        {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = trigger.focus_with_options(&options);
        } else {
            use wasm_bindgen::JsCast;
            if let Some(main) = document().get_element_by_id("noi-dung")
                && let Ok(main) = main.dyn_into::<web_sys::HtmlElement>()
            {
                let options = web_sys::FocusOptions::new();
                options.set_prevent_scroll(true);
                let _ = main.focus_with_options(&options);
            }
        }
        restore_scroll(page_scroll);
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (trigger, page_scroll);
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

/// The bar can grow with text size and safe area. Reserve its measured height rather than a
/// fixed pixel guess. Observe once, dispose with the shell; no route/editor remount is involved.
#[cfg(feature = "hydrate")]
fn observe_bottom_bar(bottom: NodeRef<html::Nav>) {
    use send_wrapper::SendWrapper;
    use wasm_bindgen::{JsCast, closure::Closure};
    let Some(bottom) = bottom.get_untracked() else {
        return;
    };
    let measured = bottom.clone();
    let callback = Closure::<dyn FnMut()>::new(move || {
        if let Some(root) = document().document_element()
            && let Ok(root) = root.dyn_into::<web_sys::HtmlElement>()
        {
            let height = measured.get_bounding_client_rect().height();
            let _ = root
                .style()
                .set_property("--vk-bottom-height", &format!("{height}px"));
        }
    });
    if let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
        observer.observe(&bottom);
        let observer = SendWrapper::new((observer, callback));
        on_cleanup(move || observer.take().0.disconnect());
    }
}
