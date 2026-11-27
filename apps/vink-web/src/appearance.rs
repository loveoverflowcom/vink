//! Device-local appearance preferences and the Web account badge projection.

use leptos::prelude::*;

use crate::account::Session;

/// Stored and root (\`data-text\`) value of Lớn. Mặc định is the absent value.
const LARGE_TEXT: &str = "150";

// Runs before CSS in both builds. The document uses system defaults until the tab confirms an
// active account. Observe the root so member choices and OS changes also update the head links.
#[cfg(not(feature = "csr"))]
pub(crate) const BOOTSTRAP: &str = include_str!("../appearance.js");

// This parser-time script updates only the toggle's attributes. The two icon shapes already
// exist in the SSR markup and CSS selects the visible one from aria-expanded.
pub(crate) const RAIL_BOOTSTRAP: &str = r#"(()=>{
    const toggle=document.getElementById('navigation-toggle');
    const desktop=matchMedia('(min-width: 1100px)').matches;
    const medium=!desktop&&matchMedia('(min-width: 760px)').matches;
    const expanded=desktop&&!toggle.closest('#shell').hasAttribute('data-compact-rail');
    toggle.setAttribute('aria-expanded',String(expanded));
    toggle.setAttribute('aria-label',expanded?toggle.dataset.collapseLabel:toggle.dataset.expandLabel);
    if(medium){
        toggle.setAttribute('aria-controls','navigation-modal');
        toggle.setAttribute('aria-haspopup','dialog');
    }
})()"#;

pub(crate) use vink_ui::{RailPreference as Rail, ThemePreference as Theme};

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
struct Preferences {
    theme: Theme,
    rail: Option<Rail>,
    large_text: bool,
}

impl Preferences {
    #[cfg(any(test, feature = "browser"))]
    fn read(mut get: impl FnMut(&str) -> Option<String>) -> Self {
        Self {
            theme: Theme::parse(get("theme").as_deref().unwrap_or_default()),
            rail: Rail::parse(get("rail").as_deref().unwrap_or_default()),
            large_text: matches!(get("text").as_deref(), Some(LARGE_TEXT | "200")),
        }
    }
}

#[derive(Default)]
struct MemberPreferences {
    principal: Option<String>,
    values: Preferences,
}

/// Device-local keys are derived from an already verified opaque principal. The hash keeps
/// the raw principal out of storage; a key, even when present, never establishes authority.
#[cfg(any(test, feature = "browser"))]
fn member_key(principal: &str, preference: &str) -> String {
    use sha2::{Digest, Sha256};

    let digest: String = Sha256::digest(principal.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("vink.ui.member.{digest}.{preference}")
}

#[derive(Clone, Copy)]
pub(crate) struct Appearance {
    session: Session,
    stored: RwSignal<MemberPreferences>,
    preferences: Memo<Preferences>,
    navigation_rail: RwSignal<Option<Rail>>,
    pub rail: Memo<Option<Rail>>,
}

impl Appearance {
    pub fn provide(session: Session) -> Self {
        let stored = RwSignal::new(MemberPreferences::default());
        let preferences = Memo::new(move |_| {
            session.appearance_principal.with(|principal| {
                stored.with(|stored| {
                    if principal.is_some() && stored.principal == *principal {
                        stored.values
                    } else {
                        Preferences::default()
                    }
                })
            })
        });
        let navigation_rail = RwSignal::<Option<Rail>>::new(None);
        let rail = Memo::new(move |_| navigation_rail.get().or_else(|| preferences.get().rail));

        #[cfg(feature = "browser")]
        {
            Effect::new(move |_| {
                let principal = session.appearance_principal.get();
                // Never read the old device-wide keys: they cannot be attributed to a member.
                let values = principal
                    .as_deref()
                    .and_then(|principal| {
                        let storage = window().local_storage().ok().flatten()?;
                        Some(Preferences::read(|name| {
                            storage
                                .get_item(&member_key(principal, name))
                                .ok()
                                .flatten()
                        }))
                    })
                    .unwrap_or_default();
                navigation_rail.set(None);
                stored.set(MemberPreferences { principal, values });
            });
            Effect::new(move |_| {
                let values = preferences.get();
                let rail = rail.get();
                if let Some(root) = document().document_element() {
                    let theme = (values.theme != Theme::System).then_some(values.theme.value());
                    for (attribute, value) in [
                        ("data-theme", theme),
                        ("data-rail", rail.map(Rail::value)),
                        ("data-text", values.large_text.then_some(LARGE_TEXT)),
                    ] {
                        if let Some(value) = value {
                            let _ = root.set_attribute(attribute, value);
                        } else {
                            let _ = root.remove_attribute(attribute);
                        }
                    }
                }
            });
        }

        let state = Self {
            session,
            stored,
            preferences,
            navigation_rail,
            rail,
        };
        provide_context(state);
        state
    }

    fn set_theme(self, theme: Theme) {
        let value = (theme != Theme::System).then_some(theme.value());
        self.update("theme", value, |prefs| prefs.theme = theme);
    }

    fn set_rail(self, rail: Option<Rail>) {
        if self.update("rail", rail.map(Rail::value), |prefs| prefs.rail = rail) {
            self.navigation_rail.set(None);
        }
    }

    /// The desktop rail button is a navigation affordance. Only the choices in Tôi persist.
    pub fn set_navigation_rail(self, rail: Rail) {
        self.navigation_rail.set(Some(rail));
    }

    fn set_large_text(self, large: bool) {
        self.update("text", large.then_some(LARGE_TEXT), |prefs| {
            prefs.large_text = large
        });
    }

    fn update(
        self,
        name: &str,
        value: Option<&str>,
        change: impl FnOnce(&mut Preferences),
    ) -> bool {
        let Some(principal) = self.session.appearance_principal.get_untracked() else {
            return false;
        };
        if !self
            .stored
            .with_untracked(|stored| stored.principal.as_deref() == Some(&principal))
        {
            return false;
        }

        self.stored.update(|stored| change(&mut stored.values));
        #[cfg(feature = "browser")]
        if let Ok(Some(storage)) = window().local_storage() {
            let key = member_key(&principal, name);
            let _ = match value {
                Some(value) => storage.set_item(&key, value),
                None => storage.remove_item(&key),
            };
        }
        #[cfg(not(feature = "browser"))]
        let _ = (name, value);
        true
    }
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
        <h3 id=id class="vk-account-setting-row">
            <span aria-hidden="true"><span data-appearance-icon="palette"></span></span>
            "Giao diện"
        </h3>
    }
}

