//! App-owned artwork composition; callers keep read/create/focus and announcement ownership.
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyArtwork {
    NoResults,
    NoPublicWorks,
    /// Desk private first-use only (#259); never a claim about public discovery.
    FirstDraft,
}

impl EmptyArtwork {
    pub const fn src(self) -> &'static str {
        match self {
            Self::NoResults => "/pkg/empty/vink-empty-no-results.webp",
            Self::NoPublicWorks => "/pkg/empty/vink-empty-no-public-works.webp",
            Self::FirstDraft => "/pkg/empty/vink-empty-first-draft.webp",
        }
    }
}

/// A decoration and an HTML state. Actions are real caller-owned controls.
/// Opt into a polite status only when the caller has no other result announcement.
#[component]
pub fn EmptyState(
    #[prop(optional)] artwork: Option<EmptyArtwork>,
    #[prop(optional)] icon: Option<vink_ui::IconName>,
    #[prop(default = vink_ui::HeadingLevel::H3)] heading: vink_ui::HeadingLevel,
    #[prop(optional)] title: Option<&'static str>,
    #[prop(into)] message: String,
    #[prop(optional)] announce: bool,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="vk-empty-state">
            {artwork.map(|artwork| view! {
                <img class="vk-empty-state__art" src=crate::asset_url(artwork.src()) alt="" width="512" height="427" decoding="async" />
            })}
            {icon.map(|name| view! {
                <span class="vk-empty-state__icon"><vink_ui::Icon name /></span>
            })}
            {title.map(|title| view! { <vink_ui::Heading class:vk-empty-state__title=true
                level=heading role=vink_ui::TypeRole::Title>{title}</vink_ui::Heading> })}
            <p class="vk-empty-state__message" role=announce.then_some("status") aria-atomic=announce.then_some("true")>{message}</p>
            {children.map(|children| view! { <div class="vk-empty-state__actions">{children()}</div> })}
        </div>
    }
}
