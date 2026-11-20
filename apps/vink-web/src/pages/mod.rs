mod desk;
mod discovery;
pub(crate) mod moderation;
pub use discovery::{Home, TagDetail};
mod discussion;
mod endorsement;
pub(crate) mod follow;
pub(crate) mod handle;
pub(crate) mod me;
pub(crate) mod profile;
pub(crate) mod publication;
pub(crate) mod work_info;

pub use desk::{DeskIndex, DraftEditor};

use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::use_params_map;

use crate::api::{self, LoadError};
use crate::canonical_url;
use crate::contract::Publication;
use crate::document::ReadingDocument;
use crate::public_reads;
use crate::ui::{adapters, patterns::ReaderHeader};
use vink_ui::{
    Heading, HeadingLevel, Icon, IconName, InlineNotice, PanelContent, PanelKind, ProblemKind,
    RecoveryAction, RecoveryIntent, StatePanel, Text, TextTone, Tone, TypeRole,
};

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
    fn redirect(&self, _href: &str) {
        #[cfg(feature = "ssr")]
        if let Some(options) = &self.0
            && let Ok(location) = http::HeaderValue::from_str(_href)
        {
            options.set_status(http::StatusCode::PERMANENT_REDIRECT);
            options.insert_header(http::header::LOCATION, location);
        }
    }

    fn set_no_store(&self) {
        #[cfg(feature = "ssr")]
        if let Some(options) = &self.0 {
            options.insert_header(
                http::header::CACHE_CONTROL,
                http::HeaderValue::from_static("no-store"),
            );
        }
    }
}

#[component]
pub fn Reader() -> impl IntoView {
    // Native Back may reuse an HTML response without validating its bylines.
    // The public API keeps its conditional reads and tab cache.
    ResponseStatus::capture().set_no_store();
    let params = use_params_map();
    let work_id = Memo::new(move |_| params.read().get("id").unwrap_or_default());
    view! { <For each=move || vec![work_id.get()] key=|id| id.clone()
    children=move |id| view! { <ReaderScope id /> } /> }
}

#[component]
fn ReaderScope(id: String) -> impl IntoView {
    let work_id = Memo::new(move |_| id.clone());
    let context = discovery::reader_context();
    let return_href = Memo::new(move |_| context.get().href(Some(&work_id.get())));
    let reader = api::reader_resource(work_id);
    let status = ResponseStatus::capture();
    view! {
        <Suspense fallback=|| view! { <p class="vk-muted" role="status">"Đang mở tác phẩm…"</p> }>
            {move || {
                let status = status.clone();
                Suspend::new(async move {
                    match reader.await {
                        Ok(reader) => {
                            if !matches!(reader, crate::contract::ReaderResponse::Published(_)) {
                                status.set(200);
                            }
                            view! { <ReaderLoaded initial=reader return_href=return_href.get_untracked() /> }.into_any()
                        }
                        Err(LoadError::NotFound) => {
                            status.set(404);
                            view! { <NotFoundContent return_href=return_href.get_untracked() /> }.into_any()
                        }
                        Err(_) => {
                            status.set(503);
                            view! {
                                <Title text="Chưa tải được tác phẩm — Vink" />
                                <Unavailable return_href=return_href.get_untracked() heading=HeadingLevel::H1 />
                            }.into_any()
                        }
                    }
                })
            }}
        </Suspense>
    }
}

