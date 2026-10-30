//! Vink web: Leptos views rendered on the server (`ssr`) and hydrated in the browser (`hydrate`).
//!
//! The two features are separate builds; never enable both. The browser build contains views,
//! the web app's own API representations and a fetch client — no service, persistence or secrets.

pub mod api;
pub mod contract;
pub mod desk_state;
pub mod document;
mod pages;
#[cfg(feature = "ssr")]
pub mod server;

use leptos::prelude::*;
use leptos_meta::{MetaTags, provide_meta_context};
use leptos_router::components::{A, Route, Router, Routes};
use leptos_router::{SsrMode, path};

use pages::{DeskIndex, DraftEditor, Home, NotFound, Reader};

/// HTML document around the app, rendered on the server only.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="vi">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="color-scheme" content="light dark" />
                <link rel="stylesheet" href="/pkg/vink.css" />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    mark_hydrated();
    view! {
        <Router>
            <a class="vk-skip-link" href="#noi-dung">
                "Bỏ qua tới nội dung"
            </a>
            <header class="vk-topbar">
                <a class="vk-brand" href="/" aria-label="Vink — trang chủ">
                    <span class="vk-brand__mark" aria-hidden="true">
                        "V"
                    </span>
                    <span class="vk-brand__name">"Vink"</span>
                </a>
                <nav class="vk-nav" aria-label="Điều hướng chính">
                    <A href="/" exact=true>
                        "Đọc"
                    </A>
                    <A href="/ban-viet">"Bàn viết"</A>
                </nav>
            </header>
            <main id="noi-dung" class="vk-main" tabindex="-1">
                <Routes fallback=NotFound>
                    <Route path=path!("/") view=Home ssr=SsrMode::Async />
                    <Route path=path!("/tac-pham/:id") view=Reader ssr=SsrMode::Async />
                    <Route path=path!("/ban-viet") view=DeskIndex />
                    <Route path=path!("/ban-viet/:id") view=DraftEditor />
                </Routes>
            </main>
            <footer class="vk-footer">
                <p>
                    "Vink · Việt Ink — bản dựng nền tảng. Chưa mở đăng ký; tác phẩm gắn nhãn [Mẫu] là dữ liệu phát triển."
                </p>
            </footer>
        </Router>
    }
}

/// Absolute URL on the public origin, for canonical links.
pub(crate) fn canonical_url(path: &str) -> String {
    #[cfg(feature = "ssr")]
    let origin = use_context::<server::SiteOrigin>()
        .map(|origin| origin.0)
        .unwrap_or_default();
    #[cfg(not(feature = "ssr"))]
    let origin = browser_origin();
    format!("{origin}{path}")
}

#[cfg(not(feature = "ssr"))]
fn browser_origin() -> String {
    #[cfg(feature = "hydrate")]
    {
        window().location().origin().unwrap_or_default()
    }
    #[cfg(not(feature = "hydrate"))]
    {
        String::new()
    }
}

/// Sets `data-hydrated` on `<html>` once the browser app is live (used by browser checks).
fn mark_hydrated() {
    #[cfg(feature = "hydrate")]
    Effect::new(|_| {
        if let Some(root) = document().document_element() {
            let _ = root.set_attribute("data-hydrated", "true");
        }
    });
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
