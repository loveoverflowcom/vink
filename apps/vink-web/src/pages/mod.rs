mod desk;
pub(crate) mod me;

pub use desk::{DeskIndex, DraftEditor};

use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::components::Form;
use leptos_router::hooks::{use_params_map, use_query_map};

use crate::api::{self, LoadError};
use crate::appearance::initials;
use crate::canonical_url;
use crate::contract::{Publication, WorkCard, WorkList};
use crate::document::DocumentView;
use crate::public_list::{explore_href, reader_href, visible_works, work_anchor};

/// HTTP status for the server response; a no-op in the browser.
#[derive(Clone)]
struct ResponseStatus(#[cfg(feature = "ssr")] Option<leptos_axum::ResponseOptions>);

impl ResponseStatus {
    /// Must be called synchronously in a component body, where the request context exists.
    fn capture() -> Self {
        Self(
            #[cfg(feature = "ssr")]
            use_context::<leptos_axum::ResponseOptions>(),
        )
    }

    fn set(&self, _code: u16) {
        #[cfg(feature = "ssr")]
        if let (Some(options), Ok(code)) = (&self.0, http::StatusCode::from_u16(_code)) {
            options.set_status(code);
        }
    }
}

/// `2026-10-30T03:04:05Z` → `30/10/2026` (UTC date).
fn display_date(timestamp: &str) -> String {
    let date = timestamp.get(..10).unwrap_or(timestamp);
    let mut parts = date.split('-');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(y), Some(m), Some(d)) => format!("{d}/{m}/{y}"),
        _ => timestamp.to_string(),
    }
}