/// Control transitions replace the whole private-safe projection; publication changes keep
/// the mounted reading/composer owner until the reader explicitly applies the new revision.
#[component]
fn ReaderLoaded(initial: crate::contract::ReaderResponse, return_href: String) -> impl IntoView {
    use crate::contract::ReaderResponse;
    let id = match &initial {
        ReaderResponse::Published(p) => p.work_id.clone(),
        ReaderResponse::Hidden { work_id } | ReaderResponse::ReviewWithdrawn { work_id, .. } => {
            work_id.clone()
        }
    };
    let live = public_reads::follow_reader(id, initial);
    let data = live.data;
    let removed = live.removed;
    let rejected = live.rejected;
    // Published revision identity is deliberately absent: background updates cannot remount
    // the article, endorsement state or composer and cannot replace a text selection.
    let control = Memo::new(move |_| {
        if removed.get() {
            return (3, None);
        }
        if rejected.get().is_some() {
            return (4, None);
        }
        match data.get() {
            ReaderResponse::Published(_) => (0, None),
            ReaderResponse::Hidden { work_id } => (1, Some((work_id, None))),
            ReaderResponse::ReviewWithdrawn {
                work_id,
                author_name,
            } => (2, Some((work_id, Some(author_name)))),
        }
    });
    move || {
        let (kind, replacement) = control.get();
        match kind {
            0 => {
                let ReaderResponse::Published(publication) = data.get_untracked() else {
                    unreachable!()
                };
                let fallback = publication.clone();
                let publication_live = public_reads::Live {
                    data: Signal::derive(move || match data.get() {
                        ReaderResponse::Published(p) => p,
                        _ => fallback.clone(),
                    }),
                    cached_unverified: live.cached_unverified,
                    refreshing: live.refreshing,
                    failed: live.failed,
                    removed: live.removed,
                    rejected: live.rejected,
                    key: live.key.clone(),
                };
                view! { <PublicationView publication live=publication_live /> }.into_any()
            }
            1 | 2 => {
                let (work_id, author_name) = replacement.unwrap();
                view! { <ReplacementView work_id author_name return_href=return_href.clone() /> }
                    .into_any()
            }
            3 => view! { <NotFoundContent return_href=return_href.clone() /> }.into_any(),
            _ => view! { <Unavailable return_href=return_href.clone() /> }.into_any(),
        }
    }
}

#[component]
fn PublicationView(
    publication: Publication,
    live: public_reads::Live<Publication>,
) -> impl IntoView {
    let latest = live.data;
    let shown = RwSignal::new(publication);
    let update = Memo::new(move |_| {
        latest.with(|l| {
            shown.with(|s| {
                l.revision_id != s.revision_id
                    || l.title != s.title
                    || l.document != s.document
                    || l.tags != s.tags
            })
        })
    });
    move || view! { <PublicationContent publication=shown.get() live=live.clone() update shown /> }
}

/// O-17@r2: the old URL after a moderator or admin rejected the public revision. Only the
/// author's display name, the fixed line and the eligible discussion; never body, title, tags,
/// description or canonical metadata of the removed revision.
#[component]
fn ReplacementView(
    work_id: String,
    author_name: Option<String>,
    return_href: String,
) -> impl IntoView {
    let context = discovery::reader_context();
    // The replacement keeps eligible comments and their public summaries. Its discussion
    // only renders confirmed counts and never exposes endorsement write controls.
    endorsement::Endorsements::provide(work_id.clone(), Callback::new(|()| {}));
    let hidden = author_name.is_none();
    let badge = if hidden {
        "Không hiển thị"
    } else {
        "Gỡ sau kiểm duyệt"
    };
    let notice = if hidden {
        "Nội dung đã được ẩn"
    } else {
        "Bản này đã bị gỡ sau khi kiểm duyệt."
    };
    let detail = if hidden {
        "Tác phẩm này không còn hiển thị với người đọc."
    } else {
        "Bản công khai của tác phẩm đã được rút sau khi kiểm duyệt."
    };
    let state = if hidden { "hidden" } else { "review-withdrawn" };
    let test_id = if hidden {
        "reader-hidden"
    } else {
        "reader-review-withdrawn"
    };
    view! {
        <Title text=format!("{notice} — Vink") />
        <Meta name="robots" content="noindex" />
        <div id="reader" class="vk-reader-page" data-state=state>
            <vink_ui::Link attr:id="reader-return" variant=vink_ui::LinkVariant::Standalone href=return_href>
                <Icon name=IconName::ArrowBack /> {move || if context.get().query.q.is_empty() { "Khám phá" } else { "Về kết quả tìm kiếm" }}
            </vink_ui::Link>
            <article id="reader-paper" class="vk-reader vk-reader--replacement" data-testid=test_id>
                <header id="reader-header" class="vk-reader__header">
                    <p class="vk-reader-replacement-badge">
                        <Icon name=IconName::VisibilityOff size=vink_ui::IconSize::Small />
                        <span>{badge}</span>
                    </p>
                    {author_name.map(|name| view! { <Text role=TypeRole::Caption tone=TextTone::Muted>{name}</Text> })}
                    <Heading level=HeadingLevel::H1 role=TypeRole::Title>
                        {notice}
                    </Heading>
                    <p class="vk-muted">{detail}</p>
                </header>
            </article>
            <discussion::Discussion work_id=work_id read_only=true />
        </div>
    }
}

