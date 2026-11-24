//! Actions: Button, IconButton, Link, FilterChip. One native target each, one dispatch per
//! activation, none while disabled or pending. Fetching, retrying and permission stay with the
//! caller; a pending Button shows the UIS-03 `LoadingIndicator` in its reserved status slot.

use leptos::prelude::*;

use crate::feedback::LoadingIndicator;
use crate::icons::IconName;
use crate::media::{Icon, IconSize};

/// Whether an action can run. Disabled and pending stay focusable (`aria-disabled`) so the
/// reason or status remains readable; activation is ignored and default actions are prevented.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ActionState {
    #[default]
    Enabled,
    Disabled {
        reason: Option<String>,
    },
    Pending {
        status: String,
    },
}

impl ActionState {
    pub fn disabled(reason: impl Into<String>) -> Self {
        Self::Disabled {
            reason: Some(reason.into()),
        }
    }

    pub fn pending(status: impl Into<String>) -> Self {
        Self::Pending {
            status: status.into(),
        }
    }

    pub const fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub const fn id(&self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled { .. } => "disabled",
            Self::Pending { .. } => "pending",
        }
    }

    /// The visible text explaining a disabled or pending action.
    pub fn note(&self) -> Option<String> {
        match self {
            Self::Enabled => None,
            Self::Disabled { reason } => reason.clone().filter(|r| !r.trim().is_empty()),
            Self::Pending { status } => Some(status.clone()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonVariant {
    #[default]
    Filled,
    Tonal,
    Outlined,
    Text,
    Danger,
    DangerText,
}

impl ButtonVariant {
    pub const ALL: [ButtonVariant; 6] = [
        Self::Filled,
        Self::Tonal,
        Self::Outlined,
        Self::Text,
        Self::Danger,
        Self::DangerText,
    ];

    /// Shared geometry and variant hook for a Button or a native link styled as one.
    /// The element still owns its behavior and accessible name.
    pub const fn classes(self) -> &'static str {
        match self {
            Self::Filled => {
                "vk-ui-button vk-ui-button--filled relative box-border inline-flex items-center gap-vk-2"
            }
            Self::Tonal => {
                "vk-ui-button vk-ui-button--tonal relative box-border inline-flex items-center gap-vk-2"
            }
            Self::Outlined => {
                "vk-ui-button vk-ui-button--outlined relative box-border inline-flex items-center gap-vk-2"
            }
            Self::Text => {
                "vk-ui-button vk-ui-button--text relative box-border inline-flex items-center gap-vk-2"
            }
            Self::Danger => {
                "vk-ui-button vk-ui-button--danger relative box-border inline-flex items-center gap-vk-2"
            }
            Self::DangerText => {
                "vk-ui-button vk-ui-button--danger-text relative box-border inline-flex items-center gap-vk-2"
            }
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Filled => "filled",
            Self::Tonal => "tonal",
            Self::Outlined => "outlined",
            Self::Text => "text",
            Self::Danger => "danger",
            Self::DangerText => "danger-text",
        }
    }
}

/// Opt-in container for icon and confirmed-toggle actions. Presentation never implies selected.
/// Standard preserves the existing transparent idle surface; Tonal uses a neutral container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ActionSurface {
    #[default]
    Standard,
    Tonal,
}

impl ActionSurface {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Tonal => "tonal",
        }
    }
}

/// Submitting is explicit: the default never submits an enclosing form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonKind {
    #[default]
    Button,
    Submit,
}

/// The note under an action, linked by `aria-describedby` when the caller gave an `id`.
fn note_view(state: Signal<ActionState>, note_id: Option<String>) -> impl IntoView {
    move || {
        state.get().note().map(|text| {
            view! { <span class="vk-ui-action__note" id=note_id.clone()>{text}</span> }
        })
    }
}

fn press_handler(
    state: Signal<ActionState>,
    on_press: Option<Callback<()>>,
) -> impl Fn(leptos::ev::MouseEvent) + 'static {
    move |ev| {
        if !state.with_untracked(ActionState::is_enabled) {
            // Also cancels an implicit form submission through a Submit button.
            ev.prevent_default();
            return;
        }
        if let Some(on_press) = on_press {
            on_press.run(());
        }
    }
}

