//! Public and private profile views share the same heading slot.
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

#[cfg(feature = "browser")]
mod browser;

#[component]
pub(crate) fn SavedProfile() -> impl IntoView {
    let params = use_params_map();
    view! {
        <For each=move || [params.read().get("author").unwrap_or_default()]
            key=Clone::clone
            children=|segment| view! { <SavedRoute segment /> }
        />
    }
}

#[component]
fn SavedRoute(segment: String) -> impl IntoView {
    let status = super::ResponseStatus::capture();
    status.set(404);
    #[cfg(feature = "browser")]
    let owner = browser::route_owner(segment);
    #[cfg(not(feature = "browser"))]
    let _ = segment;
    // The server renders the same NotFound as an unknown route. The browser can
    // replace it only after validating both the account and the canonical handle.
    move || {
        #[cfg(feature = "browser")]
        if let Some(owner) = owner.get() {
            return view! { <browser::OwnSavedProfile owner /> }.into_any();
        }
        view! { <crate::ui::empty_state::NotFoundContent /> }.into_any()
    }
}

#[component]
pub(super) fn PublicWorksSlot(handle: String) -> impl IntoView {
    #[cfg(feature = "browser")]
    let (owner, _) = browser::verified_owner(handle);
    #[cfg(not(feature = "browser"))]
    let _ = handle;
    view! {
        <div id="profile-view-slot" class="vk-profile-view-slot">
            <h2 id="author-works-heading" tabindex="-1"
                class:vk-ui-visually-hidden=move || {
                    #[cfg(feature = "browser")]
                    { owner.with(Option::is_some) }
                    #[cfg(not(feature = "browser"))]
                    { false }
                }>"Tác phẩm công khai"</h2>
            {move || -> Option<AnyView> {
                #[cfg(feature = "browser")]
                {
                    owner.with(|verified| verified.as_ref().map(|owner| view! {
                        <browser::Modes handle=owner.profile.handle.clone() saved=false />
                    }.into_any()))
                }
                #[cfg(not(feature = "browser"))]
                { None }
            }}
        </div>
    }
}