#[component]
fn PublicationContent(
    publication: Publication,
    live: public_reads::Live<Publication>,
    update: Memo<bool>,
    shown: RwSignal<Publication>,
) -> impl IntoView {
    let latest = live.data;
    let failed = live.failed;
    let refreshing = live.refreshing;
    let cached = live.cached_unverified;
    let author = Memo::new(move |_| latest.with(|p| adapters::publication_author(p, cached.get())));
    let context = discovery::reader_context();
    let id = publication.work_id.clone();
    let return_href = Memo::new(move |_| context.get().href(Some(&id)));
    let document = ReadingDocument::new(&publication.document);
    let description = document.description();
    let title_in_body = document.title_in_body();
    let body = document.render(&publication.title);
    let canonical = canonical_url(&format!("/tac-pham/{}", publication.work_id));
    let discussion_work = publication.work_id.clone();
    // LVE #73: one endorsement state per shown revision; a newer revision reported by the
    // server asks READ-02 to revalidate, which then offers “Xem bản mới”.
    let on_superseded = {
        let live = live.clone();
        Callback::new(move |()| live.refresh())
    };
    endorsement::Endorsements::provide(publication.work_id.clone(), on_superseded);
    let work_summary = api::work_endorsement_resource(publication.work_id.clone());
    let revision = publication.revision_id.clone();
    view! {
        <Title text=format!("{} — Vink", publication.title) />
        <Meta name="description" content=description />
        <Meta property="og:title" content=publication.title.clone() />
        <Meta property="og:type" content="article" />
        <Meta property="og:url" content=canonical.clone() />
        <Link rel="canonical" href=canonical />
        <div id="reader" class="vk-reader-page">
        <vink_ui::Link attr:id="reader-return" variant=vink_ui::LinkVariant::Standalone href=return_href>
            <Icon name=IconName::ArrowBack /> {move || if context.get().query.q.is_empty() { "Khám phá" } else { "Về kết quả tìm kiếm" }}
        </vink_ui::Link>
        <div id="reader-freshness" class="vk-reader-freshness" data-refreshing=move || refreshing.get().to_string()>
            <Show when=move || cached.get() && !failed.get() && !update.get()>
                <p class="vk-muted" data-testid="reader-cached" role="status">"Đang đọc bản đã tải trước đó."</p>
            </Show>
            <Show when=move || update.get()>
                <div data-testid="reader-update">
                    <InlineNotice tone=Tone::Info title="Tác phẩm vừa có bản cập nhật."
                        message="Bạn vẫn đang đọc bản đã mở. Chọn “Xem bản mới” khi muốn chuyển; trang sẽ về đầu tác phẩm."
                        action=RecoveryAction { intent: RecoveryIntent::Reconcile, label: "Xem bản mới".into() }
                        on_recover=move |_| {
                            shown.set(latest.get_untracked());
                            focus_recovery("noi-dung");
                        } />
                </div>
            </Show>
            <Show when=move || failed.get() && !update.get()>
                <div data-testid="reader-refresh-failed">
                    <InlineNotice tone=Tone::Warning
                        message="Chưa kiểm tra được bản mới. Bạn vẫn đang đọc bản đã tải trước đó."
                        action=RecoveryAction { intent: RecoveryIntent::Retry, label: "Thử lại".into() }
                        on_recover={
                            let live = live.clone();
                            move |_| live.refresh()
                        } />
                </div>
            </Show>
        </div>
        <article id="reader-paper" class="vk-reader" data-revision=publication.revision_id>
            <ReaderHeader title=publication.title author=Signal::from(author) title_in_body />
            <discovery::PublicTags tags=publication.tags />
            {body}
        </article>
        <Suspense fallback=|| ()>
            {move || {
                let revision = revision.clone();
                Suspend::new(async move {
                    let summaries = work_summary.await.ok();
                    view! { <endorsement::WorkEndorsement revision_id=revision summaries /> }
                })
            }}
        </Suspense>
        <discussion::Discussion work_id=discussion_work />
        </div>
    }
}

