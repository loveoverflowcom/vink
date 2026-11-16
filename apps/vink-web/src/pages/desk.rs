//! Bàn viết: private drafts, loaded only in the browser through the authenticated API.
//! Server rendering emits the shell and a loading state, never draft content.

use leptos::html;
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params_map;

use vink_ui::{
    ActionState, ActionSurface, Announce, Button, ButtonKind, ButtonVariant, ChoiceOption,
    EmptyKind, FieldVariant, Heading, HeadingLevel, Icon, IconName, InlineNotice, PanelContent,
    PanelKind, PlainTooltip, ProblemKind, SearchField, SearchVariant, Select, StatePanel,
    SupportingDetails, TextField, Tone, TooltipIconButton, TypeRole,
};

use crate::contract::{ApiError, DocumentDto, Draft};
use crate::desk_state::{
    DraftSession, DraftTicket, Failure, LeaveGuard, LeaveNotice, LeaveStage, SaveStatus, SaveTicket,
};
use crate::document::DocumentView;
use crate::draft_list::{CreateState, DraftSort, display_title as untitled};
use crate::editor::{BODY_HINT_ID, BODY_LABEL_ID, BodyEditor, BodySource, BodyState, TOOLBAR_ID};
use crate::navigation::NavigationGuard;
use crate::pages::work_info::TagLoad;
use crate::tag_state::{TagStatus, TagTicket};
use crate::ui::{adapters, patterns::AuthorWorkRow};

const DESK_UNAVAILABLE_SSR: &str = "Bàn viết cần JavaScript để mở bản nháp riêng.";

