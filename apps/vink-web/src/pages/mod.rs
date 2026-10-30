mod desk;

pub use desk::{DeskIndex, DraftEditor};

use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::use_params_map;

use crate::api::{self, LoadError};
use crate::canonical_url;
use crate::contract::{Publication, WorkCard};
use crate::document::DocumentView;

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
    view! {
        <Title text="Vink — Việt Ink" />
        <Meta
            name="description"
            content="Vink là không gian viết, đọc và góp ý văn chương tiếng Việt."
        />
        <Link rel="canonical" href=canonical_url("/") />
        <section class="vk-hero" aria-labelledby="hero-title">
            <p class="vk-eyebrow">"Việt Ink"</p>
            <h1 id="hero-title" class="vk-hero__title">
                "Viết chậm, đọc kỹ, lớn lên cùng nhau."
            </h1>
            <p class="vk-hero__lede">
                "Vink là không gian văn chương tiếng Việt: Bàn viết để giữ bản nháp riêng, nơi đọc yên tĩnh cho tác phẩm đã công khai và cộng đồng góp ý tử tế."
            </p>
            <div class="vk-actions">
                <a class="vk-button vk-button--filled" href="/ban-viet">
                    "Mở Bàn viết"
                </a>
            </div>
        </section>
        <section class="vk-section" aria-labelledby="works-title">
            <h2 id="works-title" class="vk-section__title">
                "Tác phẩm công khai"
            </h2>
            <Suspense fallback=|| view! { <p class="vk-muted">"Đang tải tác phẩm…"</p> }>
                {move || {
                    let status = status.clone();
                    Suspend::new(async move {
                        match works.await {
                            Ok(list) if list.works.is_empty() => {
                                view! {
                                    <div class="vk-empty">
                                        <p class="vk-empty__title">"Chưa có tác phẩm nào được công khai."</p>
                                        <p>
                                            "Vink đang ở giai đoạn dựng nền. Hãy bắt đầu một bản nháp ở Bàn viết; việc gửi duyệt và xuất bản sẽ được mở sau."
                                        </p>
                                    </div>
                                }
                                    .into_any()
                            }
                            Ok(list) => {
                                view! {
                                    <ul class="vk-cards" role="list">
                                        {list.works.into_iter().map(|w| view! { <WorkCardView work=w /> }).collect_view()}
                                    </ul>
                                }
                                    .into_any()
                            }
                            Err(_) => {
                                status.set(503);
                                view! { <Unavailable /> }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>
        </section>
    }
}

#[component]
fn WorkCardView(work: WorkCard) -> impl IntoView {
    let href = format!("/tac-pham/{}", work.work_id);
    view! {
        <li class="vk-card">
            <a class="vk-card__link" href=href>
                <span class="vk-card__title">{work.title}</span>
                <span class="vk-card__meta">
                    {work.author_display_name} " · " {display_date(&work.published_at)}
                </span>
            </a>
        </li>
    }
}

#[component]
pub fn Reader() -> impl IntoView {
    let params = use_params_map();
    let work_id = Memo::new(move |_| params.read().get("id").unwrap_or_default());
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
                            view! { <NotFoundContent /> }.into_any()
                        }
                        Err(LoadError::Unavailable) => {
                            status.set(503);
                            view! { <Unavailable /> }.into_any()
                        }
                    }
                })
            }}
        </Suspense>
    }
}

#[component]
fn PublicationView(publication: Publication) -> impl IntoView {
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
        <article class="vk-reader" data-revision=publication.revision_id>
            <header class="vk-reader__header">
                <h1 class="vk-reader__title">{publication.title}</h1>
                <p class="vk-reader__byline">
                    {publication.author_display_name} " · "
                    <time datetime=publication.published_at.clone()>
                        {display_date(&publication.published_at)}
                    </time>
                </p>
            </header>
            <DocumentView document=publication.document />
        </article>
    }
}

#[component]
fn Unavailable() -> impl IntoView {
    view! {
        <div class="vk-notice" role="alert">
            <p class="vk-notice__title">"Tạm thời chưa đọc được nội dung."</p>
            <p>"Máy chủ dữ liệu đang không phản hồi. Hãy thử tải lại sau ít phút."</p>
        </div>
    }
}

#[component]
fn NotFoundContent() -> impl IntoView {
    view! {
        <Title text="Không tìm thấy — Vink" />
        <div class="vk-notice">
            <h1 class="vk-notice__title">"Không tìm thấy trang này."</h1>
            <p>"Tác phẩm có thể chưa được công khai hoặc đường dẫn không đúng."</p>
            <p>
                <a href="/">"Về trang chủ"</a>
            </p>
        </div>
    }
}

#[component]
pub fn NotFound() -> impl IntoView {
    ResponseStatus::capture().set(404);
    view! { <NotFoundContent /> }
}
