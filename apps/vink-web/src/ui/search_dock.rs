//! Shared controls for the discovery and private Desk search docks.
use leptos::prelude::*;
use vink_ui::{IconButton, IconName};

#[component]
pub fn SearchDockLauncher(
    id: &'static str,
    label: &'static str,
    expanded: Signal<bool>,
    on_press: Callback<()>,
) -> impl IntoView {
    view! {
        <span class="vk-search-launcher">
            <IconButton id label icon=IconName::Search expanded on_press />
        </span>
    }
}

#[component]
pub fn SearchDockBack(
    id: &'static str,
    label: &'static str,
    on_press: Callback<()>,
) -> impl IntoView {
    view! {
        <span class="vk-search-close">
            <IconButton id label icon=IconName::ArrowBack on_press />
        </span>
    }
}
