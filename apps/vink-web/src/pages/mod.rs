mod desk;
mod discussion;
mod endorsement;
pub(crate) mod me;

pub use desk::{DeskIndex, DraftEditor};

use std::sync::Arc;

use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::components::Form;
use leptos_router::hooks::{use_params_map, use_query_map};

use crate::api::{self, LoadError};
use crate::canonical_url;
use crate::contract::{Publication, WorkCard, WorkList};
use crate::document::DocumentView;
use crate::public_list::{explore_href, reader_href, visible_works, work_anchor};
use crate::public_reads;
use crate::ui::{
    adapters,
    patterns::{ReaderHeader, WorkCard as WorkCardPattern},
};
use vink_ui::{
    EmptyKind, Heading, HeadingLevel, Icon, IconName, InlineNotice, PanelContent, PanelKind,
    ProblemKind, Radius, RecoveryAction, RecoveryIntent, SearchField, SearchVariant, Space,
    StatePanel, Surface, SurfaceTone, Tone, TypeRole,
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
}

#[component]
pub fn Home() -> impl IntoView {
    let works = api::works_resource();
    let status = ResponseStatus::capture();
    let params = use_query_map();
    let query = Memo::new(move |_| params.read().get("q").unwrap_or_default());
    // Editing is raw and local until the native GET form commits it. URL navigation
    // (including Back/Forward and reader return) remains the applied-query owner.
    let buffer = RwSignal::new(query.get_untracked());
    Effect::new(move || buffer.set(query.get()));
    let navigate = leptos_router::hooks::use_navigate();
    let composing = StoredValue::new(false);
    view! {
        <Title text="Khám phá — Vink" />
        <Meta
            name="description"
            content="Vink là không gian viết, đọc và góp ý văn chương tiếng Việt."
        />
        <Link rel="canonical" href=canonical_url("/") />
        <div id="explore" class="vk-explore">
            <div id="discovery-tools" class="vk-discovery-tools">
                <a class="vk-brand" href="/" aria-label="Vink — trang chủ">
                    <span class="vk-brand__mark" aria-hidden="true"></span>
                    <span>"Vink"</span>
                </a>
                <Form method="get" action="/" noscroll=true on_form_data=Arc::new(drop_unnamed_submitter) attr:role="search" attr:class="vk-public-search" attr:id="public-search"
                    on:compositionstart=move |_| composing.set_value(true)
                    on:compositionend=move |_| composing.set_value(false)
                    on:keydown=move |event: leptos::ev::KeyboardEvent| {
                        if event.key() == "Enter" && (composing.get_value() || event.is_composing() || event.key_code() == 229) {
                            event.prevent_default();
                        }
                    }>
                    <SearchField id="public-query" label="Tìm tác phẩm" name="q" variant=SearchVariant::Bar
                        help="Trong danh sách đã tải"
                        value=buffer on_input=move |input: vink_ui::TextInput| buffer.set(input.value)
                        clear_href="/" clear_id="clear-public-search" on_clear=move |_| {
                            navigate("/", leptos_router::NavigateOptions { scroll: false, ..Default::default() });
                            focus_recovery("public-query");
                        } />
                </Form>
            </div>
            <header id="explore-heading" class="vk-explore__heading">
                <Heading level=HeadingLevel::H1 role=TypeRole::Display>"Khám phá"</Heading>
                <p>"Đọc một câu chuyện, góp một ý hay, rồi viết tiếp."</p>
            </header>
            <div class="vk-discover-grid">
            <section id="community-stream" aria-labelledby="works-title">
                <Heading level=HeadingLevel::H2 role=TypeRole::Title id="works-title">"Tác phẩm công khai"</Heading>
            <Suspense fallback=|| view! { <p class="vk-muted" role="status">"Đang tải tác phẩm…"</p> }>
                {move || {
                    let status = status.clone();
                    Suspend::new(async move {
                        match works.await {
                            Ok(list) => view! { <PublicCollection list query /> }.into_any(),
                            Err(_) => {
                                status.set(503);
                                // Explore is already the return route: only the reload recovers here.
                                view! { <Unavailable /> }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>
            </section>
            <aside id="context" class="vk-explore-context" aria-label="Gợi ý viết">
                <section id="writing-prompt" class="vk-writing-prompt" aria-labelledby="prompt-title">
                    <Surface tone=SurfaceTone::Primary radius=Radius::Lg padding=Space::S4>
                    <p class="vk-eyebrow">"Một gợi ý để bắt đầu"</p>
                    <h2 id="prompt-title">"Một nơi bạn nhớ"</h2>
                    <p>"Một góc phố, căn phòng hay một người khiến bạn muốn trở về. Bạn sẽ bắt đầu từ chi tiết nào?"</p>
                    <vink_ui::Link href="/ban-viet" variant=vink_ui::LinkVariant::Standalone>"Bắt đầu viết"</vink_ui::Link>
                    </Surface>
                </section>
                <p class="vk-reading-note">"Một câu chữ được đọc kỹ cũng là một sự đồng hành."</p>
            </aside>
            </div>
        </div>
    }
}

/// READ-02: the collection follows the tab's public snapshot. A refresh swaps cards in place
/// (stable keys keep rows, focus and scroll); a failed refresh keeps the last success visible.
#[component]
fn PublicCollection(list: WorkList, query: Memo<String>) -> impl IntoView {
    let live = public_reads::follow_works(list);
    let works = live.data;
    let refreshing = live.refreshing;
    let failed = live.failed;
    let empty = Memo::new(move |_| works.read().works.is_empty());
    view! {
        <div id="public-collection" aria-busy=move || refreshing.get().to_string()
            data-refreshing=move || refreshing.get().to_string()>
            <Show when=move || failed.get()>
                <div data-testid="public-refresh-failed">
                    <InlineNotice tone=Tone::Warning
                        message="Chưa làm mới được danh sách. Bạn vẫn đang xem bản đã tải trước đó."
                        action=RecoveryAction { intent: RecoveryIntent::Retry, label: "Thử lại".into() }
                        on_recover={
                            let live = live.clone();
                            move |_| live.refresh()
                        } />
                </div>
            </Show>
            {move || if empty.get() {
                view! {
                    <div data-testid="public-empty">
                        <StatePanel content=PanelContent { kind: PanelKind::Empty(EmptyKind::FirstUse), title: "Chưa có tác phẩm nào được công khai.".into(), message: "Vink đang ở giai đoạn dựng nền. Hãy bắt đầu một bản nháp ở Bàn viết; việc gửi duyệt và xuất bản sẽ được mở sau.".into(), actions: vec![], code: None }>
                            <vink_ui::Link href="/ban-viet" variant=vink_ui::LinkVariant::Standalone>"Về Bàn viết"</vink_ui::Link>
                        </StatePanel>
                    </div>
                }.into_any()
            } else {
                view! { <PublicListView works query /> }.into_any()
            }}
        </div>
    }
}

#[component]
fn PublicListView(works: Signal<WorkList>, query: Memo<String>) -> impl IntoView {
    let total = Memo::new(move |_| works.read().works.len());
    let visible = Memo::new(move |_| visible_works(&works.read().works, &query.get()));
    #[cfg(feature = "hydrate")]
    restore_public_position(visible);
    view! {
        <div id="public-results">
            <p class="vk-muted vk-public-scope" role="status" aria-atomic="true">
                {move || if query.get().trim().is_empty() {
                    format!("Đã tải {} tác phẩm · tối đa 12.", total.get())
                } else {
                    format!("{} trong {} tác phẩm đã tải · tối đa 12.", visible.read().len(), total.get())
                }}
            </p>
            {move || {
                let works = visible.get();
                if works.is_empty() {
                    view! {
                        <div data-testid="public-no-results">
                            <StatePanel content=PanelContent { kind: PanelKind::Empty(EmptyKind::NoResults), title: "Không tìm thấy trong danh sách đã tải.".into(), message: "Thử tiêu đề, tên tác giả khác hoặc xóa tìm kiếm.".into(), actions: vec![], code: None }>
                                <vink_ui::Link href="/" attr:id="clear-public-results" variant=vink_ui::LinkVariant::Standalone
                                    on:click=move |event: leptos::ev::MouseEvent| {
                                        if event.button() == 0 && !event.meta_key() && !event.ctrl_key() && !event.shift_key() && !event.alt_key() {
                                            focus_recovery("public-query");
                                        }
                                    }>"Xóa tìm kiếm"</vink_ui::Link>
                            </StatePanel>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <ul class="vk-public-list" role="list" aria-describedby="public-query-help">
                            <For each=move || works.clone() key=|work| work.work_id.clone() children=move |work| view! { <WorkCardView work query /> } />
                        </ul>
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
    let author = adapters::card_author(&work);
    view! {
        <li id=anchor class="vk-public-row" data-work-id=work.work_id>
            <WorkCardPattern title=work.title author href on_open=move |_event| {
                #[cfg(feature = "hydrate")]
                remember_public_position(&_event, &query.get_untracked(), &return_id);
            } />
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
        <Suspense fallback=|| view! { <p class="vk-muted" role="status">"Đang mở tác phẩm…"</p> }>
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
fn PublicationView(publication: Publication) -> impl IntoView {
    let live = public_reads::follow_publication(publication.clone());
    let latest = live.data;
    let removed = live.removed;
    let shown = RwSignal::new(publication);
    let update = Memo::new(move |_| latest.with(|l| shown.with(|s| l != s)));
    let params = use_query_map();
    let not_found_href = move || {
        explore_href(
            &params.read_untracked().get("q").unwrap_or_default(),
            Some(&shown.with_untracked(|p| p.work_id.clone())),
        )
    };
    move || {
        if removed.get() {
            view! { <NotFoundContent return_href=not_found_href() /> }.into_any()
        } else {
            let live = live.clone();
            view! { <PublicationContent publication=shown.get() live update shown /> }.into_any()
        }
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
