//! Actor-free author search inside Khám phá. The hydrated Follow overlay has its own session scope.
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::use_navigate;
use vink_ui::{Heading, HeadingLevel, HiddenField, SearchField, SearchVariant, TypeRole};

use super::{ResponseStatus, focus_recovery};
use crate::api::{self, InvalidRead, LoadError};
use crate::contract::{PublicAuthorPage, PublicAuthorRef};
use crate::discovery::encode;
use crate::ui::patterns::{Author, AuthorIdentity, ProfileLink};

#[cfg(feature = "hydrate")]
mod browser;

const RESULTS_HEADING: &str = "author-search-results-heading";

pub(super) fn author_search_href(q: &str, cursor: Option<&str>) -> String {
    let mut href = "/?mode=authors".to_owned();
    if !q.trim().is_empty() {
        href.push_str("&q=");
        href.push_str(&encode(q));
    }
    if let Some(cursor) = cursor {
        href.push_str("&cursor=");
        href.push_str(&encode(cursor));
    }
    href
}

#[component]
pub(super) fn AuthorSearch(
    q: String,
    cursor: Option<String>,
    #[prop(into)] modes: ViewFn,
) -> impl IntoView {
    let status = ResponseStatus::capture();
    status.set_no_store();
    let buffer = RwSignal::new(q.clone());
    let q = StoredValue::new(q);
    let active_cursor = RwSignal::new(cursor);
    let generation = RwSignal::new(0u64);
    let composing = RwSignal::new(false);
    let renew = move || generation.update(|value| *value += 1);
    let resource = api::author_search_resource(q, active_cursor, generation);
    #[cfg(feature = "hydrate")]
    browser::observe(active_cursor, renew);

    let canonical = crate::canonical_url(&author_search_href(&q.get_value(), None));
    let clear_href = author_search_href("", None);
    let navigate = use_navigate();
    let on_clear = {
        let navigate = navigate.clone();
        move |_| {
            buffer.set(String::new());
            navigate(&clear_href, Default::default());
            focus_recovery("author-search-query");
        }
    };
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if composing.get_untracked() {
            return;
        }
        let entered = buffer.get_untracked();
        if entered == q.get_value() && active_cursor.get_untracked().is_none() {
            renew();
        } else {
            navigate(&author_search_href(&entered, None), Default::default());
        }
        focus_recovery(RESULTS_HEADING);
    };
    let suggestions = q.with_value(|q| q.trim().is_empty());

    view! {
        <Title text="Tìm tác giả · Vink" />
        <Meta name="description" content="Tìm tác giả Vink theo bút danh hoặc @handle." />
        <Link rel="canonical" href=canonical />
        <div id="explore" class="vk-explore vk-author-search">
            {modes.run()}
            <Heading level=HeadingLevel::H1 role=TypeRole::Display>"Tìm tác giả"</Heading>
            <form method="get" action="/" role="search" id="author-search"
                on:compositionstart=move |_| composing.set(true)
                on:compositionend=move |_| composing.set(false)
                on:submit=submit>
                <HiddenField name="mode" value=Signal::stored("authors") />
                <SearchField id="author-search-query" label="Tìm bút danh hoặc @handle"
                    placeholder="Bút danh hoặc @handle" name="q" variant=SearchVariant::Bar
                    value=buffer on_input=move |input: vink_ui::TextInput| buffer.set(input.value)
                    clear_href=author_search_href("", None) on_clear />
            </form>
            <section aria-labelledby=RESULTS_HEADING class="vk-author-search__results">
                <Heading id=RESULTS_HEADING level=HeadingLevel::H2 role=TypeRole::Title attr:tabindex="-1">
                    {if suggestions { "Gợi ý tác giả" } else { "Kết quả tìm tác giả" }}
                </Heading>
                <Transition fallback=search_loading>
                    {move || {
                        let status = status.clone();
                        match resource.get() {
                            Some((stamp, result)) if stamp == generation.get() => match result {
                                Ok(page) => view! {
                                    <AuthorResults page q cursor=active_cursor.get_untracked()
                                        on_refresh=Callback::new(move |()| renew()) />
                                }.into_any(),
                                Err(error) => {
                                    status.set(if error.is_invalid() { 422 } else { 503 });
                                    view! {
                                        <SearchError error q cursor=active_cursor.get_untracked()
                                            on_retry=Callback::new(move |()| renew()) />
                                    }.into_any()
                                }
                            },
                            _ => search_loading().into_any(),
                        }
                    }}
                </Transition>
            </section>
        </div>
    }
}

fn search_loading() -> impl IntoView {
    view! { <p role="status" id="author-search-loading">"Đang tải danh sách tác giả…"</p> }
}

#[component]
fn SearchError(
    error: LoadError,
    q: StoredValue<String>,
    cursor: Option<String>,
    on_retry: Callback<()>,
) -> impl IntoView {
    let (message, label, href, can_retry) = match error {
        LoadError::Invalid(InvalidRead::RestartCursor) => (
            "Vị trí trang không còn khớp với kết quả tìm.",
            "Về trang đầu",
            q.with_value(|q| author_search_href(q, None)),
            false,
        ),
        LoadError::Invalid(_) => (
            "Hãy nhập bút danh tối đa 100 ký tự hoặc @handle hiện tại.",
            "Sửa từ khóa",
            "#author-search-query".into(),
            false,
        ),
        LoadError::Unavailable | LoadError::NotFound => (
            "Chưa tải được danh sách tác giả.",
            "Thử lại",
            q.with_value(|q| author_search_href(q, cursor.as_deref())),
            true,
        ),
    };
    view! {
        <div class="vk-author-search__error">
            <p id="author-search-error" role="alert">{message}</p>
            <a class="vk-ui-link" href=href on:click=move |event| {
                if can_retry && ordinary_click(&event) {
                    event.prevent_default();
                    focus_recovery(RESULTS_HEADING);
                    on_retry.run(());
                }
            }>{label}</a>
        </div>
    }
}