#[component]
pub fn Button(
    #[prop(optional, into)] variant: Signal<ButtonVariant>,
    #[prop(optional, into)] state: Signal<ActionState>,
    #[prop(optional)] kind: ButtonKind,
    #[prop(optional)] leading: Option<IconName>,
    /// Needed to link a disabled reason or pending status (`{id}-note`).
    #[prop(optional, into)]
    id: Option<String>,
    /// Supporting text belongs to the native target; pending/disabled notes are appended.
    #[prop(optional, into)]
    described_by: Option<String>,
    #[prop(optional, into)] on_press: Option<Callback<()>>,
    /// Disclosure state belongs on the focusable button, not its action-note wrapper.
    #[prop(optional, into)]
    expanded: Option<Signal<bool>>,
    /// Overrides the visible label when the target needs context (it must start with it).
    #[prop(optional, into)]
    accessible_label: Option<Signal<String>>,
    /// `aria-haspopup` for a button that opens an overlay, e.g. `"dialog"`.
    #[prop(optional)]
    has_popup: Option<&'static str>,
    #[prop(optional)] button_ref: NodeRef<leptos::html::Button>,
    children: Children,
) -> impl IntoView {
    let note_id = id.as_ref().map(|id| format!("{id}-note"));
    let described = note_id.clone();
    view! {
        <span class="vk-ui-action">
            <button
                node_ref=button_ref
                type=match kind {
                    ButtonKind::Button => "button",
                    ButtonKind::Submit => "submit",
                }
                id=id
                class=move || variant.get().classes()
                data-state=move || state.with(ActionState::id)
                aria-disabled=move || (!state.with(ActionState::is_enabled)).then_some("true")
                aria-busy=move || state.with(|s| matches!(s, ActionState::Pending { .. })).then_some("true")
                aria-expanded=move || expanded.map(|value| value.get().to_string())
                aria-haspopup=has_popup
                aria-label=move || accessible_label.map(|label| label.get())
                aria-describedby=move || {
                    let mut ids = described_by.clone().into_iter().collect::<Vec<_>>();
                    if let Some(note) = state.with(ActionState::note).and(described.clone()) { ids.push(note); }
                    (!ids.is_empty()).then(|| ids.join(" "))
                }
                on:click=press_handler(state, on_press)
            >
                {leading.map(|name| view! { <Icon name=name size=IconSize::Small /> })}
                <span class="vk-ui-button__label">{children()}</span>
                <span class="vk-ui-button__status" data-slot="status" aria-hidden="true">
                    {move || {
                        state
                            .with(|s| matches!(s, ActionState::Pending { .. }))
                            .then(|| view! { <LoadingIndicator /> })
                    }}
                </span>
            </button>
            {note_view(state, note_id)}
        </span>
    }
}

