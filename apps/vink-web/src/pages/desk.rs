//! Bàn viết: private drafts, loaded only in the browser through the authenticated API.
//! Server rendering emits the shell and a loading state, never draft content.

use leptos::html;
use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params_map;

use vink_ui::{
    ActionState, ActionSurface, Announce, Button, ButtonKind, ButtonVariant, ChoiceOption,
    FieldVariant, Heading, HeadingLevel, IconButton, IconName, InlineNotice, PanelContent,
    PanelKind, PlainTooltip, ProblemKind, SearchField, SearchVariant, Select, SelectVariant,
    StatePanel, SupportingDetails, TextField, Tone, TooltipIconButton, TypeRole,
};

use crate::contract::{ApiError, DocumentDto, DocumentFormatDto, Draft};
use crate::desk_state::{
    DraftSession, DraftTicket, Failure, LeaveGuard, LeaveNotice, LeaveStage, SaveStatus, SaveTicket,
};
use crate::document::{ConversionPreview, NotEditable, ReadingDocument};
use crate::draft_list::{CreateState, DraftSort, display_title as untitled};
use crate::editor::{
    BODY_HINT_ID, BODY_LABEL_ID, BODY_TITLE_ERROR_ID, BodyEditor, BodySource, BodyState,
    OutlineItem, OutlineStatus, TOOLBAR_ID,
};
use crate::navigation::NavigationGuard;
use crate::pages::work_info::TagLoad;
use crate::tag_state::{TagStatus, TagTicket};
use crate::ui::{
    adapters,
    empty_state::{EmptyArtwork, EmptyState},
    patterns::AuthorWorkRow,
};

const DESK_UNAVAILABLE_SSR: &str = "Bàn viết cần JavaScript để mở bản nháp riêng.";
const DESK_SIGN_IN: &str = "/toi?next=/ban-viet";

#[component]
pub fn DeskIndex() -> impl IntoView {
    let session = crate::account::Session::current();
    view! {
        <Title text="Bàn viết — Vink" />
        <Meta name="robots" content="noindex" />
        <section id="desk" class="vk-desk vk-desk--index" aria-labelledby="desk-title">
            <Show when=move || session.scope.with(Option::is_none)>
                <DeskHeading />
                <noscript><p class="vk-desk-noscript">{DESK_UNAVAILABLE_SSR}</p></noscript>
                <DeskSession session />
            </Show>
            // Each confirmed private scope owns its DOM, requests and input buffer.
            <For each=move || session.scope.get() key=Clone::clone
                children=move |scope| view! { <DeskWorks scope session /> } />
        </section>
    }
}

#[component]
fn DeskHeading(#[prop(optional)] children: Option<Children>) -> impl IntoView {
    view! {
        <header id="heading" class="vk-desk-heading">
            <Heading level=HeadingLevel::H1 id="desk-title" attr:tabindex="-1">"Bàn viết"</Heading>
            {children.map(|slot| slot())}
        </header>
    }
}

#[component]
fn DeskSession(session: crate::account::Session) -> impl IntoView {
    use crate::session::Standing;
    let sign_in_open = crate::api::sign_in_open();
    move || {
        match session.standing.get() {
        Standing::Unknown => view! { <DeskLoading /> }.into_any(),
        Standing::Anonymous if sign_in_open => view! {
            <DeskAccess title="Bạn chưa đăng nhập" message="Đăng nhập để viết và mở bản thảo riêng của bạn.">
                <a id="desk-sign-in" class="vk-ui-button vk-ui-button--filled" href=DESK_SIGN_IN>"Đăng nhập"</a>
                <a class="vk-ui-button vk-ui-button--text" href="/">"Khám phá tác phẩm"</a>
            </DeskAccess>
        }.into_any(),
        Standing::Anonymous => view! {
            <DeskAccess title="Vink chưa mở đăng nhập" message="Trong lúc này, bạn có thể đọc các tác phẩm công khai.">
                <a class="vk-ui-button vk-ui-button--filled" href="/">"Khám phá tác phẩm"</a>
            </DeskAccess>
        }.into_any(),
        Standing::NeedsPrincipal => view! {
            <DeskAccess title="Đặt bút danh để bắt đầu" message="Hoàn tất bút danh trong Tài khoản để viết và mở bản thảo của bạn.">
                <a id="desk-pen-name" class="vk-ui-button vk-ui-button--filled" href=DESK_SIGN_IN>"Đặt bút danh"</a>
            </DeskAccess>
        }.into_any(),
        Standing::Suspended => view! {
            <DeskAccess title="Tài khoản đang bị tạm khóa" message="Hãy mở Tài khoản để xem thông tin về phiên của bạn.">
                <a class="vk-ui-button vk-ui-button--filled" href="/toi">"Mở Tài khoản"</a>
            </DeskAccess>
        }.into_any(),
        Standing::Unavailable => view! {
            <DeskAccess title="Chưa kiểm tra được phiên đăng nhập" message="Hãy kiểm tra kết nối rồi thử lại.">
                <Button id="desk-session-retry" variant=ButtonVariant::Filled on_press=move |_| session.refresh()
                    state=Signal::derive(move || if session.checking() { ActionState::pending("Đang kiểm tra…") } else { ActionState::Enabled })>
                    "Kiểm tra lại phiên"
                </Button>
            </DeskAccess>
        }.into_any(),
        Standing::Active(_) => ().into_any(),
    }
    }
}

#[component]
fn DeskAccess(title: &'static str, message: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="vk-desk-access" data-testid="desk-access">
            <span class="vk-desk-access__icon"><vink_ui::Icon name=IconName::Lock /></span>
            <div>
                <Heading level=HeadingLevel::H2 role=TypeRole::Title>{title}</Heading>
                <p class="vk-muted">{message}</p>
            </div>
            <div class="vk-desk-access__actions">{children()}</div>
        </div>
    }
}

#[component]
fn DeskLoading() -> impl IntoView {
    view! {
        <p class="vk-desk-loading vk-muted" role="status" data-testid="draft-loading">"Đang mở Bàn viết…"</p>
    }
}

