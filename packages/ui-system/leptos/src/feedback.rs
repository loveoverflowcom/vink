//! UIS-03 feedback components (`shared/states.md`): loading primitives, state panels, notices,
//! status, snackbar and dialog, plus the clock/visibility adapters that drive them. Components
//! render the caller's state and report intents; they never fetch, retry, navigate or decide.
//! Loops are CSS animations gated by `data-active` and the motion scope, so no child runs its own
//! timer; the only timers are the adapters' single scheduled wake-ups.

use leptos::prelude::*;

use crate::action::{Button, ButtonVariant, IconButton};
use crate::icons::IconName;
use crate::media::{Icon, IconSize};
use crate::state::{
    ActionPhase, Announce, Body, DialogKind, DialogRole, DismissPolicy, DismissReason,
    IndicatorSize, LoadingGate, LoadingPolicy, PanelContent, PanelKind, Problem, Progress,
    RecoveryAction, RecoveryIntent, RegionCopy, RegionNotice, RegionState, SkeletonShape,
    SnackbarMessage, Tone,
};
use crate::text::{Heading, HeadingLevel};

// Activity and clocks ---------------------------------------------------------------------------

/// Whether loops may run here (tab visible, app in the foreground, region on screen). Additive
/// like reduced motion: an inactive ancestor wins over an active descendant.
#[derive(Clone, Copy)]
pub struct MotionActivity(pub Signal<bool>);

/// Provides activity on the current owner, combined with any enclosing provider. Call it at a
/// host root (the app or preview component); inside a view use [`MotionActivityScope`].
pub fn provide_motion_activity(active: Signal<bool>) {
    let parent = use_context::<MotionActivity>();
    provide_context(MotionActivity(Signal::derive(move || {
        active.get() && parent.is_none_or(|p| p.0.get())
    })));
}

/// Activity for its children only. It renders them in a child owner, so the provider ends with
/// them instead of staying on an enclosing reactive scope that renders something else later.
#[component]
pub fn MotionActivityScope(
    #[prop(into)] active: Signal<bool>,
    children: Children,
) -> impl IntoView {
    let parent = motion_active();
    let combined = MotionActivity(Signal::derive(move || active.get() && parent.get()));
    view! { <leptos::context::Provider value=combined>{children()}</leptos::context::Provider> }
}

/// The current activity; active when no host provided one.
pub fn motion_active() -> Signal<bool> {
    use_context::<MotionActivity>().map_or(Signal::stored(true), |a| a.0)
}

fn active_attr() -> impl Fn() -> &'static str + Send + Sync + 'static {
    let active = motion_active();
    move || if active.get() { "true" } else { "false" }
}

/// Page Visibility adapter: `false` while the document is hidden (background tab, minimized
/// window). Always `true` outside the browser.
pub fn document_visibility() -> Signal<bool> {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::JsCast;
        use leptos::wasm_bindgen::closure::Closure;

        let hidden = || document().visibility_state() == web_sys::VisibilityState::Hidden;
        let visible = RwSignal::new(!hidden());
        let changed = Closure::<dyn Fn()>::new(move || visible.set(!hidden()));
        let _ = document()
            .add_event_listener_with_callback("visibilitychange", changed.as_ref().unchecked_ref());
        let listener = StoredValue::new_local(Some(changed));
        on_cleanup(move || {
            if let Some(changed) = listener.try_update_value(Option::take).flatten() {
                let _ = document().remove_event_listener_with_callback(
                    "visibilitychange",
                    changed.as_ref().unchecked_ref(),
                );
            }
        });
        visible.into()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Signal::stored(true)
    }
}

/// Time for the adapters, in milliseconds. `Manual` is a caller-driven clock (previews, tests);
/// `Browser` reads `performance.now()` and schedules one timeout per change.
#[derive(Clone, Copy)]
pub enum Clock {
    Browser,
    Manual(Signal<u64>),
}

impl Clock {
    /// Reads the time; a manual clock is tracked so its changes re-evaluate the caller.
    pub fn now(self) -> u64 {
        match self {
            Self::Manual(now) => now.get(),
            Self::Browser => browser_now(),
        }
    }
}

fn browser_now() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        window()
            .performance()
            .map_or(0, |p| p.now().max(0.0) as u64)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

/// One pending wake-up for a browser clock; replacing or dropping it cancels the old one.
#[derive(Clone, Copy)]
struct Wake {
    tick: RwSignal<u32>,
    #[cfg(target_arch = "wasm32")]
    handle: StoredValue<Option<leptos::prelude::TimeoutHandle>, LocalStorage>,
}

impl Wake {
    fn new() -> Self {
        let wake = Self {
            tick: RwSignal::new(0),
            #[cfg(target_arch = "wasm32")]
            handle: StoredValue::new_local(None),
        };
        on_cleanup(move || wake.cancel());
        wake
    }

