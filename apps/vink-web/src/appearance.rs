//! Device-local appearance preferences and the Web account badge projection.

use leptos::prelude::*;

use crate::contract::{ApiError, CurrentAccount};

pub(crate) const THEME_KEY: &str = "vink.ui.theme";
pub(crate) const RAIL_KEY: &str = "vink.ui.rail";
const TEXT_KEY: &str = "vink.ui.text";

// Runs before CSS; only mutates the document root outside the hydrated app. Hydration adopts
// theme and text scale synchronously; the rail effect reads its key. Storage denial keeps defaults intact.
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
        let theme = document()
            .document_element()
            .and_then(|root| root.get_attribute("data-theme"))
            .map(|value| Theme::parse(&value))
            .unwrap_or(Theme::System);
        #[cfg(not(feature = "hydrate"))]
        let theme = Theme::System;
        #[cfg(feature = "hydrate")]
        let large_text = document()
            .document_element()
            .and_then(|root| root.get_attribute("data-text"))
            .as_deref()
            == Some("200");
        #[cfg(not(feature = "hydrate"))]
        let large_text = false;
        let state = Self {
            theme: RwSignal::new(theme),
            rail: RwSignal::new(None),
            large_text: RwSignal::new(large_text),
        };
        provide_context(state);
        #[cfg(feature = "hydrate")]
        Effect::new(move |_| {
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

    pub fn set_rail(self, rail: Option<Rail>) {
        self.rail.set(rail);
        apply(RAIL_KEY, "data-rail", rail.map(Rail::value));
    }

    fn set_large_text(self, large: bool) {
        self.large_text.set(large);
        apply(TEXT_KEY, "data-text", large.then_some("200"));
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

type AccountState = RwSignal<Option<Result<CurrentAccount, ApiError>>>;

pub(crate) fn provide_account() {
    let account = AccountState::new(None);
    provide_context(account);
    #[cfg(feature = "hydrate")]
    {
        // Account data stays out of SSR and browser storage. Revalidate on focus in case the
        // session changed in another tab, clearing the old badge while that is being decided.
        let revision = RwSignal::new(0_u64);
        let listener = window_event_listener(leptos::ev::focus, move |_| {
            revision.update(|revision| *revision += 1);
        });
        on_cleanup(move || listener.remove());
        Effect::new(move |_| {
            let started = revision.get();
            account.set(None);
            leptos::task::spawn_local(async move {
                let result = crate::api::current_account().await;
                if revision.try_get_untracked() == Some(started) {
                    account.set(Some(result));
                }
            });
        });
    }
}

#[component]
pub(crate) fn AccountAvatar(#[prop(optional)] navigation: bool) -> impl IntoView {
    let account = expect_context::<AccountState>();
    view! {
        <span class=if navigation { "vk-ui-icon vk-nav-account-avatar" } else { "vk-account-avatar" } aria-hidden="true">
            {move || {
                let profile = account.get().and_then(Result::ok);
                let (name, key) = profile.map(|profile| (profile.display_name, profile.principal_id)).unwrap_or_default();
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

#[component]
pub(crate) fn AccountStatus(id: &'static str) -> impl IntoView {
    let account = expect_context::<AccountState>();
    view! {
        <section id=id class="vk-account" aria-label="Tài khoản">
            <div class="vk-account-status">
                <AccountAvatar />
                <div>
                    <vink_ui::Heading level=vink_ui::HeadingLevel::H2 role=vink_ui::TypeRole::Title>"Tài khoản"</vink_ui::Heading>
                    <vink_ui::Text tone=vink_ui::TextTone::Muted>
                        {move || match account.get() {
                            Some(Ok(profile)) => profile.display_name,
                            Some(Err(ApiError::Unauthenticated)) => "Chưa có tài khoản khả dụng.".into(),
                            Some(Err(_)) => "Chưa thể tải thông tin tài khoản.".into(),
                            None => "Chưa có thông tin tài khoản.".into(),
                        }}
                    </vink_ui::Text>
                </div>
                <svg class="vk-account-illustration" viewBox="0 0 224 82" aria-hidden="true" focusable="false">
                    <path fill="var(--vk-primary-container)" opacity=".72" d="M9 42c1-8 8-14 16-15 7-11 24-12 31 0 9 0 15 5 15 12 0 5-4 9-11 9H19c-6 0-11-2-10-6ZM155 34c1-6 6-9 12-9 5-10 19-10 24-2 8 0 14 5 14 10 0 6-5 9-11 9h-30c-6 0-10-3-9-8ZM175 69c2-7 9-12 16-11 5-8 17-9 23-2 6 1 10 5 10 10s-5 10-10 10h-33c-5 0-7-3-6-7Z" />
                    <path fill="var(--vk-primary)" opacity=".22" d="M26 75l32-43c23-3 39 2 51 15 16-11 33-12 51-3l-23 31H26Z" />
                    <path fill="none" stroke="var(--vk-primary-container)" stroke-width="3" stroke-linecap="round" d="M86 75l23-28m-79 27c19-6 37-5 56 1 17-5 32-5 49 0" />
                    <path fill="var(--vk-primary)" d="M119 53c4-18 15-34 34-45 4-2 12-5 14-3 2 2-2 9-4 12-11 17-24 29-44 36Z" />
                    <path fill="none" stroke="var(--vk-primary-container)" stroke-width="1.6" stroke-linecap="round" d="M119 52c13-17 25-30 41-42" />
                    <path fill="none" stroke="var(--vk-primary)" stroke-width="2.2" stroke-linecap="round" d="m121 50-8 11" />
                </svg>
            </div>
        </section>
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
