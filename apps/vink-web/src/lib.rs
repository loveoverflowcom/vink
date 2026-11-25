//! One Leptos router for the standalone browser app and the pre-cutover SSR host.
//! The CSR bundle mounts on a static document. The legacy browser entry hydrates an SSR body.

// Composition slots increase the hydration type depth without changing runtime state.
#![recursion_limit = "256"]

mod account;
pub mod api;
pub mod appearance;
pub mod contract;
pub mod desk_state;
pub mod discovery;
pub mod discussion_reads;
pub mod discussion_state;
pub mod document;
pub mod draft_list;
pub mod editor;
pub mod endorsement_state;
pub mod follow_state;
pub mod freshness;
pub mod navigation;
mod notifications;
mod pages;
pub mod public_list;
pub mod public_persistence;
pub mod public_reads;
pub mod publication_state;
pub mod read_cache;
pub mod read_events;
pub mod read_lifecycle;
pub mod search;
#[cfg(feature = "ssr")]
pub mod server;
pub mod session;
mod shell;
pub mod tag_state;
pub mod ui;

use leptos::prelude::*;
#[cfg(not(feature = "csr"))]
use leptos_meta::MetaTags;
use leptos_meta::provide_meta_context;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{SsrMode, path};

use pages::me::Me;
use pages::{DeskIndex, DraftEditor, Home, NotFound, Reader, TagDetail};
use shell::AdaptiveShell;

// The OCI build pins SSR and hydrated views to the same public asset namespace.
fn asset_url(path: &str) -> String {
    format!("{}{path}", option_env!("VINK_ASSET_PREFIX").unwrap_or(""))
}

