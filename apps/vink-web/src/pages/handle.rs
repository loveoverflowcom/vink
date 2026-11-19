use crate::{
    account::Session,
    contract::{AuthorResolution, ProfileWorks},
};
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::{use_params_map, use_query_map};
use vink_ui::Link as UiLink;

#[component]
pub fn Author() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    view! { <For
        each=move || vec![(params.read().get("author").unwrap_or_default(), query.read().get("cursor"))]
        key=Clone::clone
        children=|(segment, cursor)| match segment.strip_prefix('@') {
            Some(name) => view!{<PublicProfile name=name.to_string() cursor />}.into_any(),
            None => view!{<super::NotFound />}.into_any()
        }
    /> }
}

#[component]
fn PublicProfile(name: String, cursor: Option<String>) -> impl IntoView {
    let (generation, advance) = signal(0u64);
    let data = crate::api::author_resource(name.clone(), cursor.clone(), generation);
    let status = super::ResponseStatus::capture();
    status.set_no_store();
    #[cfg(feature = "hydrate")]
    {
        super::profile::observe_changes(move || advance.update(|stamp| *stamp += 1));
        crate::read_events::observe(move || {
            if crate::read_events::visible() {
                advance.update(|stamp| *stamp += 1);
            }
        });
    }
    view! {
        <Suspense fallback=|| view! { <p role="status">"Đang mở trang tác giả…"</p> }>
            {move || {
                let stamp = generation.get();
                match data.get() {
                    Some((result_stamp, result)) if result_stamp == stamp => {
                        render_profile(result, &status, &name, cursor.is_some(), advance)
                    }
                    _ => view! { <p role="status">"Đang mở trang tác giả…"</p> }.into_any(),
                }
            }}
        </Suspense>
    }
}

fn render_profile(
    result: Result<AuthorResolution, crate::api::LoadError>,
    status: &super::ResponseStatus,
    name: &str,
    continued: bool,
    advance: WriteSignal<u64>,
) -> AnyView {
    let (code, heading, detail) = match result {
        Ok(AuthorResolution::Current {
            author,
            bio,
            publications,
            has_public_works,
        }) if author.valid_href().is_some() => {
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
                    <header>
                        <h1 id="author-profile-heading" tabindex="-1">{author.pen_name}</h1>
                        <p>{format!("@{}", author.handle)}</p>
                    </header>
                    <super::profile::OwnProfileLink handle=author.handle />
                    {bio.map(|text| view! { <p class="vk-author-bio">{text}</p> })}
                    <section aria-labelledby="author-works-heading">
                        <h2 id="author-works-heading" tabindex="-1">"Tác phẩm công khai"</h2>
                        {match publications {
                            Some(ProfileWorks::Page { works, next_cursor }) => {
                                let items = works.into_iter().map(|work| view! {
                                    <li><UiLink
                                        attr:id=format!("profile-work-{}", work.work_id)
                                        attr:data-profile-return="true"
                                        href=format!("/tac-pham/{}", work.work_id)>{work.title}</UiLink></li>
                                }).collect_view();
                                let empty = items.is_empty();
                                let next = next_cursor.map(|cursor| format!("{}?cursor={cursor}", author.href));
                                view! {
                                    {empty.then(|| view! { <p>{if continued {
                                        "Trang này không còn tác phẩm."
                                    } else { "Chưa có tác phẩm công khai." }}</p> })}
                                    {(!empty).then(|| view! { <ul class="vk-author-works">{items}</ul> })}
                                    {next.map(|href| view! { <UiLink href>"Trang tiếp"</UiLink> })}
                                    {continued.then(|| view! { <UiLink href=author.href>"Về trang đầu"</UiLink> })}
                                }.into_any()
                            }
                            Some(ProfileWorks::Restart) => view! {
                                <p role="alert">"Trang tác phẩm không còn khớp với hồ sơ này."</p>
                                <UiLink href=author.href>"Về trang đầu"</UiLink>
                            }.into_any(),
                            None if !has_public_works => view! { <p>"Chưa có tác phẩm công khai."</p> }.into_any(),
                            None => view! {
                                <p role="alert">"Chưa tải được danh sách tác phẩm."</p>
                                <button type="button" class="vk-ui-button vk-ui-button--tonal"
                                    on:click=move |_| advance.update(|stamp| *stamp += 1)>"Thử lại"</button>
                            }.into_any(),
                        }}
                    </section>
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
        _ => (
            503,
            "Chưa tải được hồ sơ",
            "Hãy kiểm tra kết nối rồi thử lại.",
        ),
    };
    status.set(code);
    let retry_href = format!("/@{name}");
    view! {
        <Title text=format!("{heading} · Vink") />
        <section class="vk-author-public">
            <h1 id="author-profile-heading" tabindex="-1">{heading}</h1><p>{detail}</p>
            {if code == 503 {
                view! {
                    <button type="button" class="vk-ui-button vk-ui-button--tonal"
                        on:click=move |_| advance.update(|stamp| *stamp += 1)>"Thử lại"</button>
                    <noscript><a href=retry_href>"Tải lại"</a></noscript>
                }.into_any()
            } else {
                view! { <UiLink href="/">"Khám phá"</UiLink> }.into_any()
            }}
        </section>
    }
    .into_any()
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
