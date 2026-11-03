//! Shell and navigation (UIS-04, `shared/navigation.md`): a pure layout/overlay policy and the
//! Leptos primitives that render it. Destinations, their order and visibility, routes, history,
//! the navigation guard, stored preferences and route focus belong to the host; these
//! components render facts and dispatch intents. Links are real `<a href>`: the host's router
//! (or its guard) receives the click, so modifier clicks, copy link and new tabs keep working.

use leptos::html;
use leptos::prelude::*;

use crate::icons::IconName;
use crate::media::Icon;

/// Available window width class. The host measures its own available width; thresholds are
/// Web CSS px from the #7 shell and Material dp on CMP.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowClass {
    Compact,
    Medium,
    Expanded,
}

impl WindowClass {
    /// Lower bounds (medium, expanded) of the Web shell in CSS px.
    pub const CSS_PX: (f64, f64) = (760.0, 1100.0);
    /// Lower bounds (medium, expanded) of Material window size classes in dp.
    pub const DP: (f64, f64) = (600.0, 840.0);

    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Medium => "medium",
            Self::Expanded => "expanded",
        }
    }

    fn classify(width: f64, (medium, expanded): (f64, f64)) -> Self {
        if width < medium {
            Self::Compact
        } else if width < expanded {
            Self::Medium
        } else {
            Self::Expanded
        }
    }

    pub fn from_css_px(width: f64) -> Self {
        Self::classify(width, Self::CSS_PX)
    }

    pub fn from_dp(width: f64) -> Self {
        Self::classify(width, Self::DP)
    }
}

/// A person's explicit rail choice; it wins over the contextual default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RailPreference {
    Expanded,
    Compact,
}

impl RailPreference {
    pub const fn value(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::Compact => "compact",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "expanded" => Some(Self::Expanded),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }
}

/// What the current route asks of the shell (Web: the editor sets both).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NavContext {
    /// Default the rail to compact so focused work gets the width.
    pub compact_rail: bool,
    /// Hide the bottom bar so it never covers the work surface.
    pub hide_bar: bool,
}

/// The navigation frame the shell shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavLayout {
    RailExpanded,
    RailCompact,
    Bar,
    BarHidden,
}

impl NavLayout {
    /// User preference > contextual default; the bottom bar ignores rail preferences.
    pub fn resolve(
        window: WindowClass,
        context: NavContext,
        preference: Option<RailPreference>,
    ) -> Self {
        match (window, preference) {
            (WindowClass::Compact, _) if context.hide_bar => Self::BarHidden,
            (WindowClass::Compact, _) => Self::Bar,
            (_, Some(RailPreference::Expanded)) => Self::RailExpanded,
            (_, Some(RailPreference::Compact)) => Self::RailCompact,
            (WindowClass::Expanded, None) if !context.compact_rail => Self::RailExpanded,
            (_, None) => Self::RailCompact,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::RailExpanded => "rail:expanded",
            Self::RailCompact => "rail:compact",
            Self::Bar => "bar",
            Self::BarHidden => "bar:hidden",
        }
    }

    pub const fn rail_expanded(self) -> bool {
        matches!(self, Self::RailExpanded)
    }

    /// The preference the rail toggle stores: the opposite of the rail as shown now. The bottom
    /// bar has no toggle.
    pub const fn next_preference(self) -> Option<RailPreference> {
        match self {
            Self::RailExpanded => Some(RailPreference::Compact),
            Self::RailCompact => Some(RailPreference::Expanded),
            Self::Bar | Self::BarHidden => None,
        }
    }
}

/// Why an overlay closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CloseReason {
    /// Its own close action.
    Dismiss,
    Escape,
    /// Browser/system back while it was open.
    Back,
    Outside,
    /// The window changed so the overlay no longer belongs (Web: down to the bottom bar).
    Resize,
    /// A real navigation is starting.
    Navigate,
}

