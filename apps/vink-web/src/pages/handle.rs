use crate::{account::Session, contract::AuthorResolution};
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::use_params_map;

#[component]
pub fn Author() -> impl IntoView {
    let params = use_params_map();
    view! { <For each=move || vec![params.read().get("author").unwrap_or_default()] key=Clone::clone
    children=|segment| match segment.strip_prefix('@') {
        Some(name) => view!{<PublicProfile name=name.to_string() />}.into_any(),
        None => view!{<super::NotFound />}.into_any()
    } /> }
}

#[component]
fn PublicProfile(name: String) -> impl IntoView {
    let data = crate::api::author_resource(name);
    let status = super::ResponseStatus::capture();
    status.set_no_store();
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        let restore = window_event_listener_untyped("pageshow", |event| {
            if event
                .dyn_ref::<web_sys::PageTransitionEvent>()
                .is_some_and(|page| page.persisted())
            {
                // Back/forward may revive an entire document, including an obsolete
                // address. Re-enter the HTTP resolver for the current status and Location.
                let _ = window().location().reload();
            }
        });
        on_cleanup(move || restore.remove());
    }
    view! {
        <Suspense fallback=|| view! { <p role="status">"Đang mở trang tác giả…"</p> }>
            {move || data.get().map(|result| render_profile(result, &status))}
        </Suspense>
    }
}

fn render_profile(
    result: Result<AuthorResolution, crate::api::LoadError>,
    status: &super::ResponseStatus,
) -> AnyView {
    let (code, heading, detail) = match result {
        Ok(AuthorResolution::Current {
            author,
            has_public_works,
        }) => {
            let initials = author
                .pen_name
                .split_whitespace()
                .filter_map(|s| s.chars().next())
                .take(2)
                .collect::<String>()
                .to_uppercase();
            return view! {
                <Title text=format!("{} · Vink", author.pen_name) />
                <Link rel="canonical" href=crate::canonical_url(&author.href) />
                <article class="vk-author-public">
                    <MobileBrand />
                    <div class="vk-author-art" aria-hidden="true"></div>
                    <span class="vk-author-seal" aria-hidden="true">{initials}</span>
                    <header><p>"Trang tác giả"</p><h1 tabindex="-1">{author.pen_name}</h1><p>{format!("@{}", author.handle)}</p></header>
                    {(!has_public_works).then(|| view! {
                        <section><h2>"Chưa có tác phẩm công khai"</h2><p>"Tác phẩm được tác giả công khai sẽ xuất hiện ở đây."</p></section>
                    })}
                </article>
            }.into_any();
        }
        Ok(AuthorResolution::Alias { href }) => {
            #[cfg(feature = "ssr")]
            if let Some(opts) = &status.0
                && let Ok(url) = http::HeaderValue::from_str(&href)
            {
                opts.set_status(http::StatusCode::MOVED_PERMANENTLY);
                opts.insert_header(http::header::LOCATION, url);
            }
            #[cfg(feature = "hydrate")]
            {
                let location = window().location();
                let _ =
                    location.replace(&format!("{}{}", href, location.hash().unwrap_or_default()));
            }
            return view! { <a href=href>"Mở địa chỉ hiện tại"</a> }.into_any();
        }
        Ok(AuthorResolution::Gone) => (
            410,
            "Hồ sơ không còn trên Vink",
            "Tài khoản của tác giả đã được xóa.",
        ),
        Ok(AuthorResolution::Missing) => (
            404,
            "Không tìm thấy hồ sơ",
            "Hãy kiểm tra lại địa chỉ trong liên kết.",
        ),
        Err(_) => (
            503,
            "Chưa tải được hồ sơ",
            "Hãy kiểm tra kết nối rồi mở lại trang tác giả.",
        ),
    };
    status.set(code);
    view! {
        <Title text=format!("{heading} · Vink") />
        <section class="vk-author-public"><h1>{heading}</h1><p>{detail}</p><a href="/">"Khám phá"</a></section>
    }.into_any()
}

#[component]
pub fn Manage() -> impl IntoView {
    view! { <Title text="Địa chỉ tác giả · Vink" /><Meta name="robots" content="noindex" />
    <section class="vk-handle-page"><MobileBrand /><a class="vk-handle-back" href="/toi#dia-chi-tac-gia">"Tôi"</a>
        <h1>"Địa chỉ tác giả"</h1><p>"Đây là liên kết để người đọc tìm đến bạn trên Vink."</p>
        <HandleRegion entry=false /></section> }
}

#[component]
fn MobileBrand() -> impl IntoView {
    view! { <a class="vk-brand vk-handle-brand" href="/" aria-label="Vink — trang chủ">
    <span class="vk-brand__lockup" aria-hidden="true"></span></a> }
}

/// Remount for each session generation. Pending replies belong to their originating element.
#[component]
pub(super) fn HandleRegion(entry: bool) -> impl IntoView {
    let session = Session::current();
    let fixture = crate::api::local_discussion_fixture_scope().is_some();
    view! { <For each=move || if fixture { vec![] } else { session.scope.get().into_iter().collect::<Vec<_>>() }
    key=Clone::clone children=move |_| view!{ <HandleHost entry principal=session.principal.get_untracked().unwrap_or_default() /> } />
    {move || (!entry && session.scope.get().is_none()).then(|| view!{<p>"Mở Tài khoản trong Tôi để kiểm tra phiên của bạn."</p>})} }
}

#[component]
fn HandleHost(entry: bool, principal: String) -> impl IntoView {
    let host = NodeRef::<leptos::html::Div>::new();
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::prelude::*;
        #[wasm_bindgen(module = "/profile/handle.js")]
        extern "C" {
            #[wasm_bindgen(js_name=mountHandle)]
            fn mount_handle(host: &web_sys::HtmlElement, principal: &str, entry: bool);
        }
        Effect::new(move |_| {
            if let Some(node) = host.get() {
                mount_handle(&node, &principal, entry);
            }
        });
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (principal, entry);
    view! { <div id="dia-chi-tac-gia" node_ref=host></div> }
}
