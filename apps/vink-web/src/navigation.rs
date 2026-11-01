//! One guard for leaving a page that holds text the server has not confirmed.
//!
//! A page registers a check while it is open (today: the desk editor). Internal links — topbar,
//! brand, breadcrumbs and any later rail or profile link — browser back/forward, and programmatic
//! exits through [`NavigationGuard::navigate`] consult it before the router changes route. When
//! the check holds a navigation, the page asks the author and later calls
//! [`NavigationGuard::go`]. Leaving the document itself (reload, close, external link) keeps the
//! browser's own `beforeunload` prompt, owned by the page.
//!
//! Links stay plain `<a href>`: the guard listens in the capture phase and prevents the default
//! action, which the router's own link handler respects. Back/forward cannot be cancelled: the
//! guard's `popstate` listener is registered before the router's, stops a held event before the
//! router sees it and pushes the page's URL back.

use std::sync::Arc;

use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::location::Location;

type Check = Arc<dyn Fn(&str) -> bool + Send + Sync>;

#[derive(Clone, Copy)]
pub struct NavigationGuard {
    check: StoredValue<Option<(u64, Check)>>,
    registrations: StoredValue<u64>,
    router: StoredValue<Option<RouterHooks>>,
}

/// What the guard needs from `<Router>`, connected from inside it.
struct RouterHooks {
    navigate: Box<dyn Fn(&str) + Send + Sync>,
    /// Read by the browser's history listener only.
    #[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
    location: Location,
}

impl NavigationGuard {
    /// Creates the guard, provides it as context and, in the browser, listens to link clicks and
    /// history traversal. Call in the app root before `<Router>` renders: listeners on one target
    /// run in registration order, and the guard has to see `popstate` before the router.
    pub fn provide() -> Self {
        let guard = Self {
            check: StoredValue::new(None),
            registrations: StoredValue::new(0),
            router: StoredValue::new(None),
        };
        provide_context(guard);
        #[cfg(feature = "hydrate")]
        dom::listen(guard);
        guard
    }

    /// Connects the router's navigation and location. Call once, inside `<Router>`.
    pub fn connect_router(&self) {
        let navigate = use_navigate();
        self.router.set_value(Some(RouterHooks {
            navigate: Box::new(move |to: &str| navigate(to, NavigateOptions::default())),
            location: use_location(),
        }));
    }

    /// Registers the open page's check, which returns `true` when it holds a navigation to the
    /// given path. A later registration replaces it; [`Self::unregister`] removes only its own.
    pub fn register(&self, check: impl Fn(&str) -> bool + Send + Sync + 'static) -> u64 {
        let id = self.registrations.get_value() + 1;
        self.registrations.set_value(id);
        self.check.set_value(Some((id, Arc::new(check))));
        id
    }

    pub fn unregister(&self, id: u64) {
        self.check.try_update_value(|check| {
            if check.as_ref().is_some_and(|(current, _)| *current == id) {
                *check = None;
            }
        });
    }

    /// True when the open page holds a navigation to `destination` and is asking the author.
    pub fn holds(&self, destination: &str) -> bool {
        let check = self
            .check
            .try_with_value(|check| check.as_ref().map(|(_, check)| check.clone()))
            .flatten();
        check.is_some_and(|check| check(destination))
    }

    /// A navigation started by code (a menu item, a redirect), subject to the guard.
    pub fn navigate(&self, destination: &str) {
        if !self.holds(destination) {
            self.go(destination);
        }
    }

    /// A navigation the open page already decided on, after the author chose to leave.
    pub fn go(&self, destination: &str) {
        self.router.with_value(|router| {
            if let Some(router) = router {
                (router.navigate)(destination);
            }
        });
    }

    /// The route the router shows, as path, query and hash in the browser's own format.
    #[cfg(feature = "hydrate")]
    fn router_href(&self) -> Option<String> {
        self.router
            .try_with_value(|router| router.as_ref().map(|router| href(&router.location)))
            .flatten()
    }
}

pub fn use_navigation_guard() -> NavigationGuard {
    expect_context::<NavigationGuard>()
}

