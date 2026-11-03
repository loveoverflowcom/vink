//! Device-local appearance preferences. No account, manuscript or server authority lives here.

use leptos::prelude::*;

pub(crate) const THEME_KEY: &str = "vink.ui.theme";
pub(crate) const RAIL_KEY: &str = "vink.ui.rail";

// Runs before CSS; only mutates the document root outside the hydrated app. The browser effect
// reads these same keys after hydration. Storage denial leaves system/context defaults intact.
// Head links are outside the hydrated body. Observe the existing theme authority so explicit
// choices, system changes and storage-denied sessions share one icon/manifest selection.
pub(crate) const BOOTSTRAP: &str = r#"try{for(const [key,attr,allowed] of [['vink.ui.theme','data-theme',['light','dark']],['vink.ui.rail','data-rail',['expanded','compact']]]){const v=localStorage.getItem(key);if(allowed.includes(v))document.documentElement.setAttribute(attr,v)}}catch{}
(()=>{const root=document.documentElement,media=matchMedia('(prefers-color-scheme: dark)');const sync=()=>{const theme=root.dataset.theme||(media.matches?'dark':'light');document.getElementById('vink-favicon').setAttribute('href','/pkg/brand/vink-mark-'+theme+'-compact.svg');document.getElementById('vink-manifest').setAttribute('href','/pkg/brand/manifest-'+theme+'.webmanifest')};sync();new MutationObserver(sync).observe(root,{attributes:true,attributeFilter:['data-theme']});media.addEventListener('change',sync)})()"#;

/// Theme preference values and parsing are owned by the UI system; this module keeps the
/// storage key, the pre-CSS bootstrap and when to persist.
pub(crate) use vink_ui::ThemePreference as Theme;

/// Rail preference values are owned by the UI system too; the key and bootstrap stay here.
pub(crate) use vink_ui::RailPreference as Rail;

#[derive(Clone, Copy)]
pub(crate) struct Appearance {
    pub theme: RwSignal<Theme>,
    pub rail: RwSignal<Option<Rail>>,
}

impl Appearance {
    pub fn provide() -> Self {
        let state = Self {
            theme: RwSignal::new(Theme::System),
            rail: RwSignal::new(None),
        };
        provide_context(state);
        #[cfg(feature = "hydrate")]
        Effect::new(move |_| {
            state.theme.set(Theme::parse(&read(THEME_KEY)));
            state.rail.set(Rail::parse(&read(RAIL_KEY)));
        });
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

    pub fn set_rail(self, rail: Rail) {
        self.rail.set(Some(rail));
        apply(RAIL_KEY, "data-rail", Some(rail.value()));
    }
}

#[cfg(feature = "hydrate")]
fn read(key: &str) -> String {
    window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(key).ok().flatten())
        .unwrap_or_default()
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
pub(crate) fn ThemePicker(group: &'static str) -> impl IntoView {
    let appearance = expect_context::<Appearance>();
    view! {
        <vink_ui::RadioGroup id=group legend="Giao diện"
            options=vec![vink_ui::ChoiceOption::new("light", "Sáng"), vink_ui::ChoiceOption::new("dark", "Tối"), vink_ui::ChoiceOption::new("system", "Hệ thống")]
            value=Signal::derive(move || Some(appearance.theme.get().value().to_string()))
            on_change=move |value: String| appearance.set_theme(Theme::parse(&value)) />
    }
}

#[component]
pub(crate) fn AccountStatus() -> impl IntoView {
    view! {
        <div class="vk-account-status">
            <vink_ui::Avatar kind=vink_ui::AvatarKind::Account decorative=true />
            <div>
                <vink_ui::Text role=vink_ui::TypeRole::Label>"Chưa có thông tin tài khoản"</vink_ui::Text>
                <vink_ui::Text tone=vink_ui::TextTone::Muted>"Vink chưa mở đăng nhập."</vink_ui::Text>
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
