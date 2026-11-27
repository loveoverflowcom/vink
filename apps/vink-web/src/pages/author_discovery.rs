//! Actor-free author search inside Khám phá. The hydrated Follow overlay has its own session scope.
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::use_navigate;
use vink_ui::{Avatar, AvatarSize, Heading, HeadingLevel, TypeRole};

use super::{ResponseStatus, focus_recovery};
use crate::api::{self, InvalidRead, LoadError};
use crate::contract::{PublicAuthorPage, PublicAuthorRef};
use crate::discovery::{ExploreContext, ExploreQuery, author_search_href};
use crate::ui::empty_state::{EmptyArtwork, EmptyState};
use crate::ui::patterns::{Author, AuthorIdentity, ProfileLink, profile_label};

#[cfg(feature = "browser")]
mod browser;

const RESULTS_HEADING: &str = "author-search-results-heading";

#[component]
pub(super) fn AuthorSearch(
    q: String,
    cursor: Option<String>,
    clear: Callback<()>,
    refresh: RwSignal<u64>,
) -> impl IntoView {
    let status = ResponseStatus::capture();
    status.set_no_store();
    let q = StoredValue::new(q);
    let url_has_cursor = cursor.is_some();
    let active_cursor = RwSignal::new(cursor);
    let generation = RwSignal::new(0u64);
    let renew = move || generation.update(|value| *value += 1);
    let resource = api::author_search_resource(q, active_cursor, generation);
    #[cfg(feature = "browser")]
    browser::observe(active_cursor, renew);
    #[cfg(feature = "browser")]
    let return_notice = browser::return_notice(resource, active_cursor, generation);
    #[cfg(not(feature = "browser"))]
    let return_notice = RwSignal::new(false);
    #[cfg(feature = "browser")]
    let own_handle = browser::own_handle();
    #[cfg(not(feature = "browser"))]
    let own_handle = Signal::stored(None);

    let canonical = crate::canonical_url(&author_search_href(&q.get_value(), None));
    let navigate = use_navigate();
    let reset = move || {
        return_notice.set(false);
        #[cfg(feature = "browser")]
        super::clear_profile_return();
        if url_has_cursor {
            q.with_value(|q| navigate(&author_search_href(q, None), Default::default()));
        } else {
            active_cursor.set(None);
            renew();
        }
    };
    let restart = Callback::new({
        let reset = reset.clone();
        move |()| {
            reset();
            focus_recovery(RESULTS_HEADING);
        }
    });
    Effect::watch(move || refresh.get(), move |_, _, _| reset(), false);
    let suggestions = q.with_value(|q| q.trim().is_empty());

    view! {
        <Title text="Tìm tác giả · Vink" />
        <Meta name="description" content="Tìm tác giả Vink theo bút danh hoặc @handle." />
        <Link rel="canonical" href=canonical />
        <section aria-labelledby=RESULTS_HEADING class="vk-author-search vk-author-search__results grid [grid-template-columns:minmax(0,1fr)] gap-vk-3 [min-inline-size:0] wrap-anywhere">
            {q.with_value(|q| results_heading(q))}
            <Show when=move || return_notice.get()>
                <p id="author-search-notice" role="status" class="vk-author-search__note">
                    "Một hồ sơ không còn truy cập được; danh sách đã cập nhật."
                </p>
            </Show>
            <Transition fallback=move || search_loading(suggestions)>
                {move || {
                    let status = status.clone();
                    match resource.get() {
                        Some((stamp, result)) if stamp == generation.get() => match result {
                            Ok(page) => view! {
                                <AuthorResults page q cursor=active_cursor.get_untracked() clear own_handle
                                    on_refresh=Callback::new(move |()| renew()) on_restart=restart />
                            }.into_any(),
                            Err(error) => {
                                status.set(if error.is_invalid() { 422 } else { 503 });
                                view! {
                                    <SearchError error q cursor=active_cursor.get_untracked()
                                        on_retry=Callback::new(move |()| renew()) on_restart=restart />
                                }.into_any()
                            }
                        },
                        _ => search_loading(suggestions).into_any(),
                    }
                }}
            </Transition>
        </section>
    }
}

fn results_heading(q: &str) -> impl IntoView + use<> {
    let suggestions = q.trim().is_empty();
    let title = if suggestions {
        "Gợi ý tác giả".into()
    } else {
        format!("Tác giả khớp “{q}”")
    };
    view! {
        <Heading id=RESULTS_HEADING level=HeadingLevel::H2 role=TypeRole::Title attr:tabindex="-1">
            {title}
        </Heading>
        {suggestions.then(|| view! {
            <p id="author-search-note" class="vk-author-search__note">"Ai cũng thấy cùng danh sách gợi ý này."</p>
        })}
    }
}

