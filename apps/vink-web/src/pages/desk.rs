//! Bàn viết: private drafts, loaded only in the browser through the authenticated API.
//! Server rendering emits the shell and a loading state, never draft content.

use leptos::html;
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params_map;

use vink_ui::{
    ActionState, Announce, Button, ButtonKind, ButtonVariant, ChoiceOption, EmptyKind, Heading,
    HeadingLevel, InlineNotice, PanelContent, PanelKind, SearchField, Select, StatePanel,
    TextField, Tone, TypeRole,
};

use crate::contract::{ApiError, DocumentDto, Draft, DraftList};
use crate::desk_state::{
    EditorSession, Failure, LeaveGuard, LeaveNotice, LeaveStage, SaveStatus, SaveTicket,
};
use crate::document::DocumentView;
use crate::draft_list::{CreateState, DraftSort, display_title as untitled, visible_drafts};
use crate::editor::{BODY_HINT_ID, BODY_LABEL_ID, BodyEditor, BodySource, BodyState, TOOLBAR_ID};
use crate::navigation::NavigationGuard;
use crate::ui::{
    adapters,
    patterns::{DraftResume, DraftRow},
};

const DESK_UNAVAILABLE_SSR: &str = "Bàn viết cần JavaScript để mở bản nháp riêng.";

