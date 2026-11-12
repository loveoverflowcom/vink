mod desk;
mod discovery;
pub(crate) mod moderation;
pub use discovery::{Home, TagDetail};
mod discussion;
mod endorsement;
pub(crate) mod me;
pub(crate) mod work_info;

pub use desk::{DeskIndex, DraftEditor};

use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::use_params_map;

use crate::api::{self, LoadError};
use crate::canonical_url;
use crate::contract::Publication;
#[cfg(feature = "hydrate")]
use crate::contract::WorkCard;
use crate::document::DocumentView;
#[cfg(feature = "hydrate")]
use crate::public_list::work_anchor;
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
    let params = use_params_map();
    let work_id = Memo::new(move |_| params.read().get("id").unwrap_or_default());
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
                        Ok(crate::contract::ReaderResponse::Published(publication)) => {
                            let wid = publication.work_id.clone();
                            view! { <PublicationView publication reader_work_id=wid /> }.into_any()
                        }
                        Ok(crate::contract::ReaderResponse::Hidden { work_id }) => {
                            status.set(200);
                            status.set_no_store();
                            crate::public_reads::invalidate_withdrawn(&work_id);
                            view! { <ReplacementView work_id author_name=None return_href=return_href.get_untracked() /> }.into_any()
                        }
                        Ok(crate::contract::ReaderResponse::ReviewWithdrawn {
                            work_id,
                            author_name,
                        }) => {
                            status.set(200);
                            status.set_no_store();
                            crate::public_reads::invalidate_withdrawn(&work_id);
                            view! {
                                <ReplacementView
                                    work_id=work_id
                                    author_name=Some(author_name)
                                    return_href=return_href.get_untracked()
                                />
                            }
                            .into_any()
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

/// READ-02: the reader never swaps text under the reader's eyes. A changed snapshot is offered
/// ("Xem bản mới") and applied on request; an authoritative 404 removes the text at once.
#[component]
fn PublicationView(publication: Publication, reader_work_id: String) -> impl IntoView {
    let live = public_reads::follow_publication(publication.clone());
    let latest = live.data;
    let removed = live.removed;
    let shown = RwSignal::new(publication);
    let update = Memo::new(move |_| latest.with(|l| shown.with(|s| l != s)));
    let context = discovery::reader_context();
    let not_found_href = move || {
        context
            .get_untracked()
            .href(Some(&shown.with_untracked(|p| p.work_id.clone())))
    };
    let withdrawn_state = RwSignal::new(None::<(String, Option<String>)>);
    #[cfg(feature = "hydrate")]
    {
        let id = reader_work_id.clone();
        Effect::new(move |_| {
            if removed.get() {
                let id = id.clone();
                leptos::task::spawn_local(async move {
                    let replacement = match crate::api::fetch_reader(&id).await {
                        Ok(crate::contract::ReaderResponse::ReviewWithdrawn {
                            work_id,
                            author_name,
                        }) => Some((work_id, Some(author_name))),
                        Ok(crate::contract::ReaderResponse::Hidden { work_id }) => {
                            Some((work_id, None))
                        }
                        _ => None,
                    };
                    if let Some((work_id, author_name)) = replacement {
                        crate::public_reads::invalidate_withdrawn(&work_id);
                        withdrawn_state.try_set(Some((work_id, author_name)));
                    }
                });
            }
        });
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = reader_work_id;
    move || {
        if let Some((work_id, author_name)) = withdrawn_state.get() {
            view! {
                <ReplacementView
                    work_id=work_id
                    author_name=author_name
                    return_href=not_found_href()
                />
            }
            .into_any()
        } else if removed.get() {
            view! { <NotFoundContent return_href=not_found_href() /> }.into_any()
        } else {
            let live = live.clone();
            view! { <PublicationContent publication=shown.get() live update shown /> }.into_any()
        }
    }
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
    let hidden = author_name.is_none();
    let notice = if hidden {
        "Nội dung đã được ẩn"
    } else {
        "Bản này đã bị gỡ sau khi kiểm duyệt."
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
                <Icon name=IconName::ArrowBack /> "Khám phá"
            </vink_ui::Link>
            <article id="reader-paper" class="vk-reader" data-testid=test_id>
                <header id="reader-header" class="vk-reader__header">
                    {author_name.map(|name| view! { <Text role=TypeRole::Caption tone=TextTone::Muted>{name}</Text> })}
                    <Heading level=HeadingLevel::H1 role=TypeRole::Title>
                        {notice}
                    </Heading>
                </header>
            </article>
            <discussion::Discussion work_id=work_id />
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
    let author = adapters::publication_author(&publication);
    let context = discovery::reader_context();
    let id = publication.work_id.clone();
    let return_href = Memo::new(move |_| context.get().href(Some(&id)));
    let description = publication
        .document
        .blocks
        .first()
        .and_then(|b| b.lines.first())
        .cloned()
        .unwrap_or_default();
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
        <Meta name="description" content=description.clone() />
        <Meta property="og:title" content=publication.title.clone() />
        <Meta property="og:type" content="article" />
        <Meta property="og:url" content=canonical.clone() />
        <Link rel="canonical" href=canonical />
        <div id="reader" class="vk-reader-page">
        <vink_ui::Link attr:id="reader-return" variant=vink_ui::LinkVariant::Standalone href=return_href>
            <Icon name=IconName::ArrowBack /> "Khám phá"
        </vink_ui::Link>
        <div id="reader-freshness" class="vk-reader-freshness" data-refreshing=move || refreshing.get().to_string()>
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
            <ReaderHeader title=publication.title author />
            <discovery::PublicTags tags=publication.tags />
            <DocumentView document=publication.document />
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

/// Mark only the current public history entry. A new tab/modifier click must not move it.
#[cfg(feature = "hydrate")]
fn remember_public_position(
    event: &web_sys::MouseEvent,
    context: &crate::discovery::ExploreContext,
    work_id: &str,
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
        let state = history.state().unwrap_or(wasm_bindgen::JsValue::NULL);
        let _ = history.replace_state_with_url(&state, "", Some(&context.href(Some(work_id))));
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
        if id.is_empty() {
            return;
        }
        let found = visible
            .read()
            .iter()
            .any(|work| work_anchor(&work.work_id) == id);
        request_animation_frame(move || {
            // A stale animation frame must never scroll/focus a different page.
            if !window()
                .location()
                .pathname()
                .ok()
                .is_some_and(|p| p == "/" || p.starts_with("/the/"))
                || window().location().hash().ok().as_deref() != Some(hash.as_str())
            {
                return;
            }
            if let Some(row) = document().get_element_by_id(if found {
                hash.trim_start_matches('#')
            } else {
                "works-title"
            }) {
                row.scroll_into_view();
                if let Some(link) = row.query_selector("a").ok().flatten().or(Some(row))
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