    fn cancel(self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(handle) = self.handle.try_update_value(Option::take).flatten() {
            handle.clear();
        }
    }

    fn at(self, clock: Clock, now: u64, at: Option<u64>) {
        self.cancel();
        let (Clock::Browser, Some(at)) = (clock, at) else {
            return;
        };
        #[cfg(target_arch = "wasm32")]
        {
            let tick = self.tick;
            let delay = std::time::Duration::from_millis(at.saturating_sub(now));
            if let Ok(handle) = set_timeout_with_handle(move || tick.update(|n| *n += 1), delay) {
                self.handle.set_value(Some(handle));
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (now, at);
    }
}

/// Visible/slow signals of a [`LoadingGate`] following `loading`.
#[derive(Clone, Copy)]
pub struct GateView {
    pub visible: Signal<bool>,
    pub slow: Signal<bool>,
}

/// Runs a [`LoadingGate`] for `loading`: the indicator shows only after the delay, then stays
/// for the minimum, and `slow` turns on after the slow threshold. One wake-up is scheduled per
/// change (browser clock); a manual clock re-evaluates when it is advanced.
pub fn use_loading_gate(loading: Signal<bool>, clock: Clock) -> GateView {
    let gate = StoredValue::new(LoadingGate::new(LoadingPolicy::default()));
    let visible = RwSignal::new(false);
    let slow = RwSignal::new(false);
    let wake = Wake::new();
    Effect::new(move |previous: Option<bool>| {
        let loading = loading.get();
        wake.tick.track();
        let now = clock.now();
        gate.update_value(|g| match (previous, loading) {
            (Some(false) | None, true) => g.start(now),
            (Some(true), false) => g.finish(now),
            _ => {}
        });
        let g = gate.get_value();
        visible.set(g.visible(now));
        slow.set(g.slow(now));
        wake.at(clock, now, g.next_change(now));
        loading
    });
    GateView {
        visible: visible.into(),
        slow: slow.into(),
    }
}

/// Dismisses a snackbar without an action after `duration.snackbar`; one with an action waits
/// for the person.
pub fn use_snackbar_timeout(
    message: Signal<Option<SnackbarMessage>>,
    clock: Clock,
    on_dismiss: Callback<()>,
) {
    let wake = Wake::new();
    let due = StoredValue::new(None::<u64>);
    let ms = u64::from(crate::tokens::motion::DURATION_SNACKBAR.ms);
    Effect::new(move |previous: Option<Option<SnackbarMessage>>| {
        let current = message.get();
        wake.tick.track();
        let now = clock.now();
        if previous.as_ref() != Some(&current) {
            due.set_value(
                current
                    .as_ref()
                    .filter(|m| m.action.is_none())
                    .map(|_| now + ms),
            );
        }
        match due.get_value() {
            Some(at) if now >= at => {
                due.set_value(None);
                wake.cancel();
                on_dismiss.run(());
            }
            at => wake.at(clock, now, at),
        }
        current
    });
}

// Loading primitives ----------------------------------------------------------------------------

/// MicroArc: an action is pending. With `label` it is an indeterminate `progressbar`; without,
/// decorative (the button's note carries the status). Reduced motion or inactivity leaves a
/// static arc.
#[component]
pub fn LoadingIndicator(
    #[prop(optional, into)] label: Option<String>,
    #[prop(optional)] size: IndicatorSize,
) -> impl IntoView {
    let decorative = label.is_none();
    view! {
        <span
            class=match size {
                IndicatorSize::Small => "vk-ui-arc vk-ui-arc--sm",
                IndicatorSize::Medium => "vk-ui-arc",
            }
            role=(!decorative).then_some("progressbar")
            aria-label=label
            aria-hidden=decorative.then_some("true")
            data-active=active_attr()
        >
            <svg viewBox="0 0 24 24" focusable="false" aria-hidden="true">
                <circle class="vk-ui-arc__track" cx="12" cy="12" r="9"></circle>
                <circle class="vk-ui-arc__arc" cx="12" cy="12" r="9"></circle>
            </svg>
        </span>
    }
}

/// EdgeProgress: a region refreshes or a search is in flight. `Determinate` only with real
/// progress from the caller; the label is the accessible name and shows as text when the loop
/// cannot run.
#[component]
pub fn EdgeProgress(
    #[prop(optional, into)] progress: Signal<Progress>,
    #[prop(into)] label: String,
) -> impl IntoView {
    let value = move || progress.get().value();
    let name = label.clone();
    view! {
        <div
            class="vk-ui-edge"
            role="progressbar"
            aria-label=name
            aria-valuemin="0"
            aria-valuemax="100"
            aria-valuenow=move || value().map(|v| format!("{:.0}", v * 100.0))
            data-mode=move || if value().is_some() { "determinate" } else { "indeterminate" }
            data-active=active_attr()
        >
            <span class="vk-ui-edge__track" aria-hidden="true">
                <span
                    class="vk-ui-edge__bar"
                    style=move || value().map(|v| format!("inline-size: {:.1}%", v * 100.0))
                ></span>
            </span>
            <span class="vk-ui-edge__label" aria-hidden="true">{label}</span>
        </div>
    }
}

/// SurfacePulse: placeholder geometry in the reading order of the real content. Decorative;
/// the region around it names the loading state.
#[component]
pub fn Skeleton(#[prop(optional)] shape: Option<SkeletonShape>) -> impl IntoView {
    let shape = shape.unwrap_or(SkeletonShape::Card);
    let line = |class: &'static str| view! { <span class=class></span> };
    let body = match shape {
        SkeletonShape::Text { lines } => (0..lines.max(1))
            .map(|i| {
                line(if i + 1 == lines.max(1) {
                    "vk-ui-skeleton__line vk-ui-skeleton__line--short"
                } else {
                    "vk-ui-skeleton__line"
                })
            })
            .collect_view()
            .into_any(),
        SkeletonShape::Card => view! {
            <span class="vk-ui-skeleton__line vk-ui-skeleton__line--title"></span>
            <span class="vk-ui-skeleton__byline">
                <span class="vk-ui-skeleton__avatar"></span>
                <span class="vk-ui-skeleton__line vk-ui-skeleton__line--meta"></span>
            </span>
            <span class="vk-ui-skeleton__line"></span>
            <span class="vk-ui-skeleton__line vk-ui-skeleton__line--short"></span>
        }
        .into_any(),
        SkeletonShape::Byline => view! {
            <span class="vk-ui-skeleton__byline">
                <span class="vk-ui-skeleton__avatar"></span>
                <span class="vk-ui-skeleton__stack">
                    <span class="vk-ui-skeleton__line vk-ui-skeleton__line--meta"></span>
                    <span class="vk-ui-skeleton__line vk-ui-skeleton__line--short"></span>
                </span>
            </span>
        }
        .into_any(),
    };
    view! {
        <div
            class=format!("vk-ui-skeleton vk-ui-skeleton--{}", shape.id())
            aria-hidden="true"
            data-active=active_attr()
        >
            {body}
        </div>
    }
}

// Panels and notices ----------------------------------------------------------------------------

fn recover(on_recover: Option<Callback<RecoveryIntent>>, intent: RecoveryIntent) {
    if let Some(on_recover) = on_recover {
        on_recover.run(intent);
    }
}

/// Symbol → heading → explanation → primary recovery → secondary actions → code.
#[component]
pub fn StatePanel(
    #[prop(into)] content: Signal<PanelContent>,
    #[prop(optional)] heading: Option<HeadingLevel>,
    #[prop(optional, into)] on_recover: Option<Callback<RecoveryIntent>>,
    /// Caller-owned navigation or stateful controls, after the typed recovery actions.
    #[prop(optional)]
    children: Option<ChildrenFn>,
) -> impl IntoView {
    let heading = heading.unwrap_or(HeadingLevel::H2);
    move || {
        let c = content.get();
        let kind = match c.kind {
            PanelKind::Error(k) => format!("error:{}", k.id()),
            PanelKind::Cancelled => "cancelled".into(),
            PanelKind::Empty(e) => format!("empty:{}", e.id()),
        };
        let tone = c.tone().id();
        let icon = c.icon();
        let has_actions = !c.actions.is_empty() || children.is_some();
        let custom_actions = children.as_ref().map(|children| children());
        let actions = c
            .actions
            .clone()
            .into_iter()
            .enumerate()
            .map(|(i, action)| {
                let intent = action.intent;
                let variant = if i == 0 {
                    ButtonVariant::Filled
                } else {
                    ButtonVariant::Text
                };
                view! {
                    <span data-intent=intent.id()>
                        <Button variant=variant on_press=move |_| recover(on_recover, intent)>
                            {action.label}
                        </Button>
                    </span>
                }
            })
            .collect_view();
        view! {
            <div class="vk-ui-panel" data-tone=tone data-panel=kind>
                <span class="vk-ui-panel__symbol">
                    <Icon name=icon />
                </span>
                <Heading level=heading>{c.title}</Heading>
                <p class="vk-ui-panel__message">{c.message}</p>
                {has_actions.then(|| view! { <div class="vk-ui-panel__actions">{actions}{custom_actions}</div> })}
                {c.code.map(|code| view! {
                    <p class="vk-ui-panel__code">"Mã: "<code>{code.to_string()}</code></p>
                })}
            </div>
        }
    }
}

fn live_role(announce: Announce) -> Option<&'static str> {
    match announce {
        Announce::Off => None,
        Announce::Polite => Some("status"),
        Announce::Assertive => Some("alert"),
    }
}

#[component]
fn Notice(
    class: &'static str,
    tone: Tone,
    message: String,
    title: Option<String>,
    action: Option<RecoveryAction>,
    announce: Announce,
    on_recover: Option<Callback<RecoveryIntent>>,
    on_dismiss: Option<Callback<()>>,
) -> impl IntoView {
    view! {
        <div class=class data-tone=tone.id() role=live_role(announce)>
            <span class="vk-ui-notice__icon">
                <Icon name=tone.icon() />
            </span>
            <div class="vk-ui-notice__body">
                {title.map(|t| view! { <p class="vk-ui-notice__title">{t}</p> })}
                <p class="vk-ui-notice__message">{message}</p>
            </div>
            {action.map(|a| {
                let intent = a.intent;
                view! {
                    <span class="vk-ui-notice__action" data-intent=intent.id()>
                        <Button variant=ButtonVariant::Text on_press=move |_| recover(on_recover, intent)>
                            {a.label}
                        </Button>
                    </span>
                }
            })}
            {on_dismiss.map(|dismiss| view! {
                <IconButton icon=IconName::Close label="Đóng thông báo" on_press=move |_| dismiss.run(()) />
            })}
        </div>
    }
}

/// An inline notice beside the content it concerns: icon + text, never colour alone; the
/// message wraps instead of being cut.
#[component]
pub fn InlineNotice(
    #[prop(optional)] tone: Tone,
    #[prop(into)] message: String,
    #[prop(optional, into)] title: Option<String>,
    #[prop(optional)] action: Option<RecoveryAction>,
    #[prop(optional)] announce: Announce,
    #[prop(optional, into)] on_recover: Option<Callback<RecoveryIntent>>,
    #[prop(optional, into)] on_dismiss: Option<Callback<()>>,
) -> impl IntoView {
    view! {
        <Notice
            class="vk-ui-notice"
            tone=tone
            message=message
            title=title
            action=action
            announce=announce
            on_recover=on_recover
            on_dismiss=on_dismiss
        />
    }
}

/// A page- or region-wide notice at the top edge (for example a session that expired while
/// reading). It never covers controls.
#[component]
pub fn Banner(
    #[prop(optional)] tone: Tone,
    #[prop(into)] message: String,
    #[prop(optional, into)] title: Option<String>,
    #[prop(optional)] action: Option<RecoveryAction>,
    #[prop(optional)] announce: Announce,
    #[prop(optional, into)] on_recover: Option<Callback<RecoveryIntent>>,
    #[prop(optional, into)] on_dismiss: Option<Callback<()>>,
) -> impl IntoView {
    view! {
        <Notice
            class="vk-ui-notice vk-ui-notice--banner"
            tone=tone
            message=message
            title=title
            action=action
            announce=announce
            on_recover=on_recover
            on_dismiss=on_dismiss
        />
    }
}

/// The phase of one action as icon + text; a polite status, so a change is read once.
#[component]
pub fn StatusBadge(
    #[prop(into)] phase: Signal<ActionPhase>,
    #[prop(into)] label: Signal<String>,
) -> impl IntoView {
    let symbol = move || match phase.get() {
        ActionPhase::Pending => view! { <LoadingIndicator /> }.into_any(),
        ActionPhase::Idle => {
            view! { <Icon name=IconName::Check size=IconSize::Small /> }.into_any()
        }
        ActionPhase::Dirty => {
            view! { <Icon name=IconName::Edit size=IconSize::Small /> }.into_any()
        }
        ActionPhase::Cancelled => {
            view! { <Icon name=IconName::Close size=IconSize::Small /> }.into_any()
        }
        ActionPhase::Uncertain => {
            view! { <Icon name=IconName::Help size=IconSize::Small /> }.into_any()
        }
        other => view! { <Icon name=other.tone().icon() size=IconSize::Small /> }.into_any(),
    };
    view! {
        <span
            class="vk-ui-status"
            role="status"
            data-phase=move || phase.get().id()
            data-tone=move || phase.get().tone().id()
        >
            <span class="vk-ui-status__symbol">{symbol}</span>
            <span class="vk-ui-status__label">{move || label.get()}</span>
        </span>
    }
}

/// The end of a finite list: a quiet fact, not an action, not announced.
#[component]
pub fn ListEnd(#[prop(optional, into)] label: Option<String>) -> impl IntoView {
    view! {
        <p class="vk-ui-list-end">
            <span>{label.unwrap_or_else(|| "Đã hết danh sách".into())}</span>
        </p>
    }
}

/// One short message that needs no decision, with at most one action. Controlled: the caller
/// clears `message`; [`use_snackbar_timeout`] does so for messages without an action.
#[component]
pub fn Snackbar(
    #[prop(into)] message: Signal<Option<SnackbarMessage>>,
    #[prop(optional, into)] on_action: Option<Callback<()>>,
    #[prop(into)] on_dismiss: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="vk-ui-snackbar-host" role="status">
            {move || message.get().map(|m| {
                let dismiss = m.dismiss().id();
                view! {
                <div class="vk-ui-snackbar" data-dismiss=dismiss>
                    <p class="vk-ui-snackbar__text">{m.text}</p>
                    {m.action.map(|label| view! {
                        <button
                            type="button"
                            class="vk-ui-snackbar__action"
                            on:click=move |_| {
                                if let Some(on_action) = on_action {
                                    on_action.run(());
                                }
                            }
                        >
                            {label}
                        </button>
                    })}
                    <button
                        type="button"
                        class="vk-ui-snackbar__close"
                        aria-label="Đóng"
                        on:click=move |_| on_dismiss.run(())
                    >
                        <Icon name=IconName::Close size=IconSize::Small />
                    </button>
                </div>
                }
            })}
        </div>
    }
}