#[cfg(feature = "hydrate")]
fn href(location: &Location) -> String {
    let search = location.search.get_untracked();
    let hash = location.hash.get_untracked();
    let mut href = location.pathname.get_untracked();
    if !search.is_empty() {
        href.push('?');
        href.push_str(search.trim_start_matches('?'));
    }
    if !hash.is_empty() && hash != "#" {
        href.push('#');
        href.push_str(hash.trim_start_matches('#'));
    }
    href
}

#[cfg(feature = "hydrate")]
mod dom {
    use send_wrapper::SendWrapper;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use web_sys::{AddEventListenerOptions, Element, Event, HtmlAnchorElement, MouseEvent, Url};

    use super::NavigationGuard;

    pub fn listen(guard: NavigationGuard) {
        let on_click = Closure::<dyn Fn(MouseEvent)>::new(move |event: MouseEvent| {
            let Some(destination) = link_destination(&event) else {
                return;
            };
            if !same_page(&destination) && guard.holds(&destination) {
                event.prevent_default();
            }
        });
        let on_popstate = Closure::<dyn Fn(Event)>::new(move |event: Event| {
            let (Some(here), Some(page)) = (window_href(), guard.router_href()) else {
                return;
            };
            // The router has not handled this event yet: its location is still the page's.
            if path_and_query(&page) == path_and_query(&here) || !guard.holds(&here) {
                return;
            }
            event.stop_immediate_propagation();
            if let Ok(history) = leptos::prelude::window().history() {
                let _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&page));
            }
        });
        let capture = AddEventListenerOptions::new();
        capture.set_capture(true);
        let document = leptos::prelude::document();
        let window = leptos::prelude::window();
        let _ = document.add_event_listener_with_callback_and_add_event_listener_options(
            "click",
            on_click.as_ref().unchecked_ref(),
            &capture,
        );
        let _ = window
            .add_event_listener_with_callback("popstate", on_popstate.as_ref().unchecked_ref());
        let listeners = SendWrapper::new((on_click, on_popstate));
        leptos::prelude::on_cleanup(move || {
            let (on_click, on_popstate) = listeners.take();
            let document = leptos::prelude::document();
            let window = leptos::prelude::window();
            let _ = document.remove_event_listener_with_callback_and_bool(
                "click",
                on_click.as_ref().unchecked_ref(),
                true,
            );
            let _ = window.remove_event_listener_with_callback(
                "popstate",
                on_popstate.as_ref().unchecked_ref(),
            );
        });
    }

    /// Path, query and hash of a same-origin link the router would handle itself.
    fn link_destination(event: &MouseEvent) -> Option<String> {
        if event.default_prevented()
            || event.button() != 0
            || event.meta_key()
            || event.alt_key()
            || event.ctrl_key()
            || event.shift_key()
        {
            return None;
        }
        let target = event.target()?.dyn_into::<Element>().ok()?;
        let anchor = target
            .closest("a[href]")
            .ok()??
            .dyn_into::<HtmlAnchorElement>()
            .ok()?;
        let external = anchor
            .get_attribute("rel")
            .is_some_and(|rel| rel.split_ascii_whitespace().any(|part| part == "external"));
        if !anchor.target().is_empty() || anchor.has_attribute("download") || external {
            return None;
        }
        let url = Url::new(&anchor.href()).ok()?;
        let origin = leptos::prelude::window().location().origin().ok()?;
        (url.origin() == origin)
            .then(|| format!("{}{}{}", url.pathname(), url.search(), url.hash()))
    }

    fn window_href() -> Option<String> {
        let location = leptos::prelude::window().location();
        Some(format!(
            "{}{}{}",
            location.pathname().ok()?,
            location.search().ok()?,
            location.hash().ok()?
        ))
    }

    /// A link to the same path and query only moves within the page (a skip link, a fragment).
    fn same_page(destination: &str) -> bool {
        window_href().is_some_and(|here| path_and_query(&here) == path_and_query(destination))
    }

    fn path_and_query(href: &str) -> &str {
        href.split('#').next().unwrap_or(href)
    }
}