#[component]
fn Unavailable(
    /// Where to leave to. `None` on Explore itself, whose same-URL link would neither leave
    /// nor refetch the list.
    #[prop(optional, into)]
    return_href: Option<String>,
    #[prop(default = HeadingLevel::H2)] heading: HeadingLevel,
) -> impl IntoView {
    let message = if return_href.is_some() {
        "Chưa thể tải nội dung lúc này. Bạn có thể tải lại trang hoặc quay về Khám phá."
    } else {
        "Chưa thể tải danh sách lúc này. Đây không phải danh sách rỗng; bạn có thể tải lại trang."
    };
    let location = leptos_router::hooks::use_location();
    let retry_href = Memo::new(move |_| {
        let mut href = location.pathname.get();
        let search = location.search.get();
        let hash = location.hash.get();
        if !search.is_empty() {
            href.push('?');
            href.push_str(search.trim_start_matches('?'));
        }
        if !hash.is_empty() && hash != "#" {
            href.push('#');
            href.push_str(hash.trim_start_matches('#'));
        }
        href
    });
    view! {
        <div data-testid="public-unavailable" role="alert">
            <StatePanel heading content=PanelContent { kind: PanelKind::Error(ProblemKind::Unavailable), title: "Tạm thời chưa đọc được nội dung.".into(), message: message.into(), actions: vec![], code: None }>
                <vink_ui::Link href=retry_href attr:rel="external" attr:id="retry-public" variant=vink_ui::LinkVariant::Standalone
                    on:click=move |_event: leptos::ev::MouseEvent| {
                        #[cfg(feature = "hydrate")]
                        if _event.button() == 0 && !_event.meta_key() && !_event.ctrl_key() && !_event.shift_key() && !_event.alt_key() {
                            // A same-URL link with a fragment only scrolls. This intent always
                            // reloads the GET document while retaining its query AND fragment.
                            _event.prevent_default();
                            let _ = window().location().reload();
                        }
                    }>"Thử tải lại"</vink_ui::Link>
                {return_href.clone().map(|href| view! {
                    <vink_ui::Link href variant=vink_ui::LinkVariant::Standalone>"Về Khám phá"</vink_ui::Link>
                })}
            </StatePanel>
        </div>
    }
}

/// HTML omits an unnamed submitter from a GET query; `leptos::form::form_data_from_event` appends
/// it as an empty pair, turning `?q=…` into `?q=…&=`. Drop that pair before the router navigates.
fn drop_unnamed_submitter(data: &leptos::web_sys::FormData) {
    data.delete("");
}

/// The recovery control can unmount as its state resolves. Focus an enduring field/heading
/// after that render; never scroll a different route in response to a stale frame.
fn focus_recovery(id: &'static str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        let path = window().location().pathname().ok();
        request_animation_frame(move || {
            if window().location().pathname().ok() == path
                && let Some(element) = document().get_element_by_id(id)
                && let Ok(element) = element.dyn_into::<web_sys::HtmlElement>()
            {
                let _ = element.focus();
            }
        });
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = id;
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen(module = "/profile/navigation.js")]
extern "C" {
    #[wasm_bindgen(js_name = hasProfileReturn)]
    fn has_profile_return() -> bool;
    #[wasm_bindgen(js_name = clearProfileReturn)]
    fn clear_profile_return();
}