// Dialog -----------------------------------------------------------------------------------------

/// A controlled modal dialog on the native `<dialog>` with `showModal()`: the rest of the page is
/// inert and focus stays inside. Opening focuses an `[autofocus]` element or the first action
/// (put the safe choice first for destructive confirmations); closing returns focus to the
/// element that had it. Escape and outside clicks report [`DismissReason`] only when the policy
/// allows them; the caller closes by setting `open` to false. One dialog at a time: the
/// app-wide overlay coordinator is UIS-04.
#[component]
pub fn Dialog(
    #[prop(into)] id: String,
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] title: Signal<String>,
    #[prop(optional, into)] context: Option<String>,
    #[prop(optional)] role: DialogRole,
    #[prop(optional)] focus_title: bool,
    #[prop(optional)] kind: DialogKind,
    #[prop(optional)] dismiss: DismissPolicy,
    #[prop(into)] on_dismiss: Callback<DismissReason>,
    #[prop(optional, into)] actions: ViewFn,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let node = NodeRef::<leptos::html::Dialog>::new();
    let title_id = format!("{id}-title");
    let context_id = context.as_ref().map(|_| format!("{id}-context"));
    let (title_target, context_target) = (title_id.clone(), context_id.clone());
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::JsCast;
        let returned = StoredValue::new_local(None::<leptos::web_sys::HtmlElement>);
        Effect::new(move |_| {
            let want = open.get();
            let Some(el) = node.get() else { return };
            if want && !el.open() {
                returned.set_value(
                    document()
                        .active_element()
                        .and_then(|e| e.dyn_into::<leptos::web_sys::HtmlElement>().ok()),
                );
                if el.show_modal().is_ok() {
                    focus_initial(&el);
                }
            } else if !want && el.open() {
                el.close();
                if let Some(target) = returned.try_update_value(Option::take).flatten()
                    && target.is_connected()
                {
                    let options = leptos::web_sys::FocusOptions::new();
                    options.set_prevent_scroll(true);
                    let _ = target.focus_with_options(&options);
                }
            }
        });
    }
    let cancel = move |ev: leptos::ev::Event| {
        // Escape: never let the browser close a controlled dialog by itself.
        ev.prevent_default();
        if dismiss.allows(DismissReason::Escape) {
            on_dismiss.run(DismissReason::Escape);
        }
    };
    let backdrop = move |ev: leptos::ev::MouseEvent| {
        // A click whose target is the <dialog> itself landed on the backdrop, outside the panel.
        let on_backdrop = ev
            .target()
            .zip(node.get_untracked())
            .is_some_and(|(target, el)| {
                use leptos::wasm_bindgen::JsCast;
                target.dyn_ref::<leptos::web_sys::Element>() == Some(el.as_ref())
            });
        if on_backdrop && dismiss.allows(DismissReason::Outside) {
            on_dismiss.run(DismissReason::Outside);
        }
    };
    let closed = move |_| {
        // `close` is queued. An earlier close can arrive after this dialog has reopened;
        // only a dialog which is closed now may dismiss the caller's current intent.
        if node.get_untracked().is_some_and(|el| el.open()) {
            return;
        }
        // Chrome may close a dialog after a repeated, un-activated Escape even though `cancel` was
        // prevented. Keep a required decision open; otherwise report it as an Escape.
        if !open.get_untracked() {
            return;
        }
        if dismiss.allows(DismissReason::Escape) {
            on_dismiss.run(DismissReason::Escape);
        } else if let Some(el) = node.get_untracked()
            && el.show_modal().is_ok()
        {
            #[cfg(target_arch = "wasm32")]
            focus_initial(&el);
        }
    };
    view! {
        <dialog
            node_ref=node
            id=id
            class=match kind {
                DialogKind::Dialog => "vk-ui-dialog",
                DialogKind::Sheet => "vk-ui-dialog vk-ui-dialog--sheet",
            }
            role=match role {
                DialogRole::Dialog => None,
                DialogRole::AlertDialog => Some("alertdialog"),
            }
            aria-modal="true"
            aria-labelledby=title_id.clone()
            aria-describedby=context_id.clone()
            data-dismiss=match dismiss {
                DismissPolicy::Explicit => "explicit",
                DismissPolicy::EscapeOnly => "escape",
                DismissPolicy::EscapeAndOutside => "escape-outside",
            }
            on:cancel=cancel
            on:click=backdrop
            on:close=closed
        >
            <div class="vk-ui-dialog__panel">
                <h2 class="vk-ui-dialog__title" id=title_target tabindex="-1" autofocus=focus_title>{move || title.get()}</h2>
                {context.map(|text| view! { <p class="vk-ui-dialog__context" id=context_target>{text}</p> })}
                {children.map(|children| view! { <div class="vk-ui-dialog__content">{children()}</div> })}
                <div class="vk-ui-dialog__actions">{actions.run()}</div>
            </div>
        </dialog>
    }
}