#[component]
pub fn DeskIndex() -> impl IntoView {
    use crate::draft_list::paging::{DraftPages, PAGE_SIZE};
    use crate::read_lifecycle::{Intent, ReadOwner};
    let query = RwSignal::new(String::new());
    let applied = RwSignal::new(String::new());
    let sort = RwSignal::new(DraftSort::Recent);
    let composing = RwSignal::new(false);
    let creation = RwSignal::new(CreateState::Ready);
    let loading = RwSignal::new(true);
    let appending = RwSignal::new(false);
    let recovery_focus = RwSignal::new(false);
    let ready = RwSignal::new(false);
    let pages = RwSignal::new(DraftPages::default());
    let load_error = RwSignal::new(None::<ApiError>);
    let page_error = RwSignal::new(None::<ApiError>);
    let restarted = RwSignal::new(false);
    let session = crate::account::Session::current();
    let listed_scope = StoredValue::new(None::<(Option<String>, String, DraftSort, usize)>);
    let reads = StoredValue::new(ReadOwner::new((
        None::<String>,
        String::new(),
        DraftSort::Recent,
        PAGE_SIZE,
    )));
    let wake = StoredValue::new(crate::read_events::WakePolicy::default());
    #[cfg(feature = "hydrate")]
    let transport_owner = StoredValue::new(Owner::current().map(|o| o.downgrade()));
    let read_list = Callback::new(move |intent: Intent| {
        #[cfg(feature = "hydrate")]
        {
            if creation.get_untracked() == CreateState::Pending
                || session.standing.get_untracked() == crate::session::Standing::Unknown
            {
                return;
            }
            let cursor = if intent == Intent::Append {
                pages.with_untracked(|p| p.chain.next_cursor.clone())
            } else {
                None
            };
            if intent == Intent::Append && cursor.is_none() {
                return;
            }
            let Some(Some(ticket)) = reads.try_update_value(|r| r.begin(intent, cursor)) else {
                return;
            };
            let Some(owner) = transport_owner.get_value().and_then(|o| o.upgrade()) else {
                return;
            };
            wake.update_value(|w| w.started());
            loading.set(true);
            appending.set(intent == Intent::Append);
            if intent == Intent::Append {
                page_error.set(None);
            } else {
                load_error.set(None);
            }
            owner.with(|| {
                leptos::task::spawn_local_scoped_with_cancellation(async move {
                    let result = crate::api::search_author_works(
                        &ticket.scope.1,
                        ticket.scope.2,
                        ticket.scope.3,
                        ticket.cursor.as_deref(),
                    )
                    .await;
                    if reads.try_with_value(|r| r.accepts(&ticket)) != Some(true)
                        || session.scope.try_get_untracked() != Some(ticket.scope.0.clone())
                    {
                        return;
                    }
                    let next_part =
                        intent == Intent::Append && pages.with_untracked(DraftPages::next_part);
                    let old_tail = pages.with_untracked(|p| p.chain.pages > 1 || p.part > 1);
                    let anchor = (intent == Intent::Refresh)
                        .then(capture_desk_anchor)
                        .flatten();
                    let result = result.and_then(|page| {
                        if pages.try_update(|p| reads.with_value(|r| p.accept(r, &ticket, page)))
                            != Some(true)
                        {
                            return Err(ApiError::Invalid {
                                field: Some("cursor".into()),
                            });
                        }
                        Ok(())
                    });
                    if reads.try_update_value(|r| r.finish(&ticket)) != Some(true) {
                        return;
                    }
                    let success = result.is_ok();
                    let cursor_recovery = intent == Intent::Append
                        && matches!(&result, Err(ApiError::Invalid { .. }))
                        && document()
                            .active_element()
                            .is_some_and(|element| element.id() == "load-more-drafts");
                    wake.try_update_value(|w| w.settled(crate::read_events::now(), success));
                    match result {
                        Ok(()) => {
                            ready.try_set(true);
                            load_error.try_set(None);
                            page_error.try_set(None);
                            if intent != Intent::Append {
                                creation.try_update(CreateState::reconciled);
                                restarted.try_set(old_tail);
                            }
                        }
                        Err(error) => {
                            if matches!(
                                error,
                                ApiError::Unauthenticated
                                    | ApiError::Forbidden
                                    | ApiError::NotFound
                            ) {
                                pages.try_set(DraftPages::default());
                                ready.try_set(false);
                                page_error.try_set(None);
                                load_error.try_set(Some(error));
                            } else if intent == Intent::Append {
                                page_error.try_set(Some(error));
                            } else {
                                load_error.try_set(Some(error));
                            }
                        }
                    }
                    loading.try_set(false);
                    appending.try_set(false);
                    let recover = recovery_focus.try_update(std::mem::take).unwrap_or(false);
                    if cursor_recovery {
                        super::focus_recovery("restart-drafts");
                    } else if success {
                        if next_part {
                            super::focus_recovery("draft-list-title");
                        } else if recover {
                            super::focus_recovery("draft-search");
                        } else if let Some(anchor) = anchor {
                            restore_desk_anchor(anchor);
                        }
                    }
                })
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = intent;
    });
    #[cfg(feature = "hydrate")]
    {
        Effect::new(move |_| {
            let account = session.scope.get();
            if session.standing.get() == crate::session::Standing::Unknown {
                return;
            }
            let old = listed_scope.get_value();
            if old.as_ref().is_some_and(|old| old.0 != account) {
                query.set(String::new());
                applied.set(String::new());
                sort.set(DraftSort::Recent);
                composing.set(false);
                ready.set(false);
                creation.set(CreateState::Ready);
            }
            let scope = (account, applied.get(), sort.get(), PAGE_SIZE);
            if old.as_ref() == Some(&scope) {
                return;
            }
            listed_scope.set_value(Some(scope.clone()));
            reads.update_value(|r| r.change_scope(scope));
            pages.set(DraftPages::default());
            load_error.set(None);
            page_error.set(None);
            restarted.set(false);
            read_list.run(Intent::Initial);
        });
        crate::read_events::observe(move || {
            if crate::read_events::visible()
                && wake.with_value(|w| w.due(crate::read_events::now()))
            {
                read_list.run(Intent::Refresh);
            }
        });
        on_cleanup(move || {
            reads.try_update_value(|r| r.dispose());
        });
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (session, listed_scope, reads, wake, recovery_focus);
    let refresh_disabled = Signal::derive(move || {
        (loading.get() && !appending.get()) || creation.get() == CreateState::Pending
    });
    let refresh = Callback::new(move |()| {
        if refresh_disabled.get_untracked() {
            return;
        }
        recovery_focus.set(
            load_error.get_untracked().is_some()
                || page_error.get_untracked().is_some()
                || creation.get_untracked() == CreateState::Uncertain,
        );
        read_list.run(Intent::Refresh);
    });
    let more = Callback::new(move |()| {
        if !loading.get_untracked() {
            read_list.run(Intent::Append);
        }
    });
    let submit = Callback::new(move |()| {
        if composing.get_untracked() {
            return;
        }
        let text = query.get_untracked();
        if text == applied.get_untracked() {
            read_list.run(Intent::Refresh);
        } else {
            applied.set(text);
        }
    });
    let clear = Callback::new(move |()| {
        if composing.get_untracked() {
            return;
        }
        query.set(String::new());
        if applied.get_untracked().is_empty() {
            read_list.run(Intent::Refresh);
        } else {
            applied.set(String::new());
        }
        super::focus_recovery("draft-search");
    });
    let can_create = Signal::derive(move || {
        !loading.get()
            && ready.get()
            && load_error.get().is_none()
            && creation.with(CreateState::can_begin)
    });
    #[cfg(feature = "hydrate")]
    let navigate = leptos_router::hooks::use_navigate();
    let create = Callback::new(move |()| {
        #[cfg(feature = "hydrate")]
        {
            if !can_create.get_untracked()
                || !creation.try_update(CreateState::begin).unwrap_or(false)
            {
                return;
            }
            let navigate = navigate.clone();
            let scope = session.scope.get_untracked();
            leptos::task::spawn_local(async move {
                let outcome = crate::api::create_draft().await;
                // The route owner disposes this signal. A late reply must not navigate from a
                // different page (and an uncertain POST is never automatically repeated). A
                // reply for an earlier session is dropped as well: its draft is not this desk's.
                if creation.try_get_untracked().is_none()
                    || session.scope.try_get_untracked() != Some(scope)
                {
                    return;
                }
                match outcome {
                    Ok(draft) => {
                        navigate(&format!("/ban-viet/{}", draft.work_id), Default::default())
                    }
                    Err(error) => {
                        creation.update(|state| state.failed(error));
                    }
                }
            });
        }
    });
    // Only an authoritative, unfiltered head can describe an empty corpus. A filtered
    // miss (or a failed first read) says nothing about whether this owner has drafts.
    let first_use = Signal::derive(move || {
        ready.get()
            && load_error.get().is_none()
            && crate::draft_list::search_key(&applied.get()).is_empty()
            && pages.with(|p| p.loaded && p.part == 1 && p.drafts.is_empty())
    });
    view! {
        <Title text="Bàn viết — Vink" />
        <Meta name="robots" content="noindex" />
        <section id="desk" class="vk-desk vk-desk--index" aria-labelledby="desk-title">
            <header id="heading" class="vk-desk-heading">
                <Heading level=HeadingLevel::H1 id="desk-title" attr:tabindex="-1">"Bàn viết"</Heading>
                <Show when=move || ready.get() && !first_use.get()>
                    <CreateDraftButton creation can_create create />
                </Show>
            </header>
            <noscript><p class="vk-desk-noscript">{DESK_UNAVAILABLE_SSR}</p></noscript>
            <CreateNotice creation loading refresh />
            <div aria-busy=move || if loading.get() { "true" } else { "false" } data-testid="draft-content">
                <Show when=move || !ready.get() && loading.get()>
                    <p class="vk-desk-loading vk-muted" role="status" data-testid="draft-loading">"Đang mở Bàn viết…"</p>
                </Show>
                {move || load_error.get().map(|error| view! {
                    <DeskError error refresh retry_disabled=refresh_disabled retained=pages.with(|p| p.loaded) />
                })}
                // The tools/input retain one owner through loading, empty, query and error.
                <DraftListView pages query sort composing loading appending page_error load_error restarted
                    first_use creation can_create create submit clear more refresh refresh_disabled />
            </div>
            <p class="vk-desk-privacy vk-muted">"Bản nháp chỉ bạn đọc được. Số Thấm và Nhận xét thuộc bản đang công khai; lưu nháp không gửi duyệt hay công khai."</p>
        </section>
    }
}

#[cfg(feature = "hydrate")]
fn capture_desk_anchor() -> Option<(String, f64)> {
    use wasm_bindgen::JsCast;
    let rows = document().query_selector_all(".vk-draft-row").ok()?;
    for i in 0..rows.length() {
        let row = rows.item(i)?.dyn_into::<web_sys::Element>().ok()?;
        let rect = row.get_bounding_client_rect();
        if rect.bottom() > 0.0 && rect.top() < window().inner_height().ok()?.as_f64()? {
            return Some((row.id(), rect.top()));
        }
    }
    None
}
#[cfg(feature = "hydrate")]
fn restore_desk_anchor((id, top): (String, f64)) {
    let path = window().location().pathname().ok();
    request_animation_frame(move || {
        if window().location().pathname().ok() == path
            && let Some(row) = document().get_element_by_id(&id)
            && let Ok(y) = window().scroll_y()
        {
            window().scroll_to_with_x_and_y(0.0, y + row.get_bounding_client_rect().top() - top);
        }
    });
}

#[component]
fn CreateDraftButton(
    creation: RwSignal<CreateState>,
    can_create: Signal<bool>,
    create: Callback<()>,
    #[prop(default = false)] first: bool,
) -> impl IntoView {
    view! {
        <Button variant=ButtonVariant::Filled
            leading=IconName::Add id="create-draft" on_press=move |_| create.run(())
            state=Signal::derive(move || if creation.get() == CreateState::Pending { ActionState::pending("Đang tạo…") } else { blocked(!can_create.get()) })
        >
            {move || match creation.get() {
                CreateState::Pending => "Đang tạo…",
                CreateState::Reconciled => "Tạo thêm bản thảo",
                _ if first => "Viết bản thảo đầu tiên",
                _ => "Tạo bản thảo",
            }}
        </Button>
    }
}

#[component]
fn CreateNotice(
    creation: RwSignal<CreateState>,
    loading: RwSignal<bool>,
    refresh: Callback<()>,
) -> impl IntoView {
    move || {
        if creation.get() == CreateState::Reconciled {
            return view! {
                <div data-testid="create-notice">
                    <InlineNotice title="Đã tải lại danh sách bản thảo."
                        message="Hãy kiểm tra bản thảo gần đây. Nếu vẫn muốn tạo bản khác, chọn Tạo thêm bản thảo."
                        tone=Tone::Info announce=Announce::Polite />
                </div>
            }.into_any();
        }
        let (title, detail) = match creation.get() {
            CreateState::Uncertain => (
                "Chưa xác nhận được việc tạo bản thảo.",
                "Bản thảo có thể đã được tạo. Hãy tải lại danh sách và kiểm tra trước khi tạo thêm.",
            ),
            CreateState::Rejected(ApiError::Unauthenticated) => (
                "Bạn chưa đăng nhập.",
                "Cần đăng nhập để tạo bản thảo. Bạn vẫn có thể đọc các tác phẩm công khai.",
            ),
            CreateState::Rejected(ApiError::Forbidden) => (
                "Yêu cầu tạo bản thảo bị từ chối.",
                "Hãy mở lại Bàn viết để kiểm tra quyền truy cập rồi thử lại.",
            ),
            CreateState::Rejected(_) => (
                "Yêu cầu tạo bản thảo chưa hợp lệ.",
                "Hãy mở lại Bàn viết rồi thử tạo lại.",
            ),
            _ => return ().into_any(),
        };
        let kind = match creation.get() {
            CreateState::Uncertain => ProblemKind::Uncertain,
            CreateState::Rejected(ApiError::Unauthenticated) => ProblemKind::AuthRequired,
            CreateState::Rejected(ApiError::Forbidden) => ProblemKind::Forbidden,
            _ => ProblemKind::Validation,
        };
        view! {
            <div data-testid="create-notice" role="status">
                <StatePanel content=PanelContent { kind: PanelKind::Error(kind), title: title.into(), message: detail.into(), actions: vec![], code: None }>
                    <Button variant=ButtonVariant::Filled id="reconcile-drafts"
                        on_press=move |_| refresh.run(()) state=Signal::derive(move || blocked(loading.get()))>
                        {move || if loading.get() { "Đang tải lại…" } else { "Tải lại danh sách" }}
                    </Button>
                </StatePanel>
            </div>
        }.into_any()
    }
}

#[component]
fn DraftListView(
    pages: RwSignal<crate::draft_list::paging::DraftPages>,
    first_use: Signal<bool>,
    creation: RwSignal<CreateState>,
    can_create: Signal<bool>,
    create: Callback<()>,
    query: RwSignal<String>,
    sort: RwSignal<DraftSort>,
    composing: RwSignal<bool>,
    loading: RwSignal<bool>,
    appending: RwSignal<bool>,
    page_error: RwSignal<Option<ApiError>>,
    load_error: RwSignal<Option<ApiError>>,
    restarted: RwSignal<bool>,
    submit: Callback<()>,
    clear: Callback<()>,
    more: Callback<()>,
    refresh: Callback<()>,
    refresh_disabled: Signal<bool>,
) -> impl IntoView {
    let invalid_page = move || page_error.with(|e| matches!(e, Some(ApiError::Invalid { .. })));
    view! {
        <section id="drafts" class="vk-draft-list" aria-labelledby="draft-list-title">
            <Heading id="draft-list-title" level=HeadingLevel::H2 role=TypeRole::Title attr:tabindex="-1">"Tác phẩm của bạn"</Heading>
            <div class="vk-draft-controls">
                <div class="vk-draft-search-group">
                    <form role="search" class="vk-draft-search" on:submit=move |event| {
                        event.prevent_default(); submit.run(());
                    }>
                        <SearchField id="draft-search" label="Tìm tác phẩm" value=query variant=SearchVariant::Bar
                            help="Theo tên bản thảo; gồm cả tác phẩm đã đăng" on_clear=clear
                            on_input=move |input: vink_ui::TextInput| {
                                composing.set(input.composing); query.set(input.value);
                            } />
                    </form>
                    <p id="draft-scope" class="vk-muted" role="status" aria-atomic="true" data-testid="draft-result-count">
                        {move || if loading.get() && !appending.get() { "Đang tìm tác phẩm…".into() }
                            else if load_error.get().is_some() && !pages.with(|p| p.loaded) { "Chưa tải được kết quả.".into() }
                            else { format!("Đã tải {} tác phẩm.", pages.with(|p| p.drafts.len())) }}
                    </p>
                </div>
                <div id="draft-tools" class="vk-draft-tools">
                    <Select id="draft-sort" label="Sắp xếp"
                        options=vec![ChoiceOption::new("recent", "Mới cập nhật"), ChoiceOption::new("title", "Tên A–Z")]
                        value=Signal::derive(move || Some(if sort.get() == DraftSort::Title { "title" } else { "recent" }.to_string()))
                        on_change={move |value: Option<String>| sort.set(if value.as_deref() == Some("title") { DraftSort::Title } else { DraftSort::Recent })} />
                    <TooltipIconButton icon=IconName::Refresh label="Tải lại" id="refresh-drafts"
                        surface=ActionSurface::Tonal
                        state=Signal::derive(move || blocked(refresh_disabled.get())) on_press=move |_| refresh.run(()) />
                    <Show when=move || sort.get() == DraftSort::Title>
                        <p class="vk-draft-sort-help vk-muted">"Bỏ qua dấu khi sắp xếp."</p>
                    </Show>
                </div>
            </div>
            <Show when=move || restarted.get()>
                <p class="vk-muted" data-testid="draft-chain-restarted">"Đã tải lại từ đầu. Bạn có thể tải tiếp các tác phẩm phía sau."</p>
            </Show>
            <Show when=move || first_use.get()>
                <div class="vk-desk-empty" data-testid="draft-empty">
                    <StatePanel content=PanelContent { kind: PanelKind::Empty(EmptyKind::FirstUse), title: "Bắt đầu một bản thảo".into(), message: "Một trang dành riêng cho những điều bạn muốn viết.".into(), actions: vec![], code: None }>
                        <CreateDraftButton creation can_create create first=true />
                    </StatePanel>
                </div>
            </Show>
            <Show when=move || !loading.get() && !first_use.get() && load_error.get().is_none() && pages.with(|p| p.loaded && p.part == 1 && p.drafts.is_empty())>
                <div data-testid="draft-no-results">
                    <StatePanel content=PanelContent { kind: PanelKind::Empty(EmptyKind::NoResults), title: "Không tìm thấy tác phẩm.".into(), message: "Thử tiêu đề khác hoặc xóa tìm kiếm.".into(), actions: vec![], code: None }>
                        <Button variant=ButtonVariant::Filled id="clear-draft-search" on_press=move |_| clear.run(())>"Xem tất cả tác phẩm"</Button>
                    </StatePanel>
                </div>
            </Show>
            <Show when=move || pages.with(|p| p.loaded) && (load_error.get().is_some() || page_error.get().is_some() || loading.get())>
                <p class="vk-muted" data-testid="author-work-stale" role="status">"Trạng thái và số liệu đã tải trước đó; có thể đã thay đổi."</p>
            </Show>
            <ul id="draft-results" class="vk-draft-rows" aria-label="Tác phẩm đã tải" role="list">
                <For each=move || pages.with(|p| p.drafts.clone())
                    key=|draft| draft.work_id.clone()
                    children=move |draft| {
                        let id = draft.work_id.clone();
                        let fallback = adapters::author_work_summary(&draft);
                        let work = Signal::derive(move || pages.with(|p| p.drafts.iter().find(|d| d.work_id == id)
                            .map(adapters::author_work_summary).unwrap_or_else(|| fallback.clone())));
                        view! {
                            <li id=format!("draft-row-{}", draft.work_id) class="vk-draft-row" data-testid="draft-row">
                                <AuthorWorkRow work />
                            </li>
                        }
                    } />
            </ul>
            <div id="draft-continuation" class="vk-draft-continuation">
                <Show when=move || page_error.get().is_some()>
                    <div id="draft-page-error" role="alert">
                        <StatePanel content=PanelContent { kind: PanelKind::Error(ProblemKind::Unavailable), title: if invalid_page() { "Danh sách đã thay đổi." } else { "Chưa tải được phần tiếp theo." }.into(), message: if invalid_page() { "Các tác phẩm đã tải vẫn ở đây. Hãy tải lại từ đầu để tiếp tục." } else { "Các tác phẩm đã tải vẫn ở đây. Bạn có thể thử tải thêm." }.into(), actions: vec![], code: None }>
                            <Show when=invalid_page>
                                <Button variant=ButtonVariant::Filled id="restart-drafts" on_press=move |_| refresh.run(()) state=Signal::derive(move || blocked(refresh_disabled.get()))>"Tải lại từ đầu"</Button>
                            </Show>
                        </StatePanel>
                    </div>
                </Show>
                <Show when=move || pages.with(|p| p.chain.next_cursor.is_some()) && !invalid_page()>
                    <Button variant=ButtonVariant::Outlined id="load-more-drafts" on_press=move |_| more.run(())
                        state=Signal::derive(move || if appending.get() { ActionState::pending("Đang tải…") } else { blocked(refresh_disabled.get()) })>
                        {move || if appending.get() { "Đang tải…" } else if page_error.get().is_some() { "Thử tải thêm" } else if pages.with(|p| p.next_part()) { "Xem phần tiếp theo" } else { "Tải thêm" }}
                    </Button>
                </Show>
                <Show when=move || pages.with(|p| p.loaded && p.chain.next_cursor.is_none() && (!p.drafts.is_empty() || p.part > 1)) && !loading.get() && load_error.get().is_none()>
                    <p id="draft-end" class="vk-muted">"Đã xem hết kết quả."</p>
                </Show>
                <Show when=move || pages.with(|p| p.part > 1)>
                    <p class="vk-muted">{move || format!("Đang xem phần {}. Chọn Tải lại để về đầu danh sách.", pages.with(|p| p.part))}</p>
                </Show>
                <Show when=move || pages.with(|p| p.loaded && !p.drafts.is_empty())>
                    <p class="vk-muted">"Tác phẩm có thể đổi vị trí khi bản nháp được sửa. Tải lại để xem các cập nhật mới nhất."</p>
                </Show>
            </div>
        </section>
    }
}

#[component]
fn DeskError(
    error: ApiError,
    #[prop(optional)] refresh: Option<Callback<()>>,
    #[prop(optional, into)] retry_disabled: Signal<bool>,
    #[prop(optional)] retained: bool,
) -> impl IntoView {
    let kind = match error {
        ApiError::Unauthenticated => ProblemKind::AuthRequired,
        ApiError::Forbidden => ProblemKind::Forbidden,
        ApiError::NotFound => ProblemKind::NotFound,
        _ => ProblemKind::Unavailable,
    };
    // Only where this deployment accepts sign-in; elsewhere the old wording stays truthful.
    let sign_in = error == ApiError::Unauthenticated && crate::api::sign_in_open();
    let (title, detail) = match error {
        ApiError::Unavailable | ApiError::Network if retained => (
            "Chưa tải lại được bản thảo.",
            "Đang giữ danh sách tải thành công gần nhất và chữ tìm kiếm. Hãy thử tải lại.",
        ),
        ApiError::Unauthenticated if sign_in => (
            "Bạn chưa đăng nhập.",
            "Đăng nhập ở mục Tôi để mở bản thảo riêng. Trang này tự tải lại khi phiên sẵn sàng.",
        ),
        ApiError::Unauthenticated => (
            "Bạn chưa đăng nhập.",
            "Cần đăng nhập để mở bản thảo riêng. Bạn vẫn có thể đọc các tác phẩm công khai.",
        ),
        ApiError::NotFound => (
            "Không tìm thấy bản nháp này.",
            "Bản nháp không tồn tại hoặc không thuộc về bạn.",
        ),
        ApiError::Unavailable | ApiError::Network => (
            "Chưa tải được bản thảo.",
            "Chưa kết nối được máy chủ. Hãy thử mở lại sau ít phút.",
        ),
        ApiError::Forbidden => (
            "Bạn không có quyền mở bản thảo này.",
            "Hãy mở lại Bàn viết để kiểm tra quyền truy cập.",
        ),
        _ => ("Có lỗi khi mở Bàn viết.", "Hãy thử mở lại."),
    };
    view! {
        <div data-testid="desk-error" role="alert">
            <StatePanel content=PanelContent { kind: PanelKind::Error(kind), title: title.into(), message: detail.into(), actions: vec![], code: None }>
                {refresh.map(|refresh| view! {
                    <Button variant=ButtonVariant::Filled id="retry-drafts" on_press=move |_| refresh.run(())
                        state=Signal::derive(move || blocked(retry_disabled.get()))>"Thử mở lại"</Button>
                })}
                {sign_in.then(|| view! {
                    <vink_ui::Link href="/toi" variant=vink_ui::LinkVariant::Standalone attr:id="desk-sign-in">"Đăng nhập ở mục Tôi"</vink_ui::Link>
                })}
                <vink_ui::Link href="/" variant=vink_ui::LinkVariant::Standalone>"Khám phá tác phẩm"</vink_ui::Link>
            </StatePanel>
        </div>
    }
}

#[component]
pub fn DraftEditor() -> impl IntoView {
    let params = use_params_map();
    let work_id = move || params.read().get("id").unwrap_or_default();
    let reload = RwSignal::new(0_u32);
    let load_failed = StoredValue::new(false);
    let draft: LocalResource<Result<Draft, ApiError>> = LocalResource::new(move || {
        let id = work_id();
        reload.track();
        async move {
            #[cfg(feature = "hydrate")]
            {
                let result = crate::api::load_draft(id).await;
                load_failed.try_set_value(result.is_err());
                result
            }
            #[cfg(not(feature = "hydrate"))]
            {
                let _ = (id, load_failed);
                Err(ApiError::Unavailable)
            }
        }
    });
    // Typed text belongs to the person who typed it (`text_survives`). It stays on the page when
    // the session ends and when the same account signs in again, so a save can be repeated. When
    // another account takes the tab the editor is rebuilt from that account's own read: the
    // earlier text and every reply still on its way go with the old editor. A draft that could
    // not be opened is read again once the session changes.
    let session = crate::account::Session::current();
    Effect::new(move |previous: Option<(Option<String>, Option<String>)>| {
        let (scope, principal) = (session.scope.get(), session.principal.get());
        let Some((before, typed_as)) = previous else {
            return (scope, principal);
        };
        if before != scope
            && (load_failed.get_value()
                || !crate::session::text_survives(typed_as.as_deref(), principal.as_deref()))
        {
            reload.update(|n| *n += 1);
        }
        (scope, principal.or(typed_as))
    });
    view! {
        <Title text="Bản nháp — Bàn viết — Vink" />
        <Meta name="robots" content="noindex" />
        <section class="vk-desk vk-desk--editor" aria-labelledby="editor-heading">
            <header class="vk-editor-heading">
                <a class="vk-editor-heading__back" href="/ban-viet">
                    "← Bàn viết"
                </a>
                <Heading level=HeadingLevel::H1 role=TypeRole::Title id="editor-heading" class:vk-editor-heading__title=true>"Bản nháp"</Heading>
            </header>
            <noscript>
                <p class="vk-desk-noscript">{DESK_UNAVAILABLE_SSR}</p>
            </noscript>
            <Suspense fallback=|| view! { <p class="vk-muted">"Đang mở bản nháp…"</p> }>
                {move || Suspend::new(async move {
                    match draft.await {
                        Ok(draft) => view! { <Editor draft /> }.into_any(),
                        Err(error) => view! { <DeskError error /> }.into_any(),
                    }
                })}
            </Suspense>
        </section>
    }
}

/// The field that last held focus, so "Ở lại" returns the author to where they were writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Title,
    Body,
    Tags,
}

#[component]
fn Editor(draft: Draft) -> impl IntoView {
    let work_id = StoredValue::new(draft.work_id.clone());
    let title = RwSignal::new(draft.title.clone());
    let body = BodySource::new(crate::document::document_to_text(&draft.document));
    let session = RwSignal::new(DraftSession::loaded(draft.version));
    let tag_load = RwSignal::new(TagLoad::Loading);
    let leave = RwSignal::new(LeaveGuard::<DraftTicket>::default());
    let labels = RwSignal::new(crate::ui::adapters::TagLabels::default());
    let navigation = use_context::<NavigationGuard>();
    let title_input = NodeRef::<html::Input>::new();
    let title_composing = StoredValue::new(false);
    let leave_open = RwSignal::new(false);
    let keep_editing = NodeRef::<html::Button>::new();
    let leave_stay = NodeRef::<html::Button>::new();
    let leave_discard = NodeRef::<html::Button>::new();
    let last_field = StoredValue::new(Field::Body);
    let page_scroll = StoredValue::new(0.0_f64);
    let preview = RwSignal::new(None::<(String, DocumentDto)>);

    // Leaves once every save the pending intent waits for has settled with nothing unconfirmed.
    let settle_leave = move || {
        let destination = leave
            .try_update(|g| session.with_untracked(|s| g.save_settled(s)))
            .flatten();
        if let Some(destination) = destination {
            leave_open.set(false);
            restore_scroll(page_scroll);
            if let Some(navigation) = navigation {
                navigation.go(&destination);
            }
        }
    };
    // Sends the text as it is now. The ticket was taken in the same turn, so it covers exactly
    // this text. The outcome settles the session first; only then may the leave intent go.
    let send_text = move |ticket: SaveTicket, reconcile: bool| {
        let request = crate::contract::SaveDraftRequest {
            expected_version: ticket.expected_version,
            title: title.get_untracked(),
            document: crate::document::text_to_document(&body.snapshot()),
        };
        #[cfg(feature = "hydrate")]
        {
            let id = work_id.get_value();
            leptos::task::spawn_local(async move {
                use crate::desk_state::SaveRejection;
                // A missing response may hide a committed write. Keep the expected version:
                // an advanced server version needs the existing explicit conflict recovery.
                let ready = if reconcile {
                    match crate::api::load_draft(id.clone()).await {
                        Ok(current) if current.version != ticket.expected_version => {
                            Err(ApiError::VersionConflict {
                                current_version: current.version,
                            })
                        }
                        Ok(_) => Ok(()),
                        Err(error) => Err(error),
                    }
                } else {
                    Ok(())
                };
                // A GET can outlive the editor or an explicit discard. Never start a new
                // mutation after that boundary. Staying only cancels navigation, not saving.
                if !session
                    .try_with_untracked(|s| s.text.in_flight() == Some(ticket))
                    .unwrap_or(false)
                    || leave
                        .try_with_untracked(LeaveGuard::released)
                        .unwrap_or(true)
                {
                    return;
                }
                let outcome = match ready {
                    Ok(()) => crate::api::save_draft(id, request).await,
                    Err(error) => Err(error),
                };
                // The editor may have closed while the request was in flight.
                let was_save_focused = active_element_id_is("save");
                let applied = session.try_update(|s| match outcome {
                    Ok(saved) => s.text.save_succeeded(ticket, saved.version),
                    Err(ApiError::VersionConflict { current_version }) => s
                        .text
                        .save_rejected(ticket, SaveRejection::Conflict { current_version }),
                    Err(error) => s
                        .text
                        .save_rejected(ticket, SaveRejection::Failed(failure(&error))),
                });
                if applied.is_none() {
                    return;
                }
                sync_save_slot_focus(was_save_focused, session, tag_load);
                settle_leave();
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (request, work_id, reconcile, settle_leave);
    };
    // Sends exactly the selection the ticket recorded. A lost answer is reconciled by the next
    // explicit save with the same expected version (TAG-PR-03 CHOICE-03), never resent here.
    let send_tags = move |ticket: TagTicket| {
        let Some(request) = session.with_untracked(|s| {
            let selection = s.tags.as_ref()?.submitted(ticket)?;
            Some(crate::contract::SaveDraftTagsRequest {
                expected_version: ticket.expected_version,
                form: selection.form.clone(),
                themes: selection.themes.clone(),
            })
        }) else {
            return;
        };
        #[cfg(feature = "hydrate")]
        {
            let id = work_id.get_value();
            leptos::task::spawn_local(async move {
                use crate::tag_state::{Selection, TagRejection};
                let outcome = crate::api::save_draft_tags(&id, &request).await;
                let was_save_focused = active_element_id_is("save");
                let applied = session.try_update(|s| {
                    let Some(tags) = s.tags.as_mut() else {
                        return;
                    };
                    match outcome {
                        Ok(saved) => tags.save_succeeded(
                            ticket,
                            saved.version,
                            Selection::new(saved.form, saved.themes),
                        ),
                        Err(ApiError::TagConflict {
                            current_version,
                            stored,
                        }) => tags.save_rejected(
                            ticket,
                            TagRejection::Conflict {
                                current_version,
                                stored: Selection::new(stored.form, stored.themes),
                            },
                        ),
                        Err(ApiError::TagRefused(refusal)) => {
                            tags.save_rejected(ticket, TagRejection::Refused(refusal))
                        }
                        Err(error) => {
                            tags.save_rejected(ticket, TagRejection::Failed(failure(&error)))
                        }
                    }
                });
                if applied.is_none() {
                    return;
                }
                sync_save_slot_focus(was_save_focused, session, tag_load);
                settle_leave();
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (request, settle_leave);
    };
    let send = move |ticket: DraftTicket, reconcile: bool| {
        if let Some(text) = ticket.text {
            send_text(text, reconcile);
        }
        if let Some(tags) = ticket.tags {
            send_tags(tags);
        }
    };
    let save = move || {
        let reconcile =
            session.with_untracked(|s| matches!(s.text.status(), SaveStatus::Failed(_)));
        if let Some(ticket) = session.try_update(DraftSession::begin_save) {
            send(ticket, reconcile);
        }
    };
    let save_manual = move || {
        if title_composing.get_value() || body.is_composing() {
            return;
        }
        let reconcile =
            session.with_untracked(|s| matches!(s.text.status(), SaveStatus::Failed(_)));
        if let Some(ticket) = session
            .try_update(DraftSession::begin_manual_save)
            .flatten()
        {
            send(ticket, reconcile);
        }
    };
    let export = move || {
        let text = crate::document::export_text(&title.get_untracked(), &body.snapshot());
        let name = crate::document::export_file_name(&title.get_untracked());
        #[cfg(feature = "hydrate")]
        if let Err(error) = download_text(&name, &text) {
            leptos::logging::warn!("export failed: {error:?}");
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (name, text);
    };
    let focus_last_field = move || match last_field.get_value() {
        Field::Title =>
        {
            #[cfg(feature = "hydrate")]
            if let Some(input) = title_input.get_untracked() {
                let options = web_sys::FocusOptions::new();
                options.set_prevent_scroll(true);
                let _ = input.focus_with_options(&options);
            }
        }
        Field::Body => body.focus(),
        Field::Tags => {
            #[cfg(feature = "hydrate")]
            if let Some(input) = document()
                .get_element_by_id("work-tags-edit")
                .filter(|details| !details.has_attribute("open"))
                .and_then(|details| details.query_selector("summary").ok().flatten())
                .or_else(|| document().get_element_by_id(crate::pages::work_info::QUERY_ID))
            {
                use wasm_bindgen::JsCast;
                if let Ok(input) = input.dyn_into::<web_sys::HtmlElement>() {
                    let options = web_sys::FocusOptions::new();
                    options.set_prevent_scroll(true);
                    let _ = input.focus_with_options(&options);
                }
            }
        }
    };
    let stay = move || {
        leave.update(LeaveGuard::stay);
        leave_open.set(false);
        restore_scroll(page_scroll);
        request_animation_frame(move || {
            if leave_open.try_get_untracked() == Some(false) {
                focus_last_field();
                restore_scroll(page_scroll);
            }
        });
    };
    let save_and_leave = move || {
        let reconcile =
            session.with_untracked(|s| matches!(s.text.status(), SaveStatus::Failed(_)));
        let ticket = leave
            .try_update(|g| session.try_update(|s| g.save_and_leave(s)))
            .flatten()
            .flatten();
        if let Some(ticket) = ticket {
            send(ticket, reconcile);
        }
    };
    let ask_discard = move || {
        leave.update(|g| session.with_untracked(|s| g.ask_discard(s)));
        request_animation_frame(move || {
            if let Some(button) = keep_editing
                .get_untracked()
                .filter(|button| button.is_connected())
            {
                let _ = button.focus();
            }
        });
    };
    let back_to_leave_choices = move || {
        leave.update(LeaveGuard::keep_choosing);
        request_animation_frame(move || {
            if !leave_open.try_get_untracked().unwrap_or(false)
                || !leave
                    .try_with_untracked(|guard| {
                        matches!(
                            guard.pending().map(|intent| intent.stage),
                            Some(LeaveStage::Choosing { .. })
                        )
                    })
                    .unwrap_or(false)
            {
                return;
            }
            // A pending save can settle while the confirmation is open, removing discard.
            // In that case return focus to the safe, still-present choice.
            if let Some(button) = leave_discard
                .get_untracked()
                .filter(|button| button.is_connected())
                .or_else(|| {
                    leave_stay
                        .get_untracked()
                        .filter(|button| button.is_connected())
                })
            {
                let _ = button.focus();
            }
        });
    };
    let confirm_discard = move || {
        if let Some(destination) = leave.try_update(LeaveGuard::confirm_discard).flatten() {
            leave_open.set(false);
            restore_scroll(page_scroll);
            if let Some(navigation) = navigation {
                navigation.go(&destination);
            }
        }
    };
    let open_preview = move || {
        // Close the in-flow tools before opening the modal. Preview stays a direct, live
        // trigger, so Dialog never restores focus into a collapsed disclosure.
        #[cfg(feature = "hydrate")]
        if let Some(tools) = document().get_element_by_id("editor-save-details") {
            let _ = tools.remove_attribute("open");
        }
        let document = crate::document::text_to_document(&body.snapshot());
        preview.set(Some((title.get_untracked(), document)));
        remember_scroll(page_scroll);
    };

    if let Some(navigation) = navigation {
        let registration = navigation.register(move |destination| {
            let held = leave
                .try_update(|g| {
                    session
                        .try_with_untracked(|s| g.request(s, destination))
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if held {
                if !leave_open.get_untracked() {
                    remember_scroll(page_scroll);
                }
                leave_open.set(true);
            }
            held
        });
        on_cleanup(move || navigation.unregister(registration));
    }

    #[cfg(feature = "hydrate")]
    {
        let guard = window_event_listener(leptos::ev::beforeunload, move |event| {
            let unsaved =
                session.with_untracked(<DraftSession as crate::desk_state::Guarded>::has_unsaved);
            if unsaved && !leave.with_untracked(LeaveGuard::released) {
                event.prevent_default();
            }
        });
        on_cleanup(move || guard.remove());
    }

    let on_body_ready = Callback::new(move |()| {
        // A new, empty draft starts at its title; an existing one at its text.
        if title.get_untracked().is_empty() && body.snapshot().is_empty() {
            last_field.set_value(Field::Title);
        }
        focus_last_field();
    });
    let on_body_change = Callback::new(move |()| {
        session.try_update(|s| s.text.edited());
    });

    let jump_to_recovery = move |target: RecoveryTarget| {
        #[cfg(feature = "hydrate")]
        {
            use wasm_bindgen::JsCast;
            let doc = document();
            // The jump must not put a destructive choice under a still-held Enter key.
            let selector = match target {
                RecoveryTarget::TextConflict => "#editor-text-conflict",
                RecoveryTarget::TagConflict => "#editor-tag-conflict",
                RecoveryTarget::TagRefused => "#work-tags-edit > summary",
                RecoveryTarget::TagLoad => "#tags-reload",
            };
            if let Some(el) = doc
                .query_selector(selector)
                .ok()
                .flatten()
                .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
            {
                let _ = el.focus();
            }
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = target;
    };
    let on_jump_recovery = Callback::new(jump_to_recovery);

    // Deduplicate status changes: typing another character while already unsaved must not
    // mutate the polite live region. The acknowledged version stays in optional detail.
    let slot_facts = Memo::new(move |_| {
        let tags_read = tag_load.get();
        session.with(|s| SaveSlotFacts::from_session(s, tags_read))
    });
    let slot_view = Memo::new(move |_| project_save_slot(&slot_facts.get()));
    let status = move || slot_facts.with(|f| f.text);
    let tag_status = Memo::new(move |_| slot_facts.with(|f| f.tags.clone()));
    let status_label = Memo::new(move |_| slot_view.with(|v| v.status_text.clone()));
    let status_token = Memo::new(move |_| slot_view.with(|v| v.status_token.to_string()));
    let status_problem = Memo::new(move |_| slot_view.with(|v| v.problem_kind.map(str::to_string)));
    let slot_detail = Memo::new(move |_| slot_view.with(|v| v.detail_text.clone()));
    let slot_action = Memo::new(move |_| slot_view.with(|v| v.action));
    let shortcut = RwSignal::new("Lưu nháp (Ctrl/⌘ + S)".to_string());
    #[cfg(feature = "hydrate")]
    Effect::new(move |_| {
        let platform = js_sys::Reflect::get(&js_sys::global(), &"navigator".into())
            .ok()
            .and_then(|navigator| js_sys::Reflect::get(&navigator, &"platform".into()).ok())
            .and_then(|value| value.as_string())
            .unwrap_or_default();
        shortcut.set(
            if platform.contains("Mac") || platform.contains("iPhone") || platform.contains("iPad")
            {
                "Lưu nháp (⌘ + S)"
            } else {
                "Lưu nháp (Ctrl + S)"
            }
            .to_string(),
        );
    });
    let stage = move || leave.with(|g| g.pending().map(|intent| intent.stage));
    let can_discard = move || leave.with(|g| session.with(|s| g.can_discard(s)));
    let action_is_save_or_retry = move || {
        matches!(
            slot_action.get(),
            SaveSlotAction::Save { .. } | SaveSlotAction::Retry { .. }
        )
    };
    let action_is_recover = move || matches!(slot_action.get(), SaveSlotAction::Recover { .. });
    view! {
        <form
            class="vk-editor"
            aria-labelledby="editor-heading"
            on:submit=move |event| {
                event.prevent_default();
                save_manual();
            }
            on:keydown=move |event| {
                let shortcut = (event.ctrl_key() || event.meta_key())
                    && event.key().eq_ignore_ascii_case("s");
                if shortcut {
                    event.prevent_default();
                    // An IME still composing commits first; saving half a syllable helps no one.
                    if !event.is_composing() {
                        save_manual();
                    }
                }
            }
        >
            <crate::ui::patterns::EditorToolbar attr:id=TOOLBAR_ID attr:data-testid="editor-toolbar"
                paper_first=true
                status=Signal::derive(move || status_label.get())
                status_token=Signal::derive(move || status_token.get())
                status_problem=Signal::derive(move || status_problem.get())
                status_detail=Signal::derive(move || slot_detail.get())
                save_action=std::sync::Arc::new(move || view! {
                    <>
                        <Show when=action_is_save_or_retry>
                            <PlainTooltip id="save-tooltip" text=Signal::derive(move || shortcut.get())>
                                <Button
                                    variant=Signal::derive(move || match slot_action.get() {
                                        SaveSlotAction::Retry { .. } => ButtonVariant::Tonal,
                                        _ => ButtonVariant::Filled,
                                    })
                                    kind=ButtonKind::Submit
                                    id="save"
                                    described_by="save-tooltip"
                                    state=Signal::derive(move || match slot_action.get() {
                                        SaveSlotAction::Save { pending: true, .. }
                                        | SaveSlotAction::Retry { pending: true, .. } => ActionState::pending(""),
                                        _ => ActionState::Enabled,
                                    })
                                >
                                    {move || match slot_action.get() {
                                        SaveSlotAction::Save { label, pending: false } => label,
                                        SaveSlotAction::Retry { label, pending: false } => label,
                                        _ => "Đang lưu…",
                                    }}
                                </Button>
                            </PlainTooltip>
                        </Show>
                        <Show when=action_is_recover>
                            <Button
                                variant=ButtonVariant::Tonal
                                id="save-recovery-jump"
                                on_press=move |_| {
                                    if let SaveSlotAction::Recover { target, .. } = slot_action.get_untracked() {
                                        on_jump_recovery.run(target);
                                    }
                                }
                            >
                                {move || match slot_action.get() {
                                    SaveSlotAction::Recover { label, .. } => label,
                                    _ => "",
                                }}
                            </Button>
                        </Show>
                    </>
                }.into_any())
                utilities=std::sync::Arc::new(move || view! {
                    <Button variant=ButtonVariant::Text id="export" on_press=move |_| export()>
                        <Icon name=IconName::Download />"Tải bản thảo (.txt)"
                    </Button>
                }.into_any())>
                    <TooltipIconButton id="preview" icon=IconName::Visibility label="Xem trước"
                        on_press=move |_| open_preview() />
            </crate::ui::patterns::EditorToolbar>
            <Show when=move || matches!(status(), SaveStatus::Failed(_))>
                <section class="vk-editor-recovery" data-testid="save-failed" data-tone="error" aria-label="Phục hồi lưu nháp">
                    // The toolbar is the sole polite announcement owner for this state.
                    {move || view! { <InlineNotice tone=Tone::Error announce=Announce::Off
                        title=status_text(status()) message="Tải bản sao trước khi rời trang." /> }}
                    <div class="vk-notice__actions">
                        <Button variant=ButtonVariant::Text id="retry-save" on_press=move |_| save()>"Thử lại"</Button>
                        <Button variant=ButtonVariant::Text id="export-failed" on_press=move |_| export()>"Tải .txt"</Button>
                    </div>
                    <SupportingDetails id="save-failure-details" summary="Chi tiết">
                        <p>{move || match status() {
                            SaveStatus::Failed(failure) => failure_detail(failure),
                            _ => String::new(),
                        }}</p>
                    </SupportingDetails>
                </section>
            </Show>
            <Show when=move || matches!(status(), SaveStatus::Conflict { .. })>
                <section id="editor-text-conflict" tabindex="-1" class="vk-editor-recovery"
                    data-testid="conflict" data-tone="warning" aria-label="Xung đột bản nháp">
                    <InlineNotice tone=Tone::Warning announce=Announce::Off title="Có bản mới trên máy chủ."
                        message="Tải bản sao trước khi rời trang." />
                    <div class="vk-notice__actions">
                        <Button variant=ButtonVariant::Text id="keep-mine"
                            on_press=move |_| {
                                session.update(|s| s.text.keep_local_text());
                                save();
                            }>"Lưu đè bằng nội dung đang viết"</Button>
                        <Button variant=ButtonVariant::Text id="export-conflict" on_press=move |_| export()>"Tải .txt"</Button>
                    </div>
                    <SupportingDetails id="save-conflict-details" summary="Chi tiết">
                        <p>{move || status_detail(status())}</p>
                        <p>"Nội dung còn trên trang này; chưa có bản khôi phục tự động. Lưu đè sẽ thay bản trên máy chủ bằng nội dung đang viết."</p>
                    </SupportingDetails>
                </section>
            </Show>
            <div class="vk-editor-paper">
                <TextField id="draft-title" label="Tiêu đề" name="title" max_length=200 input_ref=title_input
                    variant=FieldVariant::FloatingTonal
                    placeholder="Chưa đặt tên" autocomplete="off" value=title
                    on_focus=move |_| last_field.set_value(Field::Title)
                    on_input=move |input: vink_ui::TextInput| {
                        title_composing.set_value(input.composing);
                        if title.get_untracked() != input.value {
                            title.set(input.value);
                            session.update(|s| s.text.edited());
                        }
                    } />
                // Read the entire title without replacing the native editing control. This
                // projection has no buffer of its own; native selection/undo/IME stay in input.
                <details id="draft-title-details" class="vk-editor-title-details">
                    <summary>"Xem trọn tiêu đề"</summary>
                    <p id="draft-title-full">{move || {
                        let value = title.get();
                        if value.is_empty() { "Chưa đặt tên".to_string() } else { value }
                    }}</p>
                    <Button id="edit-full-title" variant=ButtonVariant::Text on_press=move |_| {
                        #[cfg(feature = "hydrate")]
                        if let Some(input) = title_input.get_untracked() {
                            // User-requested focus also reveals the editing line. Do not select
                            // or write its value: focus preserves its directional selection.
                            let _ = input.focus();
                        }
                    }>"Chỉnh tiêu đề"</Button>
                </details>
                <div class="vk-field" on:focusin=move |_| last_field.set_value(Field::Body)>
                    <span
                        id=BODY_LABEL_ID
                        class="vk-field__label"
                        on:click=move |_| body.focus()
                    >
                        "Nội dung"
                    </span>
                    <span id=BODY_HINT_ID class="vk-ui-visually-hidden">
                        {move || shortcut.get()}
                    </span>
                    <BodyEditor source=body on_change=on_body_change on_ready=on_body_ready />
                    <Show when=move || body.state.get() == BodyState::Loading>
                        <p class="vk-muted vk-editor-body__pending">"Đang mở trình soạn thảo…"</p>
                    </Show>
                    <Show when=move || body.state.get() == BodyState::Failed>
                        <InlineNotice tone=Tone::Error announce=Announce::Assertive
                            message="Không mở được trình soạn thảo. Nội dung đã lưu trên máy chủ không bị ảnh hưởng; hãy tải lại trang."
                            attr:data-testid="editor-failed" />
                    </Show>
                    <details class="vk-editor-help">
                        <summary>"Cách ngắt dòng và chia khổ"</summary>
                        <p>"Xuống dòng để ngắt câu thơ; để một dòng trống giữa các đoạn hoặc khổ."</p>
                    </details>
                </div>
            </div>
            <div class="vk-editor-work-info" on:focusin=move |_| last_field.set_value(Field::Tags)>
                <crate::pages::work_info::WorkInfo work_id=work_id.get_value() session labels
                    on_save=Callback::new(move |()| save()) tag_load=tag_load />
            </div>
            <crate::pages::publication::PublicationPanel work_id=work_id.get_value() session
                on_save=Callback::new(move |()| save())
                is_composing=Callback::new(move |()| body.is_composing()) />
        </form>
        {
            let confirming = move || stage() == Some(LeaveStage::ConfirmingDiscard);
            let saving = move || matches!(stage(), Some(LeaveStage::Saving { .. }));
            let conflict = move || {
                matches!(status(), SaveStatus::Conflict { .. })
                    || matches!(tag_status.get(), Some(TagStatus::Conflict { .. }))
            };
            let leave_title = Signal::derive(move || {
                if confirming() {
                    "Bỏ thay đổi và rời trang?"
                } else {
                    "Rời trang khi còn thay đổi?"
                }
                .to_string()
            });
            let leave_context = Signal::derive(move || {
                if confirming() {
                    "Bạn sẽ mất những chỉnh sửa chưa lưu trên trang này. Nếu đã tải tệp .txt, bản sao chữ vẫn nằm trên máy của bạn."
                        .to_string()
                } else {
                    leave_message(
                        stage(),
                        status(),
                        tag_status.get().as_ref(),
                        session.with(|s| {
                            (
                                s.text.has_unsaved_text(),
                                s.tags.as_ref().is_some_and(|t| t.has_unsaved()),
                            )
                        }),
                    )
                }
            });
            view! {
                <vink_ui::Dialog id="leave-editor" open=leave_open
                    title=leave_title
                    context=leave_context
                    dismiss=vink_ui::DismissPolicy::EscapeOnly on_dismiss=move |_| stay()
                    actions=move || view! {
                        <Show
                            when=confirming
                            fallback=move || view! {
                                <Button
                                    variant=Signal::derive(move || if conflict() { ButtonVariant::Filled } else { ButtonVariant::Text })
                                    id="leave-stay"
                                    button_ref=leave_stay
                                    on_press=move |_| stay()
                                >
                                    "Ở lại"
                                </Button>
                                <Button
                                    variant=Signal::derive(move || if conflict() { ButtonVariant::Tonal } else { ButtonVariant::Filled })
                                    id="leave-save"
                                    described_by="leave-editor-context"
                                    state=Signal::derive(move || blocked(saving() || conflict()))
                                    on_press=move |_| save_and_leave()
                                >
                                    {move || if saving() { "Đang lưu…" } else { "Lưu và rời trang" }}
                                </Button>
                            }
                        >
                            <Button variant=ButtonVariant::Tonal
                                id="discard-back"
                                button_ref=keep_editing
                                on_press=move |_| back_to_leave_choices()
                            >
                                "Quay lại"
                            </Button>
                            <Button variant=ButtonVariant::Danger
                                id="discard-confirm"
                                on_press=move |_| confirm_discard()
                            >
                                "Bỏ thay đổi và rời"
                            </Button>
                        </Show>
                    }
                    secondary_actions=move || view! {
                        <Show when=move || !confirming() && can_discard()>
                            <Button variant=ButtonVariant::Tonal
                                id="leave-export"
                                on_press=move |_| export()
                            >
                                "Tải bản đang viết (.txt)"
                            </Button>
                            <Button variant=ButtonVariant::DangerText
                                id="leave-discard"
                                button_ref=leave_discard
                                on_press=move |_| ask_discard()
                            >
                                "Rời mà không lưu…"
                            </Button>
                        </Show>
                    }
                    attr:data-testid="leave-editor" />
            }
        }
        <vink_ui::Dialog id="draft-preview" open=Signal::derive(move || preview.with(Option::is_some))
            title="Xem trước bản nháp"
            context="Dùng chữ đang có trên trang, kể cả phần chưa lưu. Chỉ bạn thấy; chưa gửi duyệt, chưa công khai."
            focus_title=true
            dismiss=vink_ui::DismissPolicy::EscapeOnly
            on_dismiss=move |_| { preview.set(None); restore_scroll(page_scroll); }
            actions=move || view! {
                <Button variant=ButtonVariant::Tonal
                    id="preview-close"
                    on_press=move |_| {
                        preview.set(None);
                        restore_scroll(page_scroll);
                    }
                >
                    "Đóng"
                </Button>
            }
            attr:data-testid="draft-preview">
            {move || {
                preview
                    .get()
                    .map(|(title, document)| {
                        let empty = document.blocks.is_empty();
                        view! {
                            <article class="vk-preview" lang="vi">
                                <Heading level=HeadingLevel::H2 role=TypeRole::Headline>{untitled(&title)}</Heading>
                                {if empty {
                                    view! { <p class="vk-muted">"Chưa có nội dung."</p> }.into_any()
                                } else {
                                    view! { <DocumentView document /> }.into_any()
                                }}
                            </article>
                        }
                    })
            }}
        </vink_ui::Dialog>
    }
}

fn remember_scroll(page_scroll: StoredValue<f64>) {
    #[cfg(feature = "hydrate")]
    page_scroll.set_value(window().scroll_y().unwrap_or_default());
    #[cfg(not(feature = "hydrate"))]
    let _ = page_scroll;
}

fn restore_scroll(page_scroll: StoredValue<f64>) {
    #[cfg(feature = "hydrate")]
    if let Some(y) = page_scroll.try_get_value() {
        window().scroll_to_with_x_and_y(window().scroll_x().unwrap_or_default(), y);
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = page_scroll;
}

/// Saves `text` as a UTF-8 file through the browser's download, without leaving the page.
#[cfg(feature = "hydrate")]
fn download_text(file_name: &str, text: &str) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/plain;charset=utf-8");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    // A detached link: no navigation guard or router sees this click.
    let link: web_sys::HtmlAnchorElement = document().create_element("a")?.unchecked_into();
    link.set_href(&url);
    link.set_download(file_name);
    link.click();
    set_timeout(
        move || {
            let _ = web_sys::Url::revoke_object_url(&url);
        },
        std::time::Duration::from_secs(10),
    );
    Ok(())
}

fn status_text(status: SaveStatus) -> String {
    match status {
        SaveStatus::Saved { .. } => "Đã lưu",
        SaveStatus::Unsaved => "Chưa lưu",
        SaveStatus::Saving => "Đang lưu…",
        SaveStatus::Conflict { .. } => "Có bản mới trên máy chủ.",
        failed @ SaveStatus::Failed(_) if save_is_uncertain(failed) => "Chưa xác nhận lưu.",
        SaveStatus::Failed(_) => "Chưa lưu được.",
    }
    .into()
}

fn status_detail(status: SaveStatus) -> String {
    match status {
        SaveStatus::Saved { version } => format!("Đã lưu bản nháp phiên bản {version}."),
        SaveStatus::Unsaved => "Thay đổi trên trang này chưa được lưu lên máy chủ.".into(),
        SaveStatus::Saving => {
            "Đang chờ máy chủ xác nhận bản nháp đã gửi. Chữ gõ thêm vẫn cần được lưu.".into()
        }
        SaveStatus::Conflict { server_version } => format!(
            "Máy chủ đang ở phiên bản {server_version}. Nội dung đang viết chưa được lưu vào bản đó."
        ),
        SaveStatus::Failed(failure) => failure_detail(failure),
    }
}

fn failure_detail(failure: Failure) -> String {
    let uncertainty = if save_is_uncertain(SaveStatus::Failed(failure)) {
        " Máy chủ chưa xác nhận."
    } else {
        ""
    };
    format!(
        "{}{uncertainty} Nội dung còn trên trang này; chưa có bản khôi phục tự động.",
        failure_reason(failure)
    )
}

fn save_is_uncertain(status: SaveStatus) -> bool {
    matches!(
        status,
        SaveStatus::Failed(Failure::Network | Failure::Unavailable | Failure::Unexpected)
    )
}

pub(crate) fn failure_reason(failure: Failure) -> &'static str {
    match failure {
        Failure::Network => "Mất phản hồi từ máy chủ; chưa rõ bản nháp đã được lưu hay chưa.",
        Failure::Unavailable => "Máy chủ tạm không phục vụ.",
        Failure::Unauthenticated => {
            "Phiên đăng nhập đã hết nên chưa lưu được. Hãy đăng nhập lại ở mục Tôi trong một thẻ khác rồi lưu lại."
        }
        Failure::Forbidden => "Yêu cầu bị từ chối.",
        Failure::NotFound => "Không tìm thấy bản nháp.",
        Failure::Invalid => "Nội dung không hợp lệ.",
        Failure::Unexpected => "Phản hồi không xác định; chưa rõ bản nháp đã được lưu hay chưa.",
    }
}

fn leave_message(
    stage: Option<LeaveStage<DraftTicket>>,
    status: SaveStatus,
    tags: Option<&TagStatus>,
    (text_unsaved, tags_unsaved): (bool, bool),
) -> String {
    let tags_not_saved = match tags {
        Some(TagStatus::Conflict { .. }) => Some(
            "Thẻ đã đổi ở nơi khác. Không thể lưu đè từ đây. Ở lại để chọn thẻ cần giữ."
                .to_string(),
        ),
        Some(TagStatus::Refused(_)) => Some(
            "Thẻ chưa lưu được: máy chủ từ chối một thẻ. Ở lại để bỏ hoặc thay thẻ đó.".to_string(),
        ),
        Some(TagStatus::Failed(failure)) => Some(format!(
            "Thẻ chưa lưu được. {} Lựa chọn vẫn còn trên trang này.",
            tag_failure_reason(*failure)
        )),
        _ => None,
    };
    let not_saved = match status {
        SaveStatus::Conflict { .. } => {
            "Có bản mới trên máy chủ. Ở lại để xử lý xung đột hoặc tải bản đang viết.".into()
        }
        SaveStatus::Failed(failure) => format!(
            "Chưa lưu được. {} Chữ vẫn còn trên trang này.",
            failure_reason(failure)
        ),
        _ => tags_not_saved
            .clone()
            .unwrap_or_else(|| "Chữ vẫn còn trên trang này.".into()),
    };
    let blocked = matches!(status, SaveStatus::Failed(_) | SaveStatus::Conflict { .. })
        || tags_not_saved.is_some();
    match stage {
        Some(LeaveStage::Saving { .. }) => {
            "Đang lưu… Trang sẽ rời khi máy chủ xác nhận bản mới nhất.".into()
        }
        Some(LeaveStage::Choosing {
            notice: Some(LeaveNotice::NewerEdits),
        }) => "Đã lưu, nhưng còn thay đổi mới hơn chưa lưu. Lưu tiếp rồi rời, hoặc ở lại.".into(),
        Some(LeaveStage::Choosing {
            notice: Some(LeaveNotice::NotSaved),
        }) => not_saved,
        _ if blocked => not_saved,
        _ if tags_unsaved && !text_unsaved => "Thẻ đã chọn chưa được máy chủ xác nhận. Ở lại để chọn tiếp, hoặc lưu rồi rời trang.".into(),
        _ if tags_unsaved => "Chữ và thẻ gần nhất chưa được máy chủ xác nhận. Ở lại để viết tiếp, hoặc lưu rồi rời trang.".into(),
        _ => "Những thay đổi gần nhất chưa được máy chủ xác nhận. Ở lại để viết tiếp, hoặc lưu rồi rời trang.".into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryTarget {
    TextConflict,
    TagConflict,
    TagRefused,
    TagLoad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveSlotAction {
    None,
    Save {
        label: &'static str,
        pending: bool,
    },
    Retry {
        label: &'static str,
        pending: bool,
    },
    Recover {
        label: &'static str,
        target: RecoveryTarget,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SaveSlotFacts {
    text: SaveStatus,
    text_pending_newer: bool,
    tags: Option<TagStatus>,
    tags_read: TagLoad,
    tags_pending_newer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SaveSlotView {
    status_text: String,
    status_token: &'static str,
    problem_kind: Option<&'static str>,
    detail_text: String,
    action: SaveSlotAction,
}

impl SaveSlotFacts {
    fn from_session(session: &DraftSession, tags_read: TagLoad) -> Self {
        Self {
            text: session.text.status(),
            text_pending_newer: session.text.has_pending_newer_edits(),
            tags: session.tag_status(),
            tags_read,
            tags_pending_newer: session
                .tags
                .as_ref()
                .is_some_and(|t| t.has_pending_newer_selection()),
        }
    }
}

#[cfg(feature = "hydrate")]
fn active_element_id_is(id: &str) -> bool {
    document().active_element().is_some_and(|el| el.id() == id)
}

#[cfg(feature = "hydrate")]
fn sync_save_slot_focus(
    was_save_focused: bool,
    session: RwSignal<DraftSession>,
    tag_load: RwSignal<TagLoad>,
) {
    if !was_save_focused {
        return;
    }
    use wasm_bindgen::JsCast;
    request_animation_frame(move || {
        let doc = document();
        if doc.get_element_by_id("save").is_some()
            || doc
                .active_element()
                .is_some_and(|el| !el.tag_name().eq_ignore_ascii_case("BODY"))
        {
            return;
        }
        // Resolve the active slot after DOM updates. A disposed editor cannot hand off focus
        // to a different draft, and intervening edits may have changed the destination.
        let Some(next_action) = session.try_with_untracked(|s| {
            let tags_read = tag_load.try_get_untracked().unwrap_or(TagLoad::Loading);
            project_save_slot(&SaveSlotFacts::from_session(s, tags_read)).action
        }) else {
            return;
        };
        let target_selector = match next_action {
            SaveSlotAction::None => Some("#editor-save-details > summary"),
            SaveSlotAction::Recover { .. } => Some("#save-recovery-jump"),
            SaveSlotAction::Save { .. } | SaveSlotAction::Retry { .. } => None,
        };
        if let Some(target) = target_selector
            .and_then(|sel| doc.query_selector(sel).ok().flatten())
            .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = target.focus_with_options(&options);
        }
    });
}

fn project_save_slot(facts: &SaveSlotFacts) -> SaveSlotView {
    let text_detail = match (facts.text, facts.text_pending_newer) {
        (SaveStatus::Saved { version }, _) => {
            format!("Đã lưu bản nháp phiên bản {version} trên máy chủ.")
        }
        (SaveStatus::Saving, true) => {
            "Đang chờ máy chủ xác nhận bản đã gửi. Đã gõ thêm chữ mới trong lúc lưu; phần mới vẫn cần được lưu tiếp.".into()
        }
        (SaveStatus::Saving, false) => {
            "Đang chờ máy chủ xác nhận bản nháp đã gửi. Chữ gõ thêm vẫn cần được lưu.".into()
        }
        (SaveStatus::Unsaved, _) => {
            "Thay đổi chữ trên trang này chưa được lưu lên máy chủ.".into()
        }
        (other, _) => status_detail(other),
    };
    let tag_detail = match (&facts.tags, facts.tags_read, facts.tags_pending_newer) {
        (Some(tags), _, true) => format!(
            "{} Đã chọn thẻ khác trong lúc chờ; lựa chọn mới nhất vẫn cần được lưu tiếp.",
            tag_status_detail(tags)
        ),
        (Some(tags), _, false) => tag_status_detail(tags),
        (None, TagLoad::Loading, _) => "Đang tải thông tin thẻ từ máy chủ.".into(),
        (None, TagLoad::Ready, _) => "Chưa có thông tin thẻ.".into(),
        (None, TagLoad::Failed(_), _) => {
            "Không tải được thông tin thẻ từ máy chủ; kiểm tra mục Thông tin tác phẩm bên dưới."
                .into()
        }
    };

    if matches!(facts.text, SaveStatus::Saving) || matches!(facts.tags, Some(TagStatus::Saving)) {
        let status_text = match (facts.text, facts.tags.as_ref()) {
            (SaveStatus::Saving, Some(TagStatus::Saving)) => "Đang lưu chữ và thẻ…",
            (SaveStatus::Saving, Some(TagStatus::Unsaved)) => "Đang lưu chữ; thẻ chưa lưu…",
            (
                SaveStatus::Saving,
                Some(TagStatus::Conflict { .. } | TagStatus::Refused(_) | TagStatus::Failed(_)),
            ) => "Đang lưu chữ; thẻ cần kiểm tra…",
            (SaveStatus::Saving, _) if facts.text_pending_newer => "Đang lưu; có chữ mới…",
            (SaveStatus::Saving, None) if matches!(facts.tags_read, TagLoad::Failed(_)) => {
                "Đang lưu chữ; chưa tải được thẻ."
            }
            (SaveStatus::Saving, _) => "Đang lưu…",
            (SaveStatus::Saved { .. }, Some(TagStatus::Saving)) if facts.tags_pending_newer => {
                "Đang lưu thẻ; có thay đổi mới…"
            }
            (SaveStatus::Saved { .. }, Some(TagStatus::Saving)) => "Đang lưu thẻ…",
            (SaveStatus::Unsaved, Some(TagStatus::Saving)) => "Chưa lưu chữ; đang lưu thẻ…",
            (_, Some(TagStatus::Saving)) => "Chữ cần kiểm tra; đang lưu thẻ…",
            _ => "Đang lưu…",
        };
        return SaveSlotView {
            status_text: status_text.into(),
            status_token: "saving",
            problem_kind: None,
            detail_text: format!("{text_detail} {tag_detail}"),
            action: SaveSlotAction::Save {
                label: "Đang lưu…",
                pending: true,
            },
        };
    }

    if let SaveStatus::Conflict { .. } = facts.text {
        let status_text = match facts.tags.as_ref() {
            Some(TagStatus::Unsaved) => "Có bản mới hơn trên máy chủ; thẻ chưa lưu.",
            Some(TagStatus::Failed(_) | TagStatus::Refused(_) | TagStatus::Conflict { .. }) => {
                "Có bản mới hơn trên máy chủ; thẻ cũng cần xử lý."
            }
            _ => "Có bản mới trên máy chủ.",
        };
        return SaveSlotView {
            status_text: status_text.into(),
            status_token: "conflict",
            problem_kind: Some("conflict"),
            detail_text: format!("{text_detail} {tag_detail}"),
            action: match facts.tags {
                Some(TagStatus::Unsaved) => SaveSlotAction::Save {
                    label: "Lưu thẻ",
                    pending: false,
                },
                Some(TagStatus::Failed(_)) => SaveSlotAction::Retry {
                    label: "Thử lưu lại thẻ",
                    pending: false,
                },
                _ => SaveSlotAction::Recover {
                    label: "Xử lý xung đột",
                    target: RecoveryTarget::TextConflict,
                },
            },
        };
    }

    if let SaveStatus::Failed(failure) = facts.text {
        let text_uncertain = save_is_uncertain(SaveStatus::Failed(failure));
        let problem_kind = Some(if text_uncertain {
            "uncertain"
        } else {
            "rejected"
        });
        let status_text = if text_uncertain {
            match facts.tags.as_ref() {
                None if matches!(facts.tags_read, TagLoad::Failed(_)) => {
                    "Chưa xác nhận lưu chữ; chưa tải được thẻ."
                }
                None | Some(TagStatus::Saved { .. }) => "Chưa xác nhận lưu.",
                Some(TagStatus::Unsaved) => "Chưa xác nhận lưu chữ; thẻ chưa lưu.",
                Some(TagStatus::Failed(tag_failure))
                    if crate::tag_state::uncertain(*tag_failure) =>
                {
                    "Chưa xác nhận lưu chữ và thẻ."
                }
                Some(TagStatus::Conflict { .. }) => "Chưa xác nhận lưu chữ; thẻ đã đổi ở nơi khác.",
                Some(TagStatus::Refused(_)) => "Chưa xác nhận lưu chữ; thẻ bị từ chối.",
                _ => "Chưa xác nhận lưu chữ; thẻ chưa lưu được.",
            }
        } else {
            match facts.tags.as_ref() {
                None if matches!(facts.tags_read, TagLoad::Failed(_)) => {
                    "Chưa lưu được chữ; chưa tải được thẻ."
                }
                None | Some(TagStatus::Saved { .. }) => "Chưa lưu được.",
                Some(TagStatus::Unsaved) => "Chưa lưu được chữ; thẻ chưa lưu.",
                Some(TagStatus::Conflict { .. }) => "Chưa lưu được chữ; thẻ đã đổi ở nơi khác.",
                Some(TagStatus::Refused(_)) => "Chưa lưu được chữ; thẻ bị từ chối.",
                Some(TagStatus::Failed(tag_failure))
                    if crate::tag_state::uncertain(*tag_failure) =>
                {
                    "Chưa lưu được chữ; chưa xác nhận lưu thẻ."
                }
                _ => "Chưa lưu được chữ và thẻ.",
            }
        };
        return SaveSlotView {
            status_text: status_text.into(),
            status_token: "failed",
            problem_kind,
            detail_text: format!("{text_detail} {tag_detail}"),
            action: SaveSlotAction::Retry {
                label: "Thử lưu lại",
                pending: false,
            },
        };
    }

    let text_dirty = matches!(facts.text, SaveStatus::Unsaved);
    match facts.tags.as_ref() {
        Some(TagStatus::Conflict { .. }) => {
            let (status_text, status_token, action) = if text_dirty {
                (
                    "Chưa lưu chữ; thẻ đã đổi ở nơi khác.",
                    "conflict",
                    SaveSlotAction::Save {
                        label: "Lưu chữ",
                        pending: false,
                    },
                )
            } else {
                (
                    "Đã lưu chữ; thẻ đã đổi ở nơi khác.",
                    "conflict",
                    SaveSlotAction::Recover {
                        label: "Xử lý xung đột thẻ",
                        target: RecoveryTarget::TagConflict,
                    },
                )
            };
            SaveSlotView {
                status_text: status_text.into(),
                status_token,
                problem_kind: Some("conflict"),
                detail_text: format!("{text_detail} {tag_detail}"),
                action,
            }
        }
        Some(TagStatus::Refused(_)) => {
            let (status_text, status_token, action) = if text_dirty {
                (
                    "Chưa lưu chữ; thẻ bị từ chối.",
                    "unsaved",
                    SaveSlotAction::Save {
                        label: "Lưu chữ",
                        pending: false,
                    },
                )
            } else {
                (
                    "Đã lưu chữ; thẻ bị từ chối.",
                    "failed",
                    SaveSlotAction::Recover {
                        label: "Kiểm tra thẻ",
                        target: RecoveryTarget::TagRefused,
                    },
                )
            };
            SaveSlotView {
                status_text: status_text.into(),
                status_token,
                problem_kind: Some("refused"),
                detail_text: format!("{text_detail} {tag_detail}"),
                action,
            }
        }
        Some(TagStatus::Failed(tag_failure)) => {
            let uncertain = crate::tag_state::uncertain(*tag_failure);
            let status_text = match (text_dirty, uncertain) {
                (false, true) => "Đã lưu chữ; chưa xác nhận lưu thẻ.",
                (false, false) => "Đã lưu chữ; thẻ chưa lưu được.",
                (true, true) => "Chưa lưu chữ; chưa xác nhận lưu thẻ.",
                (true, false) => "Chưa lưu chữ; thẻ chưa lưu được.",
            };
            let label = if text_dirty {
                "Lưu"
            } else {
                "Thử lưu lại"
            };
            SaveSlotView {
                status_text: status_text.into(),
                status_token: "failed",
                problem_kind: Some(if uncertain { "uncertain" } else { "rejected" }),
                detail_text: format!("{text_detail} {tag_detail}"),
                action: SaveSlotAction::Retry {
                    label,
                    pending: false,
                },
            }
        }
        Some(TagStatus::Unsaved) => {
            let (status_text, problem_kind) = if text_dirty {
                ("Chưa lưu chữ và thẻ", None)
            } else {
                ("Đã lưu chữ; chưa lưu thẻ", Some("partial"))
            };
            SaveSlotView {
                status_text: status_text.into(),
                status_token: "unsaved",
                problem_kind,
                detail_text: format!("{text_detail} {tag_detail}"),
                action: SaveSlotAction::Save {
                    label: "Lưu",
                    pending: false,
                },
            }
        }
        Some(TagStatus::Saved { .. }) => {
            if text_dirty {
                SaveSlotView {
                    status_text: "Chưa lưu chữ".into(),
                    status_token: "unsaved",
                    problem_kind: Some("partial"),
                    detail_text: format!("{text_detail} {tag_detail}"),
                    action: SaveSlotAction::Save {
                        label: "Lưu",
                        pending: false,
                    },
                }
            } else {
                let version = match facts.text {
                    SaveStatus::Saved { version } => version,
                    _ => 0,
                };
                SaveSlotView {
                    status_text: "Đã lưu".into(),
                    status_token: "saved",
                    problem_kind: None,
                    detail_text: format!(
                        "Đã lưu bản nháp phiên bản {version} và thẻ đã chọn trên máy chủ."
                    ),
                    action: SaveSlotAction::None,
                }
            }
        }
        Some(TagStatus::Saving) | None => {
            if text_dirty {
                SaveSlotView {
                    status_text: match facts.tags_read {
                        TagLoad::Failed(_) => "Chưa lưu chữ; chưa tải được thẻ",
                        _ => "Chưa lưu chữ",
                    }
                    .into(),
                    status_token: "unsaved",
                    problem_kind: None,
                    detail_text: format!("{text_detail} {tag_detail}"),
                    action: SaveSlotAction::Save {
                        label: "Lưu",
                        pending: false,
                    },
                }
            } else {
                let (status_text, status_token, action) = match facts.tags_read {
                    TagLoad::Failed(_) => (
                        "Đã lưu chữ; chưa tải được thẻ",
                        "failed",
                        SaveSlotAction::Recover {
                            label: "Kiểm tra tải thẻ",
                            target: RecoveryTarget::TagLoad,
                        },
                    ),
                    TagLoad::Loading | TagLoad::Ready => {
                        ("Đã lưu chữ; đang tải thẻ…", "loading", SaveSlotAction::None)
                    }
                };
                SaveSlotView {
                    status_text: status_text.into(),
                    status_token,
                    problem_kind: Some("partial"),
                    detail_text: format!("{text_detail} {tag_detail}"),
                    action,
                }
            }
        }
    }
}

/// The toolbar's one status line for both parts. Text states keep their words; tag states are
/// named as tags, so a saved text never reads as saved tags.
#[cfg_attr(not(test), allow(dead_code))]
fn draft_status_text(text: SaveStatus, tags: Option<&TagStatus>) -> String {
    project_save_slot(&SaveSlotFacts {
        text,
        text_pending_newer: false,
        tags: tags.cloned(),
        tags_read: if tags.is_some() {
            TagLoad::Ready
        } else {
            TagLoad::Loading
        },
        tags_pending_newer: false,
    })
    .status_text
}

#[cfg_attr(not(test), allow(dead_code))]
fn draft_status_token(text: SaveStatus, tags: Option<&TagStatus>) -> &'static str {
    project_save_slot(&SaveSlotFacts {
        text,
        text_pending_newer: false,
        tags: tags.cloned(),
        tags_read: if tags.is_some() {
            TagLoad::Ready
        } else {
            TagLoad::Loading
        },
        tags_pending_newer: false,
    })
    .status_token
}

#[cfg_attr(not(test), allow(dead_code))]
fn draft_status_detail(text: SaveStatus, tags: Option<&TagStatus>) -> String {
    project_save_slot(&SaveSlotFacts {
        text,
        text_pending_newer: false,
        tags: tags.cloned(),
        tags_read: if tags.is_some() {
            TagLoad::Ready
        } else {
            TagLoad::Loading
        },
        tags_pending_newer: false,
    })
    .detail_text
}

/// The short state shown beside “Thông tin tác phẩm”.
pub(crate) fn tag_status_label(status: &TagStatus) -> &'static str {
    match status {
        TagStatus::Saved { .. } => "Đã lưu thẻ",
        TagStatus::Unsaved => "Chưa lưu thẻ",
        TagStatus::Saving => "Đang lưu thẻ…",
        TagStatus::Conflict { .. } => "Thẻ đã đổi ở nơi khác",
        TagStatus::Failed(failure) if crate::tag_state::uncertain(*failure) => {
            "Chưa xác nhận lưu thẻ"
        }
        TagStatus::Failed(_) | TagStatus::Refused(_) => "Thẻ chưa lưu được",
    }
}

pub(crate) fn tag_status_detail(status: &TagStatus) -> String {
    match status {
        TagStatus::Saved { .. } => "Thẻ đã lưu.".into(),
        TagStatus::Unsaved => "Thẻ có thay đổi chưa lưu.".into(),
        TagStatus::Saving => "Đang chờ máy chủ xác nhận thẻ đã gửi.".into(),
        TagStatus::Conflict { .. } => {
            "Thẻ đã được lưu khác ở nơi khác; chưa ghi đè lựa chọn nào.".into()
        }
        TagStatus::Refused(refusal) => crate::ui::adapters::tag_refusal_text(refusal),
        TagStatus::Failed(failure) => format!(
            "{} Lựa chọn còn trên trang này.",
            tag_failure_reason(*failure)
        ),
    }
}

pub(crate) fn tag_failure_reason(failure: Failure) -> &'static str {
    match failure {
        Failure::Network => "Mất phản hồi từ máy chủ; chưa rõ thẻ đã được lưu hay chưa.",
        Failure::Unexpected => "Phản hồi không xác định; chưa rõ thẻ đã được lưu hay chưa.",
        other => failure_reason(other),
    }
}

#[cfg(feature = "hydrate")]
pub(crate) fn failure(error: &ApiError) -> Failure {
    match error {
        ApiError::Network => Failure::Network,
        ApiError::Unavailable => Failure::Unavailable,
        ApiError::Unauthenticated => Failure::Unauthenticated,
        ApiError::Forbidden => Failure::Forbidden,
        ApiError::NotFound => Failure::NotFound,
        ApiError::Invalid { .. } => Failure::Invalid,
        // Draft routes never answer the discussion or endorsement codes; treat them like any
        // unexpected reply.
        ApiError::VersionConflict { .. }
        | ApiError::CommentsClosed
        | ApiError::TargetUnavailable
        | ApiError::KeyReused
        | ApiError::CommentRemoved
        | ApiError::SelfEndorsement
        | ApiError::EndorsementNotPermitted
        | ApiError::EndorsementConflict { .. }
        | ApiError::RevisionChanged { .. }
        | ApiError::TagConflict { .. }
        | ApiError::TagRefused(_)
        | ApiError::SnapshotConflict { .. }
        | ApiError::GenerationConflict { .. }
        | ApiError::RevisionRejected
        | ApiError::UnknownRevision
        | ApiError::WorkUnavailable
        | ApiError::Unexpected { .. } => Failure::Unexpected,
    }
}

/// The primitive uses aria-disabled and retains focus; the application still guards effects.
fn blocked(disabled: bool) -> ActionState {
    if disabled {
        ActionState::Disabled { reason: None }
    } else {
        ActionState::Enabled
    }
}

#[cfg(test)]
mod editor_presentation_tests {
    use super::*;
    use crate::desk_state::EditorSession;
    use crate::tag_state::TagStatus;

    #[test]
    fn the_toolbar_names_tag_states_and_keeps_text_words_when_tags_are_saved() {
        let saved_tags = TagStatus::Saved { version: 2 };
        let text = SaveStatus::Saved { version: 7 };
        assert_eq!(draft_status_text(text, Some(&saved_tags)), "Đã lưu");
        assert_eq!(draft_status_token(text, Some(&saved_tags)), "saved");
        assert_eq!(draft_status_text(text, None), "Đã lưu chữ; đang tải thẻ…");
        assert_eq!(draft_status_token(text, None), "loading");
        assert_eq!(
            draft_status_text(text, Some(&TagStatus::Unsaved)),
            "Đã lưu chữ; chưa lưu thẻ",
            "a tag-only change names the partial state truthfully"
        );
        assert_eq!(draft_status_token(text, Some(&TagStatus::Saving)), "saving");
        assert_eq!(
            draft_status_text(text, Some(&TagStatus::Failed(Failure::Network))),
            "Đã lưu chữ; chưa xác nhận lưu thẻ."
        );
        assert_eq!(
            draft_status_text(text, Some(&TagStatus::Failed(Failure::Unauthenticated))),
            "Đã lưu chữ; thẻ chưa lưu được."
        );
        // Text conflict keeps conflict priority while still naming dirty tags truthfully.
        let conflict = SaveStatus::Conflict { server_version: 9 };
        assert_eq!(
            draft_status_text(conflict, Some(&TagStatus::Unsaved)),
            "Có bản mới hơn trên máy chủ; thẻ chưa lưu."
        );
        assert_eq!(
            draft_status_text(conflict, Some(&saved_tags)),
            "Có bản mới trên máy chủ."
        );
        let detail = draft_status_detail(text, Some(&TagStatus::Unsaved));
        assert!(detail.contains("phiên bản 7") && detail.contains("Thẻ có thay đổi chưa lưu."));
        assert!(!draft_status_detail(text, Some(&saved_tags)).contains("phiên bản 2"));
    }

    #[test]
    fn unified_save_slot_projection_covers_clean_partial_pending_refused_and_conflict() {
        use crate::contract::{TagRefusal, TagRefusalCode};
        use crate::tag_state::Selection;

        let saved_tags = TagStatus::Saved { version: 2 };
        let refused = TagStatus::Refused(TagRefusal {
            code: TagRefusalCode::Retired,
            field: Some("themes".into()),
            tag_id: Some("ky-uc".into()),
            successor_id: None,
        });

        // Clean state: no prominent Save CTA, explicit server explanation, no local/device save claim.
        let clean = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Saved { version: 5 },
            text_pending_newer: false,
            tags: Some(saved_tags.clone()),
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
        });
        assert_eq!(clean.action, SaveSlotAction::None);
        assert_eq!(clean.status_token, "saved");
        assert!(clean.detail_text.contains("phiên bản 5"));
        assert!(clean.detail_text.contains("trên máy chủ"));
        assert!(
            !clean.detail_text.contains("Đã lưu trên máy")
                && !clean.detail_text.contains("trên thiết bị")
        );

        // Failed tag catalog/draft load while text saved is disclosed truthfully in detail.
        let tag_load_failed = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Saved { version: 5 },
            text_pending_newer: false,
            tags: None,
            tags_read: TagLoad::Failed(Failure::Unavailable),
            tags_pending_newer: false,
        });
        assert_eq!(
            tag_load_failed.action,
            SaveSlotAction::Recover {
                label: "Kiểm tra tải thẻ",
                target: RecoveryTarget::TagLoad,
            }
        );
        assert_eq!(tag_load_failed.status_text, "Đã lưu chữ; chưa tải được thẻ");

        // Text dirty + tags saved vs tags dirty + text saved vs both dirty.
        let text_only = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Unsaved,
            text_pending_newer: false,
            tags: Some(saved_tags.clone()),
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
        });
        assert_eq!(text_only.status_text, "Chưa lưu chữ");
        assert_eq!(
            text_only.action,
            SaveSlotAction::Save {
                label: "Lưu",
                pending: false
            }
        );

        let both_dirty = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Unsaved,
            text_pending_newer: false,
            tags: Some(TagStatus::Unsaved),
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
        });
        assert_eq!(both_dirty.status_text, "Chưa lưu chữ và thẻ");
        assert_eq!(both_dirty.problem_kind, None);

        // Pending with newer local edits while saving keeps newer edit truth visible.
        let pending_newer = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Saving,
            text_pending_newer: true,
            tags: Some(saved_tags.clone()),
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
        });
        assert_eq!(pending_newer.status_token, "saving");
        assert_eq!(pending_newer.status_text, "Đang lưu; có chữ mới…");
        assert!(pending_newer.detail_text.contains("gõ thêm chữ mới"));

        // Refused tags with clean text routes directly to tag fix, never offering fake Save/retry.
        let refused_clean_text = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Saved { version: 5 },
            text_pending_newer: false,
            tags: Some(refused.clone()),
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
        });
        assert_eq!(
            refused_clean_text.action,
            SaveSlotAction::Recover {
                label: "Kiểm tra thẻ",
                target: RecoveryTarget::TagRefused,
            }
        );
        assert_eq!(refused_clean_text.problem_kind, Some("refused"));

        // Refused tags with dirty text offers saving text only.
        let refused_dirty_text = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Unsaved,
            text_pending_newer: false,
            tags: Some(refused),
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
        });
        assert_eq!(
            refused_dirty_text.action,
            SaveSlotAction::Save {
                label: "Lưu chữ",
                pending: false
            }
        );

        // Tag conflict with clean text routes to tag conflict recovery; text conflict routes to text recovery.
        let tag_conflict = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Saved { version: 5 },
            text_pending_newer: false,
            tags: Some(TagStatus::Conflict {
                server_version: 3,
                stored: Selection::default(),
            }),
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
        });
        assert_eq!(
            tag_conflict.action,
            SaveSlotAction::Recover {
                label: "Xử lý xung đột thẻ",
                target: RecoveryTarget::TagConflict,
            }
        );

        let text_conflict = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Conflict { server_version: 9 },
            text_pending_newer: false,
            tags: Some(saved_tags),
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
        });
        assert_eq!(
            text_conflict.action,
            SaveSlotAction::Recover {
                label: "Xử lý xung đột",
                target: RecoveryTarget::TextConflict,
            }
        );
    }

    #[test]
    fn the_leave_dialog_says_which_part_is_unsaved_or_blocked() {
        let text = SaveStatus::Saved { version: 3 };
        let saved = TagStatus::Saved { version: 1 };
        assert!(
            leave_message(None, text, Some(&TagStatus::Unsaved), (false, true))
                .starts_with("Thẻ đã chọn")
        );
        assert!(
            leave_message(
                None,
                SaveStatus::Unsaved,
                Some(&TagStatus::Unsaved),
                (true, true)
            )
            .starts_with("Chữ và thẻ")
        );
        assert!(
            leave_message(None, SaveStatus::Unsaved, Some(&saved), (true, false))
                .starts_with("Những thay đổi gần nhất")
        );
        let notice = Some(LeaveStage::Choosing {
            notice: Some(LeaveNotice::NotSaved),
        });
        assert!(
            leave_message(
                notice,
                text,
                Some(&TagStatus::Failed(Failure::Unavailable)),
                (false, true)
            )
            .starts_with("Thẻ chưa lưu được.")
        );
        let stored = crate::tag_state::Selection::default();
        assert!(
            leave_message(
                None,
                text,
                Some(&TagStatus::Conflict {
                    server_version: 4,
                    stored
                }),
                (false, true)
            )
            .contains("Không thể lưu đè từ đây.")
        );
    }

    #[test]
    fn compact_saved_status_keeps_only_the_acknowledged_version_in_detail() {
        let mut session = EditorSession::loaded(6);
        session.edited();
        let ticket = session.begin_save().expect("edited snapshot");
        session.edited();
        session.save_succeeded(ticket, 7);
        assert_eq!(status_text(session.status()), "Chưa lưu");
        assert!(!status_detail(session.status()).contains("Đã lưu"));
        let ticket = session.begin_save().expect("newer text still needs saving");
        session.save_succeeded(ticket, 8);
        assert_eq!(status_text(session.status()), "Đã lưu");
        assert_eq!(
            status_detail(session.status()),
            "Đã lưu bản nháp phiên bản 8."
        );
    }

    #[test]
    fn unknown_rejected_and_conflict_have_distinct_truthful_recovery_copy() {
        for failure in [Failure::Network, Failure::Unavailable, Failure::Unexpected] {
            assert_eq!(
                status_text(SaveStatus::Failed(failure)),
                "Chưa xác nhận lưu."
            );
            assert!(failure_detail(failure).contains("Máy chủ chưa xác nhận."));
        }
        for failure in [
            Failure::Unauthenticated,
            Failure::Forbidden,
            Failure::NotFound,
            Failure::Invalid,
        ] {
            assert_eq!(status_text(SaveStatus::Failed(failure)), "Chưa lưu được.");
            assert!(failure_detail(failure).starts_with(failure_reason(failure)));
            assert!(!failure_detail(failure).contains("Máy chủ chưa xác nhận."));
        }
        for failure in [Failure::Network, Failure::Unauthenticated] {
            assert!(
                failure_detail(failure)
                    .contains("Nội dung còn trên trang này; chưa có bản khôi phục tự động.")
            );
        }
        let conflict = SaveStatus::Conflict { server_version: 9 };
        assert_eq!(status_text(conflict), "Có bản mới trên máy chủ.");
        assert!(status_detail(conflict).contains("phiên bản 9"));
        assert!(status_detail(conflict).contains("chưa được lưu vào bản đó"));
    }
}