impl CloseReason {
    pub const ALL: [Self; 6] = [
        Self::Dismiss,
        Self::Escape,
        Self::Back,
        Self::Outside,
        Self::Resize,
        Self::Navigate,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Dismiss => "dismiss",
            Self::Escape => "escape",
            Self::Back => "back",
            Self::Outside => "outside",
            Self::Resize => "resize",
            Self::Navigate => "navigate",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|reason| reason.id() == id)
    }

    /// Where focus goes after the overlay closes. A navigation leaves focus to the new route.
    pub const fn focus_return(self, trigger_visible: bool) -> FocusReturn {
        match (self, trigger_visible) {
            (Self::Navigate, _) => FocusReturn::Leave,
            (_, true) => FocusReturn::Trigger,
            (_, false) => FocusReturn::Main,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FocusReturn {
    Trigger,
    /// The host's main region, when the trigger is gone or hidden.
    Main,
    /// Do not move focus; the route that is opening owns it.
    Leave,
}

impl FocusReturn {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Trigger => "trigger",
            Self::Main => "main",
            Self::Leave => "leave",
        }
    }
}

/// `open` found another modal active.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayBusy {
    pub active: String,
}

/// An overlay that `close`, `back` or `escape` closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Closed {
    pub id: String,
    pub reason: CloseReason,
}

/// One active modal at a time. A small owner the host keeps next to its overlays; it does not
/// render, focus or listen to anything.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OverlayLayer {
    active: Option<String>,
}

impl OverlayLayer {
    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    pub fn is_open(&self, id: &str) -> bool {
        self.active() == Some(id)
    }

    /// Opening the active overlay again is a no-op; another one is refused.
    pub fn open(&mut self, id: &str) -> Result<(), OverlayBusy> {
        match &self.active {
            Some(active) if active != id => Err(OverlayBusy {
                active: active.clone(),
            }),
            _ => {
                self.active = Some(id.to_string());
                Ok(())
            }
        }
    }

    /// Closes `id` if it is the active overlay; a late close of anything else is stale.
    pub fn close(&mut self, id: &str, reason: CloseReason) -> Option<Closed> {
        if self.is_open(id) {
            self.take(reason)
        } else {
            None
        }
    }

    /// Back closes the active overlay before anything leaves the screen; `None` lets the host's
    /// router or back stack continue.
    pub fn back(&mut self) -> Option<Closed> {
        self.take(CloseReason::Back)
    }

    pub fn escape(&mut self) -> Option<Closed> {
        self.take(CloseReason::Escape)
    }

    fn take(&mut self, reason: CloseReason) -> Option<Closed> {
        self.active.take().map(|id| Closed { id, reason })
    }
}

/// A destination the host offers. The host decides which exist (from real capabilities) and in
/// which order; `key` is stable for tests and active state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavDestination {
    pub key: String,
    pub label: String,
    pub icon: IconName,
    pub href: String,
}

impl NavDestination {
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        icon: IconName,
        href: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            icon,
            href: href.into(),
        }
    }
}

/// A plain primary click a host without a link-handling router takes over. Modifier and
/// non-primary clicks keep the browser's own behaviour (new tab, copy, download).
fn plain_click(event: &leptos::ev::MouseEvent) -> bool {
    event.button() == 0
        && !(event.meta_key() || event.ctrl_key() || event.shift_key() || event.alt_key())
}

/// A route destination: a real link, `aria-current="page"` when active. With `on_navigate` (hosts
/// whose router does not capture links, such as the preview) a plain click is dispatched instead.
#[component]
pub fn NavItem(
    destination: NavDestination,
    #[prop(into)] active: Signal<bool>,
    #[prop(optional)] on_navigate: Option<Callback<String>>,
) -> impl IntoView {
    let NavDestination {
        key,
        label,
        icon,
        href,
    } = destination;
    let dispatched = key.clone();
    view! {
        <a
            class="vk-ui-nav-item"
            href=href
            data-destination=key
            aria-current=move || active.get().then_some("page")
            on:click=move |event: leptos::ev::MouseEvent| {
                if let Some(navigate) = on_navigate
                    && plain_click(&event)
                {
                    event.prevent_default();
                    navigate.run(dispatched.clone());
                }
            }
        >
            <Icon name=icon />
            <span class="vk-ui-nav-item__label">{label}</span>
        </a>
    }
}

