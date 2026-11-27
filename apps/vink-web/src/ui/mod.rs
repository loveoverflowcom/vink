//! Adapters own API DTOs; Vink compositions own only view facts and intents.
pub mod adapters;
pub mod empty_state;
pub use vink_patterns as patterns;
pub mod search_dock;

use leptos::prelude::*;
use vink_ui::{Clock, EdgeProgress, IndicatorSize, LoadingIndicator, use_loading_gate};

/// A request-owned placeholder. Unmounting it immediately yields to the answer and cancels the gate.
#[component]
pub(crate) fn PendingRead(label: &'static str) -> impl IntoView {
    let gate = use_loading_gate(Signal::stored(true), Clock::Browser);
    view! {
        <span class="vk-pending-read" role="status">
            <span class="vk-ui-visually-hidden">{label}</span>
            <Show when=move || gate.visible.get()>
                <LoadingIndicator size=IndicatorSize::Medium />
            </Show>
            <Show when=move || gate.slow.get()>
                <span>"Vẫn đang tải…"</span>
            </Show>
        </span>
    }
}

/// Overlays the retained list; rows and their focus stay in normal flow.
#[component]
pub(crate) fn RefreshProgress(active: Signal<bool>) -> impl IntoView {
    let gate = use_loading_gate(active, Clock::Browser);
    view! {
        <Show when=move || active.get() && gate.visible.get()>
            <div class="vk-refresh-progress"><EdgeProgress label="Đang làm mới" /></div>
        </Show>
    }
}