fn ordinary_click(event: &leptos::ev::MouseEvent) -> bool {
    !event.default_prevented()
        && event.button() == 0
        && !(event.ctrl_key() || event.meta_key() || event.shift_key() || event.alt_key())
}

#[derive(Clone)]
struct AuthorRow {
    author: PublicAuthorRef,
    cursor: Option<String>,
}

#[derive(Clone)]
struct SearchResults {
    rows: Vec<AuthorRow>,
    next: Option<String>,
    appending: bool,
    append_error: Option<LoadError>,
}

#[component]
fn AuthorResults(
    page: PublicAuthorPage,
    q: StoredValue<String>,
    cursor: Option<String>,
    on_refresh: Callback<()>,
) -> impl IntoView {
    let results = RwSignal::new(SearchResults {
        rows: rows(page.authors, cursor.as_deref()),
        next: page.next_cursor,
        appending: false,
        append_error: None,
    });
    #[cfg(feature = "hydrate")]
    let (append, membership) = browser::enhance(q, results);
    #[cfg(not(feature = "hydrate"))]
    let _ = on_refresh;

    let has_cursor = cursor.is_some();
    let first_page = q.with_value(|q| author_search_href(q, None));
    view! {
        <Show when=move || results.with(|state| state.rows.is_empty())>
            <p id="author-search-empty" role="status">{
                if q.with_value(|q| q.trim().is_empty()) {
                    "Chưa có gợi ý tác giả."
                } else {
                    "Không tìm thấy tác giả phù hợp."
                }
            }</p>
        </Show>
        <ul class="vk-rel-list vk-author-search__list" id="author-search-list" role="list">
            <For each=move || results.with(|state| state.rows.clone())
                key=|row| (row.author.handle.clone(), row.author.pen_name.clone())
                children=move |row| {
                    let author = row.author;
                    let handle = author.handle.clone();
                    let id = format!("search-author-{handle}");
                    let name = q.with_value(|q| super::discovery::highlighted_text(&author.pen_name, q));
                    let byline = Author {
                        profile: author.valid_href().map(|href| ProfileLink {
                            href: href.into(), handle: handle.clone(),
                        }),
                        name: author.pen_name,
                        key: None,
                        published_at: String::new(),
                        neutral: false,
                    };
                    view! {
                        <li class="vk-rel-item vk-author-search__row" data-author-cursor=row.cursor>
                            <AuthorIdentity author=byline id=Some(id) name_view=Some(name) />
                            {
                                #[cfg(feature = "hydrate")]
                                { view! { <browser::AuthorFollow handle membership on_refresh /> } }
                                #[cfg(not(feature = "hydrate"))]
                                { let _ = handle; }
                            }
                        </li>
                    }
                }
            />
        </ul>
        <div class="vk-rel-pagination">
            {move || results.with(|state| state.append_error.as_ref().map(|error| {
                let (message, restart) = match error {
                    LoadError::Invalid(InvalidRead::RestartCursor) =>
                        ("Danh sách đã thay đổi. Hãy tìm lại từ trang đầu.", true),
                    _ => ("Chưa tải thêm được tác giả.", false),
                };
                view! { <div class="vk-author-search__error" role="alert">
                    <p>{message}</p>
                    {restart.then(|| view! { <a class="vk-ui-link" href=q.with_value(|q| author_search_href(q, None))>"Về trang đầu"</a> })}
                </div> }
            }))}
            <Show when=move || results.with(|state| state.appending)>
                <p role="status">"Đang tải thêm tác giả…"</p>
            </Show>
            <Show when=move || results.with(|state| state.next.is_some())>
                <a id="author-search-more" class="vk-ui-link"
                    aria-disabled=move || results.with(|state| state.appending).then_some("true")
                    href=move || results.with(|state| q.with_value(|q| author_search_href(q, state.next.as_deref())))
                    on:click=move |event| {
                        #[cfg(feature = "hydrate")]
                        if ordinary_click(&event) && append() {
                            event.prevent_default();
                        }
                        #[cfg(not(feature = "hydrate"))]
                        let _ = event;
                    }>
                    {move || results.with(|state| if state.append_error.is_some() {
                        "Thử tải thêm"
                    } else { "Tải thêm tác giả" })}
                </a>
            </Show>
            {has_cursor.then(|| view! { <a class="vk-ui-link" href=first_page>"Về trang đầu"</a> })}
        </div>
    }
}

fn rows(authors: Vec<PublicAuthorRef>, cursor: Option<&str>) -> Vec<AuthorRow> {
    authors
        .into_iter()
        .filter(|author| author.valid_href().is_some())
        .map(|author| AuthorRow {
            author,
            cursor: cursor.map(str::to_owned),
        })
        .collect()
}