/// A shell utility that opens an overlay (Vink: Tôi at the foot of the rail), not a route.
#[component]
pub fn NavUtility(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    icon: IconName,
    /// The overlay element this button opens.
    #[prop(into)]
    controls: String,
    #[prop(into)] expanded: Signal<bool>,
    /// The utility's own page is the current route.
    #[prop(optional, into)]
    current: Signal<bool>,
    on_open: Callback<()>,
    #[prop(optional)] node_ref: NodeRef<html::Button>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            id=id
            class="vk-ui-nav-item vk-ui-nav-utility"
            node_ref=node_ref
            aria-haspopup="dialog"
            aria-controls=controls
            aria-expanded=move || expanded.get().to_string()
            aria-current=move || current.get().then_some("page")
            on:click=move |_| on_open.run(())
        >
            <Icon name=icon />
            <span class="vk-ui-nav-item__label">{label}</span>
        </button>
    }
}

/// Destinations in host order; a dispatch callback only when the host has no link router.
fn items(
    destinations: Vec<(NavDestination, Signal<bool>)>,
    on_navigate: Option<Callback<String>>,
) -> AnyView {
    destinations
        .into_iter()
        .map(|(destination, active)| match on_navigate {
            Some(navigate) => {
                view! { <NavItem destination active on_navigate=navigate /> }.into_any()
            }
            None => view! { <NavItem destination active /> }.into_any(),
        })
        .collect_view()
        .into_any()
}

/// Rail anatomy: brand → permanent toggle → primary destinations → account foot. The tooltip is
/// rendered after the rail, outside its scrolling container, so containment cannot clip it.
/// IDs derive from `id`: `{id}-toggle`, `{id}-tooltip`, `{id}-primary`, `{id}-account`.
#[component]
pub fn NavigationRail(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    #[prop(into)] nav_label: String,
    /// The rail as shown now (the host resolves [`NavLayout`]).
    #[prop(into)]
    expanded: Signal<bool>,
    #[prop(into)] expand_label: String,
    #[prop(into)] collapse_label: String,
    on_toggle: Callback<()>,
    destinations: Vec<(NavDestination, Signal<bool>)>,
    #[prop(optional)] on_navigate: Option<Callback<String>>,
    #[prop(optional, into)] brand: ViewFn,
    #[prop(optional, into)] account: ViewFn,
) -> impl IntoView {
    let tooltip = RwSignal::new(false);
    let toggle = NodeRef::<html::Button>::new();
    let tip = NodeRef::<html::Span>::new();
    let (toggle_id, tooltip_id) = (format!("{id}-toggle"), format!("{id}-tooltip"));
    let (primary_id, account_id) = (format!("{id}-primary"), format!("{id}-account"));
    let name = {
        let (expand, collapse) = (expand_label.clone(), collapse_label.clone());
        move || {
            if expanded.get() {
                collapse.clone()
            } else {
                expand.clone()
            }
        }
    };
    let accessible_name = name.clone();
    let described = tooltip_id.clone();
    let show = move || {
        tooltip.set(true);
        place_tooltip(toggle, tip);
    };

    #[cfg(target_arch = "wasm32")]
    {
        let escape = window_event_listener(leptos::ev::keydown, move |event| {
            if event.key() == "Escape" {
                tooltip.set(false);
            }
        });
        on_cleanup(move || escape.remove());
    }

    let primary = items(destinations, on_navigate);
    let controls = id.clone();

    view! {
        <aside id=id class="vk-ui-rail" aria-label=label>
            <div class="vk-ui-rail__brand">{brand.run()}</div>
            <div class="vk-ui-rail__toggle">
                <button
                    type="button"
                    id=toggle_id
                    class="vk-ui-rail-toggle"
                    node_ref=toggle
                    aria-label=accessible_name
                    aria-expanded=move || expanded.get().to_string()
                    aria-controls=controls
                    aria-describedby=move || tooltip.get().then(|| described.clone())
                    on:click=move |_| {
                        on_toggle.run(());
                        tooltip.set(false);
                    }
                    on:mouseenter=move |_| show()
                    on:mouseleave=move |_| tooltip.set(false)
                    on:focus=move |_| show()
                    on:blur=move |_| tooltip.set(false)
                >
                    {move || {
                        let icon = if expanded.get() { IconName::MenuOpen } else { IconName::Menu };
                        view! { <Icon name=icon /> }
                    }}
                </button>
            </div>
            <nav id=primary_id class="vk-ui-rail__primary" aria-label=nav_label>
                {primary}
            </nav>
            <div id=account_id class="vk-ui-rail__account">{account.run()}</div>
        </aside>
        <span id=tooltip_id role="tooltip" class="vk-ui-tooltip" node_ref=tip hidden=move || !tooltip.get()>
            {name}
        </span>
    }
}

