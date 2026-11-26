//! A static EMO-01A document with optional hydration, served only by Preview.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    use leptos::prelude::*;
    let path = std::env::args_os().nth(1).expect("output HTML path");
    let html = Owner::new().with(|| vink_ui_preview::expressions::specimen().to_html());
    std::fs::write(
        path,
        format!(
            "<!doctype html><html lang=\"vi\"><head><meta charset=\"utf-8\">\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             <meta name=\"robots\" content=\"noindex\">\
             <title>Thử biểu cảm · Mẫu — Vink</title>\
             <link rel=\"stylesheet\" href=\"/vink-preview.css\">\
             <script type=\"module\">import init, {{ hydrate }} from '/pkg/expression_hydration.js';\
             await init(); hydrate();</script></head><body>{html}</body></html>"
        ),
    )
}

#[cfg(all(target_arch = "wasm32", feature = "media-hydrate"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(vink_ui_preview::expressions::specimen);
}

#[cfg(target_arch = "wasm32")]
fn main() {}