// An SSR document can be at an earlier history entry when hydration starts.
// Identify its route without reflecting the requested path, query or retired Handle into HTML.
#[cfg(not(feature = "csr"))]
fn route_fingerprint(path: &str, query: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(path.as_bytes());
    hash.update([0]);
    hash.update(query.as_bytes());
    hash.finalize()[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// HTML document around the app, rendered on the server only.
#[cfg(not(feature = "csr"))]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    let fixture = api::local_discussion_fixture_scope();
    let rendered_route = use_context::<leptos_router::location::RequestUrl>()
        .and_then(|request| request.parse().ok())
        .map(|url| route_fingerprint(url.path(), url.search()));
    view! {
        <!DOCTYPE html>
        <html lang="vi" data-rendered-route=rendered_route>
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="color-scheme" content="light dark" />
                {fixture.map(|scope| view! { <meta name="vink-local-discussion-fixture" content=scope /> })}
                {api::sign_in_open().then(|| view! { <meta name="vink-sign-in" content="open" /> })}
                {api::following_feed_enabled().then(|| view! { <meta name="vink-following-feed" content="enabled" /> })}
                <link id="vink-favicon" rel="icon" type="image/svg+xml" href=asset_url("/pkg/brand/vpen-r1-favicon.svg") />
                <link id="vink-manifest" rel="manifest" href=asset_url("/pkg/brand/vpen-r1-manifest-light.webmanifest") />
                <script inner_html=appearance::BOOTSTRAP></script>
                <link as="image" rel="preload" href=asset_url("/pkg/brand/vpen-r1-lockup-light.svg") type="image/svg+xml" />
                <link as="image" rel="preload" href=asset_url("/pkg/brand/vpen-r1-lockup-dark.svg") type="image/svg+xml" />
                // Faces every current page renders above the fold: UI body text and the
                // reading/heading serif. The 600 face is left to CSS discovery.
                // `as` first: after a string value the macro would read it as a cast.
                <link
                    as="font"
                    rel="preload"
                    href=asset_url("/pkg/fonts/be-vietnam-pro-400.woff2")
                    type="font/woff2"
                    crossorigin="anonymous"
                />
                <link
                    as="font"
                    rel="preload"
                    href=asset_url("/pkg/fonts/literata-variable.woff2")
                    type="font/woff2"
                    crossorigin="anonymous"
                />
                <link rel="stylesheet" href=asset_url("/pkg/vink.css?brand=vpen-r1") />
                <HydrationScripts options root=option_env!("VINK_ASSET_PREFIX").unwrap_or("").to_string() />
                <MetaTags />
            </head>
            <body class="m-0 flex min-h-screen flex-col">
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    #[cfg(feature = "hydrate")]
    mark_hydrated();
    navigation::NavigationGuard::provide();
    view! {
        <leptos_meta::Title formatter=format_page_title />
        <Router>
            <Chrome />
        </Router>
    }
}

// leptos_meta 0.8.7 writes the server title directly into HTML. The browser setter
// takes plain text, so it must receive the original title during and after hydration.
fn format_page_title(title: String) -> String {
    #[cfg(feature = "ssr")]
    {
        html_escape::encode_text(&title).into_owned()
    }
    #[cfg(not(feature = "ssr"))]
    {
        title
    }
}

/// Everything inside the router. Connects the shared [`navigation::NavigationGuard`], which
/// links, back/forward and later navigation surfaces consult before leaving unconfirmed text.
#[component]
fn Chrome() -> impl IntoView {
    navigation::use_navigation_guard().connect_router();
    #[cfg(feature = "browser")]
    {
        use wasm_bindgen::prelude::*;
        #[wasm_bindgen(module = "/profile/navigation.js")]
        extern "C" {
            #[wasm_bindgen(js_name = mountProfileNavigation)]
            fn mount_profile_navigation();
        }
        Effect::new(move |_| mount_profile_navigation());
    }
    view! {
        <AdaptiveShell>
            <main id="noi-dung" class="vk-main w-full [margin:0_auto]" tabindex="-1">
                <Routes fallback=NotFound>
                    <Route path=path!("/") view=Home ssr=SsrMode::Async />
                    <Route path=path!("/the/:slug") view=TagDetail ssr=SsrMode::Async />
                    <Route path=path!("/tac-pham/:id") view=Reader ssr=SsrMode::Async />
                    <Route path=path!("/ban-viet") view=DeskIndex />
                    <Route path=path!("/ban-viet/:id") view=DraftEditor />
                    <Route path=path!("/kiem-duyet") view=pages::moderation::ModerationHome />
                    <Route path=path!("/kiem-duyet/ca/:id") view=pages::moderation::CasePage />
                    <Route path=path!("/kiem-duyet/phan-doi/:id") view=pages::moderation::AppealPage />
                    <Route path=path!("/ban-viet/:id/kiem-duyet") view=pages::desk_review::LegacyDeskReview />
                    <Route path=path!("/thong-bao") view=notifications::InboxPage />
                    <Route path=path!("/thong-bao/nhan-xet/:id/:version") view=notifications::CommentDecisionPage />
                    <Route path=path!("/tac-pham/:id/tham/:revision") view=notifications::EndorsersPage />
                    <Route path=path!("/toi") view=Me />
                    <Route path=path!("/toi/chinh-ho-so") view=pages::profile::EditProfile />
                    <Route path=path!("/toi/dia-chi") view=pages::profile::LegacyHandle />
                    <Route path=path!("/:author") view=pages::handle::Author ssr=SsrMode::Async />
                    <Route path=path!("/:author/nguoi-theo-doi") view=pages::follow::FollowersList ssr=SsrMode::Async />
                    <Route path=path!("/:author/dang-theo-doi") view=pages::follow::FollowingList ssr=SsrMode::Async />
                </Routes>
            </main>
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
    #[cfg(feature = "browser")]
    {
        window().location().origin().unwrap_or_default()
    }
    #[cfg(not(feature = "browser"))]
    {
        String::new()
    }
}

/// Sets `data-hydrated` on `<html>` once the browser app is live (used by browser checks).
#[cfg(feature = "hydrate")]
fn mark_hydrated() {
    Effect::new(|_| {
        if let Some(root) = document().document_element() {
            let _ = root.set_attribute("data-hydrated", "true");
        }
    });
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(module = "/profile/navigation.js")]
    extern "C" {
        #[wasm_bindgen(js_name = canHydrateCurrentEntry)]
        fn can_hydrate_current_entry(fingerprint: &str) -> bool;
    }

    let location = window().location();
    if let (Ok(path), Ok(query)) = (location.pathname(), location.search())
        && !can_hydrate_current_entry(&route_fingerprint(
            &path,
            query.strip_prefix('?').unwrap_or(&query),
        ))
    {
        return;
    }
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}

/// A static document has no server-rendered nodes or hydration payload.
#[cfg(feature = "csr")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mount() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
