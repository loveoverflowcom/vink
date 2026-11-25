//! Device-local appearance preferences and the Web account badge projection.

use leptos::prelude::*;

pub(crate) const THEME_KEY: &str = "vink.ui.theme";
pub(crate) const RAIL_KEY: &str = "vink.ui.rail";
const TEXT_KEY: &str = "vink.ui.text";
/// Stored and root (`data-text`) value of Lớn; Mặc định is the absent value (#413).
const LARGE_TEXT: &str = "150";

// Runs before CSS; only mutates the document root outside the hydrated app. Hydration adopts
// all three preferences synchronously from that root. Storage denial keeps defaults intact.
// Head links are outside the hydrated body. Observe the existing theme authority so explicit
// choices, system changes and storage-denied sessions share one icon/manifest selection.
// Text size: `150` is the canonical Lớn value (#413). A pre-#413 `200` means the same choice, so
// it is normalized here, before CSS and first paint, and rewritten when storage allows.
#[cfg(not(feature = "csr"))]
pub(crate) const BOOTSTRAP: &str = include_str!("../appearance.js");

// The head bootstrap cannot see SSR controls yet. Run directly after each group's markup,
// before the parser yields to paint, and change properties only (not the hydrated DOM shape).
// Read the root authority rather than storage again; hydration adopts this same preference.
const CHOICE_BOOTSTRAP: &str = r#"(()=>{const group=document.currentScript.previousElementSibling,value=document.documentElement.dataset[group.parentElement.dataset.preference];if(value)for(const input of group.querySelectorAll('input[type=radio]'))input.checked=input.value===value})()"#;

// This parser-time script updates only the toggle's attributes. The two icon shapes already
// exist in the SSR markup and CSS selects the visible one from aria-expanded.
pub(crate) const RAIL_BOOTSTRAP: &str = r#"(()=>{
    const toggle=document.getElementById('navigation-toggle');
    const desktop=matchMedia('(min-width: 1100px)').matches;
    const medium=!desktop&&matchMedia('(min-width: 760px)').matches;
    const preferred=document.documentElement.dataset.rail;
    const expanded=desktop&&(preferred==='expanded'||(!preferred&&!toggle.closest('#shell').hasAttribute('data-compact-rail')));
    toggle.setAttribute('aria-expanded',String(expanded));
    toggle.setAttribute('aria-label',expanded?toggle.dataset.collapseLabel:toggle.dataset.expandLabel);
    if(medium){
        toggle.setAttribute('aria-controls','navigation-modal');
        toggle.setAttribute('aria-haspopup','dialog');
    }
})()"#;

/// Theme preference values and parsing are owned by the UI system; this module keeps the
/// storage key, the pre-CSS bootstrap and when to persist.
pub(crate) use vink_ui::ThemePreference as Theme;

/// Rail preference values are owned by the UI system too; the key and bootstrap stay here.
pub(crate) use vink_ui::RailPreference as Rail;

#[derive(Clone, Copy)]
pub(crate) struct Appearance {
    pub theme: RwSignal<Theme>,
    pub rail: RwSignal<Option<Rail>>,
    large_text: RwSignal<bool>,
}

impl Appearance {
    pub fn provide() -> Self {
        #[cfg(feature = "browser")]
        let (theme, rail, large_text) = {
            let root = document().document_element();
            let preference = |name| root.as_ref().and_then(|root| root.get_attribute(name));
            (
                Theme::parse(preference("data-theme").as_deref().unwrap_or_default()),
                Rail::parse(preference("data-rail").as_deref().unwrap_or_default()),
                preference("data-text").as_deref() == Some(LARGE_TEXT),
            )
        };
        #[cfg(not(feature = "browser"))]
        let (theme, rail, large_text) = (Theme::System, None, false);
        let state = Self {
            theme: RwSignal::new(theme),
            rail: RwSignal::new(rail),
            large_text: RwSignal::new(large_text),
        };
        provide_context(state);
        state
    }

    pub fn set_theme(self, theme: Theme) {
        self.theme.set(theme);
        apply(
            THEME_KEY,
            "data-theme",
            (theme != Theme::System).then_some(theme.value()),
        );
    }

    pub fn set_rail(self, rail: Option<Rail>) {
        self.rail.set(rail);
        apply(RAIL_KEY, "data-rail", rail.map(Rail::value));
    }

    fn set_large_text(self, large: bool) {
        self.large_text.set(large);
        apply(TEXT_KEY, "data-text", large.then_some(LARGE_TEXT));
    }
}

fn apply(key: &str, attribute: &str, value: Option<&str>) {
    #[cfg(feature = "browser")]
    {
        if let Some(root) = document().document_element() {
            if let Some(value) = value {
                let _ = root.set_attribute(attribute, value);
            } else {
                let _ = root.remove_attribute(attribute);
            }
        }
        if let Ok(Some(storage)) = window().local_storage() {
            let _ = match value {
                Some(value) => storage.set_item(key, value),
                None => storage.remove_item(key),
            };
        }
    }
    #[cfg(not(feature = "browser"))]
    let _ = (key, attribute, value);
}