#[component]
pub(crate) fn AppearanceChoices(prefix: &'static str) -> impl IntoView {
    let appearance = expect_context::<Appearance>();
    view! {
        <div class="vk-appearance-region">
            <AppearanceRow prefix preference="theme" label="Chế độ màu"
                options=&[("system", "Hệ thống"), ("light", "Sáng"), ("dark", "Tối")]
                value=Signal::derive(move || Some(appearance.preferences.get().theme.value().to_owned()))
                on_change=Callback::new(move |value: String| appearance.set_theme(Theme::parse(&value))) />
            <AppearanceRow prefix preference="text" label="Cỡ chữ"
                options=&[("100", "Mặc định"), (LARGE_TEXT, "Lớn")]
                value=Signal::derive(move || Some(if appearance.preferences.get().large_text { LARGE_TEXT } else { "100" }.to_owned()))
                on_change=Callback::new(move |value: String| appearance.set_large_text(value == LARGE_TEXT)) />
            <AppearanceRow prefix preference="rail" label="Thanh điều hướng"
                help="Trên màn hình vừa, thanh bên mở rộng khi bạn bấm nút điều hướng."
                options=&[("auto", "Tự động"), ("expanded", "Mở rộng"), ("compact", "Thu gọn")]
                value=Signal::derive(move || Some(appearance.preferences.get().rail.map_or("auto", Rail::value).to_owned()))
                on_change=Callback::new(move |value: String| appearance.set_rail(Rail::parse(&value))) />
        </div>
    }
}

// RadioGroup owns the group name, description, native inputs and checked state.
#[component]
fn AppearanceRow(
    prefix: &'static str,
    preference: &'static str,
    label: &'static str,
    #[prop(optional, into)] help: Option<String>,
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
            <h4 id=format!("{id}-title")>{label}</h4>
            <vink_ui::RadioGroup id legend=label options value on_change nostrip:help=help />
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
    fn absent_or_invalid_member_preferences_use_system_defaults() {
        assert_eq!(Preferences::read(|_| None), Preferences::default());
        assert_eq!(
            Preferences::read(|_| Some("invalid".into())),
            Preferences::default()
        );
    }

    #[test]
    fn member_storage_isolated_from_other_members_and_unowned_legacy() {
        use std::collections::HashMap;

        let storage = HashMap::from([
            (member_key("account-a", "theme"), "dark"),
            (member_key("account-a", "rail"), "compact"),
            (member_key("account-a", "text"), "200"),
            ("vink.ui.text".into(), "200"),
        ]);
        let read = |id| {
            Preferences::read(|name| storage.get(&member_key(id, name)).map(|v| v.to_string()))
        };
        assert_eq!(
            read("account-a"),
            Preferences {
                theme: Theme::Dark,
                rail: Some(Rail::Compact),
                large_text: true,
            }
        );
        assert_eq!(read("account-b"), Preferences::default());
        assert!(!member_key("account-a", "theme").contains("account-a"));
    }
}