#[cfg(target_arch = "wasm32")]
fn focus_initial(el: &leptos::web_sys::HtmlDialogElement) {
    use leptos::wasm_bindgen::JsCast;
    let target = el
        .query_selector("[autofocus]")
        .ok()
        .flatten()
        .or_else(|| {
            el.query_selector(".vk-ui-dialog__actions button")
                .ok()
                .flatten()
        })
        .or_else(|| el.query_selector("button").ok().flatten());
    if let Some(target) = target.and_then(|t| t.dyn_into::<leptos::web_sys::HtmlElement>().ok()) {
        let _ = target.focus();
    }
}

// Region ----------------------------------------------------------------------------------------

/// A region whose content loads: EdgeProgress? → notice? → body (skeleton | panel | content).
/// Content stays mounted while it refreshes or a refresh fails, so focus, scroll and selection
/// survive; one polite status names the state and changes only when its text does. An unlawful
/// state renders a visible diagnostic instead of guessing.
#[component]
pub fn ContentRegion(
    #[prop(into)] state: Signal<RegionState>,
    #[prop(into)] label: String,
    /// The caller's projection for an error panel or a refresh notice of the same kind.
    #[prop(optional, into)]
    problem: Signal<Option<Problem>>,
    /// Copy and actions for an empty panel.
    #[prop(optional, into)]
    empty: Signal<Option<PanelContent>>,
    #[prop(optional)] copy: Option<RegionCopy>,
    #[prop(optional)] skeleton: Option<SkeletonShape>,
    /// From [`use_loading_gate`]; until it is true the skeleton keeps its space but stays hidden.
    #[prop(optional, into)]
    indicator_visible: Option<Signal<bool>>,
    #[prop(optional)] heading: Option<HeadingLevel>,
    #[prop(optional, into)] on_recover: Option<Callback<RecoveryIntent>>,
    children: ChildrenFn,
) -> impl IntoView {
    let on_recover = on_recover.unwrap_or_else(|| Callback::new(|_| {}));
    let copy = StoredValue::new(copy.unwrap_or_default());
    let shown = indicator_visible.unwrap_or(Signal::stored(true));
    let layout = Memo::new(move |_| state.get().layout());
    let body = Memo::new(move |_| layout.get().map(|l| l.body));
    let problem_for = move |kind| {
        problem
            .get()
            .filter(|p| p.kind == kind)
            .unwrap_or_else(|| Problem::fallback(kind))
    };
    let panel = move || -> Option<PanelContent> {
        match body.get() {
            Ok(Body::Panel(PanelKind::Error(kind))) => {
                Some(PanelContent::from_problem(&problem_for(kind)))
            }
            Ok(Body::Panel(PanelKind::Empty(kind))) => Some(empty.get().unwrap_or_else(|| {
                let (title, message) = match kind {
                    crate::state::EmptyKind::FirstUse => {
                        ("Chưa có gì ở đây", "Nội dung sẽ xuất hiện khi có.")
                    }
                    crate::state::EmptyKind::NoResults => {
                        ("Không có kết quả phù hợp", "Thử từ khóa hoặc bộ lọc khác.")
                    }
                };
                PanelContent::empty(kind, title, message, Vec::new()).expect("no actions")
            })),
            Ok(Body::Panel(PanelKind::Cancelled)) => {
                let c = copy.get_value();
                Some(PanelContent::cancelled(
                    c.cancelled_title,
                    c.cancelled_message,
                    Some(c.reload),
                ))
            }
            _ => None,
        }
    };
    let notice = move || {
        let notice = layout.get().ok()?.notice?;
        let c = copy.get_value();
        let retry = |p: &Problem| {
            p.actions
                .iter()
                .find(|a| a.intent == RecoveryIntent::Retry)
                .cloned()
        };
        Some(match notice {
            RegionNotice::Slow => (Tone::Info, None, c.slow, None),
            RegionNotice::LowContent => (Tone::Info, None, c.low_content, None),
            RegionNotice::Partial => {
                let action = problem.get().as_ref().and_then(retry);
                (Tone::Warning, None, c.partial, action)
            }
            RegionNotice::Refresh(kind) => {
                let p = problem_for(kind);
                let action = retry(&p);
                (kind.tone(), Some(p.title), p.message, action)
            }
        })
    };
    let announcement = Memo::new(move |_| match layout.get() {
        Err(_) => String::new(),
        Ok(l) => {
            if let Some((_, title, message, _)) = notice() {
                title.unwrap_or(message)
            } else if let Some(p) = panel() {
                p.title
            } else if l.body == Body::Skeleton && shown.get() || l.progress {
                copy.get_value().loading
            } else {
                String::new()
            }
        }
    });
    let loading = move || {
        layout
            .get()
            .is_ok_and(|l| l.progress || l.body == Body::Skeleton)
    };
    let shape = skeleton.unwrap_or(SkeletonShape::Card);
    view! {
        <section
            class="vk-ui-region"
            aria-label=label
            aria-busy=move || loading().then_some("true")
            data-layout=move || match layout.get() {
                Ok(l) => l.id(),
                Err(u) => format!("!{}", u.id()),
            }
        >
            <p class="vk-ui-visually-hidden" role="status">{move || announcement.get()}</p>
            {move || layout.get().is_ok_and(|l| l.progress).then(|| {
                let c = copy.get_value();
                view! { <div class="vk-ui-region__progress"><EdgeProgress label=c.loading /></div> }
            })}
            {move || notice().map(|(tone, title, message, action)| view! {
                <Notice
                    class="vk-ui-notice"
                    tone=tone
                    title=title
                    message=message
                    action=action
                    announce=Announce::Off
                    on_recover=Some(on_recover)
                    on_dismiss=None
                />
            })}
            {move || match body.get() {
                Err(reason) => view! {
                    <p class="vk-ui-region__unlawful" data-unlawful=reason.id()>
                        {format!("Trạng thái không hợp lệ: {}", reason.id())}
                    </p>
                }
                .into_any(),
                Ok(Body::Skeleton) => view! {
                    <div class="vk-ui-region__skeleton" data-shown=move || if shown.get() { "true" } else { "false" }>
                        <Skeleton shape=shape />
                    </div>
                }
                .into_any(),
                Ok(Body::Panel(_)) => view! {
                    <StatePanel content=Signal::derive(move || panel().expect("panel body")) heading=heading.unwrap_or(HeadingLevel::H3) on_recover=on_recover />
                }
                .into_any(),
                Ok(Body::Content) => ().into_any(),
            }}
            <Show when=move || body.get() == Ok(Body::Content)>
                <div class="vk-ui-region__content">{children()}</div>
            </Show>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Content, EmptyKind, Phase, ProblemKind};
    use crate::tests::render;

    fn region(state: RegionState) -> String {
        render(move || {
            view! {
                <ContentRegion state=state label="Tác phẩm [Mẫu]">
                    <p class="probe">"Nội dung thật"</p>
                </ContentRegion>
            }
            .into_any()
        })
    }

    #[test]
    fn regions_render_their_layout_and_keep_content_while_refreshing() {
        let html = region(RegionState::new(Content::Absent, Phase::LOADING));
        assert!(
            html.contains("vk-ui-skeleton") && html.contains("aria-busy=\"true\""),
            "{html}"
        );
        assert!(!html.contains("probe"), "{html}");

        let html = region(RegionState::new(Content::PRESENT, Phase::LOADING));
        for needle in [
            "probe",
            "role=\"progressbar\"",
            "aria-busy=\"true\"",
            "data-layout=\"content|+|-\"",
        ] {
            assert!(html.contains(needle), "{needle} in {html}");
        }
        assert!(!html.contains("aria-valuenow"), "indeterminate: {html}");

        let html = region(RegionState::new(
            Content::PRESENT,
            Phase::Failed(ProblemKind::Network),
        ));
        assert!(
            html.contains("probe") && html.contains("data-tone=\"error\""),
            "{html}"
        );
        assert!(html.contains("Chưa kết nối được"), "fallback copy: {html}");

        let html = region(RegionState::new(
            Content::Empty(EmptyKind::NoResults),
            Phase::Idle,
        ));
        assert!(
            html.contains("data-panel=\"empty:no-results\"")
                && !html.contains("data-tone=\"error\""),
            "{html}"
        );

        let html = region(RegionState::new(Content::Absent, Phase::Cancelled));
        assert!(
            html.contains("data-panel=\"cancelled\"") && html.contains("data-tone=\"neutral\""),
            "{html}"
        );
        assert!(html.contains("data-intent=\"retry\""), "{html}");

        let html = region(RegionState::new(
            Content::PRESENT,
            Phase::Failed(ProblemKind::Forbidden),
        ));
        assert!(
            html.contains("data-unlawful=\"revoked\"") && !html.contains("probe"),
            "{html}"
        );
    }

    #[test]
    fn panels_offer_only_the_callers_lawful_actions_and_a_safe_code() {
        let problem = Problem::fallback(ProblemKind::Uncertain)
            .with_code("draft.create.uncertain")
            .with_action(RecoveryIntent::Reconcile, "Kiểm tra lại")
            .unwrap();
        let html = render(move || {
            let content = PanelContent::from_problem(&problem);
            view! { <StatePanel content=content /> }.into_any()
        });
        for needle in [
            "data-panel=\"error:uncertain\"",
            "data-tone=\"warning\"",
            "data-intent=\"reconcile\"",
            "Mã: <code>draft.create.uncertain</code>",
            "<h2",
        ] {
            assert!(html.contains(needle), "{needle} in {html}");
        }
        assert!(!html.contains("data-intent=\"retry\""), "{html}");
    }

    #[test]
    fn panel_action_slot_keeps_real_navigation_and_typed_recovery_in_the_panel() {
        let html = render(|| {
            let content = PanelContent::from_problem(
                &Problem::fallback(ProblemKind::Unavailable)
                    .with_action(RecoveryIntent::Retry, "Thử lại")
                    .unwrap(),
            );
            view! {
                <StatePanel content=content>
                    <crate::Link href="/ban-viet">"Về Bàn viết"</crate::Link>
                </StatePanel>
            }
            .into_any()
        });
        let actions = html.split("vk-ui-panel__actions").nth(1).unwrap();
        assert!(actions.contains("data-intent=\"retry\""), "{html}");
        assert!(actions.contains("href=\"/ban-viet\""), "{html}");
        assert!(actions.contains("Về Bàn viết"), "{html}");
        assert_eq!(html.matches("href=\"/ban-viet\"").count(), 1, "{html}");
    }

    #[test]
    fn notices_announce_only_as_asked() {
        let polite =
            render(|| view! { <InlineNotice tone=Tone::Info message="Đã lưu" /> }.into_any());
        assert!(polite.contains("role=\"status\""), "{polite}");
        let assertive = render(|| {
            view! { <Banner tone=Tone::Warning message="Phiên đã hết hạn" announce=Announce::Assertive /> }.into_any()
        });
        assert!(assertive.contains("role=\"alert\""), "{assertive}");
        let off = render(|| {
            view! { <InlineNotice tone=Tone::Error message="x" announce=Announce::Off /> }
                .into_any()
        });
        assert!(!off.contains("role="), "{off}");
    }

    #[test]
    fn indicators_never_invent_progress_and_dialogs_start_closed() {
        let html = render(|| view! { <EdgeProgress label="Đang làm mới" /> }.into_any());
        assert!(
            html.contains("data-mode=\"indeterminate\"") && !html.contains("aria-valuenow"),
            "{html}"
        );
        let html = render(|| {
            view! { <EdgeProgress label="Đang tải lên" progress=Progress::Determinate(0.42) /> }
                .into_any()
        });
        assert!(html.contains("aria-valuenow=\"42\""), "{html}");
        let html = render(|| view! { <LoadingIndicator /> }.into_any());
        assert!(
            html.contains("aria-hidden=\"true\"") && !html.contains("progressbar"),
            "{html}"
        );
        let html = render(|| {
            view! {
                <Dialog id="d" open=false title="Xóa bản nháp?" role=DialogRole::AlertDialog on_dismiss=|_| {}>
                    "Nội dung"
                </Dialog>
            }
            .into_any()
        });
        assert!(
            html.contains("role=\"alertdialog\"") && !html.contains(" open"),
            "{html}"
        );
        assert!(html.contains("aria-labelledby=\"d-title\""), "{html}");
    }
}