#[component]
fn DeskWorks(scope: String, session: crate::account::Session) -> AnyView {
    use crate::draft_list::paging::DraftPages;
    #[cfg(feature = "hydrate")]
    use crate::draft_list::paging::PAGE_SIZE;
    use crate::read_lifecycle::{Intent, ReadOwner};
    let query = RwSignal::new(String::new());
    let applied = RwSignal::new(String::new());
    let sort = RwSignal::new(DraftSort::Recent);
    let composing = RwSignal::new(false);
    let creation = RwSignal::new(CreateState::Ready);
    let create_format = RwSignal::new(DocumentFormatDto::Default);
    let loading = RwSignal::new(true);
    let appending = RwSignal::new(false);
    let recovery_focus = RwSignal::new(false);
    let ready = RwSignal::new(false);
    let pages = RwSignal::new(DraftPages::default());
    let load_error = RwSignal::new(None::<ApiError>);
    let page_error = RwSignal::new(None::<ApiError>);
    let access_error = RwSignal::new(None::<ApiError>);
    let restarted = RwSignal::new(false);
    let scope = StoredValue::new(scope);
    let reads = StoredValue::new(ReadOwner::new((String::new(), DraftSort::Recent)));
    let wake = StoredValue::new(crate::read_events::WakePolicy::default());
    #[cfg(feature = "hydrate")]
    let in_scope = move || {
        scope.try_with_value(|bound| {
            session
                .scope
                .try_with_untracked(|now| now.as_ref() == Some(bound))
        }) == Some(Some(true))
    };
    #[cfg(feature = "hydrate")]
    let transport_owner = StoredValue::new(Owner::current().map(|o| o.downgrade()));
    let read_list = Callback::new(move |intent: Intent| {
        #[cfg(feature = "hydrate")]
        {
            if (intent == Intent::Refresh && creation.get_untracked() == CreateState::Pending)
                || access_error.with_untracked(Option::is_some)
                || !in_scope()
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
            let creation_at_read_start = creation.get_untracked();
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
                        &ticket.scope.0,
                        ticket.scope.1,
                        PAGE_SIZE,
                        ticket.cursor.as_deref(),
                    )
                    .await;
                    if reads.try_with_value(|r| r.accepts(&ticket)) != Some(true)
                        || !in_scope()
                        || access_error.with_untracked(Option::is_some)
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
                                // A GET started before a failed POST cannot reconcile its outcome.
                                creation.try_update(|state| {
                                    if *state == creation_at_read_start {
                                        state.reconciled();
                                    }
                                });
                                restarted.try_set(old_tail);
                            }
                        }
                        // The HTTP adapter already asks the shared session owner to reconcile
                        // a 401 or suspension. Hide this scope immediately while it checks.
                        Err(error @ (ApiError::Unauthenticated | ApiError::Forbidden)) => {
                            access_error.try_set(Some(error));
                            pages.try_set(DraftPages::default());
                        }
                        Err(ApiError::NotFound) => {
                            pages.try_set(DraftPages::default());
                            ready.try_set(false);
                            page_error.try_set(None);
                            load_error.try_set(Some(ApiError::NotFound));
                        }
                        Err(error) if intent == Intent::Append => {
                            page_error.try_set(Some(error));
                        }
                        Err(error) => {
                            load_error.try_set(Some(error));
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
            let requested = (applied.get(), sort.get());
            reads.update_value(|r| r.change_scope(requested));
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
    let _ = (session, scope, reads, wake, recovery_focus);
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
        ready.get() && access_error.with(Option::is_none) && creation.with(CreateState::can_begin)
    });
    #[cfg(feature = "hydrate")]
    let navigate = leptos_router::hooks::use_navigate();
    let create = Callback::new(move |()| {
        #[cfg(feature = "hydrate")]
        {
            if !in_scope()
                || !can_create.get_untracked()
                || !creation.try_update(CreateState::begin).unwrap_or(false)
            {
                return;
            }
            let navigate = navigate.clone();
            let picked_format = create_format.get_untracked();
            leptos::task::spawn_local(async move {
                let outcome = crate::api::create_draft_with_format(picked_format).await;
                // The route owner disposes this signal. A late reply must not navigate from a
                // different page (and an uncertain POST is never automatically repeated). A
                // reply for an earlier session is dropped as well: its draft is not this desk's.
                if creation.try_get_untracked().is_none()
                    || !in_scope()
                    || access_error.with_untracked(Option::is_some)
                {
                    return;
                }
                match outcome {
                    Ok(draft) => {
                        navigate(&format!("/ban-viet/{}", draft.work_id), Default::default())
                    }
                    Err(ApiError::Unauthenticated) => {
                        access_error.set(Some(ApiError::Unauthenticated));
                        pages.set(DraftPages::default());
                    }
                    Err(error) => {
                        creation.update(|state| state.failed(error));
                    }
                }
            });
        }
    });
    // Emptiness needs a successful head page; a failed or unanswered read never establishes it.
    // A retained empty head stays while the same scope refreshes (UIS-03 `Empty + Loading`):
    // a new scope resets `pages`, so its stale answer cannot be shown as the new one.
    let empty_head = move || {
        load_error.get().is_none() && pages.with(|p| p.loaded && p.part == 1 && p.drafts.is_empty())
    };
    // Only an unfiltered head without continuation describes an empty corpus. A filtered
    // miss says nothing about whether this owner has drafts.
    let first_use = Signal::derive(move || {
        ready.get()
            && empty_head()
            && crate::draft_list::search_key(&applied.get()).is_empty()
            && pages.with(|p| p.chain.next_cursor.is_none())
    });
    let no_results = Signal::derive(move || !first_use.get() && empty_head());
    view! {
        <DeskHeading>
            <Show when=move || ready.get() && !first_use.get() && access_error.with(Option::is_none)>
                <CreateDraftControls creation can_create create create_format />
            </Show>
        </DeskHeading>
        {move || access_error.get().map(|error| view! {
            <DeskAccess title=if error == ApiError::Unauthenticated { "Phiên đăng nhập cần được kiểm tra lại" } else { "Bạn không có quyền xem danh sách tác phẩm" }
                message="Mở Tài khoản để kiểm tra phiên hiện tại.">
                <a class="vk-ui-button vk-ui-button--filled" href="/toi">"Mở Tài khoản"</a>
            </DeskAccess>
        })}
        <Show when=move || access_error.with(Option::is_none)>
            <section id="drafts" class="vk-drafts-region" aria-labelledby="draft-list-title">
                <Heading id="draft-list-title" level=HeadingLevel::H2 role=TypeRole::Title attr:tabindex="-1">"Tác phẩm của bạn"</Heading>
                <div class="vk-desk-works" data-testid="draft-content"
                    aria-busy=move || if loading.get() { "true" } else { "false" }>
                    <CreateNotice creation loading refresh />
                    <Show when=move || !ready.get() && loading.get()>
                        <DeskLoading />
                    </Show>
                    {move || load_error.get().map(|error| view! {
                        // Retrying helps only an answer that may change: a proxy 5xx or 429 arrives
                        // as `Unexpected`. A missing principal or invalid query would fail again.
                        <DeskError refresh=matches!(error, ApiError::Unavailable | ApiError::Network | ApiError::Unexpected { .. }).then_some(refresh)
                            error retry_disabled=refresh_disabled retained=pages.with(|p| p.loaded) />
                    })}
                    // The tools/input retain one owner through loading, empty, query and error.
                    <DraftListView pages query sort composing loading appending ready page_error load_error restarted
                        first_use no_results creation can_create create create_format submit clear more refresh refresh_disabled />
                </div>
                <p class="vk-desk-privacy vk-muted">"Bản nháp chỉ bạn đọc được. Số Thấm và Nhận xét thuộc bản đang công khai; lưu nháp không gửi duyệt hay công khai."</p>
            </section>
        </Show>
    }.into_any()
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
fn CreateDraftControls(
    creation: RwSignal<CreateState>,
    can_create: Signal<bool>,
    create: Callback<()>,
    create_format: RwSignal<DocumentFormatDto>,
    #[prop(default = false)] first: bool,
) -> impl IntoView {
    view! {
        <div class="vk-desk-create">
            <Select id="create-draft-format" label="Định dạng bản thảo"
                variant=SelectVariant::Inset16 required=true
                options=vec![ChoiceOption::new("default", "Mặc định"), ChoiceOption::new("markdown", "Markdown")]
                value=Signal::derive(move || Some(create_format.get().as_str().to_string()))
                disabled=Signal::derive(move || !can_create.get())
                on_change=Callback::new(move |picked: Option<String>| {
                    if let Some(next) = picked.as_deref().and_then(DocumentFormatDto::parse) {
                        create_format.set(next);
                    }
                })
            />
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
        </div>
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
    no_results: Signal<bool>,
    creation: RwSignal<CreateState>,
    can_create: Signal<bool>,
    create: Callback<()>,
    create_format: RwSignal<DocumentFormatDto>,
    query: RwSignal<String>,
    sort: RwSignal<DraftSort>,
    composing: RwSignal<bool>,
    loading: RwSignal<bool>,
    appending: RwSignal<bool>,
    ready: RwSignal<bool>,
    page_error: RwSignal<Option<ApiError>>,
    load_error: RwSignal<Option<ApiError>>,
    restarted: RwSignal<bool>,
    submit: Callback<()>,
    clear: Callback<()>,
    more: Callback<()>,
    refresh: Callback<()>,
    refresh_disabled: Signal<bool>,
) -> AnyView {
    let invalid_page = move || page_error.with(|e| matches!(e, Some(ApiError::Invalid { .. })));
    let stale = move || {
        pages.with(|p| p.loaded)
            && (load_error.with(Option::is_some)
                || page_error.with(Option::is_some)
                || (loading.get() && !appending.get()))
    };
    let opening = move || !ready.get() && loading.get();
    view! {
        <div class="vk-draft-list">
            <div class="vk-draft-controls">
                <form role="search" class="vk-draft-search" on:submit=move |event| {
                    event.prevent_default(); submit.run(());
                }>
                    <SearchField id="draft-search" label="Tìm tác phẩm" value=query variant=SearchVariant::Bar
                        help="Theo tên bản thảo; gồm cả tác phẩm đã đăng" on_clear=clear
                        on_input=move |input: vink_ui::TextInput| {
                            composing.set(input.composing); query.set(input.value);
                        } />
                </form>
                <div id="draft-tools" class="vk-draft-tools">
                    <Select id="draft-sort" label="Sắp xếp" variant=SelectVariant::Inset16
                        options=vec![ChoiceOption::new("recent", "Mới cập nhật"), ChoiceOption::new("title", "Tên A–Z")]
                        value=Signal::derive(move || Some(if sort.get() == DraftSort::Title { "title" } else { "recent" }.to_string()))
                        on_change={move |value: Option<String>| sort.set(if value.as_deref() == Some("title") { DraftSort::Title } else { DraftSort::Recent })} />
                    <TooltipIconButton icon=IconName::Refresh label="Tải lại" id="refresh-drafts"
                        surface=ActionSurface::Tonal
                        state=Signal::derive(move || if loading.get() && !appending.get() {
                            ActionState::pending("Đang tải lại…")
                        } else { blocked(refresh_disabled.get()) }) on_press=move |_| refresh.run(()) />
                </div>
                <p id="draft-scope" class="vk-muted" class:vk-ui-visually-hidden=move || first_use.get()
                    hidden=opening role=move || (!opening()).then_some("status") aria-atomic="true" data-testid="draft-result-count">
                    <span data-testid=move || stale().then_some("author-work-stale")>
                        {move || if loading.get() && !appending.get() && pages.with(|p| p.loaded) { "Đang tải lại…".into() }
                        else if loading.get() && !appending.get() { "Đang tìm tác phẩm…".into() }
                        else if stale() { "Đang xem kết quả cũ.".into() }
                        else if load_error.with(Option::is_some) { "Chưa tải được kết quả.".into() }
                        else if no_results.get() { "Không có kết quả".into() }
                        else { format!("Hiển thị {} tác phẩm", pages.with(|p| p.drafts.len())) }}
                    </span>
                </p>
                <Show when=move || sort.get() == DraftSort::Title>
                    <p class="vk-draft-sort-help vk-muted">"Bỏ qua dấu khi sắp xếp."</p>
                </Show>
            </div>
            <Show when=move || restarted.get()>
                <p class="vk-muted" data-testid="draft-chain-restarted">"Đã tải lại từ đầu. Bạn có thể tải tiếp các tác phẩm phía sau."</p>
            </Show>
            <Show when=move || first_use.get()>
                <div data-testid="draft-empty">
                    <EmptyState artwork=EmptyArtwork::FirstDraft title="Chưa có tác phẩm nào" message="Một trang dành riêng cho những điều bạn muốn viết.">
                        <CreateDraftControls creation can_create create create_format first=true />
                    </EmptyState>
                </div>
            </Show>
            <Show when=move || no_results.get()>
                <div data-testid="draft-no-results">
                    <EmptyState artwork=EmptyArtwork::NoResults message="Thử tìm theo tên khác hoặc xem tất cả tác phẩm.">
                        <Button variant=ButtonVariant::Tonal id="clear-draft-search" on_press=move |_| clear.run(())>"Xem tất cả tác phẩm"</Button>
                    </EmptyState>
                </div>
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
        </div>
    }.into_any()
}

#[component]
fn DeskError(
    error: ApiError,
    #[prop(optional_no_strip)] refresh: Option<Callback<()>>,
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
                    <vink_ui::Link href=DESK_SIGN_IN variant=vink_ui::LinkVariant::Standalone attr:id="desk-sign-in">"Đăng nhập ở mục Tôi"</vink_ui::Link>
                })}
                <vink_ui::Link href="/" variant=vink_ui::LinkVariant::Standalone>"Khám phá tác phẩm"</vink_ui::Link>
            </StatePanel>
        </div>
    }
}