/// Mark only the current public history entry. A new tab/modifier click must not move it.
#[cfg(feature = "hydrate")]
fn remember_public_position(
    event: &web_sys::MouseEvent,
    context: &crate::discovery::ExploreContext,
    work_id: &str,
    navigate: &impl Fn(&str, leptos_router::NavigateOptions),
) {
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
        clear_profile_return();
        // Use the router's replace so its path stack also learns the row fragment. A raw
        // replaceState leaves the prior path in Leptos: Back doesn't pop Reader, so opening
        // that same Reader again may render its tree without pushing its URL.
        navigate(
            &context.href(Some(work_id)),
            leptos_router::NavigateOptions {
                replace: true,
                scroll: false,
                state: leptos_router::location::State::new(history.state().ok()),
                ..Default::default()
            },
        );
    }
}

/// Restore a row fragment once for the settled route, not on background list refresh.
#[cfg(feature = "hydrate")]
fn restore_public_position(stream: NodeRef<leptos::html::Section>, settled: Memo<bool>) {
    use leptos_router::hooks::use_location;
    use wasm_bindgen::JsCast;

    let location = use_location();
    Effect::new(move |_| {
        let path = location.pathname.get();
        let query = location.search.get();
        let hash = location.hash.get();
        let id = hash.trim_start_matches('#');
        if id != "community-stream" && !id.starts_with("work-") {
            return;
        }
        if !settled.get() {
            return;
        }
        let Some(stream) = stream.get() else {
            return;
        };
        let align = move || {
            let address = window().location();
            // Own the exact mounted result region and URL. Same row IDs can occur in
            // several queries, even when a stale callback sees the same path and hash.
            // A later author click can intentionally leave that row fragment in place.
            if !stream.is_connected()
                || has_profile_return()
                || address.pathname().ok().as_deref() != Some(path.as_str())
                || address
                    .search()
                    .ok()
                    .as_deref()
                    .map(|s| s.trim_start_matches('?'))
                    != Some(query.as_str())
                || address.hash().ok().as_deref() != Some(hash.as_str())
            {
                return;
            }
            let row = document()
                .get_element_by_id(hash.trim_start_matches('#'))
                .filter(|row| {
                    row.class_list().contains("vk-public-row") && stream.contains(Some(row))
                });
            let anchor = row.as_ref().and_then(|row| {
                row.query_selector(".vk-public-row__link")
                    .ok()
                    .flatten()
                    .and_then(|link| link.dyn_into::<web_sys::HtmlElement>().ok())
            });
            let target = anchor.as_ref().unwrap_or(&stream);
            // The result may arrive after the reader has already focused search or another
            // control. Leave both their focus and viewport alone in that case.
            if document().active_element().is_some_and(|active| {
                active.tag_name() != "BODY"
                    && active.id() != "community-stream"
                    && !row.as_ref().is_some_and(|row| row == &active)
                    && !target.is_same_node(Some(&active))
            }) {
                return;
            }
            if let (Some(row), Some(_)) = (row, anchor.as_ref()) {
                row.scroll_into_view();
            } else {
                stream.scroll_into_view();
            }
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = target.focus_with_options(&options);
        };

        // A newly mounted result can still use the previous viewport's row sizes in its
        // first rendering opportunity. Align once after that layout has been painted.
        let pending = StoredValue::new(None);
        if let Ok(first) = request_animation_frame_with_handle(move || {
            pending.set_value(request_animation_frame_with_handle(align).ok());
        }) {
            pending.set_value(Some(first));
            on_cleanup(move || {
                if let Some(Some(frame)) = pending.try_get_value() {
                    frame.cancel();
                }
            });
        }
    });
}

#[component]
fn NotFoundContent(#[prop(default = "/".to_string())] return_href: String) -> impl IntoView {
    view! {
        <Title text="Không tìm thấy — Vink" />
        <div>
            <StatePanel heading=HeadingLevel::H1 content=PanelContent { kind: PanelKind::Error(ProblemKind::NotFound), title: "Không tìm thấy trang này.".into(), message: "Tác phẩm có thể chưa được công khai hoặc đường dẫn không đúng.".into(), actions: vec![], code: None }>
                <vink_ui::Link href=return_href.clone() variant=vink_ui::LinkVariant::Standalone>"Về Khám phá"</vink_ui::Link>
            </StatePanel>
        </div>
    }
}

#[component]
pub fn NotFound() -> impl IntoView {
    ResponseStatus::capture().set(404);
    view! { <NotFoundContent /> }
}