/// Delegates to the UI system's `avatar-identity-v1` rule (first and last word, NFC, Vietnamese
/// uppercase). Callers without a real display name get the app's neutral mark; fixture names
/// never establish identity.
pub fn initials(display_name: Option<&str>) -> String {
    vink_ui::identity::initials(display_name).unwrap_or_else(|| "•".into())
}

#[component]
pub(crate) fn AccountAvatar(#[prop(optional)] navigation: bool) -> impl IntoView {
    // Account data stays out of SSR and browser storage; the tab's session owner supplies it.
    let standing = crate::account::Session::current().standing;
    let size = if navigation {
        vink_ui::AvatarSize::Small
    } else {
        vink_ui::AvatarSize::Medium
    };
    view! {
        <span class=if navigation { "vk-ui-icon vk-nav-account-avatar" } else { "vk-account-avatar" } aria-hidden="true">
            {move || {
                let (name, key) = match standing.get() {
                    crate::session::Standing::Active(profile) => (profile.display_name, profile.principal_id),
                    _ => Default::default(),
                };
                view! { <vink_ui::Avatar name identity_key=key size=size kind=vink_ui::AvatarKind::Account decorative=true fallback_text="—" /> }
            }}
        </span>
    }
}

#[component]
pub(crate) fn AppearanceTitle(id: &'static str) -> impl IntoView {
    view! {
        <h2 id=id class="vk-appearance-title">
            <span class="vk-appearance-mark" aria-hidden="true"><span data-appearance-icon="palette"></span></span>
            "Giao diện"
        </h2>
    }
}

#[component]
pub(crate) fn AppearanceChoices(prefix: &'static str) -> impl IntoView {
    let appearance = expect_context::<Appearance>();
    view! {
        <div class="vk-appearance-region">
            <AppearanceRow prefix preference="theme" label="Chế độ màu" icon="system"
                options=&[("system", "Hệ thống"), ("light", "Sáng"), ("dark", "Tối")]
                value=Signal::derive(move || Some(appearance.theme.get().value().to_owned()))
                on_change=Callback::new(move |value: String| appearance.set_theme(Theme::parse(&value))) />
            <AppearanceRow prefix preference="text" label="Cỡ chữ" icon="100"
                options=&[("100", "Mặc định"), (LARGE_TEXT, "Lớn")]
                value=Signal::derive(move || Some(if appearance.large_text.get() { LARGE_TEXT } else { "100" }.to_owned()))
                on_change=Callback::new(move |value: String| appearance.set_large_text(value == LARGE_TEXT)) />
            <AppearanceRow prefix preference="rail" label="Thanh điều hướng" icon="rail"
                options=&[("auto", "Tự động"), ("expanded", "Mở rộng"), ("compact", "Thu gọn")]
                value=Signal::derive(move || Some(appearance.rail.get().map_or("auto", Rail::value).to_owned()))
                on_change=Callback::new(move |value: String| appearance.set_rail(Rail::parse(&value))) />
        </div>
    }
}

// RadioGroup owns the accessible group name, native inputs and checked state.
// The visible heading and glyph identify the group on Tôi, the only appearance surface (#413).
#[component]
fn AppearanceRow(
    prefix: &'static str,
    preference: &'static str,
    label: &'static str,
    icon: &'static str,
    options: &'static [(&'static str, &'static str)],
    value: Signal<Option<String>>,
    on_change: Callback<String>,
) -> impl IntoView {
    let id = format!("{prefix}-{preference}");
    let options = options
        .iter()
        .map(|&(value, label)| vink_ui::ChoiceOption::new(value, label))
        .collect();
    view! {
        <div class="vk-appearance-row">
            <h3 id=format!("{id}-title")><span data-appearance-icon=icon aria-hidden="true"></span>{label}</h3>
            <div class="vk-appearance-region__choices" data-preference=preference>
                <vink_ui::RadioGroup id legend=label options value on_change />
                <script inner_html=CHOICE_BOOTSTRAP></script>
                {(preference == "rail").then(|| view! {
                    <p class="vk-appearance-rail-hint">"Trên màn hình vừa, thanh bên mở rộng khi bạn bấm nút điều hướng."</p>
                })}
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vietnamese_initials_use_first_and_last_words() {
        for (name, expected) in [
            (Some("An Nhiên"), "AN"),
            (Some("  đặng   thị Hồng  "), "ĐH"),
            (Some("Nguyễn"), "N"),
            (Some("a\u{0301}nh thu"), "ÁT"),
            (Some("\n\t"), "•"),
            (Some("🌸🌸"), "•"),
            (None, "•"),
        ] {
            assert_eq!(initials(name), expected);
        }
    }

    #[test]
    fn unknown_preferences_fall_back_to_system_and_context() {
        assert_eq!(Theme::parse("broken"), Theme::System);
        assert_eq!(Theme::parse("dark"), Theme::Dark);
        assert_eq!(Rail::parse("220"), None);
        assert_eq!(Rail::parse("compact"), Some(Rail::Compact));
    }
}
