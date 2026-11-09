//! The same test specimen is rendered into static SSR HTML and hydrated in Chrome.

use leptos::prelude::*;
use vink_ui::identity_vectors::BROKEN_WEBP_BASE64;
use vink_ui::{ImageFrame, ImageState};

const STATIC_SAMPLE: &str = "/fixtures/sample.webp";

fn specimen() -> impl IntoView {
    let broken = format!("data:image/webp;base64,{BROKEN_WEBP_BASE64}");
    let early = RwSignal::new(ImageState::ready(&broken));
    let late = RwSignal::new(ImageState::ready(STATIC_SAMPLE));
    view! {
        <main class="vk-scope">
            <div id="before-hydration">
                <ImageFrame image=early alt="[Mẫu] Bìa tập thơ" />
                <button id="recover-early" on:click=move |_| early.set(ImageState::ready(STATIC_SAMPLE))>
                    "Đổi sang ảnh hợp lệ"
                </button>
            </div>
            <div id="after-hydration">
                <ImageFrame image=late alt="" />
                <button id="break-late" on:click=move |_| late.set(ImageState::ready(&broken))>
                    "Đổi sang ảnh lỗi"
                </button>
                <button id="recover-late" on:click=move |_| late.set(ImageState::ready(STATIC_SAMPLE))>
                    "Đổi sang ảnh hợp lệ"
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
             <link rel=\"stylesheet\" href=\"/tokens.css\">\
             <link rel=\"stylesheet\" href=\"/vink-ui.css\"></head><body>{html}</body></html>"
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
