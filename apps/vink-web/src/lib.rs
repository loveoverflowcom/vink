//! Vink web: Leptos views rendered on the server (`ssr`) and hydrated in the browser (`hydrate`).
//!
//! The two features are separate builds; never enable both. The browser build contains views,
//! the web app's own API representations and a fetch client — no service, persistence or secrets.

// Composition slots increase the hydration type depth without changing runtime state.
#![recursion_limit = "256"]

mod account;
pub mod api;
pub mod appearance;
pub mod contract;
pub mod desk_state;
pub mod discovery;
pub mod discussion_state;
pub mod document;
pub mod draft_list;
pub mod editor;
pub mod endorsement_state;
mod expressions;
pub mod freshness;
pub mod navigation;
mod pages;
pub mod public_list;
pub mod public_reads;
pub mod search;
#[cfg(feature = "ssr")]
pub mod server;
pub mod session;
mod shell;
pub mod tag_state;
pub mod ui;

use leptos::prelude::*;
use leptos_meta::{MetaTags, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{SsrMode, path};

use pages::me::Me;
use pages::{DeskIndex, DraftEditor, Home, NotFound, Reader, TagDetail};
use shell::AdaptiveShell;

/// HTML document around the app, rendered on the server only.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    let fixture = api::local_discussion_fixture_scope();
    view! {
        <!DOCTYPE html>
        <html lang="vi">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="color-scheme" content="light dark" />
                {fixture.map(|scope| view! { <meta name="vink-local-discussion-fixture" content=scope /> })}
                {api::sign_in_open().then(|| view! { <meta name="vink-sign-in" content="open" /> })}
                <link id="vink-favicon" rel="icon" type="image/svg+xml" href="/pkg/brand/vpen-r1-favicon.svg" />
                <link id="vink-manifest" rel="manifest" href="/pkg/brand/vpen-r1-manifest-light.webmanifest" />
                <script inner_html=appearance::BOOTSTRAP></script>
                <link as="image" rel="preload" href="/pkg/brand/vpen-r1-lockup-light.svg" type="image/svg+xml" />
                <link as="image" rel="preload" href="/pkg/brand/vpen-r1-lockup-dark.svg" type="image/svg+xml" />
                // Faces every current page renders above the fold: UI body text and the
                // reading/heading serif. The 600 face is left to CSS discovery.
                // `as` first: after a string value the macro would read it as a cast.
                <link
                    as="font"
                    rel="preload"
                    href="/pkg/fonts/be-vietnam-pro-400.woff2"
                    type="font/woff2"
                    crossorigin="anonymous"
                />
                <link
                    as="font"
                    rel="preload"
                    href="/pkg/fonts/literata-variable.woff2"
                    type="font/woff2"
                    crossorigin="anonymous"
                />
                <link rel="stylesheet" href="/pkg/vink.css?brand=vpen-r1" />
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
    navigation::NavigationGuard::provide();
    view! {
        <Router>
            <Chrome />
        </Router>
    }
}

/// Everything inside the router. Connects the shared [`navigation::NavigationGuard`], which
/// links, back/forward and later navigation surfaces consult before leaving unconfirmed text.
#[component]
fn Chrome() -> impl IntoView {
    navigation::use_navigation_guard().connect_router();
    view! {
        <AdaptiveShell>
            <main id="noi-dung" class="vk-main" tabindex="-1">
                <Routes fallback=NotFound>
                    <Route path=path!("/") view=Home ssr=SsrMode::Async />
                    <Route path=path!("/the/:slug") view=TagDetail ssr=SsrMode::Async />
                    <Route path=path!("/tac-pham/:id") view=Reader ssr=SsrMode::Async />
                    <Route path=path!("/ban-viet") view=DeskIndex />
                    <Route path=path!("/ban-viet/:id") view=DraftEditor />
                    <Route path=path!("/toi") view=Me />
                    <Route path=path!("/thu-nghiem/bieu-cam") view=expressions::ExpressionPilot />
                </Routes>
            </main>
            <footer class="vk-footer">
                <vink_ui::SupportingDetails id="build-info" summary="Bản thử nghiệm">
                    <p>"Chưa mở đăng nhập. Nội dung gắn Mẫu là dữ liệu thử."</p>
                </vink_ui::SupportingDetails>
            </footer>
        </AdaptiveShell>
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
