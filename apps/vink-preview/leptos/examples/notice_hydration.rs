//! The same Notice / `view!` `class:` + tuple specimen is rendered into static SSR HTML and hydrated in Chrome.

use leptos::prelude::*;
use vink_ui::{Announce, Banner, InlineNotice, Tone};

fn specimen() -> impl IntoView {
    let banner = RwSignal::new(false);
    view! {
        <main class="vk-scope">
            <div id="static-notices">
                <InlineNotice tone=Tone::Info message="Thông báo nội tuyến SSR" announce=Announce::Off />
                <Banner tone=Tone::Warning message="Thông báo đầu trang SSR" announce=Announce::Off />
            </div>
            <div id="reactive-notice">
                <div
                    id="macro-notice-probe"
                    class="vk-ui-notice flex flex-wrap items-start gap-y-vk-2 gap-x-vk-3 py-vk-3 bg-vk-info-container text-vk-on-info-container"
                    class:vk-ui-notice--banner=move || banner.get()
                    class:rounded-none=move || banner.get()
                    class:rounded-vk-md=move || !banner.get()
                    class:px-vk-6=move || banner.get()
                    class:px-vk-4=move || !banner.get()
                    class=("sm:gap-x-vk-6", move || banner.get())
                >
                    <p class="m-0">"Mẫu hydrate macro view!"</p>
                </div>
                <button id="toggle-banner" type="button" on:click=move |_| banner.update(|b| *b = !*b)>
                    "Đổi chế độ banner"
                </button>
            </div>
        </main>
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    let path = std::env::args_os().nth(1).expect("output HTML path");
    let html = Owner::new().with(|| specimen().to_html());
    std::fs::write(
        path,
        format!(
            "<!doctype html><html lang=\"vi\"><head><meta charset=\"utf-8\">\
             <link rel=\"stylesheet\" href=\"/vink-preview.css\"></head><body>{html}</body></html>"
        ),
    )
}

#[cfg(all(target_arch = "wasm32", feature = "media-hydrate"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(specimen);
}

#[cfg(target_arch = "wasm32")]
fn main() {}