#[component]
pub fn Home() -> impl IntoView {
    let works = api::works_resource();
    let status = ResponseStatus::capture();
    let params = use_query_map();
    let query = Memo::new(move |_| params.read().get("q").unwrap_or_default());
    view! {
        <Title text="Khám phá — Vink" />
        <Meta
            name="description"
            content="Vink là không gian viết, đọc và góp ý văn chương tiếng Việt."
        />
        <Link rel="canonical" href=canonical_url("/") />
        <div id="explore" class="vk-explore">
            <header id="explore-heading" class="vk-explore__heading">
                <h1 class="vk-explore__title">"Khám phá"</h1>
                <p>"Đọc một câu chuyện, góp một ý hay, rồi viết tiếp."</p>
            </header>
            <div class="vk-discover-grid">
            <section id="community-stream" aria-labelledby="works-title">
                <h2 id="works-title" class="vk-section__title">"Tác phẩm công khai"</h2>
                <p id="public-scope" class="vk-muted vk-public-scope">
                    "Tìm theo tiêu đề hoặc tên tác giả trong tối đa 12 tác phẩm mới công khai đã tải, theo ngày đăng. Không tìm toàn bộ thư viện."
                </p>
                <Form method="get" action="/" noscroll=true attr:role="search" attr:class="vk-public-search" attr:id="public-search">
                    <label class="vk-field" for="public-query">
                        <span class="vk-field__label">"Tìm trong danh sách"</span>
                        <input id="public-query" class="vk-input" type="search" name="q"
                            value=move || query.get() prop:value=move || query.get()
                            aria-describedby="public-scope" />
                    </label>
                    <div class="vk-public-search__actions">
                        <button class="vk-button vk-button--tonal" type="submit">"Tìm"</button>
                        <Show when=move || !query.get().is_empty()>
                            <a id="clear-public-search" class="vk-button vk-button--text" href="/">"Xóa tìm kiếm"</a>
                        </Show>
                    </div>
                </Form>
            <Suspense fallback=|| view! { <p class="vk-muted" role="status">"Đang tải tác phẩm…"</p> }>
                {move || {
                    let status = status.clone();
                    Suspend::new(async move {
                        match works.await {
                            Ok(list) if list.works.is_empty() => {
                                view! {
                                    <div class="vk-empty" data-testid="public-empty">
                                        <p class="vk-empty__title">"Chưa có tác phẩm nào được công khai."</p>
                                        <p>
                                            "Vink đang ở giai đoạn dựng nền. Hãy bắt đầu một bản nháp ở Bàn viết; việc gửi duyệt và xuất bản sẽ được mở sau."
                                        </p>
                                    </div>
                                }
                                    .into_any()
                            }
                            Ok(list) => {
                                view! { <PublicListView list query /> }
                                    .into_any()
                            }
                            Err(_) => {
                                status.set(503);
                                view! { <Unavailable return_href=explore_href(&query.get_untracked(), None) /> }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>
            </section>
            <aside id="context" class="vk-explore-context" aria-label="Gợi ý viết">
                <section id="writing-prompt" class="vk-writing-prompt" aria-labelledby="prompt-title">
                    <p class="vk-eyebrow">"Một gợi ý để bắt đầu"</p>
                    <h2 id="prompt-title">"Một nơi bạn nhớ"</h2>
                    <p>"Một góc phố, căn phòng hay một người khiến bạn muốn trở về. Bạn sẽ bắt đầu từ chi tiết nào?"</p>
                    <a class="vk-button vk-button--tonal" href="/ban-viet">"Bắt đầu viết"</a>
                </section>
                <p class="vk-reading-note">"Một câu chữ được đọc kỹ cũng là một sự đồng hành."</p>
            </aside>
            </div>
        </div>
    }
}

#[component]
fn PublicListView(list: WorkList, query: Memo<String>) -> impl IntoView {
    let total = list.works.len();
    let visible = Memo::new(move |_| visible_works(&list.works, &query.get()));
    #[cfg(feature = "hydrate")]
    restore_public_position(visible);
    view! {
        <div id="public-results">
            <p class="vk-muted vk-public-scope" role="status">
                {move || format!("{} trong {total} tác phẩm đã tải.", visible.read().len())}
            </p>
            {move || {
                let works = visible.get();
                if works.is_empty() {
                    view! {
                        <div class="vk-empty" data-testid="public-no-results">
                            <p class="vk-empty__title">"Không có tác phẩm khớp trong danh sách này."</p>
                            <p>"Thử tiêu đề, tên tác giả khác hoặc xóa tìm kiếm để xem lại danh sách đã tải."</p>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <ul class="vk-public-list" role="list" aria-describedby="public-scope">
                            {works.into_iter().map(|work| view! { <WorkCardView work query /> }).collect_view()}
                        </ul>
                        <p id="feed-end" class="vk-feed-end">"Bạn đã xem hết danh sách này."</p>
                    }.into_any()
                }
            }}
        </div>
    }
}

#[component]
fn WorkCardView(work: WorkCard, query: Memo<String>) -> impl IntoView {
    let work_id = work.work_id.clone();
    let anchor = work_anchor(&work.work_id);
    let href = Memo::new(move |_| reader_href(&work_id, &query.get()));
    #[cfg(feature = "hydrate")]
    let return_id = work.work_id.clone();
    let avatar = initials(Some(&work.author_display_name));
    view! {
        <li id=anchor class="vk-public-row" data-work-id=work.work_id>
            <p class="vk-public-row__byline">
                <span class="vk-avatar" aria-hidden="true">{avatar}</span>
                <span>
                    <span class="vk-public-row__author">{work.author_display_name}</span>
                    <time datetime=work.published_at.clone()>
                        {display_date(&work.published_at)} " (UTC)"
                    </time>
                </span>
            </p>
            <a class="vk-public-row__link" href=move || href.get() on:click=move |_event| {
                #[cfg(feature = "hydrate")]
                remember_public_position(&_event, &query.get_untracked(), &return_id);
            }>
                <h3 class="vk-public-row__title">{work.title}</h3>
                <span class="vk-public-row__open">"Đọc tác phẩm" <span aria-hidden="true">"→"</span></span>
            </a>
        </li>
    }
}

#[component]
pub fn Reader() -> impl IntoView {
    let params = use_params_map();
    let work_id = Memo::new(move |_| params.read().get("id").unwrap_or_default());
    let query = use_query_map();
    let return_href = Memo::new(move |_| {
        explore_href(
            &query.read().get("q").unwrap_or_default(),
            Some(&work_id.get()),
        )
    });
    let publication = api::publication_resource(work_id);
    let status = ResponseStatus::capture();
    view! {
        <Suspense fallback=|| view! { <p class="vk-muted">"Đang mở tác phẩm…"</p> }>
            {move || {
                let status = status.clone();
                Suspend::new(async move {
                    match publication.await {
                        Ok(publication) => view! { <PublicationView publication /> }.into_any(),
                        Err(LoadError::NotFound) => {
                            status.set(404);
                            view! { <NotFoundContent return_href=return_href.get_untracked() /> }.into_any()
                        }
                        Err(LoadError::Unavailable) => {
                            status.set(503);
                            view! { <Unavailable return_href=return_href.get_untracked() /> }.into_any()
                        }
                    }
                })
            }}
        </Suspense>
    }
}

#[component]
fn PublicationView(publication: Publication) -> impl IntoView {
    let params = use_query_map();
    let id = publication.work_id.clone();
    let return_href =
        Memo::new(move |_| explore_href(&params.read().get("q").unwrap_or_default(), Some(&id)));
    let description = publication
        .document
        .blocks
        .first()
        .and_then(|b| b.lines.first())
        .cloned()
        .unwrap_or_default();
    let canonical = canonical_url(&format!("/tac-pham/{}", publication.work_id));
    view! {
        <Title text=format!("{} — Vink", publication.title) />
        <Meta name="description" content=description.clone() />
        <Meta property="og:title" content=publication.title.clone() />
        <Meta property="og:type" content="article" />
        <Meta property="og:url" content=canonical.clone() />
        <Link rel="canonical" href=canonical />
        <div id="reader" class="vk-reader-page">
        <a id="reader-return" class="vk-reader-return" href=move || return_href.get()>
            <span aria-hidden="true">"← "</span> "Khám phá"
        </a>
        <article id="reader-paper" class="vk-reader" data-revision=publication.revision_id>
            <header id="reader-header" class="vk-reader__header">
                <p class="vk-eyebrow">"Bản công khai"</p>
                <h1 class="vk-reader__title">{publication.title}</h1>
                <p class="vk-reader__byline">
                    {publication.author_display_name} " · "
                    <time datetime=publication.published_at.clone()>
                        {display_date(&publication.published_at)} " (UTC)"
                    </time>
                </p>
            </header>
            <DocumentView document=publication.document />
        </article>
        </div>
    }
}

#[component]
fn Unavailable(#[prop(default = "/".to_string())] return_href: String) -> impl IntoView {
    view! {
        <div class="vk-notice" role="alert" data-testid="public-unavailable">
            <p class="vk-notice__title">"Tạm thời chưa đọc được nội dung."</p>
            <p>"Chưa thể tải nội dung lúc này. Bạn có thể tải lại trang hoặc quay về Khám phá."</p>
            <p><a href="" rel="external">"Thử tải lại"</a></p>
            <a class="vk-button vk-button--tonal" href=return_href>"Về Khám phá"</a>
        </div>
    }
}

/// Mark only the current public history entry. A new tab/modifier click must not move it.
#[cfg(feature = "hydrate")]
fn remember_public_position(event: &web_sys::MouseEvent, query: &str, work_id: &str) {
    if event.default_prevented()
        || event.button() != 0
        || event.meta_key()
        || event.ctrl_key()
        || event.alt_key()
        || event.shift_key()
    {
        return;
    }
    if let Ok(history) = window().history() {
        let state = history.state().unwrap_or(wasm_bindgen::JsValue::NULL);
        let _ =
            history.replace_state_with_url(&state, "", Some(&explore_href(query, Some(work_id))));
    }
}

/// Run after the async list has mounted, including history traversal and initial reload.
#[cfg(feature = "hydrate")]
fn restore_public_position(visible: Memo<Vec<WorkCard>>) {
    use leptos_router::hooks::use_location;
    use wasm_bindgen::JsCast;

    let location = use_location();
    Effect::new(move |_| {
        let hash = location.hash.get();
        let id = hash.trim_start_matches('#');
        if !visible
            .read()
            .iter()
            .any(|work| work_anchor(&work.work_id) == id)
        {
            return;
        }
        request_animation_frame(move || {
            // A stale animation frame must never scroll/focus a different page.
            if window().location().pathname().ok().as_deref() != Some("/")
                || window().location().hash().ok().as_deref() != Some(hash.as_str())
            {
                return;
            }
            if let Some(row) = document().get_element_by_id(hash.trim_start_matches('#')) {
                row.scroll_into_view();
                if let Ok(Some(link)) = row.query_selector("a")
                    && let Ok(link) = link.dyn_into::<web_sys::HtmlElement>()
                {
                    let options = web_sys::FocusOptions::new();
                    options.set_prevent_scroll(true);
                    let _ = link.focus_with_options(&options);
                }
            }
        });
    });
}

#[component]
fn NotFoundContent(#[prop(default = "/".to_string())] return_href: String) -> impl IntoView {
    view! {
        <Title text="Không tìm thấy — Vink" />
        <div class="vk-notice">
            <h1 class="vk-notice__title">"Không tìm thấy trang này."</h1>
            <p>"Tác phẩm có thể chưa được công khai hoặc đường dẫn không đúng."</p>
            <p>
                <a href=return_href>"Về Khám phá"</a>
            </p>
        </div>
    }
}

#[component]
pub fn NotFound() -> impl IntoView {
    ResponseStatus::capture().set(404);
    view! { <NotFoundContent /> }
}