/// An explicit stop instead of a silent conversion: the stored source stays untouched.
#[component]
fn FormatNotEditable(reason: NotEditable) -> impl IntoView {
    let (title, message) = match reason {
        NotEditable::CarriageReturn => (
            "Bàn viết chưa mở được bản nháp này.",
            "Bản nháp có dấu xuống dòng CR hoặc CRLF. Bàn viết chưa giữ nguyên được kiểu này khi lưu; nội dung vẫn được giữ nguyên.",
        ),
    };
    view! {
        <div data-testid="desk-format-unavailable" role="alert">
            <StatePanel content=PanelContent {
                kind: PanelKind::Error(ProblemKind::Unavailable),
                title: title.into(),
                message: message.into(),
                actions: vec![],
                code: None,
            }>
                <vink_ui::Link href="/ban-viet" variant=vink_ui::LinkVariant::Standalone>"Về Bàn viết"</vink_ui::Link>
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
                    <vink_ui::Icon name=IconName::ArrowBack size=vink_ui::IconSize::Small />
                    <span>"Bàn viết"</span>
                </a>
                <Heading level=HeadingLevel::H1 role=TypeRole::Title id="editor-heading" class:vk-editor-heading__title=true>"Bản nháp"</Heading>
            </header>
            <noscript>
                <p class="vk-desk-noscript">{DESK_UNAVAILABLE_SSR}</p>
            </noscript>
            <Suspense fallback=|| view! { <p class="vk-muted">"Đang mở bản nháp…"</p> }>
                {move || Suspend::new(async move {
                    match draft.await {
                        // The editor saves its buffer verbatim as a default document. A draft it
                        // could not save back unchanged (Markdown until #256/#262, or CR line
                        // endings that CodeMirror normalizes) is not opened.
                        Ok(draft) => match crate::document::desk_editable(&draft.document) {
                            Ok(()) => view! { <Editor draft /> }.into_any(),
                            Err(reason) => view! { <FormatNotEditable reason /> }.into_any(),
                        },
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
    let format = RwSignal::new(draft.document.format);
    let title = RwSignal::new(draft.title.clone());
    let body = BodySource::new(draft.document.format, draft.document.source.clone());
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
    let outline_open = RwSignal::new(false);
    let outline_panel = NodeRef::<html::Dialog>::new();
    #[cfg(feature = "hydrate")]
    observe_outline_layout(outline_panel);
    let close_outline = move || {
        #[cfg(feature = "hydrate")]
        if let Some(panel) = outline_panel.get_untracked() {
            panel.close();
        }
        outline_open.set(false);
        focus_element_by_id("outline-toggle");
    };
    let heading_menu_open = RwSignal::new(false);
    let convert_preview = RwSignal::new(None::<ConversionPreview>);
    let converting = RwSignal::new(false);
    let conversion_notice = RwSignal::new(None::<String>);
    let format_select_sync = RwSignal::new(0_u32);
    let just_saved = RwSignal::new(false);
    #[cfg(feature = "hydrate")]
    type JustSavedTimer = Option<(i32, wasm_bindgen::closure::Closure<dyn FnMut()>)>;
    #[cfg(feature = "hydrate")]
    let just_saved_timer: StoredValue<JustSavedTimer, LocalStorage> = StoredValue::new_local(None);

    let clear_just_saved = move || {
        just_saved.try_set(false);
        #[cfg(feature = "hydrate")]
        just_saved_timer.try_update_value(|slot| {
            if let Some((handle, _)) = slot.take() {
                window().clear_timeout_with_handle(handle);
            }
        });
    };
    let arm_just_saved = move || {
        clear_just_saved();
        just_saved.try_set(true);
        #[cfg(feature = "hydrate")]
        {
            use wasm_bindgen::JsCast;
            let callback = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
                just_saved.try_set(false);
            });
            if let Ok(handle) = window().set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                3000,
            ) {
                just_saved_timer.try_set_value(Some((handle, callback)));
            }
        }
    };
    #[cfg(feature = "hydrate")]
    on_cleanup(move || {
        just_saved_timer.try_update_value(|slot| {
            if let Some((handle, _)) = slot.take() {
                window().clear_timeout_with_handle(handle);
            }
        });
    });

    let reset_format_select = move || {
        format_select_sync.update(|n| *n = n.wrapping_add(1));
        #[cfg(feature = "hydrate")]
        {
            use wasm_bindgen::JsCast;
            if let Some(el) = document()
                .get_element_by_id("editor-format-select")
                .and_then(|node| node.dyn_into::<web_sys::HtmlSelectElement>().ok())
            {
                el.set_value(format.get_untracked().as_str());
            }
        }
    };

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
        let current_format = format.get_untracked();
        let source = body.snapshot();
        let request = match current_format {
            DocumentFormatDto::Default => crate::contract::SaveDraftRequest {
                expected_version: ticket.expected_version,
                // Trim only the separate title; keep the live input and exact source.
                title: Some(title.get_untracked().trim().to_owned()),
                document: crate::document::default_document(&source),
            },
            DocumentFormatDto::Markdown => crate::contract::SaveDraftRequest {
                expected_version: ticket.expected_version,
                title: None,
                document: crate::document::markdown_document(&source),
            },
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
                let mut saved_clean = false;
                let applied = session.try_update(|s| {
                    match outcome {
                        Ok(saved) => s.text.save_succeeded(ticket, saved.version),
                        Err(ApiError::VersionConflict { current_version }) => s
                            .text
                            .save_rejected(ticket, SaveRejection::Conflict { current_version }),
                        Err(ApiError::Invalid { field }) if field.as_deref() == Some("title") => s
                            .text
                            .save_rejected(ticket, SaveRejection::Failed(Failure::InvalidTitle)),
                        Err(error) => s
                            .text
                            .save_rejected(ticket, SaveRejection::Failed(failure(&error))),
                    }
                    saved_clean = !crate::desk_state::Guarded::has_unsaved(s);
                });
                if applied.is_none() {
                    return;
                }
                if saved_clean {
                    arm_just_saved();
                }
                sync_save_slot_focus(was_save_focused, session, tag_load);
                settle_leave();
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (request, work_id, reconcile, settle_leave, arm_just_saved);
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
                let mut saved_clean = false;
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
                    saved_clean = !crate::desk_state::Guarded::has_unsaved(s);
                });
                if applied.is_none() {
                    return;
                }
                if saved_clean {
                    arm_just_saved();
                }
                sync_save_slot_focus(was_save_focused, session, tag_load);
                settle_leave();
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (request, settle_leave, arm_just_saved);
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
        clear_just_saved();
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
        clear_just_saved();
        let reconcile =
            session.with_untracked(|s| matches!(s.text.status(), SaveStatus::Failed(_)));
        if let Some(ticket) = session
            .try_update(DraftSession::begin_manual_save)
            .flatten()
        {
            send(ticket, reconcile);
        }
    };
    let focus_last_field = move || match last_field.get_value() {
        Field::Title if format.get_untracked() == DocumentFormatDto::Default => {
            #[cfg(feature = "hydrate")]
            if let Some(input) = title_input.get_untracked() {
                let options = web_sys::FocusOptions::new();
                options.set_prevent_scroll(true);
                let _ = input.focus_with_options(&options);
            }
        }
        Field::Title | Field::Body => body.focus(),
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
        let source = body.snapshot();
        let (preview_title, document) = match format.get_untracked() {
            DocumentFormatDto::Default => (
                title.get_untracked(),
                crate::document::default_document(&source),
            ),
            DocumentFormatDto::Markdown => (
                crate::document::markdown_first_h1(&source),
                crate::document::markdown_document(&source),
            ),
        };
        preview.set(Some((preview_title, document)));
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
        Effect::new(move |_| {
            let dirty_or_pending = session.with(crate::desk_state::Guarded::has_unsaved);
            if dirty_or_pending {
                clear_just_saved();
            }
        });
    }

    let on_body_ready = Callback::new(move |()| {
        // A new, empty default draft starts at its title; existing drafts or Markdown start at body.
        if format.get_untracked() == DocumentFormatDto::Default
            && title.get_untracked().is_empty()
            && body.snapshot().is_empty()
        {
            last_field.set_value(Field::Title);
        } else {
            last_field.set_value(Field::Body);
        }
        focus_last_field();
    });
    let on_body_change = Callback::new(move |()| {
        clear_just_saved();
        session.try_update(|s| s.text.edited());
    });

    let cancel_conversion = move || {
        if converting.get_untracked() {
            return;
        }
        convert_preview.set(None);
        reset_format_select();
        restore_scroll(page_scroll);
    };
    let on_format_change = Callback::new(move |selected: Option<String>| {
        if converting.get_untracked() {
            reset_format_select();
            return;
        }
        let Some(target) = selected.as_deref().and_then(DocumentFormatDto::parse) else {
            reset_format_select();
            return;
        };
        let current = format.get_untracked();
        if target == current {
            return;
        }
        if body.is_composing() || title_composing.get_value() {
            reset_format_select();
            return;
        }
        let blocked_by_session = session.with_untracked(|s| !s.can_convert());
        if blocked_by_session {
            reset_format_select();
            return;
        }
        let source = body.snapshot();
        let current_title = title.get_untracked();
        let conversion =
            crate::document::preview_format_conversion(current, target, &current_title, &source);
        convert_preview.set(Some(conversion));
        remember_scroll(page_scroll);
    });
    let apply_conversion = move || {
        let Some(conversion) = convert_preview.get_untracked() else {
            return;
        };
        if conversion.title_too_long()
            || converting.get_untracked()
            || body.is_composing()
            || title_composing.get_value()
        {
            return;
        }
        let Some(ticket) = session
            .try_update(|s| {
                if s.can_convert() {
                    s.text.begin_format_conversion()
                } else {
                    None
                }
            })
            .flatten()
        else {
            return;
        };
        conversion_notice.set(None);
        converting.set(true);
        clear_just_saved();
        let request = match conversion.target_format {
            DocumentFormatDto::Default => crate::contract::SaveDraftRequest {
                expected_version: ticket.expected_version,
                title: Some(conversion.title.trim().to_owned()),
                document: crate::document::default_document(&conversion.source),
            },
            DocumentFormatDto::Markdown => crate::contract::SaveDraftRequest {
                expected_version: ticket.expected_version,
                title: None,
                document: crate::document::markdown_document(&conversion.source),
            },
        };
        #[cfg(feature = "hydrate")]
        {
            let id = work_id.get_value();
            leptos::task::spawn_local(async move {
                use crate::desk_state::SaveRejection;
                let mut outcome = crate::api::save_draft(id.clone(), request)
                    .await
                    .map(|ack| ack.version);
                // A dropped reply can hide a committed PUT. Read back the exact proposed
                // version, format, title and bytes. Reconciliation never issues another write.
                if matches!(
                    outcome,
                    Err(ApiError::Network
                        | ApiError::Unavailable
                        | ApiError::Unexpected { .. }
                        | ApiError::VersionConflict { .. })
                ) && session.try_with_untracked(|s| s.text.in_flight() == Some(ticket))
                    == Some(true)
                    && let Ok(current) = crate::api::load_draft(id.clone()).await
                    && current.work_id == id
                {
                    if ticket.expected_version.checked_add(1) == Some(current.version)
                        && current.document.format == conversion.target_format
                        && current.document.source == conversion.source
                        && current.title == conversion.title
                    {
                        outcome = Ok(current.version);
                    } else if current.version != ticket.expected_version {
                        outcome = Err(ApiError::VersionConflict {
                            current_version: current.version,
                        });
                    }
                }
                converting.try_set(false);
                let mut converted_clean = false;
                let applied = session.try_update(|s| {
                    if s.text.in_flight() != Some(ticket) {
                        return false;
                    }
                    match &outcome {
                        Ok(version) => s.text.save_succeeded(ticket, *version),
                        Err(ApiError::VersionConflict { current_version }) => s.text.save_rejected(
                            ticket,
                            SaveRejection::Conflict {
                                current_version: *current_version,
                            },
                        ),
                        Err(error) => s
                            .text
                            .save_rejected(ticket, SaveRejection::Failed(failure(error))),
                    }
                    converted_clean = !crate::desk_state::Guarded::has_unsaved(s);
                    true
                });
                if applied != Some(true) {
                    return;
                }
                if outcome.is_ok() {
                    let next_title = conversion.title.trim().to_owned();
                    format.set(conversion.target_format);
                    title.set(next_title);
                    outline_open.set(false);
                    heading_menu_open.set(false);
                    body.apply_converted_document(conversion.target_format, &conversion.source);
                    if converted_clean {
                        arm_just_saved();
                    }
                } else if let Err(error) = &outcome {
                    let why = match error {
                        ApiError::VersionConflict { .. } => {
                            "Bản trên máy chủ đã thay đổi. Hãy xử lý xung đột trước khi đổi lại định dạng."
                        }
                        _ => failure_reason(failure(error)),
                    };
                    conversion_notice.set(Some(format!(
                        "Chưa xác nhận đổi định dạng. {why} Nội dung gốc vẫn còn trên trang này."
                    )));
                }
                convert_preview.set(None);
                reset_format_select();
                restore_scroll(page_scroll);
                settle_leave();
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (request, work_id, arm_just_saved);
    };

    let jump_to_recovery = move |target: RecoveryTarget| {
        #[cfg(feature = "hydrate")]
        {
            use wasm_bindgen::JsCast;
            let doc = document();
            // The jump must not put a destructive choice under a still-held Enter key.
            let selector = match target {
                RecoveryTarget::Title if format.get_untracked() == DocumentFormatDto::Default => {
                    "#draft-title"
                }
                RecoveryTarget::Title => {
                    let heading = body.outline.with_untracked(|outline| {
                        outline
                            .items
                            .iter()
                            .find(|item| item.is_title)
                            .map(|item| (item.from, outline.generation))
                    });
                    if !heading
                        .is_some_and(|(from, generation)| body.jump_to_heading(from, generation))
                    {
                        body.focus();
                    }
                    return;
                }
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
    let title_invalid = move || status() == SaveStatus::Failed(Failure::InvalidTitle);
    let markdown_title_invalid =
        move || title_invalid() && format.get() == DocumentFormatDto::Markdown;
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
    let format_options = vec![
        ChoiceOption::new("default", "Mặc định"),
        ChoiceOption::new("markdown", "Markdown"),
    ];
    let selected_format = Signal::derive(move || {
        let _ = format_select_sync.get();
        Some(format.get().as_str().to_string())
    });
    let md_tools_disabled = move || body.state.get() != BodyState::Ready || converting.get();
    let format_select_disabled =
        Signal::derive(move || converting.get() || session.with(|s| !s.can_convert()));
    view! {
        <form
            class="vk-editor"
            aria-labelledby="editor-heading"
            data-format=move || format.get().as_str()
            data-outline-open=move || if outline_open.get() { "true" } else { "false" }
            data-just-saved=move || if just_saved.get() { "true" } else { "false" }
            on:submit=move |event| {
                event.prevent_default();
                if convert_preview.with_untracked(Option::is_some) {
                    return;
                }
                save_manual();
            }
            on:keydown=move |event| {
                let shortcut = (event.ctrl_key() || event.meta_key())
                    && event.key().eq_ignore_ascii_case("s");
                if shortcut {
                    event.prevent_default();
                    if convert_preview.with_untracked(Option::is_some) {
                        return;
                    }
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
                                    <span
                                        class="vk-draft-action-content"
                                        data-draft-icon=move || match slot_action.get() {
                                            SaveSlotAction::Save { icon, .. }
                                            | SaveSlotAction::Retry { icon, .. } => icon.id(),
                                            _ => DraftActionIcon::Save.id(),
                                        }
                                    >
                                        {move || match slot_action.get() {
                                            SaveSlotAction::Save { icon, .. }
                                            | SaveSlotAction::Retry { icon, .. } => {
                                                render_draft_action_icon(icon).into_any()
                                            }
                                            _ => render_draft_action_icon(DraftActionIcon::Save).into_any(),
                                        }}
                                        <span>
                                            {move || match slot_action.get() {
                                                SaveSlotAction::Save { label, .. } => label,
                                                SaveSlotAction::Retry { label, .. } => label,
                                                _ => "Đang lưu…",
                                            }}
                                        </span>
                                    </span>
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
                >
                    <TooltipIconButton id="preview" icon=IconName::Visibility label="Xem trước"
                        on_press=move |_| open_preview() />
            </crate::ui::patterns::EditorToolbar>
            <Show when=move || matches!(status(), SaveStatus::Failed(_)) && !title_invalid()>
                <section class="vk-editor-recovery" data-testid="save-failed" data-tone="error" aria-label="Phục hồi lưu nháp">
                    // The toolbar is the sole polite announcement owner for this state.
                    {move || view! { <InlineNotice tone=Tone::Error announce=Announce::Off
                        title=status_text(status()) message="Nội dung đang viết vẫn còn trên trang này. Chọn Thử lại hoặc giữ trang mở." /> }}
                    <div class="vk-notice__actions">
                        <Button variant=ButtonVariant::Text id="retry-save" on_press=move |_| save()>"Thử lại"</Button>
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
                        message="Nội dung đang viết vẫn còn trên trang này. Bạn có thể lưu đè hoặc ở lại đối chiếu." />
                    <div class="vk-notice__actions">
                        <Button variant=ButtonVariant::Text id="keep-mine"
                            on_press=move |_| {
                                session.update(|s| s.text.keep_local_text());
                                save();
                            }>"Lưu đè bằng nội dung đang viết"</Button>
                    </div>
                    <SupportingDetails id="save-conflict-details" summary="Chi tiết">
                        <p>{move || status_detail(status())}</p>
                        <p>"Nội dung còn trên trang này; chưa có bản khôi phục tự động. Lưu đè sẽ thay bản trên máy chủ bằng nội dung đang viết."</p>
                    </SupportingDetails>
                </section>
            </Show>
            <div class="vk-editor-workspace">
                <div class="vk-editor-paper">
                    <Show when=move || format.get() == DocumentFormatDto::Markdown>
                        <div
                            id="editor-markdown-toolbar"
                            class="vk-md-toolbar"
                            role="toolbar"
                            aria-label="Công cụ soạn thảo Markdown"
                        >
                            <div
                                class="vk-md-toolbar__formats"
                                on:mousedown=move |ev| ev.prevent_default()
                            >
                                <div
                                    class="vk-md-heading-wrap"
                                    on:keydown=move |ev| {
                                        if ev.key() == "Escape" && heading_menu_open.get_untracked() {
                                            ev.prevent_default();
                                            heading_menu_open.set(false);
                                            focus_element_by_id("md-heading-tool");
                                        }
                                    }
                                >
                                    <Button
                                        id="md-heading-tool"
                                        variant=ButtonVariant::Tonal
                                        expanded=Signal::derive(move || heading_menu_open.get())
                                        state=Signal::derive(move || blocked(md_tools_disabled()))
                                        on_press=move |_| heading_menu_open.update(|open| *open = !*open)
                                    >
                                        "H"
                                    </Button>
                                    <Show when=move || heading_menu_open.get()>
                                        <div
                                            id="md-heading-menu"
                                            class="vk-md-heading-menu"
                                            role="menu"
                                            aria-label="Chọn cấp tiêu đề"
                                        >
                                            {(0..=6_u8)
                                                .map(|level| {
                                                    let label = if level == 0 {
                                                        "Đoạn văn".to_string()
                                                    } else {
                                                        format!("Tiêu đề {level}")
                                                    };
                                                    view! {
                                                        <div class="vk-md-heading-menu__entry" role="none">
                                                            <Button
                                                                variant=ButtonVariant::Text
                                                                on_press=move |_| {
                                                                    heading_menu_open.set(false);
                                                                    body.apply_heading(level);
                                                                }
                                                            >
                                                                <span
                                                                    class="vk-md-heading-menu__item"
                                                                    data-heading-level=level.to_string()
                                                                >
                                                                    {label}
                                                                </span>
                                                            </Button>
                                                        </div>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    </Show>
                                </div>
                                <Button
                                    id="md-bold"
                                    variant=ButtonVariant::Tonal
                                    state=Signal::derive(move || blocked(md_tools_disabled()))
                                    on_press=move |_| {
                                        heading_menu_open.set(false);
                                        body.apply_inline_format("bold");
                                    }
                                >
                                    "B"
                                </Button>
                                <Button
                                    id="md-italic"
                                    variant=ButtonVariant::Tonal
                                    state=Signal::derive(move || blocked(md_tools_disabled()))
                                    on_press=move |_| {
                                        heading_menu_open.set(false);
                                        body.apply_inline_format("italic");
                                    }
                                >
                                    "I"
                                </Button>
                                <Button
                                    id="md-link"
                                    variant=ButtonVariant::Tonal
                                    state=Signal::derive(move || blocked(md_tools_disabled()))
                                    on_press=move |_| {
                                        heading_menu_open.set(false);
                                        body.apply_inline_format("link");
                                    }
                                >
                                    "Liên kết"
                                </Button>
                            </div>
                            <Button
                                id="outline-toggle"
                                variant=Signal::derive(move || {
                                    if outline_open.get() {
                                        ButtonVariant::Filled
                                    } else {
                                        ButtonVariant::Tonal
                                    }
                                })
                                expanded=Signal::derive(move || outline_open.get())
                                on_press=move |_| {
                                    heading_menu_open.set(false);
                                    outline_open.update(|open| *open = !*open);
                                }
                            >
                                "Mục lục"
                            </Button>
                        </div>
                    </Show>
                    <Show when=move || format.get() == DocumentFormatDto::Default>
                        <TextField id="draft-title" label="Tiêu đề" name="title" max_length=200 input_ref=title_input
                            variant=FieldVariant::FloatingTonal
                            placeholder="Chưa đặt tên" autocomplete="off" value=title
                            error=Signal::derive(move || title_invalid().then(||
                                "Tiêu đề cần tối đa 200 ký tự trên một dòng. Hãy bỏ ký tự điều khiển.".to_owned()
                            ))
                            on_focus=move |_| last_field.set_value(Field::Title)
                            on_input=move |input: vink_ui::TextInput| {
                                title_composing.set_value(input.composing);
                                if title.get_untracked() != input.value {
                                    clear_just_saved();
                                    title.set(input.value);
                                    session.update(|s| s.text.edited());
                                }
                            } />
                        <TitleDisclosure title input=title_input />
                    </Show>
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
                        <p id=BODY_TITLE_ERROR_ID class="vk-ui-field__error" hidden=move || !markdown_title_invalid()>
                            {move || markdown_title_invalid().then_some(
                                "Sửa tiêu đề cấp 1 đầu tiên để có tối đa 200 ký tự trên một dòng. Hãy bỏ ký tự điều khiển."
                            )}
                        </p>
                        <BodyEditor source=body on_change=on_body_change on_ready=on_body_ready />
                        <Show when=move || body.state.get() == BodyState::Loading>
                            <p class="vk-muted vk-editor-body__pending">"Đang mở trình soạn thảo…"</p>
                        </Show>
                        <Show when=move || body.state.get() == BodyState::Failed>
                            <InlineNotice tone=Tone::Error announce=Announce::Assertive
                                message="Không mở được trình soạn thảo. Nội dung đã lưu trên máy chủ không bị ảnh hưởng; hãy tải lại trang."
                                attr:data-testid="editor-failed" />
                        </Show>
                    </div>
                </div>
                <Show when=move || format.get() == DocumentFormatDto::Markdown && outline_open.get()>
                    <dialog
                        node_ref=outline_panel
                        id="editor-outline-panel"
                        class="vk-editor-outline"
                        aria-labelledby="editor-outline-title"
                        on:cancel=move |ev: leptos::ev::Event| {
                            ev.prevent_default();
                            close_outline();
                        }
                        on:click=move |ev: leptos::ev::MouseEvent| {
                            #[cfg(feature = "hydrate")]
                            if let Some(panel) = outline_panel.get_untracked()
                                && ev.target().as_ref() == Some(panel.as_ref())
                            {
                                let r = panel.get_bounding_client_rect();
                                let (x, y) = (f64::from(ev.client_x()), f64::from(ev.client_y()));
                                if x < r.left() || x >= r.right() || y < r.top() || y >= r.bottom() {
                                    close_outline();
                                }
                            }
                            #[cfg(not(feature = "hydrate"))]
                            let _ = ev;
                        }
                        on:keydown=move |ev| {
                            if ev.key() == "Escape" {
                                ev.prevent_default();
                                ev.stop_propagation();
                                close_outline();
                            }
                        }
                    >
                        <div class="vk-editor-outline__head">
                            <h2 id="editor-outline-title" class="vk-editor-outline__heading">"Mục lục"</h2>
                            <IconButton
                                id="editor-outline-close"
                                icon=IconName::Close
                                label="Đóng mục lục"
                                on_press=move |_| {
                                    close_outline();
                                }
                            />
                        </div>
                        {move || {
                            let snapshot = body.outline.get();
                            if body.state.get() == BodyState::Failed {
                                view! { <p class="vk-editor-outline__empty">"Không mở được mục lục. Hãy tải lại trang."</p> }.into_any()
                            } else if body.state.get() != BodyState::Ready || (snapshot.items.is_empty() && snapshot.status == OutlineStatus::Partial) {
                                view! {
                                    <p class="vk-editor-outline__empty">"Đang đọc mục lục…"</p>
                                }
                                .into_any()
                            } else if snapshot.items.is_empty() {
                                view! {
                                    <p class="vk-editor-outline__empty">
                                        "Chưa có đề mục. Dùng # hoặc nút H để tạo mục lục."
                                    </p>
                                }
                                .into_any()
                            } else {
                                let partial = snapshot.status == OutlineStatus::Partial;
                                view! {
                                    <nav class="vk-editor-outline__nav" aria-labelledby="editor-outline-title">
                                        {render_outline_items(
                                            snapshot.items,
                                            snapshot.generation,
                                            snapshot.active_from,
                                            body,
                                            outline_open,
                                        )}
                                        <Show when=move || partial>
                                            <p class="vk-editor-outline__empty">"Đang cập nhật phần còn lại…"</p>
                                        </Show>
                                    </nav>
                                }
                                .into_any()
                            }
                        }}
                    </dialog>
                </Show>
            </div>
            <div class="vk-editor-work-info" on:focusin=move |_| last_field.set_value(Field::Tags)>
                <crate::pages::work_info::WorkInfo work_id=work_id.get_value() session labels
                    on_save=Callback::new(move |()| save()) tag_load=tag_load />
            </div>
            <section
                id="editor-format-region"
                class="vk-editor-format"
                data-testid="editor-format-region"
                aria-labelledby="editor-format-heading"
            >
                <div class="vk-editor-format__head">
                    <Heading level=HeadingLevel::H2 role=TypeRole::Title id="editor-format-heading">
                        "Định dạng bản thảo"
                    </Heading>
                </div>
                <Select
                    id="editor-format-select"
                    label="Chọn định dạng bản thảo"
                    options=format_options.clone()
                    value=selected_format
                    variant=SelectVariant::Inset16
                    disabled=format_select_disabled
                    on_change=on_format_change
                />
                {move || conversion_notice.get().map(|text| view! {
                    <p data-testid="conversion-notice" role="alert">{text}</p>
                })}
                <p class="vk-muted vk-editor-format__hint">
                    "Mặc định giữ tiêu đề riêng; Markdown bật Live Preview và lấy H1 đầu tiên làm tên tác phẩm."
                </p>
            </section>
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
                    "Bạn sẽ mất những chỉnh sửa chưa lưu trên trang này."
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
                        let empty = !crate::document::has_text(&document);
                        let document = ReadingDocument::new(&document);
                        let title_in_body = document.title_in_body();
                        let title = untitled(&title);
                        let body = if empty {
                            view! { <p class="vk-muted">"Chưa có nội dung."</p> }.into_any()
                        } else {
                            document.render(&title)
                        };
                        view! {
                            <article class="vk-preview" lang="vi">
                                {(!title_in_body).then(|| view! { <Heading level=HeadingLevel::H2 role=TypeRole::Headline>{title}</Heading> })}
                                {body}
                            </article>
                        }
                    })
            }}
        </vink_ui::Dialog>
        <vink_ui::Dialog
            id="format-convert-dialog"
            open=Signal::derive(move || convert_preview.with(Option::is_some))
            title=Signal::derive(move || {
                match convert_preview.get().map(|p| p.target_format) {
                    Some(DocumentFormatDto::Markdown) => "Đổi định dạng bản thảo sang Markdown?",
                    Some(DocumentFormatDto::Default) => "Đổi định dạng bản thảo sang Mặc định?",
                    None => "Đổi định dạng bản thảo?",
                }
                .to_string()
            })
            context=Signal::derive(move || {
                match convert_preview.get().map(|p| p.target_format) {
                    Some(DocumentFormatDto::Markdown) => {
                        "Tiêu đề hiện tại sẽ được đưa vào đầu bài bằng dòng # nếu chưa có H1. Thao tác này lưu thẳng bản nháp và bật Live Preview."
                    }
                    Some(DocumentFormatDto::Default) => {
                        "H1 đầu tiên (nếu có) sẽ dùng cho ô Tiêu đề riêng. Toàn bộ nội dung văn bản giữ nguyên; thao tác này lưu thẳng bản nháp."
                    }
                    None => "",
                }
                .to_string()
            })
            focus_title=true
            dismiss=vink_ui::DismissPolicy::EscapeOnly
            on_dismiss=move |_| cancel_conversion()
            actions=move || view! {
                <Button
                    variant=ButtonVariant::Text
                    id="format-convert-cancel"
                    state=Signal::derive(move || blocked(converting.get()))
                    on_press=move |_| cancel_conversion()
                >
                    "Hủy"
                </Button>
                <Button
                    variant=ButtonVariant::Filled
                    id="format-convert-apply"
                    state=Signal::derive(move || if converting.get() { ActionState::pending("") } else { blocked(convert_preview.with(|p| p.as_ref().is_none_or(ConversionPreview::title_too_long))) })
                    on_press=move |_| apply_conversion()
                >
                    {move || if converting.get() { "Đang đổi…" } else { "Áp dụng" }}
                </Button>
            }
            attr:data-testid="format-convert-dialog"
        >
            {move || {
                convert_preview.get().map(|p| {
                    let target_name = if p.target_format == DocumentFormatDto::Markdown {
                        "Markdown (Live Preview)"
                    } else {
                        "Mặc định"
                    };
                    let title_label = untitled(&p.title);
                    view! {
                        <div class="vk-format-preview">
                            <p class="vk-format-preview__target">
                                <strong>"Đích: "</strong>{target_name}
                            </p>
                            <p class="vk-format-preview__title">
                                <strong>"Tiêu đề sau chuyển đổi: "</strong>{title_label}
                            </p>
                            {p.title_too_long().then(|| view! {
                                <p role="alert">"Tiêu đề sau chuyển đổi dài quá 200 ký tự. Hãy hủy và rút ngắn tiêu đề hoặc H1 đầu tiên trước khi đổi định dạng."</p>
                            })}
                            <p class="vk-muted">{p.note}</p>
                            <pre class="vk-format-preview__source">{p.source}</pre>
                        </div>
                    }
                })
            }}
        </vink_ui::Dialog>
    }
}

#[component]
fn TitleDisclosure(title: RwSignal<String>, input: NodeRef<html::Input>) -> impl IntoView {
    let details = NodeRef::<html::Details>::new();
    let measure = NodeRef::<html::Span>::new();
    #[cfg(feature = "hydrate")]
    Effect::new(move |_| {
        use wasm_bindgen::{JsCast, closure::Closure};

        let (Some(input), Some(details), Some(measure)) =
            (input.get(), details.get(), measure.get())
        else {
            return;
        };
        let sync = {
            let input = input.clone();
            move || {
                let width = input.client_width();
                let clipped =
                    width > 0 && !input.value().is_empty() && input.scroll_width() > width;
                if !clipped && !details.hidden() {
                    // Only recover focus if the disclosure itself is about to lose it.
                    if document()
                        .active_element()
                        .is_some_and(|active| details.contains(Some(&active)))
                    {
                        let options = web_sys::FocusOptions::new();
                        options.set_prevent_scroll(true);
                        let _ = input.focus_with_options(&options);
                    }
                    details.set_open(false);
                }
                details.set_hidden(!clipped);
            }
        };
        sync();
        let callback = Closure::<dyn FnMut()>::new(sync);
        if let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
            observer.observe(&input);
            // The input's box may stay the same size while its text or font metrics change.
            observer.observe(&measure);
            let cleanup = send_wrapper::SendWrapper::new((observer, callback));
            on_cleanup(move || {
                let (observer, _callback) = cleanup.take();
                observer.disconnect();
            });
        }
    });
    view! {
        <span class="vk-editor-title-measure" aria-hidden="true">
            <span node_ref=measure>{move || title.get()}</span>
        </span>
        <details id="draft-title-details" class="vk-editor-title-details" node_ref=details hidden>
            <summary>"Xem trọn tiêu đề"</summary>
            <p id="draft-title-full">{move || title.get()}</p>
            <Button id="edit-full-title" variant=ButtonVariant::Text on_press=move |_| {
                #[cfg(feature = "hydrate")]
                if let Some(input) = input.get_untracked() {
                    let _ = input.focus();
                }
                #[cfg(not(feature = "hydrate"))]
                let _ = input;
            }>"Chỉnh tiêu đề"</Button>
        </details>
    }
}

fn outline_item_tier(level: u8, is_first_h1: bool) -> (&'static str, u8) {
    if is_first_h1 {
        ("vk-editor-outline__item vk-editor-outline__item--title", 0)
    } else {
        match level {
            1 | 2 => ("vk-editor-outline__item vk-editor-outline__item--h2", 0),
            3 => ("vk-editor-outline__item vk-editor-outline__item--sub", 1),
            4 => ("vk-editor-outline__item vk-editor-outline__item--sub", 2),
            _ => ("vk-editor-outline__item vk-editor-outline__item--sub", 3),
        }
    }
}

// The CSS container query owns the breakpoint. Native modal focus containment follows the
// actual painted layout, including changes to the rail width or text scale while open.
#[cfg(feature = "hydrate")]
fn observe_outline_layout(node: NodeRef<html::Dialog>) {
    use wasm_bindgen::{JsCast, closure::Closure};
    Effect::new(move |_| {
        let Some(panel) = node.get() else { return };
        let sync = move || {
            if !panel.is_connected() {
                return;
            }
            let modal = window()
                .get_computed_style(&panel)
                .ok()
                .flatten()
                .and_then(|css| css.get_property_value("position").ok())
                .is_some_and(|position| position == "fixed");
            if panel.open() && panel.matches(":modal").unwrap_or(false) == modal {
                return;
            }
            if panel.open() {
                panel.close();
            }
            if modal {
                let _ = panel.show_modal();
                focus_element_by_id("editor-outline-close");
            } else {
                panel.show();
                let _ = panel.set_attribute("role", "complementary");
            }
            if modal {
                let _ = panel.remove_attribute("role");
            }
        };
        sync();
        let callback = Closure::<dyn FnMut()>::new(sync);
        if let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
            if let Some(panel) = node.get_untracked() {
                observer.observe(&panel);
                if let Some(parent) = panel.parent_element() {
                    observer.observe(&parent);
                }
            }
            let cleanup = send_wrapper::SendWrapper::new((observer, callback));
            on_cleanup(move || {
                let (observer, _callback) = cleanup.take();
                observer.disconnect();
            });
        }
    });
}

fn focus_element_by_id(id: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(el) = document()
            .get_element_by_id(id)
            .and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = el.focus_with_options(&options);
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = id;
}

fn render_outline_items(
    items: Vec<OutlineItem>,
    generation: u64,
    active_from: Option<usize>,
    body: BodySource,
    outline_open: RwSignal<bool>,
) -> AnyView {
    render_outline_level(
        &mut items.into_iter().peekable(),
        0,
        generation,
        active_from,
        body,
        outline_open,
    )
}

fn render_outline_level(
    items: &mut std::iter::Peekable<std::vec::IntoIter<OutlineItem>>,
    parent_level: u8,
    generation: u64,
    active_from: Option<usize>,
    body: BodySource,
    outline_open: RwSignal<bool>,
) -> AnyView {
    let mut rows = Vec::new();
    while items.peek().is_some_and(|i| i.level > parent_level) {
        let item = items.next().expect("peeked heading");
        let (tier_class, indent) = outline_item_tier(item.level, item.is_title);
        let from = item.from;
        let children = items
            .peek()
            .is_some_and(|next| next.level > item.level)
            .then(|| {
                render_outline_level(
                    items,
                    item.level,
                    generation,
                    active_from,
                    body,
                    outline_open,
                )
            });
        rows.push(view! {
            <li class="vk-editor-outline__entry" style=format!("--outline-indent: {indent}")>
                <Button variant=ButtonVariant::Text attr:aria-current=(active_from == Some(from)).then_some("location")
                    on_press=move |_| {
                        if body.jump_to_heading(from, generation) {
                            #[cfg(feature = "hydrate")]
                            if let Some(panel) = document().get_element_by_id("editor-outline-panel")
                                && panel.matches(":modal").unwrap_or(false) {
                                use wasm_bindgen::JsCast;
                                if let Some(dialog) = panel.dyn_ref::<web_sys::HtmlDialogElement>() { dialog.close(); }
                                outline_open.set(false);
                                body.focus();
                            }
                            #[cfg(not(feature = "hydrate"))]
                            let _ = outline_open;
                        }
                    }>
                    <span class=tier_class data-heading-level=item.level.to_string() data-heading-from=from.to_string()>
                        {if item.title.is_empty() { "Chưa đặt tên".to_owned() } else { item.title }}
                    </span>
                </Button>
                {children}
            </li>
        });
    }
    view! { <ul class="vk-editor-outline__list" role="list">{rows}</ul> }.into_any()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DraftActionIcon {
    Send,
    Save,
    Spin,
}

impl DraftActionIcon {
    fn id(self) -> &'static str {
        match self {
            Self::Send => "send",
            Self::Save => "save",
            Self::Spin => "spin",
        }
    }
}

fn render_draft_action_icon(icon: DraftActionIcon) -> AnyView {
    match icon {
        DraftActionIcon::Send => view! {
            <svg
                class="vk-draft-action-icon"
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                focusable="false"
            >
                <path d="M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z" />
                <path d="m21.854 2.147-10.94 10.939" />
            </svg>
        }
        .into_any(),
        DraftActionIcon::Save => view! {
            <svg
                class="vk-draft-action-icon"
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                focusable="false"
            >
                <path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" />
                <path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7" />
                <path d="M7 3v4a1 1 0 0 0 1 1h7" />
            </svg>
        }
        .into_any(),
        DraftActionIcon::Spin => view! {
            <svg
                class="vk-draft-action-icon vk-draft-action-icon--spinner"
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                focusable="false"
            >
                <path d="M21 12a9 9 0 1 1-6.219-8.56" />
            </svg>
        }
        .into_any(),
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
        Failure::InvalidTitle => "Tiêu đề chưa hợp lệ.",
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
            "Có bản mới trên máy chủ. Ở lại để lưu đè hoặc xử lý xung đột.".into()
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
    Title,
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
        icon: DraftActionIcon,
    },
    Retry {
        label: &'static str,
        pending: bool,
        icon: DraftActionIcon,
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
    first_send: bool,
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
            first_send: session.text.acknowledged_version() <= 1,
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
        let sending_first = facts.first_send && matches!(facts.text, SaveStatus::Saving);
        let status_text = match (facts.text, facts.tags.as_ref()) {
            (SaveStatus::Saving, Some(TagStatus::Saving)) => "Đang lưu chữ và thẻ…",
            (SaveStatus::Saving, Some(TagStatus::Unsaved)) => "Đang lưu chữ; thẻ chưa lưu…",
            (
                SaveStatus::Saving,
                Some(TagStatus::Conflict { .. } | TagStatus::Refused(_) | TagStatus::Failed(_)),
            ) => "Đang lưu chữ; thẻ cần kiểm tra…",
            (SaveStatus::Saving, _) if facts.text_pending_newer && sending_first => {
                "Đang gửi; có chữ mới…"
            }
            (SaveStatus::Saving, _) if facts.text_pending_newer => "Đang lưu; có chữ mới…",
            (SaveStatus::Saving, None) if matches!(facts.tags_read, TagLoad::Failed(_)) => {
                "Đang lưu chữ; chưa tải được thẻ."
            }
            (SaveStatus::Saving, _) if sending_first => "Đang gửi…",
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
                label: if sending_first {
                    "Đang gửi…"
                } else {
                    "Đang lưu…"
                },
                pending: true,
                icon: DraftActionIcon::Spin,
            },
        };
    }

    let tag_action = match facts.tags {
        Some(TagStatus::Unsaved) => Some(SaveSlotAction::Save {
            label: "Lưu thẻ",
            pending: false,
            icon: DraftActionIcon::Save,
        }),
        Some(TagStatus::Failed(_)) => Some(SaveSlotAction::Retry {
            label: "Thử lưu lại thẻ",
            pending: false,
            icon: DraftActionIcon::Save,
        }),
        _ => None,
    };
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
            action: tag_action.unwrap_or(SaveSlotAction::Recover {
                label: "Xử lý xung đột",
                target: RecoveryTarget::TextConflict,
            }),
        };
    }

    if let SaveStatus::Failed(failure) = facts.text {
        let text_uncertain = save_is_uncertain(SaveStatus::Failed(failure));
        let problem_kind = Some(if text_uncertain {
            "uncertain"
        } else {
            "rejected"
        });
        let title_invalid = failure == Failure::InvalidTitle;
        let status_text = if title_invalid {
            "Tiêu đề chưa hợp lệ."
        } else if text_uncertain {
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
            action: if title_invalid {
                tag_action.unwrap_or(SaveSlotAction::Recover {
                    label: "Sửa tiêu đề",
                    target: RecoveryTarget::Title,
                })
            } else {
                SaveSlotAction::Retry {
                    label: "Thử lưu lại",
                    pending: false,
                    icon: DraftActionIcon::Save,
                }
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
                        icon: DraftActionIcon::Save,
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
                        icon: DraftActionIcon::Save,
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
            let use_send = facts.first_send && text_dirty;
            let label = if use_send {
                "Gửi"
            } else if text_dirty {
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
                    icon: if use_send {
                        DraftActionIcon::Send
                    } else {
                        DraftActionIcon::Save
                    },
                },
            }
        }
        Some(TagStatus::Unsaved) => {
            let (status_text, problem_kind) = if text_dirty {
                ("Chưa lưu chữ và thẻ", None)
            } else {
                ("Đã lưu chữ; chưa lưu thẻ", Some("partial"))
            };
            let use_send = facts.first_send && text_dirty;
            SaveSlotView {
                status_text: status_text.into(),
                status_token: "unsaved",
                problem_kind,
                detail_text: format!("{text_detail} {tag_detail}"),
                action: SaveSlotAction::Save {
                    label: if use_send { "Gửi" } else { "Lưu" },
                    pending: false,
                    icon: if use_send {
                        DraftActionIcon::Send
                    } else {
                        DraftActionIcon::Save
                    },
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
                        label: if facts.first_send { "Gửi" } else { "Lưu" },
                        pending: false,
                        icon: if facts.first_send {
                            DraftActionIcon::Send
                        } else {
                            DraftActionIcon::Save
                        },
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
                        label: if facts.first_send { "Gửi" } else { "Lưu" },
                        pending: false,
                        icon: if facts.first_send {
                            DraftActionIcon::Send
                        } else {
                            DraftActionIcon::Save
                        },
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
        first_send: false,
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
        first_send: false,
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
        first_send: false,
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
        | ApiError::SelfFollow
        | ApiError::RelationshipListPrivate
        | ApiError::ProfileGone
        | ApiError::FollowActorUnavailable
        | ApiError::FollowConflict { .. }
        | ApiError::FollowKeyReused
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
            first_send: false,
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
            first_send: false,
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
            first_send: false,
        });
        assert_eq!(text_only.status_text, "Chưa lưu chữ");
        assert_eq!(
            text_only.action,
            SaveSlotAction::Save {
                label: "Lưu",
                pending: false,
                icon: DraftActionIcon::Save,
            }
        );

        let both_dirty = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Unsaved,
            text_pending_newer: false,
            tags: Some(TagStatus::Unsaved),
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
            first_send: false,
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
            first_send: false,
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
            first_send: false,
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
            first_send: false,
        });
        assert_eq!(
            refused_dirty_text.action,
            SaveSlotAction::Save {
                label: "Lưu chữ",
                pending: false,
                icon: DraftActionIcon::Save,
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
            first_send: false,
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
            first_send: false,
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
    fn first_send_uses_gui_label_until_version_two_is_acknowledged() {
        let send_idle = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Unsaved,
            text_pending_newer: false,
            tags: None,
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
            first_send: true,
        });
        assert_eq!(
            send_idle.action,
            SaveSlotAction::Save {
                label: "Gửi",
                pending: false,
                icon: DraftActionIcon::Send,
            }
        );

        let send_pending = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Saving,
            text_pending_newer: false,
            tags: None,
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
            first_send: true,
        });
        assert_eq!(send_pending.status_text, "Đang gửi…");
        assert_eq!(
            send_pending.action,
            SaveSlotAction::Save {
                label: "Đang gửi…",
                pending: true,
                icon: DraftActionIcon::Spin,
            }
        );

        let subsequent_save = project_save_slot(&SaveSlotFacts {
            text: SaveStatus::Unsaved,
            text_pending_newer: false,
            tags: None,
            tags_read: TagLoad::Ready,
            tags_pending_newer: false,
            first_send: false,
        });
        assert_eq!(
            subsequent_save.action,
            SaveSlotAction::Save {
                label: "Lưu",
                pending: false,
                icon: DraftActionIcon::Save,
            }
        );
    }

    #[test]
    fn outline_item_tier_assigns_title_to_first_h1_and_caps_subheadings() {
        assert_eq!(
            outline_item_tier(1, true),
            ("vk-editor-outline__item vk-editor-outline__item--title", 0)
        );
        assert_eq!(
            outline_item_tier(2, false),
            ("vk-editor-outline__item vk-editor-outline__item--h2", 0)
        );
        assert_eq!(
            outline_item_tier(3, false),
            ("vk-editor-outline__item vk-editor-outline__item--sub", 1)
        );
        assert_eq!(
            outline_item_tier(4, false),
            ("vk-editor-outline__item vk-editor-outline__item--sub", 2)
        );
        assert_eq!(
            outline_item_tier(5, false),
            ("vk-editor-outline__item vk-editor-outline__item--sub", 3)
        );
        assert_eq!(
            outline_item_tier(6, false),
            ("vk-editor-outline__item vk-editor-outline__item--sub", 3)
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