fn search_loading(suggestions: bool) -> impl IntoView {
    view! {
        <p role="status" id="author-search-loading">
            {if suggestions { "Đang tải gợi ý tác giả…" } else { "Đang tìm tác giả…" }}
        </p>
    }
}

#[component]
fn SearchError(
    error: LoadError,
    q: StoredValue<String>,
    cursor: Option<String>,
    on_retry: Callback<()>,
    on_restart: Callback<()>,
) -> impl IntoView {
    let restart = error == LoadError::Invalid(InvalidRead::RestartCursor);
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
            "Chưa tìm được lúc này",
            "Thử lại",
            q.with_value(|q| author_search_href(q, cursor.as_deref())),
            true,
        ),
    };
    view! {
        <div class="vk-author-search__error grid gap-vk-2" id="author-search-error" role="alert">
            {if can_retry {
                view! {
                    <Heading level=HeadingLevel::H3 role=TypeRole::Title>{message}</Heading>
                    <p>"Kết quả chưa tải xong – chưa thể nói là không có tác giả."</p>
                }.into_any()
            } else {
                view! { <p>{message}</p> }.into_any()
            }}
            <a class="vk-ui-link" href=href on:click=move |event| {
                if restart && ordinary_click(&event) {
                    event.prevent_default();
                    on_restart.run(());
                } else if can_retry && ordinary_click(&event) {
                    event.prevent_default();
                    focus_recovery(RESULTS_HEADING);
                    on_retry.run(());
                }
            }>{label}</a>
        </div>
    }
}

