//! Bàn viết: private drafts, loaded only in the browser through the authenticated API.
//! Server rendering emits the shell and a loading state, never draft content.

use leptos::html;
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params_map;

use crate::contract::{ApiError, DocumentDto, Draft, DraftList};
use crate::desk_state::{
    EditorSession, Failure, LeaveGuard, LeaveNotice, LeaveStage, SaveStatus, SaveTicket,
};
use crate::document::DocumentView;
use crate::draft_list::{
    CreateState, DraftSort, display_title as untitled, updated_at_label, visible_drafts,
};
use crate::editor::{BODY_HINT_ID, BODY_LABEL_ID, BodyEditor, BodySource, BodyState, TOOLBAR_ID};
use crate::navigation::NavigationGuard;

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
                <h1 id="desk-title" class="vk-page-title">"Bàn viết"</h1>
                <Show when=move || drafts.get().is_some_and(|result| result.is_ok_and(|list| !list.drafts.is_empty()))>
                    <CreateDraftButton creation can_create create />
                </Show>
            </header>
            <noscript><p class="vk-notice">{DESK_UNAVAILABLE_SSR}</p></noscript>
            <CreateNotice creation loading refresh />
            <div aria-busy=move || if loading.get() { "true" } else { "false" } data-testid="draft-content">
                <Suspense fallback=|| view! {
                    <p class="vk-desk-loading vk-muted" role="status" data-testid="draft-loading">"Đang mở Bàn viết…"</p>
                }>
                    {move || Suspend::new(async move {
                        match drafts.await {
                            Ok(list) if list.drafts.is_empty() => view! {
                                <div class="vk-empty vk-desk-empty" data-testid="draft-empty">
                                    <h2 class="vk-empty__title">"Bắt đầu một bản thảo"</h2>
                                    <p>"Một trang dành riêng cho những điều bạn muốn viết."</p>
                                    <CreateDraftButton creation can_create create first=true />
                                </div>
                            }.into_any(),
                            Ok(list) => view! { <DraftListView list query sort refresh refresh_disabled /> }.into_any(),
                            Err(error) => view! {
                                <DeskError error />
                                <button class="vk-button vk-button--tonal" type="button"
                                    data-testid="retry-drafts" on:click=move |_| refresh.run(())
                                    disabled=move || loading.get()>"Thử mở lại"</button>
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
        <button type="button"
            class=if first { "vk-button vk-button--filled" } else { "vk-button vk-button--tonal" }
            data-testid="create-draft" on:click=move |_| create.run(())
            disabled=move || !can_create.get()
        >
            {move || match creation.get() {
                CreateState::Pending => "Đang tạo…",
                CreateState::Reconciled => "Tạo thêm bản thảo",
                _ if first => "Viết bản thảo đầu tiên",
                _ => "Tạo bản thảo",
            }}
        </button>
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
            <div class="vk-notice vk-notice--warning" role="status" data-testid="create-notice">
                <p class="vk-notice__title">{title}</p><p>{detail}</p>
                <button class="vk-button vk-button--tonal" type="button" data-testid="reconcile-drafts"
                    on:click=move |_| refresh.run(()) disabled=move || loading.get()>
                    {move || if loading.get() { "Đang tải lại…" } else { "Tải lại danh sách" }}
                </button>
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
            <p id="resume-label" class="vk-resume__label">"Trở lại trang đang viết"</p>
            <h2 class="vk-resume__title">{untitled(&newest.title)}</h2>
            <DraftMetadata version=newest.version updated_at=newest.updated_at />
            <a class="vk-button vk-button--filled vk-resume__action" data-testid="resume-draft"
                href=format!("/ban-viet/{}", newest.work_id)>"Viết tiếp"</a>
        </section>
        <section id="drafts" class="vk-draft-list" aria-labelledby="draft-list-title">
            <h2 id="draft-list-title" class="vk-section__title">"Bản thảo gần đây"</h2>
            <div id="draft-tools" class="vk-draft-tools">
                <label class="vk-field vk-draft-search">
                    <span class="vk-field__label">"Tìm theo tiêu đề"</span>
                    <input type="search" data-testid="draft-search" placeholder="Có dấu hoặc không dấu"
                        aria-describedby="draft-scope" prop:value=move || query.get()
                        on:input=move |event| query.set(event_target_value(&event)) />
                </label>
                <label class="vk-field">
                    <span class="vk-field__label">"Sắp xếp"</span>
                    <select data-testid="draft-sort" prop:value=move || if sort.get() == DraftSort::Title { "title" } else { "recent" }
                        on:change=move |event| sort.set(if event_target_value(&event) == "title" { DraftSort::Title } else { DraftSort::Recent })>
                        <option value="recent">"Mới cập nhật"</option>
                        <option value="title">"Tên A–Z (không dấu)"</option>
                    </select>
                </label>
                <button class="vk-button vk-button--text" type="button" data-testid="refresh-drafts"
                    disabled=move || refresh_disabled.get() on:click=move |_| refresh.run(())>"Tải lại"</button>
            </div>
            <p id="draft-scope" class="vk-muted" data-testid="draft-scope">{scope}</p>
            <p class="vk-muted" role="status" data-testid="draft-result-count">
                {move || format!("Hiển thị {} / {count} bản thảo đã tải", visible.with(Vec::len))}
            </p>
            <Show when=move || visible.with(Vec::is_empty)>
                <div class="vk-empty" data-testid="draft-no-results">
                    <p class="vk-empty__title">"Không có tiêu đề phù hợp"</p>
                    <p>"Thử một từ khác hoặc xem lại các bản thảo đã tải."</p>
                    <button class="vk-button vk-button--tonal" type="button" data-testid="clear-draft-search"
                        on:click=move |_| query.set(String::new())>"Xem các bản thảo đã tải"</button>
                </div>
            </Show>
            <ul id="draft-results" class="vk-draft-rows" aria-label="Bản thảo đã tải" role="list">
                <For each=move || visible.get() key=|draft| draft.work_id.clone() children=move |draft| view! {
                    <li class="vk-draft-row" data-testid="draft-row">
                        <a class="vk-draft-row__link" href=format!("/ban-viet/{}", draft.work_id)>
                            <span class="vk-draft-row__title">{untitled(&draft.title)}</span>
                            <DraftMetadata version=draft.version updated_at=draft.updated_at />
                            <span class="vk-draft-row__open">"Mở bản thảo →"</span>
                        </a>
                    </li>
                } />
            </ul>
        </section>
    }.into_any()
}

#[component]
fn DraftMetadata(version: i64, updated_at: String) -> impl IntoView {
    let label = updated_at_label(&updated_at);
    view! {
        <span class="vk-draft-meta">
            <span>"Phiên bản " {version}</span>
            <span>"Cập nhật " <time datetime=updated_at>{label}</time></span>
        </span>
    }
    .into_any()
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
        _ => ("Có lỗi khi mở Bàn viết.", "Hãy thử mở lại."),
    };
    view! {
        <div class="vk-notice" role="alert" data-testid="desk-error">
            <p class="vk-notice__title">{title}</p><p>{detail}</p>
            <a class="vk-button vk-button--text" href="/">"Khám phá tác phẩm"</a>
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
                <h1 id="editor-heading" class="vk-editor-heading__title">
                    "Bản nháp"
                </h1>
            </header>
            <noscript>
                <p class="vk-notice">{DESK_UNAVAILABLE_SSR}</p>
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
    let initial_title = draft.title.clone();
    let title = StoredValue::new(initial_title.clone());
    let body = BodySource::new(crate::document::document_to_text(&draft.document));
    let session = RwSignal::new(EditorSession::loaded(draft.version));
    let leave = RwSignal::new(LeaveGuard::default());
    let navigation = use_context::<NavigationGuard>();
    let title_input = NodeRef::<html::Input>::new();
    let leave_dialog = NodeRef::<html::Dialog>::new();
    let preview_dialog = NodeRef::<html::Dialog>::new();
    let keep_editing = NodeRef::<html::Button>::new();
    let last_field = StoredValue::new(Field::Body);
    let page_scroll = StoredValue::new(0.0_f64);
    let preview = RwSignal::new(None::<(String, DocumentDto)>);

    // Sends the text as it is now. The ticket was taken in the same turn, so it covers exactly
    // this text. The outcome settles the session first; only then may the leave intent go.
    let send = move |ticket: SaveTicket| {
        let request = crate::contract::SaveDraftRequest {
            expected_version: ticket.expected_version,
            title: title.get_value(),
            document: crate::document::text_to_document(&body.snapshot()),
        };
        #[cfg(feature = "hydrate")]
        {
            let id = work_id.get_value();
            leptos::task::spawn_local(async move {
                use crate::desk_state::SaveRejection;
                let outcome = crate::api::save_draft(id, request).await;
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
                    close_dialog(leave_dialog, page_scroll);
                    if let Some(navigation) = navigation {
                        navigation.go(&destination);
                    }
                }
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (request, work_id, navigation, page_scroll);
    };
    let save = move || {
        if let Some(ticket) = session.try_update(EditorSession::begin_save).flatten() {
            send(ticket);
        }
    };
    let export = move || {
        let text = crate::document::export_text(&title.get_value(), &body.snapshot());
        let name = crate::document::export_file_name(&title.get_value());
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
        close_dialog(leave_dialog, page_scroll);
        focus_last_field();
    };
    let save_and_leave = move || {
        let ticket = leave
            .try_update(|g| session.try_update(|s| g.save_and_leave(s)))
            .flatten()
            .flatten();
        if let Some(ticket) = ticket {
            send(ticket);
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
            close_dialog(leave_dialog, page_scroll);
            if let Some(navigation) = navigation {
                navigation.go(&destination);
            }
        }
    };
    let open_preview = move || {
        let document = crate::document::text_to_document(&body.snapshot());
        preview.set(Some((title.get_value(), document)));
        open_dialog(preview_dialog, page_scroll);
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
                open_dialog(leave_dialog, page_scroll);
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
        if title.get_value().is_empty() && body.snapshot().is_empty() {
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
            <div id=TOOLBAR_ID class="vk-editor-toolbar" data-testid="editor-toolbar">
                <p
                    class="vk-save-status"
                    role="status"
                    aria-live="polite"
                    data-testid="save-status"
                    data-status=move || status().token()
                >
                    {move || status_text(status())}
                </p>
                <div class="vk-editor-toolbar__actions">
                    <button
                        type="button"
                        class="vk-button vk-button--text"
                        data-testid="preview"
                        on:click=move |_| open_preview()
                    >
                        "Xem trước"
                    </button>
                    <button
                        type="button"
                        class="vk-button vk-button--text"
                        data-testid="export"
                        on:click=move |_| export()
                    >
                        "Tải .txt"
                    </button>
                    <button
                        type="submit"
                        class="vk-button vk-button--filled"
                        data-testid="save"
                        disabled=move || !session.with(EditorSession::can_save)
                    >
                        "Lưu nháp"
                    </button>
                </div>
            </div>
            <Show when=move || matches!(status(), SaveStatus::Failed(_))>
                <div class="vk-notice vk-notice--error" role="alert" data-testid="save-failed">
                    <p class="vk-notice__title">"Chưa lưu được bản nháp."</p>
                    <p>
                        {move || match status() {
                            SaveStatus::Failed(failure) => failure_reason(failure),
                            _ => "",
                        }}
                        " Chữ vẫn còn nguyên trên trang này. Hãy thử lưu lại, hoặc tải bản đang viết để giữ một bản trên máy."
                    </p>
                    <div class="vk-notice__actions">
                        <button
                            type="button"
                            class="vk-button vk-button--tonal"
                            data-testid="retry-save"
                            on:click=move |_| save()
                        >
                            "Thử lưu lại"
                        </button>
                        <button
                            type="button"
                            class="vk-button vk-button--text"
                            on:click=move |_| export()
                        >
                            "Tải bản đang viết"
                        </button>
                    </div>
                </div>
            </Show>
            <Show when=move || matches!(status(), SaveStatus::Conflict { .. })>
                <div class="vk-notice vk-notice--warning" role="alert" data-testid="conflict">
                    <p class="vk-notice__title">"Bản nháp đã được lưu ở nơi khác."</p>
                    <p>
                        "Nội dung bạn đang viết vẫn còn nguyên trên trang này. Bạn có thể lưu đè bằng nội dung này, hoặc tải bản đang viết trước khi mở lại bản trên máy chủ."
                    </p>
                    <div class="vk-notice__actions">
                        <button
                            type="button"
                            class="vk-button vk-button--tonal"
                            data-testid="keep-mine"
                            on:click=move |_| {
                                session.update(EditorSession::keep_local_text);
                                save();
                            }
                        >
                            "Lưu đè bằng nội dung đang viết"
                        </button>
                        <button
                            type="button"
                            class="vk-button vk-button--text"
                            on:click=move |_| export()
                        >
                            "Tải bản đang viết"
                        </button>
                    </div>
                </div>
            </Show>
            <div class="vk-editor-paper">
                <label class="vk-field">
                    <span class="vk-field__label">"Tiêu đề"</span>
                    <input
                        class="vk-input vk-input--title"
                        name="title"
                        lang="vi"
                        maxlength="200"
                        placeholder="Chưa đặt tên"
                        autocomplete="off"
                        node_ref=title_input
                        prop:value=initial_title
                        on:focus=move |_| last_field.set_value(Field::Title)
                        on:input=move |event| {
                            title.set_value(event_target_value(&event));
                            session.update(EditorSession::edited);
                        }
                    />
                </label>
                <div class="vk-field" on:focusin=move |_| last_field.set_value(Field::Body)>
                    <span
                        id=BODY_LABEL_ID
                        class="vk-field__label"
                        on:click=move |_| body.focus()
                    >
                        "Nội dung"
                    </span>
                    <span id=BODY_HINT_ID class="vk-field__hint">
                        "Xuống dòng để ngắt câu thơ; để một dòng trống giữa các đoạn hoặc khổ. Ctrl/⌘ + S để lưu."
                    </span>
                    <BodyEditor source=body on_change=on_body_change on_ready=on_body_ready />
                    <Show when=move || body.state.get() == BodyState::Loading>
                        <p class="vk-muted vk-editor-body__pending">"Đang mở trình soạn thảo…"</p>
                    </Show>
                    <Show when=move || body.state.get() == BodyState::Failed>
                        <p class="vk-notice vk-notice--error" role="alert" data-testid="editor-failed">
                            "Không mở được trình soạn thảo. Nội dung đã lưu trên máy chủ không bị ảnh hưởng; hãy tải lại trang."
                        </p>
                    </Show>
                </div>
            </div>
        </form>
        <dialog
            class="vk-dialog"
            aria-labelledby="leave-editor-title"
            data-testid="leave-editor"
            node_ref=leave_dialog
            on:cancel=move |event: leptos::ev::Event| {
                event.prevent_default();
                stay();
            }
        >
            <Show
                when=move || stage() == Some(LeaveStage::ConfirmingDiscard)
                fallback=move || {
                    let saving = move || matches!(stage(), Some(LeaveStage::Saving { .. }));
                    view! {
                        <h2 id="leave-editor-title" class="vk-dialog__title">
                            "Rời trang khi còn chữ chưa lưu?"
                        </h2>
                        <p class="vk-dialog__body" data-testid="leave-message">
                            {move || leave_message(stage(), status())}
                        </p>
                        <div class="vk-dialog__actions">
                            <button
                                type="button"
                                class="vk-button vk-button--text"
                                data-testid="leave-stay"
                                on:click=move |_| stay()
                            >
                                "Ở lại"
                            </button>
                            <button
                                type="button"
                                class="vk-button vk-button--filled"
                                data-testid="leave-save"
                                disabled=move || {
                                    saving() || matches!(status(), SaveStatus::Conflict { .. })
                                }
                                on:click=move |_| save_and_leave()
                            >
                                {move || if saving() { "Đang lưu…" } else { "Lưu và rời trang" }}
                            </button>
                        </div>
                        <Show when=can_discard>
                            <div class="vk-dialog__recovery" data-testid="leave-recovery">
                                <button
                                    type="button"
                                    class="vk-button vk-button--tonal"
                                    data-testid="leave-export"
                                    on:click=move |_| export()
                                >
                                    "Tải bản đang viết (.txt)"
                                </button>
                                <button
                                    type="button"
                                    class="vk-button vk-button--text vk-button--danger"
                                    data-testid="leave-discard"
                                    on:click=move |_| ask_discard()
                                >
                                    "Rời mà không lưu…"
                                </button>
                            </div>
                        </Show>
                    }
                }
            >
                <h2 id="leave-editor-title" class="vk-dialog__title">
                    "Bỏ chữ chưa lưu và rời trang?"
                </h2>
                <p class="vk-dialog__body">
                    "Chữ chưa được máy chủ xác nhận sẽ mất khỏi trang này. Tệp .txt đã tải (nếu có) chỉ nằm trên máy của bạn, không phải bản lưu trên máy chủ."
                </p>
                <div class="vk-dialog__actions">
                    <button
                        type="button"
                        class="vk-button vk-button--text"
                        data-testid="discard-back"
                        node_ref=keep_editing
                        on:click=move |_| leave.update(LeaveGuard::keep_choosing)
                    >
                        "Quay lại"
                    </button>
                    <button
                        type="button"
                        class="vk-button vk-button--danger-filled"
                        data-testid="discard-confirm"
                        on:click=move |_| confirm_discard()
                    >
                        "Bỏ thay đổi và rời"
                    </button>
                </div>
            </Show>
        </dialog>
        <dialog
            class="vk-dialog vk-dialog--preview"
            aria-labelledby="draft-preview-title"
            data-testid="draft-preview"
            node_ref=preview_dialog
            on:close=move |_| {
                restore_scroll(page_scroll);
                preview.set(None);
            }
        >
            // Opening focuses the heading, so a long draft opens at its start, not at "Đóng".
            <p id="draft-preview-title" class="vk-dialog__eyebrow" tabindex="-1" autofocus>
                "Xem trước bản nháp"
            </p>
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
                                <h2 class="vk-reader__title">{untitled(&title)}</h2>
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
                <button
                    type="button"
                    class="vk-button vk-button--tonal"
                    data-testid="preview-close"
                    on:click=move |_| {
                        if let Some(dialog) = preview_dialog.get_untracked() {
                            dialog.close();
                        }
                    }
                >
                    "Đóng"
                </button>
            </div>
        </dialog>
    }
}

fn open_dialog(dialog: NodeRef<html::Dialog>, page_scroll: StoredValue<f64>) {
    #[cfg(feature = "hydrate")]
    if let Some(dialog) = dialog.get_untracked()
        && !dialog.open()
    {
        page_scroll.set_value(window().scroll_y().unwrap_or_default());
        let _ = dialog.show_modal();
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (dialog, page_scroll);
}

/// Closes a dialog and keeps the page where the author left it; returning focus to the trigger
/// would otherwise scroll it into view.
fn close_dialog(dialog: NodeRef<html::Dialog>, page_scroll: StoredValue<f64>) {
    if let Some(dialog) = dialog.get_untracked()
        && dialog.open()
    {
        dialog.close();
        restore_scroll(page_scroll);
    }
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
        SaveStatus::Unsaved => "Chưa lưu".into(),
        SaveStatus::Saving => "Đang lưu…".into(),
        SaveStatus::Conflict { server_version } => {
            format!("Xung đột · máy chủ ở phiên bản {server_version}")
        }
        SaveStatus::Failed(_) => "Chưa lưu được".into(),
    }
}

fn failure_reason(failure: Failure) -> &'static str {
    match failure {
        Failure::Network => "Mất kết nối tới máy chủ.",
        Failure::Unavailable => "Máy chủ tạm không phục vụ.",
        Failure::Unauthenticated => "Phiên đăng nhập không hợp lệ.",
        Failure::Forbidden => "Yêu cầu bị từ chối.",
        Failure::NotFound => "Không tìm thấy bản nháp.",
        Failure::Invalid => "Nội dung không hợp lệ.",
        Failure::Unexpected => "Lỗi không xác định.",
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
