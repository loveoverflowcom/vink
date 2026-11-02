//! Theme scopes: the only components in this foundation slice.

use leptos::prelude::*;

use crate::theme::{Density, MotionPreference, Preferences, RoleOverride, ThemePreference};

/// The root of a Vink UI subtree. Applies the injected preferences as `data-*` attributes on a
/// `.vk-scope` element; `style/tokens.css` resolves them. It never reads storage or the system
/// itself — the caller owns persistence. An invalid override set renders no overrides and a
/// `data-override-error` attribute instead of an invalid custom property.
#[component]
pub fn UiRoot(
    #[prop(into)] preferences: Signal<Preferences>,
    #[prop(optional, into)] overrides: Signal<Vec<RoleOverride>>,
    #[prop(optional, into)] label: Option<String>,
    children: Children,
) -> impl IntoView {
    let theme = Signal::derive(move || Some(preferences.get().theme));
    let density = Signal::derive(move || Some(preferences.get().density));
    let reduce = Signal::derive(move || preferences.get().motion == MotionPreference::Reduce);
    scope(
        "vk-ui vk-scope",
        theme,
        density,
        reduce,
        overrides,
        label,
        children,
    )
}

/// A nested scope. `None` inherits from the enclosing scope; an explicit theme re-themes the
/// subtree (including `System` inside an explicit parent). `reduce_motion` can only add
/// reduction.
#[component]
pub fn ThemeScope(
    #[prop(optional, into)] theme: Signal<Option<ThemePreference>>,
    #[prop(optional, into)] density: Signal<Option<Density>>,
    #[prop(optional, into)] reduce_motion: Signal<bool>,
    #[prop(optional, into)] overrides: Signal<Vec<RoleOverride>>,
    #[prop(optional, into)] label: Option<String>,
    children: Children,
) -> impl IntoView {
    scope(
        "vk-scope",
        theme,
        density,
        reduce_motion,
        overrides,
        label,
        children,
    )
}

fn scope(
    class: &'static str,
    theme: Signal<Option<ThemePreference>>,
    density: Signal<Option<Density>>,
    reduce: Signal<bool>,
    overrides: Signal<Vec<RoleOverride>>,
    label: Option<String>,
    children: Children,
) -> impl IntoView {
    let style = Memo::new(move |_| RoleOverride::style(&overrides.get()));
    view! {
        <div
            class=class
            data-theme=move || theme.get().map(ThemePreference::value)
            data-density=move || density.get().map(Density::value)
            data-motion=move || reduce.get().then_some("reduce")
            data-override-error=move || style.get().err().map(|e| format!("{e:?}"))
            style=move || style.get().ok().filter(|s| !s.is_empty())
            aria-label=label
        >
            {children()}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::ColorRole;

    fn render(view: impl FnOnce() -> AnyView + 'static) -> String {
        let owner = Owner::new();
        owner.with(|| view().to_html())
    }

    #[test]
    fn root_renders_injected_preferences_as_scope_attributes() {
        let html = render(|| {
            let preferences = Preferences {
                theme: ThemePreference::Dark,
                density: Density::Compact,
                motion: MotionPreference::Reduce,
            };
            view! { <UiRoot preferences=Signal::stored(preferences)>"x"</UiRoot> }.into_any()
        });
        for needle in [
            "class=\"vk-ui vk-scope\"",
            "data-theme=\"dark\"",
            "data-density=\"compact\"",
            "data-motion=\"reduce\"",
        ] {
            assert!(html.contains(needle), "{needle} in {html}");
        }
    }

    #[test]
    fn nested_scope_inherits_unless_told_and_rejects_invalid_overrides() {
        let html = render(|| {
            let bad = vec![RoleOverride {
                role: ColorRole::Primary,
                to: ColorRole::Primary,
            }];
            view! { <ThemeScope overrides=bad>"x"</ThemeScope> }.into_any()
        });
        assert!(html.contains("class=\"vk-scope\""), "{html}");
        assert!(!html.contains("data-theme"), "inherits: {html}");
        assert!(!html.contains("data-motion"), "{html}");
        assert!(
            html.contains("data-override-error=\"SelfReference(Primary)\""),
            "{html}"
        );
        assert!(!html.contains("--vk-primary"), "{html}");
    }
}