/// Anchors the tooltip outside the rail's edge, level with the toggle, as laid out now (rail
/// width, scroll, text size).
fn place_tooltip(toggle: NodeRef<html::Button>, tip: NodeRef<html::Span>) {
    #[cfg(target_arch = "wasm32")]
    if let (Some(toggle), Some(tip)) = (toggle.get_untracked(), tip.get_untracked()) {
        let rect = toggle.get_bounding_client_rect();
        let edge = toggle
            .closest(".vk-ui-rail")
            .ok()
            .flatten()
            .map_or(rect.right(), |rail| rail.get_bounding_client_rect().right());
        let style = web_sys::HtmlElement::style(&tip);
        let _ = style.set_property("--vk-ui-tooltip-x", &format!("{edge}px"));
        let _ = style.set_property(
            "--vk-ui-tooltip-y",
            &format!("{}px", rect.top() + rect.height() / 2.0),
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (toggle, tip);
}

/// Bottom bar for narrow windows. In the browser it measures its own height (text size and safe
/// area grow it) into `--vk-ui-bar-height` on the scaffold, which reserves that much space.
#[component]
pub fn BottomNavigation(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    destinations: Vec<(NavDestination, Signal<bool>)>,
    #[prop(optional)] on_navigate: Option<Callback<String>>,
) -> impl IntoView {
    let bar = NodeRef::<html::Nav>::new();
    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_| observe_height(bar));
    let entries = items(destinations, on_navigate);
    view! {
        <nav id=id class="vk-ui-bottom-nav" aria-label=label node_ref=bar>
            {entries}
        </nav>
    }
}

/// Observe once, disconnect with the bar; no route or editor remount is involved.
#[cfg(target_arch = "wasm32")]
fn observe_height(bar: NodeRef<html::Nav>) {
    use leptos::wasm_bindgen::JsCast;
    use leptos::wasm_bindgen::closure::Closure;

    let Some(bar) = bar.get_untracked() else {
        return;
    };
    let measured = bar.clone();
    let callback = Closure::<dyn FnMut()>::new(move || {
        if let Ok(Some(scaffold)) = measured.closest(".vk-ui-scaffold")
            && let Ok(scaffold) = scaffold.dyn_into::<web_sys::HtmlElement>()
        {
            let height = measured.get_bounding_client_rect().height();
            let _ = scaffold
                .style()
                .set_property("--vk-ui-bar-height", &format!("{height}px"));
        }
    });
    if let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
        observer.observe(&bar);
        let observer = StoredValue::new_local(Some((observer, callback)));
        on_cleanup(move || {
            if let Some((observer, _)) = observer.try_update_value(Option::take).flatten() {
                observer.disconnect();
            }
        });
    }
}

/// Header of narrow layouts: brand and route actions; never the primary destinations.
#[component]
pub fn TopBar(
    #[prop(optional, into)] brand: ViewFn,
    #[prop(optional, into)] actions: ViewFn,
) -> impl IntoView {
    view! {
        <header class="vk-ui-top-bar">
            <div class="vk-ui-top-bar__brand">{brand.run()}</div>
            <div class="vk-ui-top-bar__actions">{actions.run()}</div>
        </header>
    }
}

