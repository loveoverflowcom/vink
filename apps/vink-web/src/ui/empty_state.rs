//! App-owned artwork composition; callers keep read/create/focus and announcement ownership.
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyArtwork {
    NoResults,
    NoPublicWorks,
    /// Reserved for the Desk consumer (#259), never a claim about public discovery.
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

/// A decoration and one HTML state sentence. Actions are real caller-owned controls.
/// Opt into a polite status only when the caller has no other result announcement.
#[component]
pub fn EmptyState(
    artwork: EmptyArtwork,
    #[prop(into)] message: String,
    #[prop(optional)] announce: bool,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="vk-empty-state">
            <img class="vk-empty-state__art" src=artwork.src() alt="" width="512" height="427" decoding="async" />
            <p class="vk-empty-state__message" role=announce.then_some("status") aria-atomic=announce.then_some("true")>{message}</p>
            {children.map(|children| view! { <div class="vk-empty-state__actions">{children()}</div> })}
        </div>
    }
}