#[component]
fn SearchEmpty(q: StoredValue<String>, clear: Callback<()>) -> impl IntoView {
    let q = q.get_value();
    if q.trim().is_empty() {
        return view! { <div id="author-search-empty" role="status">
            <EmptyState artwork=EmptyArtwork::NoResults message="Chưa có gợi ý tác giả." />
        </div> }
        .into_any();
    }
    let works = ExploreContext {
        query: ExploreQuery::new(q.clone(), Vec::new(), None),
        ..Default::default()
    };
    view! {
        <div id="author-search-empty" role="status">
            <EmptyState artwork=EmptyArtwork::NoResults message=format!("Chưa có tác giả khớp “{q}”. Thử bút danh khác, hoặc nhập chính xác @handle.")>
            <div class="vk-author-search__actions flex flex-wrap justify-center gap-y-vk-1 gap-x-vk-4">
                <a id="author-search-in-works" class="vk-ui-link vk-author-search__in-works" href=works.href(None)>"Tìm trong tác phẩm"</a>
                <a id="author-search-clear" class="vk-ui-link" href=author_search_href("", None)
                    on:click=move |event| {
                        if ordinary_click(&event) {
                            event.prevent_default();
                            clear.run(());
                        }
                    }>"Xóa tìm kiếm"</a>
            </div>
            </EmptyState>
        </div>
    }.into_any()
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
    clear: Callback<()>,
    on_refresh: Callback<()>,
    on_restart: Callback<()>,
    own_handle: Signal<Option<String>>,
) -> impl IntoView {
    let results = RwSignal::new(SearchResults {
        rows: rows(page.authors, cursor.as_deref()),
        next: page.next_cursor,
        appending: false,
        append_error: None,
    });
    #[cfg(feature = "browser")]
    let (append, membership) = browser::enhance(q, results);
    #[cfg(not(feature = "browser"))]
    let _ = (on_refresh, own_handle);

    let has_cursor = cursor.is_some();
    view! {
        <Show when=move || results.with(|state| state.rows.is_empty())>
            <SearchEmpty q clear />
        </Show>
        // R4.2 (#405): separate author cards use bg-content and the card gap.
        <ul class="vk-rel-list vk-author-search__list gap-vk-card-gap" id="author-search-list" role="list">
            <For each=move || results.with(|state| state.rows.clone())
                key=|row| (row.author.handle.clone(), row.author.pen_name.clone())
                children=move |row| {
                    let author = row.author;
                    let handle = author.handle.clone();
                    let id = format!("search-author-{handle}");
                    let identity = q.with_value(|q| search_author_identity(author, id, q));
                    view! {
                        <li class="vk-rel-item vk-author-search__row [background:var(--vk-bg-content)] [border:0] rounded-vk-card [padding:min(var(--vk-space-3),14px)_clamp(16px,4vw,20px)]" data-author-cursor=row.cursor>
                            {identity}
                            {
                                #[cfg(feature = "browser")]
                                { view! { <browser::AuthorFollow handle membership own_handle on_refresh /> } }
                                #[cfg(not(feature = "browser"))]
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
                view! { <div class="vk-author-search__error grid gap-vk-2" role="alert">
                    <p>{message}</p>
                    {restart.then(|| view! { <FirstSearchPage q on_restart /> })}
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
                        #[cfg(feature = "browser")]
                        if ordinary_click(&event) && append() {
                            event.prevent_default();
                        }
                        #[cfg(not(feature = "browser"))]
                        let _ = event;
                    }>
                    {move || results.with(|state| if state.append_error.is_some() {
                        "Thử tải thêm"
                    } else { "Tải thêm tác giả" })}
                </a>
            </Show>
            <Show when=move || has_cursor && !results.with(|state|
                state.append_error == Some(LoadError::Invalid(InvalidRead::RestartCursor)))>
                <FirstSearchPage q on_restart />
            </Show>
        </div>
    }
}

fn search_author_identity(author: PublicAuthorRef, id: String, q: &str) -> AnyView {
    let q = q.trim();
    let handle_query = q.starts_with('@');
    let profile = author.valid_href().map(|href| ProfileLink {
        href: href.into(),
        handle: author.handle.clone(),
    });
    if let Some(profile) = &profile
        && let Some(requested) = q.strip_prefix('@')
        && profile.handle.eq_ignore_ascii_case(requested)
    {
        // AuthorIdentity supports highlighted names. Only an exact Handle hit needs this
        // rendering of the verified link with the mark in the Handle slot.
        let label = profile_label(&author.pen_name, &profile.handle);
        let handle = format!("@{}", profile.handle);
        let blank = author.pen_name.trim().is_empty();
        let name = if blank {
            "Chưa có tên tác giả".into()
        } else {
            author.pen_name.clone()
        };
        let avatar_name = if blank {
            String::new()
        } else {
            author.pen_name
        };
        return view! {
            <div class="vk-ui-byline vk-ui-byline--feed">
                <a class="vk-ui-byline__identity" id=id aria-label=label data-profile-return="true" href=profile.href.clone()>
                    <Avatar name=avatar_name size=AvatarSize::Feed decorative=true />
                    <span class="vk-ui-byline__text">
                        <span class="vk-ui-byline__name vk-ui-identity-name" data-blank=blank.then_some("true")>{name}</span>
                        <span class="vk-ui-byline__handle vk-ui-identity-handle"><mark class="vk-search-mark">{handle}</mark></span>
                    </span>
                </a>
            </div>
        }.into_any();
    }
    let name_view =
        (!handle_query).then(|| super::discovery::highlighted_text(&author.pen_name, q));
    let author = Author {
        profile,
        name: author.pen_name,
        key: None,
        published_at: String::new(),
        neutral: false,
    };
    view! { <AuthorIdentity author id=Some(id) name_view /> }.into_any()
}

#[component]
fn FirstSearchPage(q: StoredValue<String>, on_restart: Callback<()>) -> impl IntoView {
    view! {
        <a id="author-search-first" class="vk-ui-link" href=q.with_value(|q| author_search_href(q, None))
            on:click=move |event| {
                if ordinary_click(&event) {
                    event.prevent_default();
                    on_restart.run(());
                }
            }>"Về trang đầu"</a>
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

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    #[test]
    fn headings_and_loading_labels_follow_the_query() {
        Owner::new().with(|| {
            let suggestions = results_heading("").to_html();
            assert!(suggestions.contains("Gợi ý tác giả"));
            assert!(suggestions.contains("id=\"author-search-note\""));
            assert!(suggestions.contains("Ai cũng thấy cùng danh sách gợi ý này."));

            let matching = results_heading("Bút danh Đỗ").to_html();
            assert!(matching.contains("id=\"author-search-results-heading\""));
            assert!(matching.contains("tabindex=\"-1\""));
            assert!(matching.contains("Tác giả khớp “Bút danh Đỗ”"));
            assert!(!matching.contains("author-search-note"));

            assert!(
                search_loading(true)
                    .to_html()
                    .contains("Đang tải gợi ý tác giả…")
            );
            assert!(
                search_loading(false)
                    .to_html()
                    .contains("Đang tìm tác giả…")
            );
        });
    }

    #[test]
    fn empty_search_links_keep_the_query_in_works_and_clear_in_authors() {
        Owner::new().with(|| {
            let clear = Callback::new(|()| {});
            let q = StoredValue::new("Đỗ & bạn".to_string());
            let missing = view! { <SearchEmpty q clear /> }.to_html();
            assert!(missing.contains("Chưa có tác giả khớp “Đỗ &amp; bạn”."));
            assert!(missing.contains("Thử bút danh khác, hoặc nhập chính xác @handle."));
            assert!(missing.contains("href=\"/?q=%C4%90%E1%BB%97%20%26%20b%E1%BA%A1n\""));
            assert!(missing.contains("href=\"/?mode=authors\""));
            assert!(missing.contains("id=\"author-search-empty\" role=\"status\""));

            q.set_value("  ".into());
            let suggestions = view! { <SearchEmpty q clear /> }.to_html();
            assert!(suggestions.contains("Chưa có gợi ý tác giả."));
            assert!(!suggestions.contains("author-search-clear"));
            assert!(!suggestions.contains("author-search-in-works"));
        });
    }

    #[test]
    fn failure_calls_for_retry_with_the_same_cursor_and_keeps_invalid_recovery() {
        Owner::new().with(|| {
            let render = |error| {
                let q = StoredValue::new("bút danh".to_string());
                let noop = Callback::new(|()| {});
                view! { <SearchError error q cursor=Some("page-2".into()) on_retry=noop on_restart=noop /> }
                    .to_html()
            };
            for error in [LoadError::Unavailable, LoadError::NotFound] {
                let retry = render(error);
                assert!(retry.contains("id=\"author-search-error\" role=\"alert\""));
                assert!(retry.contains("Chưa tìm được lúc này"));
                assert!(retry.contains("Kết quả chưa tải xong – chưa thể nói là không có tác giả."));
                assert!(retry.contains("Thử lại"));
                assert!(retry.contains("&amp;cursor=page-2\""));
                assert!(!retry.contains("author-search-empty"));
            }
            let restart = render(LoadError::Invalid(InvalidRead::RestartCursor));
            assert!(restart.contains("Vị trí trang không còn khớp với kết quả tìm."));
            assert!(restart.contains("Về trang đầu"));
            assert!(!restart.contains("cursor=page-2"));

            let invalid = render(LoadError::Invalid(InvalidRead::Fallback));
            assert!(invalid.contains("Hãy nhập bút danh tối đa 100 ký tự hoặc @handle hiện tại."));
            assert!(invalid.contains("href=\"#author-search-query\""));
            assert!(invalid.contains("Sửa từ khóa"));
        });
    }

    #[test]
    fn exact_handle_mark_preserves_canonical_text_and_accessible_identity() {
        Owner::new().with(|| {
            let render = |q| {
                search_author_identity(
                    PublicAuthorRef {
                        pen_name: "Đỗ Ánh".into(),
                        handle: "do-anh".into(),
                        href: "/@do-anh".into(),
                    },
                    "search-author-do-anh".into(),
                    q,
                )
                .to_html()
            };
            let matching = render(" @DO-ANH ");
            let plain = render("");
            assert!(plain.contains("<span class=\"vk-ui-byline__handle vk-ui-identity-handle\">@do-anh</span>"));
            assert!(matching.contains("<span class=\"vk-ui-byline__name vk-ui-identity-name\">Đỗ Ánh</span>"));
            let marked = "<span class=\"vk-ui-byline__handle vk-ui-identity-handle\"><mark class=\"vk-search-mark\">@do-anh</mark></span>";
            assert!(matching.contains(marked));
            assert_eq!(matching.matches("<mark").count(), 1);
            for html in [&plain, &matching] {
                assert_eq!(html.matches("class=\"vk-ui-byline__handle vk-ui-identity-handle\"").count(), 1);
                assert_eq!(html.matches("<a ").count(), 1);
                assert!(html.contains("aria-label=\"Mở hồ sơ Đỗ Ánh (@do-anh)\""));
                assert!(html.contains("href=\"/@do-anh\""));
                assert!(html.contains("data-profile-return=\"true\""));
                assert!(html.contains("aria-hidden=\"true\""));
            }
            assert!(!render("@do").contains("<mark"));
            assert!(render("do").contains("<mark class=\"vk-search-mark\">Đỗ</mark>"));
        });
    }
}