/// An icon-only action. `label` is its accessible name; `pressed` makes it a toggle.
#[component]
pub fn IconButton(
    icon: IconName,
    #[prop(into)] label: String,
    #[prop(optional, into)] surface: Signal<ActionSurface>,
    #[prop(optional, into)] state: Signal<ActionState>,
    #[prop(optional, into)] pressed: Option<Signal<bool>>,
    /// Optional alternate icon for the pressed state of a toggle.
    #[prop(optional)]
    pressed_icon: Option<IconName>,
    #[prop(optional, into)] id: Option<String>,
    /// Optional supporting information; the accessible name remains `label`.
    #[prop(optional, into)]
    described_by: Option<String>,
    /// Disclosure state (`aria-expanded`) when the button opens a region or dialog.
    #[prop(optional, into)]
    expanded: Option<Signal<bool>>,
    #[prop(optional, into)] on_press: Option<Callback<()>>,
) -> impl IntoView {
    let note_id = id.as_ref().map(|id| format!("{id}-note"));
    let described = note_id.clone();
    view! {
        <span class="vk-ui-action">
            <button
                type="button"
                id=id
                class="vk-ui-icon-button"
                data-surface=move || surface.get().id()
                aria-label=label
                aria-expanded=move || expanded.map(|value| value.get().to_string())
                aria-pressed=move || pressed.map(|p| if p.get() { "true" } else { "false" })
                data-state=move || state.with(ActionState::id)
                aria-disabled=move || (!state.with(ActionState::is_enabled)).then_some("true")
                aria-busy=move || state.with(|s| matches!(s, ActionState::Pending { .. })).then_some("true")
                aria-describedby=move || {
                    let mut ids = described_by.clone().into_iter().collect::<Vec<_>>();
                    if let Some(note) = state.with(ActionState::note).and(described.clone()) { ids.push(note); }
                    (!ids.is_empty()).then(|| ids.join(" "))
                }
                on:click=press_handler(state, on_press)
            >
                {move || {
                    let name = match pressed_icon {
                        Some(alternate) if pressed.is_some_and(|p| p.get()) => alternate,
                        _ => icon,
                    };
                    view! { <Icon name /> }
                }}
            </button>
            {note_view(state, note_id)}
        </span>
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LinkTarget {
    #[default]
    SameTab,
    /// Opens a new tab with `noopener noreferrer` and says so in text and icon.
    NewTab,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LinkVariant {
    /// Underlined inside running text.
    #[default]
    Inline,
    /// A standalone navigation link with the control height.
    Standalone,
}

/// `href` when it is a relative URL or http(s)/mailto; anything else (`javascript:`, `data:`,
/// protocol-relative `//host`, embedded whitespace or control characters) is refused.
pub fn safe_href(href: &str) -> Option<&str> {
    let href = href.trim();
    if href.is_empty() || href.starts_with("//") || href.chars().any(char::is_whitespace) {
        return None;
    }
    if href.chars().any(char::is_control) {
        return None;
    }
    let end = href.find(['/', '?', '#']).unwrap_or(href.len());
    match href[..end].split_once(':') {
        None => Some(href),
        Some((scheme, _)) => ["http", "https", "mailto"]
            .iter()
            .any(|s| s.eq_ignore_ascii_case(scheme))
            .then_some(href),
    }
}

/// A real `<a href>`: the browser keeps middle/modifier clicks, context menus and history. A
/// refused `href` renders an anchor without href (`data-invalid-href`), so it cannot navigate.
#[component]
pub fn Link(
    #[prop(into)] href: Signal<String>,
    #[prop(optional)] target: LinkTarget,
    #[prop(optional)] variant: LinkVariant,
    children: Children,
) -> impl IntoView {
    let class = match variant {
        LinkVariant::Inline => "vk-ui-link",
        LinkVariant::Standalone => "vk-ui-link vk-ui-link--standalone",
    };
    let target_href = Signal::derive(move || href.with(|h| safe_href(h).map(str::to_string)));
    let new_tab = target == LinkTarget::NewTab;
    view! {
        <a
            class=class
            href=move || target_href.get()
            data-invalid-href=move || target_href.with(|h| h.is_none()).then_some("true")
            target=new_tab.then_some("_blank")
            rel=new_tab.then_some("noopener noreferrer")
        >
            {children()}
            {new_tab.then(|| view! {
                <Icon name=IconName::OpenInNew size=IconSize::Small />
                <span class="vk-ui-visually-hidden">" (mở tab mới)"</span>
            })}
        </a>
    }
    .into_any()
}

/// A toggle chip: `aria-pressed`, a check mark when selected, never colour alone. A disabled
/// reason is shown under it and linked when the caller gives an `id`.
#[component]
pub fn FilterChip(
    #[prop(into)] label: String,
    #[prop(into)] selected: Signal<bool>,
    #[prop(optional, into)] state: Signal<ActionState>,
    #[prop(optional, into)] id: Option<String>,
    #[prop(into)] on_toggle: Callback<bool>,
) -> impl IntoView {
    let note_id = id.as_ref().map(|id| format!("{id}-note"));
    let described = note_id.clone();
    view! {
        <span class="vk-ui-action">
            <button
                type="button"
                id=id
                class="vk-ui-chip"
                aria-pressed=move || if selected.get() { "true" } else { "false" }
                data-state=move || state.with(ActionState::id)
                aria-disabled=move || (!state.with(ActionState::is_enabled)).then_some("true")
                aria-describedby=move || state.with(ActionState::note).and(described.clone())
                on:click=move |ev| {
                    if !state.with_untracked(ActionState::is_enabled) {
                        ev.prevent_default();
                        return;
                    }
                    on_toggle.run(!selected.get_untracked());
                }
            >
                <span class="vk-ui-chip__check">
                    {move || selected.get().then(|| view! { <Icon name=IconName::Check size=IconSize::Small /> })}
                </span>
                <span>{label}</span>
            </button>
            {note_view(state, note_id)}
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::render;

    #[test]
    fn links_keep_safe_targets_and_refuse_script_urls() {
        for ok in [
            "/tac-pham/1",
            "tac-pham/1",
            "#mo-dau",
            "?q=thu",
            "https://vink.vn/a",
            "HTTP://x.y",
            "mailto:a@b.vn",
        ] {
            assert_eq!(safe_href(ok), Some(ok), "{ok}");
        }
        for bad in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "java\tscript:alert(1)",
            "data:text/html,x",
            "//evil.example",
            "vbscript:x",
            "",
        ] {
            assert_eq!(safe_href(bad), None, "{bad:?}");
        }
        let html = render(|| view! { <Link href="javascript:alert(1)">"x"</Link> }.into_any());
        assert!(
            !html.contains(" href=") && html.contains("data-invalid-href"),
            "{html}"
        );
        let html = render(|| {
            view! { <Link href="https://a.vn" target=LinkTarget::NewTab>"Nguồn"</Link> }.into_any()
        });
        for needle in [
            "target=\"_blank\"",
            "rel=\"noopener noreferrer\"",
            "(mở tab mới)",
        ] {
            assert!(html.contains(needle), "{needle} in {html}");
        }
    }

    #[test]
    fn button_recipe_preserves_native_targets_and_all_variant_hooks() {
        for variant in ButtonVariant::ALL {
            let button = render(move || view! { <Button variant>"Mở"</Button> }.into_any());
            let anchor = render(move || {
                view! { <Link href="/toi" attr:class=variant.classes()>"Mở"</Link> }.into_any()
            });
            let hook = format!("vk-ui-button--{}", variant.id());
            assert!(
                button.contains("<button") && button.contains("type=\"button\""),
                "{button}"
            );
            assert!(
                anchor.starts_with("<a ") && anchor.contains("href=\"/toi\""),
                "{anchor}"
            );
            for token in [
                hook.as_str(),
                "relative",
                "box-border",
                "inline-flex",
                "items-center",
                "gap-vk-2",
            ] {
                assert!(button.contains(token), "{token}: {button}");
                assert!(anchor.contains(token), "{token}: {anchor}");
            }
        }
    }

    #[test]
    fn button_supporting_description_and_state_note_belong_to_the_native_target() {
        let html = render(|| {
            view! {
                <Button id="save" described_by="save-tooltip"
                    state=ActionState::pending("Đang lưu…")>"Lưu"</Button>
            }
            .into_any()
        });
        let button = html
            .split("<button")
            .nth(1)
            .expect("native target")
            .split('>')
            .next()
            .unwrap();
        assert!(
            button.contains("aria-describedby=\"save-tooltip save-note\""),
            "{html}"
        );
        assert!(html.contains("id=\"save-note\""), "{html}");
    }

    #[test]
    fn buttons_never_submit_by_default_and_link_their_reason() {
        let html = render(|| view! { <Button>"Lưu"</Button> }.into_any());
        assert!(html.contains("type=\"button\""), "{html}");
        assert!(!html.contains("aria-disabled"), "{html}");
        assert!(!html.contains("aria-expanded"), "{html}");
        let html = render(|| view! { <Button expanded=false>"Xem tiêu đề"</Button> }.into_any());
        let button = html
            .split("<button")
            .nth(1)
            .unwrap()
            .split('>')
            .next()
            .unwrap();
        assert!(button.contains("aria-expanded=\"false\""), "{html}");
        let html = render(|| {
            view! {
                <Button id="save" state=ActionState::disabled("Chưa có thay đổi")>"Lưu"</Button>
            }
            .into_any()
        });
        for needle in [
            "aria-disabled=\"true\"",
            "aria-describedby=\"save-note\"",
            "id=\"save-note\"",
            "Chưa có thay đổi",
            "data-state=\"disabled\"",
        ] {
            assert!(html.contains(needle), "{needle} in {html}");
        }
        let html = render(|| {
            view! {
                <Button id="send" kind=ButtonKind::Submit state=ActionState::pending("Đang lưu…")>
                    "Lưu"
                </Button>
            }
            .into_any()
        });
        for needle in [
            "type=\"submit\"",
            "aria-busy=\"true\"",
            ">Lưu<",
            "data-slot=\"status\"",
            "vk-ui-arc",
        ] {
            assert!(html.contains(needle), "{needle} in {html}");
        }
        let blank = ActionState::Disabled {
            reason: Some("  ".into()),
        };
        assert_eq!(blank.note(), None);
    }

    #[test]
    fn icon_buttons_and_chips_expose_their_state() {
        let html = render(|| {
            view! {
                <IconButton icon=IconName::Edit label="Sửa tiêu đề" pressed=Signal::stored(true) />
            }
            .into_any()
        });
        assert!(html.contains("aria-label=\"Sửa tiêu đề\""), "{html}");
        assert!(html.contains("aria-pressed=\"true\""), "{html}");
        assert!(
            html.contains("aria-hidden=\"true\""),
            "icon is decorative: {html}"
        );
        assert!(
            !html.contains("aria-expanded"),
            "no disclosure unless asked: {html}"
        );
        let html = render(|| {
            view! {
                <IconButton icon=IconName::Search label="Tìm tác phẩm" expanded=Signal::stored(false) />
            }
            .into_any()
        });
        assert!(html.contains("aria-expanded=\"false\""), "{html}");
        let html = render(|| {
            view! { <FilterChip label="Thơ" selected=Signal::stored(true) on_toggle=|_| {} /> }
                .into_any()
        });
        assert!(
            html.contains("aria-pressed=\"true\"") && html.contains("<svg"),
            "{html}"
        );
    }
    #[test]
    fn surfaces_are_independent_of_selection_and_state() {
        assert_eq!(ActionSurface::default(), ActionSurface::Standard);
        for surface in [ActionSurface::Standard, ActionSurface::Tonal] {
            for pressed in [None, Some(false), Some(true)] {
                let html = render(move || {
                    let mut props = IconButtonProps::builder()
                        .icon(IconName::Edit)
                        .label("Sửa tiêu đề".to_string())
                        .surface(surface)
                        .state(ActionState::pending("Đang lưu…"))
                        .id("surface".to_string())
                        .build();
                    props.pressed = pressed.map(Signal::stored);
                    IconButton(props).into_any()
                });
                let target = html
                    .split("<button")
                    .nth(1)
                    .unwrap()
                    .split('>')
                    .next()
                    .unwrap();
                assert!(
                    target.contains(&format!("data-surface=\"{}\"", surface.id())),
                    "{html}"
                );
                assert!(
                    target.contains("type=\"button\"")
                        && target.contains("aria-disabled=\"true\"")
                        && target.contains("aria-busy=\"true\""),
                    "{html}"
                );
                assert_eq!(
                    target.contains("aria-pressed="),
                    pressed.is_some(),
                    "{html}"
                );
                if let Some(pressed) = pressed {
                    assert!(
                        target.contains(&format!("aria-pressed=\"{pressed}\"")),
                        "{html}"
                    );
                }
                assert_eq!(html.matches("<button").count(), 1, "{html}");
            }
        }
    }
}