#[component]
pub fn DeskIndex() -> impl IntoView {
    let query = RwSignal::new(String::new());
    let sort = RwSignal::new(DraftSort::Recent);
    let creation = RwSignal::new(CreateState::Ready);
    let loading = RwSignal::new(true);
    let drafts: LocalResource<Result<DraftList, ApiError>> =
        LocalResource::new(move || async move {
            let result = load_drafts().await;
            if result.is_ok() {
                creation.try_update(CreateState::reconciled);
            }
            loading.try_update(|value| *value = false);
            result
        });
    let refresh = Callback::new(move |()| {
        if loading.get_untracked()
            || creation.with_untracked(|state| *state == CreateState::Pending)
        {
            return;
        }
        loading.set(true);
        drafts.refetch();
    });
    let refresh_disabled =
        Signal::derive(move || loading.get() || creation.get() == CreateState::Pending);
    let can_create = Signal::derive(move || {
        !loading.get()
            && drafts.get().is_some_and(|result| result.is_ok())
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
            leptos::task::spawn_local(async move {
                let outcome = crate::api::create_draft().await;
                // The route owner disposes this signal. A late reply must not navigate from a
                // different page (and an uncertain POST is never automatically repeated).
                if creation.try_get_untracked().is_none() {
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
    view! {
        <Title text="Bàn viết — Vink" />
        <Meta name="robots" content="noindex" />
        <section id="desk" class="vk-desk vk-desk--index" aria-labelledby="desk-title">
            <header id="heading" class="vk-desk-heading">
                <Heading level=HeadingLevel::H1 id="desk-title">"Bàn viết"</Heading>
                <Show when=move || drafts.get().is_some_and(|result| result.is_ok_and(|list| !list.drafts.is_empty()))>
                    <CreateDraftButton creation can_create create />
                </Show>
            </header>
            <noscript><p class="vk-desk-noscript">{DESK_UNAVAILABLE_SSR}</p></noscript>
            <CreateNotice creation loading refresh />
            <div aria-busy=move || if loading.get() { "true" } else { "false" } data-testid="draft-content">
                <Suspense fallback=|| view! {
                    <p class="vk-desk-loading vk-muted" role="status" data-testid="draft-loading">"Đang mở Bàn viết…"</p>
                }>
                    {move || Suspend::new(async move {
                        match drafts.await {
                            Ok(list) if list.drafts.is_empty() => view! {
                                <div class="vk-desk-empty" data-testid="draft-empty">
                                    <StatePanel content=PanelContent { kind: PanelKind::Empty(EmptyKind::FirstUse), title: "Bắt đầu một bản thảo".into(), message: "Một trang dành riêng cho những điều bạn muốn viết.".into(), actions: vec![], code: None } />
                                    <CreateDraftButton creation can_create create first=true />
                                </div>
                            }.into_any(),
                            Ok(list) => view! { <DraftListView list query sort refresh refresh_disabled /> }.into_any(),
                            Err(error) => view! {
                                <DeskError error />
                                <Button variant=ButtonVariant::Tonal
                                    id="retry-drafts" on_press=move |_| refresh.run(())
                                    state=Signal::derive(move || blocked(loading.get()))>"Thử mở lại"</Button>
                            }.into_any(),
                        }
                    })}
                </Suspense>
            </div>
            <p class="vk-desk-privacy vk-muted">
                "Bản nháp chỉ bạn đọc được. Lưu nháp không gửi duyệt hay công khai."
            </p>
        </section>
    }
}

async fn load_drafts() -> Result<DraftList, ApiError> {
    #[cfg(feature = "hydrate")]
    {
        crate::api::list_drafts().await
    }
    #[cfg(not(feature = "hydrate"))]
    {
        Err(ApiError::Unavailable)
    }
}

#[component]
fn CreateDraftButton(
    creation: RwSignal<CreateState>,
    can_create: Signal<bool>,
    create: Callback<()>,
    #[prop(default = false)] first: bool,
) -> impl IntoView {
    view! {
        <Button variant=if first { ButtonVariant::Filled } else { ButtonVariant::Tonal }
            id="create-draft" on_press=move |_| create.run(())
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
        let (title, detail) = match creation.get() {
            CreateState::Uncertain => (
                "Chưa xác nhận được việc tạo bản thảo.",
                "Bản thảo có thể đã được tạo. Hãy tải lại danh sách và kiểm tra trước khi tạo thêm.",
            ),
            CreateState::Reconciled => (
                "Đã tải lại danh sách bản thảo.",
                "Hãy kiểm tra bản thảo gần đây. Nếu vẫn muốn tạo bản khác, chọn Tạo thêm bản thảo.",
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
        view! {
            <div data-testid="create-notice">
                <InlineNotice title=title message=detail tone=Tone::Warning announce=Announce::Polite />
                <Button variant=ButtonVariant::Tonal id="reconcile-drafts"
                    on_press=move |_| refresh.run(()) state=Signal::derive(move || blocked(loading.get()))>
                    {move || if loading.get() { "Đang tải lại…" } else { "Tải lại danh sách" }}
                </Button>
            </div>
        }.into_any()
    }
}

#[component]
fn DraftListView(
    list: DraftList,
    query: RwSignal<String>,
    sort: RwSignal<DraftSort>,
    refresh: Callback<()>,
    refresh_disabled: Signal<bool>,
) -> impl IntoView {
    // GET /drafts already returns the newest accessible card first. Search/sort only changes
    // the list below; resume never fetches a manuscript or changes with a title query.
    let newest = list.drafts[0].clone();
    let count = list.drafts.len();
    let cards = StoredValue::new(list.drafts);
    let visible = Memo::new(move |_| {
        cards.with_value(|cards| visible_drafts(cards, &query.get(), sort.get()))
    });
    let scope = if count >= 100 {
        "Đã tải 100 bản thảo gần đây nhất. Tìm kiếm chỉ trong danh sách này; có thể còn bản thảo khác.".to_string()
    } else {
        format!("Đã tải {count} bản thảo gần đây. Tìm kiếm chỉ trong danh sách này.")
    };
    view! {
        <section id="resume" class="vk-resume" aria-labelledby="resume-label" data-testid="draft-resume">
            <DraftResume draft=adapters::draft_summary(&newest) />
        </section>
        <section id="drafts" class="vk-draft-list" aria-labelledby="draft-list-title">
            <Heading id="draft-list-title" level=HeadingLevel::H2 role=TypeRole::Title>"Bản thảo gần đây"</Heading>
            <div id="draft-tools" class="vk-draft-tools">
                <SearchField id="draft-search" label="Tìm theo tiêu đề" value=query
                    placeholder="Có dấu hoặc không dấu" on_input=move |input: vink_ui::TextInput| query.set(input.value) />
                <Select id="draft-sort" label="Sắp xếp"
                    options=vec![ChoiceOption::new("recent", "Mới cập nhật"), ChoiceOption::new("title", "Tên A–Z (không dấu)")]
                    value=Signal::derive(move || Some(if sort.get() == DraftSort::Title { "title" } else { "recent" }.to_string()))
                    on_change={move |value: Option<String>| sort.set(if value.as_deref() == Some("title") { DraftSort::Title } else { DraftSort::Recent })} />
                <Button variant=ButtonVariant::Text id="refresh-drafts"
                    state=Signal::derive(move || blocked(refresh_disabled.get())) on_press=move |_| refresh.run(())>"Tải lại"</Button>
            </div>
            <p id="draft-scope" class="vk-muted" data-testid="draft-scope">{scope}</p>
            <p class="vk-muted" role="status" data-testid="draft-result-count">
                {move || format!("Hiển thị {} / {count} bản thảo đã tải", visible.with(Vec::len))}
            </p>
            <Show when=move || visible.with(Vec::is_empty)>
                <div data-testid="draft-no-results">
                    <StatePanel content=PanelContent { kind: PanelKind::Empty(EmptyKind::NoResults), title: "Không có tiêu đề phù hợp".into(), message: "Thử một từ khác hoặc xem lại các bản thảo đã tải.".into(), actions: vec![], code: None } />
                    <Button variant=ButtonVariant::Tonal id="clear-draft-search"
                        on_press=move |_| query.set(String::new())>"Xem các bản thảo đã tải"</Button>
                </div>
            </Show>
            <ul id="draft-results" class="vk-draft-rows" aria-label="Bản thảo đã tải" role="list">
                <For each=move || visible.get() key=|draft| draft.work_id.clone() children=move |draft| view! {
                    <li class="vk-draft-row" data-testid="draft-row">
                        <DraftRow draft=adapters::draft_summary(&draft) />
                    </li>
                } />
            </ul>
        </section>
    }.into_any()
}

#[component]
fn DeskError(error: ApiError) -> impl IntoView {
    let (title, detail) = match error {
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
        <div data-testid="desk-error">
            <InlineNotice title=title message=detail tone=Tone::Error announce=Announce::Assertive />
            <vink_ui::Link href="/" variant=vink_ui::LinkVariant::Standalone>"Khám phá tác phẩm"</vink_ui::Link>
        </div>
    }
}

#[component]
pub fn DraftEditor() -> impl IntoView {
    let params = use_params_map();
    let work_id = move || params.read().get("id").unwrap_or_default();
    let draft: LocalResource<Result<Draft, ApiError>> = LocalResource::new(move || {
        let id = work_id();
        async move {
            #[cfg(feature = "hydrate")]
            {
                crate::api::load_draft(id).await
            }
            #[cfg(not(feature = "hydrate"))]
            {
                let _ = id;
                Err(ApiError::Unavailable)
            }
        }
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
}

#[component]
fn Editor(draft: Draft) -> impl IntoView {
    let work_id = StoredValue::new(draft.work_id.clone());
    let title = RwSignal::new(draft.title.clone());
    let body = BodySource::new(crate::document::document_to_text(&draft.document));
    let session = RwSignal::new(EditorSession::loaded(draft.version));
    let leave = RwSignal::new(LeaveGuard::default());
    let navigation = use_context::<NavigationGuard>();
    let title_input = NodeRef::<html::Input>::new();
    let leave_open = RwSignal::new(false);
    let keep_editing = NodeRef::<html::Button>::new();
    let last_field = StoredValue::new(Field::Body);
    let page_scroll = StoredValue::new(0.0_f64);
    let preview = RwSignal::new(None::<(String, DocumentDto)>);

    // Sends the text as it is now. The ticket was taken in the same turn, so it covers exactly
    // this text. The outcome settles the session first; only then may the leave intent go.
    let send = move |ticket: SaveTicket, reconcile: bool| {
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
                    .try_with_untracked(|s| s.in_flight() == Some(ticket))
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
                let applied = session.try_update(|s| match outcome {
                    Ok(saved) => s.save_succeeded(ticket, saved.version),
                    Err(ApiError::VersionConflict { current_version }) => {
                        s.save_rejected(ticket, SaveRejection::Conflict { current_version })
                    }
                    Err(error) => s.save_rejected(ticket, SaveRejection::Failed(failure(&error))),
                });
                if applied.is_none() {
                    return;
                }
                let destination = leave
                    .try_update(|g| session.with_untracked(|s| g.save_settled(s, ticket)))
                    .flatten();
                if let Some(destination) = destination {
                    leave_open.set(false);
                    restore_scroll(page_scroll);
                    if let Some(navigation) = navigation {
                        navigation.go(&destination);
                    }
                }
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (request, work_id, navigation, page_scroll, reconcile);
    };
    let save = move || {
        let reconcile = session.with_untracked(|s| matches!(s.status(), SaveStatus::Failed(_)));
        if let Some(ticket) = session.try_update(EditorSession::begin_save).flatten() {
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
    };
    let stay = move || {
        leave.update(LeaveGuard::stay);
        leave_open.set(false);
        restore_scroll(page_scroll);
        request_animation_frame(move || {
            focus_last_field();
            restore_scroll(page_scroll);
        });
    };
    let save_and_leave = move || {
        let reconcile = session.with_untracked(|s| matches!(s.status(), SaveStatus::Failed(_)));
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
            if let Some(button) = keep_editing.get_untracked() {
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
                remember_scroll(page_scroll);
                leave_open.set(true);
            }
            held
        });
        on_cleanup(move || navigation.unregister(registration));
    }

    #[cfg(feature = "hydrate")]
    {
        let guard = window_event_listener(leptos::ev::beforeunload, move |event| {
            let unsaved = session.with_untracked(EditorSession::has_unsaved_text);
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
        session.try_update(EditorSession::edited);
    });

    let status = move || session.with(EditorSession::status);
    let stage = move || leave.with(|g| g.pending().map(|intent| intent.stage));
    let can_discard = move || leave.with(|g| session.with(|s| g.can_discard(s)));
    view! {
        <form
            class="vk-editor"
            aria-labelledby="editor-heading"
            on:submit=move |event| {
                event.prevent_default();
                save();
            }
            on:keydown=move |event| {
                let shortcut = (event.ctrl_key() || event.meta_key())
                    && event.key().eq_ignore_ascii_case("s");
                if shortcut {
                    event.prevent_default();
                    // An IME still composing commits first; saving half a syllable helps no one.
                    if !event.is_composing() && !body.is_composing() {
                        save();
                    }
                }
            }
        >
            <crate::ui::patterns::EditorToolbar attr:id=TOOLBAR_ID attr:data-testid="editor-toolbar"
                status=Signal::derive(move || status_text(status()).to_string())
                status_token=Signal::derive(move || status().token().to_string())>
                    <Button variant=ButtonVariant::Text
                        id="preview"
                        on_press=move |_| open_preview()
                    >
                        "Xem trước"
                    </Button>
                    <Button variant=ButtonVariant::Text
                        id="export"
                        on_press=move |_| export()
                    >
                        "Tải .txt"
                    </Button>
                    <Button variant=ButtonVariant::Filled
                        kind=ButtonKind::Submit

                        id="save"
                        state=Signal::derive(move || if status() == SaveStatus::Saving { ActionState::pending("Đang lưu…") } else { blocked(!session.with(EditorSession::can_save)) })
                    >
                        "Lưu nháp"
                    </Button>
            </crate::ui::patterns::EditorToolbar>
            <Show when=move || matches!(status(), SaveStatus::Failed(_))>
                <div data-testid="save-failed">
                    {move || view! { <InlineNotice tone=Tone::Error announce=Announce::Polite
                        title=if save_is_uncertain(status()) { "Chưa xác nhận lưu bản nháp." } else { "Chưa lưu được bản nháp." }
                        message=format!("{} Chữ vẫn còn nguyên trên trang này. Chọn Thử lưu lại để đối chiếu bản trên máy chủ, hoặc tải bản đang viết để giữ trên máy.", match status() { SaveStatus::Failed(failure) => failure_reason(failure), _ => "" }) /> }}
                    <div class="vk-notice__actions">
                        <Button variant=ButtonVariant::Tonal
                            id="retry-save"
                            on_press=move |_| save()
                        >
                            "Thử lưu lại"
                        </Button>
                        <Button variant=ButtonVariant::Text
                            on_press=move |_| export()
                        >
                            "Tải bản đang viết"
                        </Button>
                    </div>
                </div>
            </Show>
            <Show when=move || matches!(status(), SaveStatus::Conflict { .. })>
                <div data-testid="conflict">
                    <InlineNotice tone=Tone::Warning announce=Announce::Polite title="Có phiên bản mới trên máy chủ."
                        message="Nội dung bạn đang viết vẫn còn nguyên trên trang này. Bạn có thể lưu đè bằng nội dung này, hoặc tải bản đang viết trước khi mở lại bản trên máy chủ." />
                    <div class="vk-notice__actions">
                        <Button variant=ButtonVariant::Tonal
                            id="keep-mine"
                            on_press=move |_| {
                                session.update(EditorSession::keep_local_text);
                                save();
                            }
                        >
                            "Lưu đè bằng nội dung đang viết"
                        </Button>
                        <Button variant=ButtonVariant::Text
                            on_press=move |_| export()
                        >
                            "Tải bản đang viết"
                        </Button>
                    </div>
                </div>
            </Show>
            <div class="vk-editor-paper">
                <TextField id="draft-title" label="Tiêu đề" name="title" max_length=200 input_ref=title_input
                    placeholder="Chưa đặt tên" autocomplete="off" value=title
                    on_focus=move |_| last_field.set_value(Field::Title)
                    on_input=move |input: vink_ui::TextInput| {
                        if title.get_untracked() != input.value {
                            title.set(input.value);
                            session.update(EditorSession::edited);
                        }
                    } />
                <div class="vk-field" on:focusin=move |_| last_field.set_value(Field::Body)>
                    <span
                        id=BODY_LABEL_ID
                        class="vk-field__label"
                        on:click=move |_| body.focus()
                    >
                        "Nội dung"
                    </span>
                    <span id=BODY_HINT_ID class="vk-field__hint">
                        "Ctrl/⌘ + S để lưu nháp."
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
        </form>
        <vink_ui::Dialog id="leave-editor" open=leave_open
            title=Signal::derive(move || if stage() == Some(LeaveStage::ConfirmingDiscard) { "Bỏ chữ chưa lưu và rời trang?" } else { "Rời trang khi còn chữ chưa lưu?" }.to_string())
            dismiss=vink_ui::DismissPolicy::EscapeOnly on_dismiss=move |_| stay()
            attr:data-testid="leave-editor">
            <Show
                when=move || stage() == Some(LeaveStage::ConfirmingDiscard)
                fallback=move || {
                    let saving = move || matches!(stage(), Some(LeaveStage::Saving { .. }));
                    view! {
                        <p class="vk-dialog__body" data-testid="leave-message">
                            {move || leave_message(stage(), status())}
                        </p>
                        <div class="vk-dialog__actions">
                            <Button variant=ButtonVariant::Text
                                id="leave-stay"
                                on_press=move |_| stay()
                            >
                                "Ở lại"
                            </Button>
                            <Button variant=ButtonVariant::Filled
                                id="leave-save"
                                state=Signal::derive(move || blocked(saving() || matches!(status(), SaveStatus::Conflict { .. })))
                                on_press=move |_| save_and_leave()
                            >
                                {move || if saving() { "Đang lưu…" } else { "Lưu và rời trang" }}
                            </Button>
                        </div>
                        <Show when=can_discard>
                            <div class="vk-dialog__recovery" data-testid="leave-recovery">
                                <Button variant=ButtonVariant::Tonal
                                    id="leave-export"
                                    on_press=move |_| export()
                                >
                                    "Tải bản đang viết (.txt)"
                                </Button>
                                <Button variant=ButtonVariant::Danger
                                    id="leave-discard"
                                    on_press=move |_| ask_discard()
                                >
                                    "Rời mà không lưu…"
                                </Button>
                            </div>
                        </Show>
                    }
                }
            >
                <p class="vk-dialog__body">
                    "Chữ chưa được máy chủ xác nhận sẽ mất khỏi trang này. Tệp .txt đã tải (nếu có) chỉ nằm trên máy của bạn, không phải bản lưu trên máy chủ."
                </p>
                <div class="vk-dialog__actions">
                    <Button variant=ButtonVariant::Text
                        id="discard-back"
                        button_ref=keep_editing
                        on_press=move |_| leave.update(LeaveGuard::keep_choosing)
                    >
                        "Quay lại"
                    </Button>
                    <Button variant=ButtonVariant::Danger
                        id="discard-confirm"
                        on_press=move |_| confirm_discard()
                    >
                        "Bỏ thay đổi và rời"
                    </Button>
                </div>
            </Show>
        </vink_ui::Dialog>
        <vink_ui::Dialog id="draft-preview" open=Signal::derive(move || preview.with(Option::is_some))
            title="Xem trước bản nháp" focus_title=true
            dismiss=vink_ui::DismissPolicy::EscapeOnly
            on_dismiss=move |_| { preview.set(None); restore_scroll(page_scroll); }
            attr:data-testid="draft-preview">
            <p class="vk-dialog__body vk-muted">
                "Dùng chữ đang có trên trang, kể cả phần chưa lưu. Chỉ bạn thấy; chưa gửi duyệt, chưa công khai."
            </p>
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
            <div class="vk-dialog__actions">
                <Button variant=ButtonVariant::Tonal
                    id="preview-close"
                    on_press=move |_| {
                        preview.set(None);
                        restore_scroll(page_scroll);
                    }
                >
                    "Đóng"
                </Button>
            </div>
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
        SaveStatus::Saved { version } => format!("Đã lưu · phiên bản {version}"),
        SaveStatus::Unsaved => "Chưa lưu thay đổi".into(),
        SaveStatus::Saving => "Đang lưu…".into(),
        SaveStatus::Conflict { server_version } => {
            format!("Xung đột · máy chủ ở phiên bản {server_version}")
        }
        failed @ SaveStatus::Failed(_) if save_is_uncertain(failed) => "Chưa xác nhận lưu".into(),
        SaveStatus::Failed(_) => "Chưa lưu được".into(),
    }
}

fn save_is_uncertain(status: SaveStatus) -> bool {
    matches!(
        status,
        SaveStatus::Failed(Failure::Network | Failure::Unavailable | Failure::Unexpected)
    )
}

fn failure_reason(failure: Failure) -> &'static str {
    match failure {
        Failure::Network => "Mất phản hồi từ máy chủ; chưa rõ bản nháp đã được lưu hay chưa.",
        Failure::Unavailable => "Máy chủ tạm không phục vụ.",
        Failure::Unauthenticated => "Phiên đăng nhập không hợp lệ.",
        Failure::Forbidden => "Yêu cầu bị từ chối.",
        Failure::NotFound => "Không tìm thấy bản nháp.",
        Failure::Invalid => "Nội dung không hợp lệ.",
        Failure::Unexpected => "Phản hồi không xác định; chưa rõ bản nháp đã được lưu hay chưa.",
    }
}

fn leave_message(stage: Option<LeaveStage>, status: SaveStatus) -> String {
    let not_saved = match status {
        SaveStatus::Conflict { server_version } => format!(
            "Bản nháp đã được lưu ở nơi khác (phiên bản {server_version}), nên không lưu đè từ đây. Ở lại để chọn lưu đè, hoặc tải bản đang viết."
        ),
        SaveStatus::Failed(failure) => format!(
            "Chưa lưu được. {} Chữ vẫn còn trên trang này.",
            failure_reason(failure)
        ),
        _ => "Chữ vẫn còn trên trang này.".into(),
    };
    match stage {
        Some(LeaveStage::Saving { .. }) => {
            "Đang lưu… Trang sẽ rời khi máy chủ xác nhận bản mới nhất.".into()
        }
        Some(LeaveStage::Choosing {
            notice: Some(LeaveNotice::NewerEdits),
        }) => "Đã lưu, nhưng còn chữ mới hơn chưa lưu. Lưu tiếp rồi rời, hoặc ở lại.".into(),
        Some(LeaveStage::Choosing {
            notice: Some(LeaveNotice::NotSaved),
        }) => not_saved,
        _ if matches!(status, SaveStatus::Failed(_) | SaveStatus::Conflict { .. }) => not_saved,
        _ => "Những thay đổi gần nhất chưa được máy chủ xác nhận. Ở lại để viết tiếp, hoặc lưu rồi rời trang.".into(),
    }
}

#[cfg(feature = "hydrate")]
fn failure(error: &ApiError) -> Failure {
    match error {
        ApiError::Network => Failure::Network,
        ApiError::Unavailable => Failure::Unavailable,
        ApiError::Unauthenticated => Failure::Unauthenticated,
        ApiError::Forbidden => Failure::Forbidden,
        ApiError::NotFound => Failure::NotFound,
        ApiError::Invalid { .. } => Failure::Invalid,
        ApiError::VersionConflict { .. } | ApiError::Unexpected { .. } => Failure::Unexpected,
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