/// The adaptive frame. Without `fixed` the browser resolves [`NavLayout`] in CSS (media queries
/// plus the host's `:root[data-rail]`) so server HTML has the right width before hydration;
/// `fixed` states it (previews, tests). Changing the layout changes attributes only: the
/// content, `#main` and any editor inside stay mounted.
#[component]
pub fn AdaptiveScaffold(
    #[prop(optional, into)] id: Option<String>,
    #[prop(into)] main_id: String,
    #[prop(into)] skip_label: String,
    #[prop(optional, into)] fixed: Option<Signal<NavLayout>>,
    #[prop(optional, into)] context: Signal<NavContext>,
    #[prop(optional, into)] rail: ViewFn,
    #[prop(optional, into)] top_bar: ViewFn,
    #[prop(optional, into)] bar: ViewFn,
    #[prop(optional, into)] overlays: ViewFn,
    children: Children,
) -> impl IntoView {
    let skip = format!("#{main_id}");
    // The skip link moves focus itself: a link router would treat the fragment as a navigation
    // and leave focus on the link.
    let skip_to = main_id.clone();
    let skip_click = move |event: leptos::ev::MouseEvent| {
        #[cfg(target_arch = "wasm32")]
        {
            use leptos::wasm_bindgen::JsCast;
            if let Some(main) = document()
                .get_element_by_id(&skip_to)
                .and_then(|main| main.dyn_into::<web_sys::HtmlElement>().ok())
            {
                event.prevent_default();
                let _ = main.focus();
                main.scroll_into_view();
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (event, &skip_to);
    };
    view! {
        <div
            id=id
            class="vk-ui-scaffold"
            data-layout=move || fixed.map_or("auto", |layout| layout.get().id())
            data-compact-rail=move || context.get().compact_rail.then_some("")
            data-hide-bar=move || context.get().hide_bar.then_some("")
        >
            <a class="vk-ui-skip-link" href=skip on:click=skip_click>{skip_label}</a>
            {rail.run()}
            <div class="vk-ui-scaffold__content">
                {top_bar.run()}
                {children()}
            </div>
            {bar.run()}
            {overlays.run()}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation_vectors as vectors;
    use crate::tests::render;

    fn window(id: &str) -> WindowClass {
        [
            WindowClass::Compact,
            WindowClass::Medium,
            WindowClass::Expanded,
        ]
        .into_iter()
        .find(|w| w.id() == id)
        .unwrap_or_else(|| panic!("window {id}"))
    }

    fn context(id: &str) -> NavContext {
        NavContext {
            compact_rail: id.contains("compact-rail"),
            hide_bar: id.contains("hide-bar"),
        }
    }

    #[test]
    fn window_classes_match_the_shared_breakpoints() {
        assert_eq!(
            vectors::CSS_PX,
            (WindowClass::CSS_PX.0 as u32, WindowClass::CSS_PX.1 as u32)
        );
        assert_eq!(
            vectors::DP,
            (WindowClass::DP.0 as u32, WindowClass::DP.1 as u32)
        );
        for &(unit, width, class) in vectors::WINDOWS {
            let got = match unit {
                "px" => WindowClass::from_css_px(f64::from(width)),
                _ => WindowClass::from_dp(f64::from(width)),
            };
            assert_eq!(got.id(), class, "{unit} {width}");
        }
        assert_eq!(WindowClass::from_css_px(759.5), WindowClass::Compact);
    }

    #[test]
    fn layouts_follow_preference_over_context_and_never_rail_on_compact() {
        for &(w, c, p, expected) in vectors::LAYOUTS {
            let preference = (p != "-").then(|| RailPreference::parse(p).expect(p));
            let got = NavLayout::resolve(window(w), context(c), preference);
            assert_eq!(got.id(), expected, "({w}, {c}, {p})");
        }
        assert_eq!(vectors::LAYOUTS.len(), 36);
    }

    #[test]
    fn the_toggle_flips_the_rail_as_shown() {
        for &(shown, next) in vectors::TOGGLES {
            let layout = [
                NavLayout::RailExpanded,
                NavLayout::RailCompact,
                NavLayout::Bar,
                NavLayout::BarHidden,
            ]
            .into_iter()
            .find(|l| l.id() == shown)
            .unwrap();
            assert_eq!(
                layout.next_preference().map_or("-", RailPreference::value),
                next,
                "{shown}"
            );
        }
        assert_eq!(RailPreference::parse("220"), None);
    }

    fn step(layer: &mut OverlayLayer, op: &str) -> String {
        let closed = |c: Option<Closed>, none: &str| {
            c.map_or(none.to_string(), |c| {
                format!("closed:{}:{}", c.id, c.reason.id())
            })
        };
        match op.split(':').collect::<Vec<_>>()[..] {
            ["open", id] => layer
                .open(id)
                .map_or_else(|busy| format!("busy:{}", busy.active), |()| "ok".into()),
            ["close", id, reason] => closed(
                layer.close(id, CloseReason::parse(reason).unwrap()),
                "stale",
            ),
            ["back"] => closed(layer.back(), "pass"),
            ["escape"] => closed(layer.escape(), "pass"),
            _ => panic!("operation {op}"),
        }
    }

    #[test]
    fn one_modal_is_active_and_back_closes_it_before_leaving() {
        for &(name, steps) in vectors::OVERLAY {
            let mut layer = OverlayLayer::default();
            for &(op, expected) in steps {
                assert_eq!(step(&mut layer, op), expected, "{name}: {op}");
            }
        }
    }

    #[test]
    fn focus_returns_to_a_visible_trigger_or_main_and_navigation_leaves_it() {
        for &(reason, visible, expected) in vectors::FOCUS {
            let reason = CloseReason::parse(reason).unwrap();
            assert_eq!(
                reason.focus_return(visible).id(),
                expected,
                "{reason:?} {visible}"
            );
        }
    }

    fn destination(key: &str, label: &str, href: &str) -> NavDestination {
        NavDestination::new(key, label, IconName::Explore, href)
    }

    #[test]
    fn rail_renders_real_links_a_permanent_toggle_and_an_outside_tooltip() {
        let html = render(|| {
            view! {
                <NavigationRail
                    id="navigation"
                    label="Không gian Vink"
                    nav_label="Điều hướng chính"
                    expanded=false
                    expand_label="Mở rộng điều hướng"
                    collapse_label="Thu gọn điều hướng"
                    on_toggle=Callback::new(|_| ())
                    destinations=vec![
                        (destination("explore", "Khám phá", "/"), Signal::stored(true)),
                        (destination("desk", "Bàn viết", "/ban-viet"), Signal::stored(false)),
                    ]
                />
            }
            .into_any()
        });
        let explore = html.find("Khám phá").unwrap();
        assert!(explore < html.find("Bàn viết").unwrap(), "{html}");
        for needle in [
            "href=\"/\"",
            "href=\"/ban-viet\"",
            "aria-current=\"page\"",
            "id=\"navigation-toggle\"",
            "aria-expanded=\"false\"",
            "aria-controls=\"navigation\"",
            "aria-label=\"Mở rộng điều hướng\"",
            "data-icon=\"menu\"",
            "id=\"navigation-primary\"",
            "id=\"navigation-tooltip\"",
        ] {
            assert!(html.contains(needle), "{needle} in {html}");
        }
        assert_eq!(html.matches("aria-current=\"page\"").count(), 1, "{html}");
        assert!(!html.contains("role=\"tab"), "{html}");
        // The tooltip is a sibling after </aside>, outside the scrolling rail.
        assert!(
            html.find("</aside>").unwrap() < html.find("role=\"tooltip\"").unwrap(),
            "{html}"
        );
    }

    #[test]
    fn scaffold_resolves_in_css_unless_fixed_and_keeps_the_skip_link() {
        let auto = render(|| {
            view! {
                <AdaptiveScaffold
                    main_id="noi-dung"
                    skip_label="Bỏ qua tới nội dung"
                    context=NavContext { compact_rail: true, hide_bar: true }
                >
                    <main id="noi-dung">"x"</main>
                </AdaptiveScaffold>
            }
            .into_any()
        });
        for needle in [
            "data-layout=\"auto\"",
            "data-compact-rail",
            "data-hide-bar",
            "href=\"#noi-dung\"",
        ] {
            assert!(auto.contains(needle), "{needle} in {auto}");
        }
        let fixed = render(|| {
            view! {
                <AdaptiveScaffold main_id="m" skip_label="s" fixed=NavLayout::Bar>
                    <main id="m">"x"</main>
                </AdaptiveScaffold>
            }
            .into_any()
        });
        assert!(fixed.contains("data-layout=\"bar\""), "{fixed}");
        assert!(!fixed.contains("data-hide-bar"), "{fixed}");
    }

    #[test]
    fn utility_is_a_dialog_trigger_not_a_route() {
        let html = render(|| {
            view! {
                <NavUtility
                    id="profile-trigger"
                    label="Tôi"
                    icon=IconName::Person
                    controls="profile-menu"
                    expanded=true
                    on_open=Callback::new(|_| ())
                />
            }
            .into_any()
        });
        for needle in [
            "<button",
            "type=\"button\"",
            "aria-haspopup=\"dialog\"",
            "aria-controls=\"profile-menu\"",
            "aria-expanded=\"true\"",
        ] {
            assert!(html.contains(needle), "{needle} in {html}");
        }
        assert!(!html.contains("href"), "{html}");
    }
}
