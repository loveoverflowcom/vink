//! Device-local appearance preferences. No account, manuscript or server authority lives here.

use leptos::prelude::*;
use unicode_normalization::UnicodeNormalization;

pub(crate) const THEME_KEY: &str = "vink.ui.theme";
pub(crate) const RAIL_KEY: &str = "vink.ui.rail";

// Runs before CSS; only mutates the document root outside the hydrated app. The browser effect
// reads these same keys after hydration. Storage denial leaves system/context defaults intact.
pub(crate) const BOOTSTRAP: &str = r#"try{for(const [key,attr,allowed] of [['vink.ui.theme','data-theme',['light','dark']],['vink.ui.rail','data-rail',['expanded','compact']]]){const v=localStorage.getItem(key);if(allowed.includes(v))document.documentElement.setAttribute(attr,v)}}catch{}"#;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

impl Theme {
    #[cfg(any(feature = "hydrate", test))]
    pub fn parse(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rail {
    Expanded,
    Compact,
}

impl Rail {
    #[cfg(any(feature = "hydrate", test))]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "expanded" => Some(Self::Expanded),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::Compact => "compact",
        }
    }
}

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

/// First and last word, Vietnamese uppercase, preserving a composed first letter. Callers
/// without a real display name get a neutral mark; fixture names never establish identity.
pub fn initials(display_name: Option<&str>) -> String {
    let name: String = display_name.unwrap_or_default().nfc().collect();
    let mut words = name.split_whitespace();
    let Some(first) = words.next() else {
        return "•".into();
    };
    let mut letters: String = first.chars().take(1).collect();
    if let Some(last) = words.next_back() {
        letters.extend(last.chars().take(1));
    }
    letters.to_uppercase()
}

#[component]
pub(crate) fn ThemePicker(group: &'static str) -> impl IntoView {
    let appearance = expect_context::<Appearance>();
    view! {
        <fieldset class="vk-appearance">
            <legend>"Giao diện"</legend>
            <div class="vk-theme-choices">
                {[(Theme::Light, "Sáng"), (Theme::Dark, "Tối"), (Theme::System, "Hệ thống")]
                    .into_iter()
                    .map(|(theme, label)| view! {
                        <label class="vk-theme-choice">
                            <input
                                type="radio"
                                name=group
                                value=theme.value()
                                prop:checked=move || appearance.theme.get() == theme
                                on:change=move |_| appearance.set_theme(theme)
                            />
                            <span>{label}</span>
                        </label>
                    })
                    .collect_view()}
            </div>
        </fieldset>
    }
}

#[component]
pub(crate) fn AccountStatus() -> impl IntoView {
    view! {
        <div class="vk-account-status">
            <span class="vk-avatar" aria-hidden="true">{initials(None)}</span>
            <div>
                <p>"Chưa có thông tin tài khoản"</p>
                <p class="vk-muted">"Vink chưa mở đăng nhập."</p>
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
