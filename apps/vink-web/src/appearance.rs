//! Device-local appearance preferences and the Web account badge projection.

use leptos::prelude::*;

pub(crate) const THEME_KEY: &str = "vink.ui.theme";
pub(crate) const RAIL_KEY: &str = "vink.ui.rail";
const TEXT_KEY: &str = "vink.ui.text";

// Runs before CSS; only mutates the document root outside the hydrated app. Hydration adopts
// all three preferences synchronously from that root. Storage denial keeps defaults intact.
// Head links are outside the hydrated body. Observe the existing theme authority so explicit
// choices, system changes and storage-denied sessions share one icon/manifest selection.
pub(crate) const BOOTSTRAP: &str = r#"try{for(const [key,attr,allowed] of [['vink.ui.theme','data-theme',['light','dark']],['vink.ui.rail','data-rail',['expanded','compact']],['vink.ui.text','data-text',['200']]]){const v=localStorage.getItem(key);if(allowed.includes(v))document.documentElement.setAttribute(attr,v)}}catch{}
(()=>{const root=document.documentElement,media=matchMedia('(prefers-color-scheme: dark)');const sync=()=>{const theme=root.dataset.theme||(media.matches?'dark':'light');document.getElementById('vink-favicon').setAttribute('href','/pkg/brand/vpen-r1-mark-'+theme+'.svg');document.getElementById('vink-manifest').setAttribute('href','/pkg/brand/vpen-r1-manifest-'+theme+'.webmanifest')};sync();new MutationObserver(sync).observe(root,{attributes:true,attributeFilter:['data-theme']});media.addEventListener('change',sync)})()"#;

// The head bootstrap cannot see SSR controls yet. Run directly after each group's markup,
// before the parser yields to paint, and change properties only (not the hydrated DOM shape).
// Read the root authority rather than storage again; hydration adopts this same preference.
const CHOICE_BOOTSTRAP: &str = r#"(()=>{const group=document.currentScript.previousElementSibling,value=document.documentElement.dataset[group.parentElement.dataset.preference];if(value)for(const input of group.querySelectorAll('input[type=radio]'))input.checked=input.value===value})()"#;

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
        #[cfg(feature = "hydrate")]
        let (theme, rail, large_text) = {
            let root = document().document_element();
            let preference = |name| root.as_ref().and_then(|root| root.get_attribute(name));
            (
                Theme::parse(preference("data-theme").as_deref().unwrap_or_default()),
                Rail::parse(preference("data-rail").as_deref().unwrap_or_default()),
                preference("data-text").as_deref() == Some("200"),
            )
        };
        #[cfg(not(feature = "hydrate"))]
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
        apply(TEXT_KEY, "data-text", large.then_some("200"));
    }
}

fn apply(key: &str, attribute: &str, value: Option<&str>) {
    #[cfg(feature = "hydrate")]
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
    #[cfg(not(feature = "hydrate"))]
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
    view! {
        <span class=if navigation { "vk-ui-icon vk-nav-account-avatar" } else { "vk-account-avatar" } aria-hidden="true">
            {move || {
                let (name, key) = match standing.get() {
                    crate::session::Standing::Active(profile) => (profile.display_name, profile.principal_id),
                    _ => Default::default(),
                };
                view! { <vink_ui::Avatar name identity_key=key kind=vink_ui::AvatarKind::Account decorative=true fallback_text="—" /> }
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
                options=&[("100", "100%"), ("200", "200%")]
                value=Signal::derive(move || Some(if appearance.large_text.get() { "200" } else { "100" }.to_owned()))
                on_change=Callback::new(move |value: String| appearance.set_large_text(value == "200")) />
            <AppearanceRow prefix preference="rail" label="Thanh điều hướng" icon="rail"
                options=&[("auto", "Tự động"), ("expanded", "Mở rộng"), ("compact", "Thu gọn")]
                value=Signal::derive(move || Some(appearance.rail.get().map_or("auto", Rail::value).to_owned()))
                on_change=Callback::new(move |value: String| appearance.set_rail(Rail::parse(&value))) />
        </div>
    }
}

// RadioGroup still owns the native inputs and checked state. The visible heading and glyph
// let the same row lay out horizontally on Tôi and stack inside the narrower panel.
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
                    <p class="vk-appearance-rail-hint">"Áp dụng khi có thanh bên."</p>
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
