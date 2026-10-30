//! Bàn viết: private drafts, loaded only in the browser through the authenticated API.
//! Server rendering emits the shell and a loading state, never draft content.

use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params_map;

use crate::contract::{ApiError, Draft, DraftList};
use crate::desk_state::{EditorSession, Failure, SaveStatus};

const DESK_UNAVAILABLE_SSR: &str = "Bàn viết cần JavaScript để mở bản nháp riêng.";

#[component]
pub fn DeskIndex() -> impl IntoView {
    let drafts: LocalResource<Result<DraftList, ApiError>> = LocalResource::new(load_drafts);
    view! {
        <Title text="Bàn viết — Vink" />
        <Meta name="robots" content="noindex" />
        <section class="vk-desk" aria-labelledby="desk-title">
            <h1 id="desk-title" class="vk-page-title">
                "Bàn viết"
            </h1>
            <p class="vk-muted">
                "Bản nháp ở đây chỉ chủ sở hữu đọc được. Lưu nháp không gửi duyệt và không công khai."
            </p>
            <noscript>
                <p class="vk-notice">{DESK_UNAVAILABLE_SSR}</p>
            </noscript>
            <Suspense fallback=|| view! { <p class="vk-muted">"Đang mở Bàn viết…"</p> }>
                {move || Suspend::new(async move {
                    match drafts.await {
                        Ok(list) => view! { <DraftListView list /> }.into_any(),
                        Err(error) => view! { <DeskError error /> }.into_any(),
                    }
                })}
            </Suspense>
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
fn DraftListView(list: DraftList) -> impl IntoView {
    let creating = RwSignal::new(false);
    let create_failed = RwSignal::new(false);
    #[cfg(feature = "hydrate")]
    let navigate = leptos_router::hooks::use_navigate();
    let create = move |_| {
        if creating.get_untracked() {
            return;
        }
        creating.set(true);
        create_failed.set(false);
        #[cfg(feature = "hydrate")]
        let navigate = navigate.clone();
        #[cfg(feature = "hydrate")]
        leptos::task::spawn_local(async move {
            match crate::api::create_draft().await {
                Ok(draft) => {
                    navigate(&format!("/ban-viet/{}", draft.work_id), Default::default());
                }
                Err(_) => {
                    creating.set(false);
                    create_failed.set(true);
                }
            }
        });
    };
    let items = list
        .drafts
        .into_iter()
        .map(|draft| {
            let href = format!("/ban-viet/{}", draft.work_id);
            let title = untitled(&draft.title);
            view! {
                <li class="vk-card">
                    <a class="vk-card__link" href=href>
                        <span class="vk-card__title">{title}</span>
                        <span class="vk-card__meta">"Phiên bản " {draft.version}</span>
                    </a>
                </li>
            }
        })
        .collect_view();
    view! {
        <div class="vk-actions">
            <button
                type="button"
                class="vk-button vk-button--filled"
                data-testid="create-draft"
                on:click=create
                disabled=move || creating.get()
            >
                {move || if creating.get() { "Đang tạo…" } else { "Tạo bản nháp mới" }}
            </button>
        </div>
        <Show when=move || create_failed.get()>
            <p class="vk-notice" role="alert">
                "Chưa tạo được bản nháp. Hãy thử lại."
            </p>
        </Show>
        <ul class="vk-cards" role="list" aria-label="Bản nháp của bạn">
            {items}
        </ul>
    }
}

fn untitled(title: &str) -> String {
    if title.is_empty() {
        "Chưa đặt tên".to_string()
    } else {
        title.to_string()
    }
}

#[component]
fn DeskError(error: ApiError) -> impl IntoView {
    let (title, detail) = match error {
        ApiError::Unauthenticated => (
            "Bạn chưa đăng nhập.",
            "Vink chưa chọn hệ thống đăng nhập (D-05). Môi trường local có thể bật danh tính mẫu; xem docs/development.",
        ),
        ApiError::NotFound => (
            "Không tìm thấy bản nháp này.",
            "Bản nháp không tồn tại hoặc không thuộc về bạn.",
        ),
        ApiError::Unavailable | ApiError::Network => (
            "Chưa kết nối được máy chủ.",
            "Dữ liệu chưa bị thay đổi. Hãy tải lại trang sau ít phút.",
        ),
        _ => ("Có lỗi khi mở Bàn viết.", "Hãy tải lại trang."),
    };
    view! {
        <div class="vk-notice" role="alert" data-testid="desk-error">
            <p class="vk-notice__title">{title}</p>
            <p>{detail}</p>
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
        <section class="vk-desk">
            <p class="vk-breadcrumb">
                <a href="/ban-viet">"← Bàn viết"</a>
            </p>
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

#[component]
fn Editor(draft: Draft) -> impl IntoView {
    let work_id = StoredValue::new(draft.work_id.clone());
    let initial_text = crate::document::document_to_text(&draft.document);
    let initial_title = draft.title.clone();
    let text = StoredValue::new(initial_text.clone());
    let title = StoredValue::new(initial_title.clone());
    let session = RwSignal::new(EditorSession::loaded(draft.version));

    let save = move || {
        let Some(ticket) = session.try_update(EditorSession::begin_save).flatten() else {
            return;
        };
        let request = crate::contract::SaveDraftRequest {
            expected_version: ticket.expected_version,
            title: title.get_value(),
            document: crate::document::text_to_document(&text.get_value()),
        };
        #[cfg(feature = "hydrate")]
        leptos::task::spawn_local(async move {
            use crate::desk_state::SaveRejection;
            let outcome = crate::api::save_draft(work_id.get_value(), request).await;
            session.update(|s| match outcome {
                Ok(saved) => s.save_succeeded(ticket, saved.version),
                Err(ApiError::VersionConflict { current_version }) => {
                    s.save_rejected(ticket, SaveRejection::Conflict { current_version })
                }
                Err(error) => s.save_rejected(ticket, SaveRejection::Failed(failure(&error))),
            });
        });
        #[cfg(not(feature = "hydrate"))]
        let _ = (request, work_id);
    };

    #[cfg(feature = "hydrate")]
    {
        let guard = window_event_listener(leptos::ev::beforeunload, move |event| {
            if session.with_untracked(EditorSession::has_unsaved_text) {
                event.prevent_default();
            }
        });
        on_cleanup(move || guard.remove());
    }

    let status = move || session.with(EditorSession::status);
    view! {
        <form
            class="vk-editor"
            aria-label="Soạn bản nháp"
            on:submit=move |event| {
                event.prevent_default();
                save();
            }
            on:keydown=move |event| {
                if (event.ctrl_key() || event.meta_key()) && event.key().eq_ignore_ascii_case("s") {
                    event.prevent_default();
                    save();
                }
            }
        >
            <label class="vk-field">
                <span class="vk-field__label">"Tiêu đề"</span>
                <input
                    class="vk-input vk-input--title"
                    name="title"
                    lang="vi"
                    maxlength="200"
                    placeholder="Chưa đặt tên"
                    autocomplete="off"
                    prop:value=initial_title
                    on:input=move |event| {
                        title.set_value(event_target_value(&event));
                        session.update(EditorSession::edited);
                    }
                />
            </label>
            <label class="vk-field">
                <span class="vk-field__label">"Nội dung"</span>
                <span class="vk-field__hint">
                    "Xuống dòng để ngắt câu thơ; để một dòng trống giữa các đoạn hoặc khổ."
                </span>
                <textarea
                    class="vk-input vk-textarea"
                    name="body"
                    lang="vi"
                    spellcheck="false"
                    rows="18"
                    prop:value=initial_text
                    on:input=move |event| {
                        text.set_value(event_target_value(&event));
                        session.update(EditorSession::edited);
                    }
                ></textarea>
            </label>
            <div class="vk-editor__bar">
                <p
                    class="vk-save-status"
                    role="status"
                    aria-live="polite"
                    data-testid="save-status"
                    data-status=move || status().token()
                >
                    {move || status_text(status())}
                </p>
                <button
                    type="submit"
                    class="vk-button vk-button--filled"
                    data-testid="save"
                    disabled=move || !session.with(EditorSession::can_save)
                >
                    "Lưu nháp"
                </button>
            </div>
            <Show when=move || matches!(status(), SaveStatus::Conflict { .. })>
                <div class="vk-notice vk-notice--warning" role="alert" data-testid="conflict">
                    <p class="vk-notice__title">"Bản nháp đã được lưu ở nơi khác."</p>
                    <p>
                        "Nội dung bạn đang viết vẫn còn nguyên trên trang này. Bạn có thể lưu đè bằng nội dung này, hoặc sao chép nó trước khi tải lại bản trên máy chủ."
                    </p>
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
                </div>
            </Show>
        </form>
    }
}

fn status_text(status: SaveStatus) -> String {
    match status {
        SaveStatus::Saved { version } => format!("Đã lưu · phiên bản {version}"),
        SaveStatus::Unsaved => "Chưa lưu".into(),
        SaveStatus::Saving => "Đang lưu…".into(),
        SaveStatus::Conflict { server_version } => {
            format!("Xung đột: máy chủ đang ở phiên bản {server_version}")
        }
        SaveStatus::Failed(failure) => {
            let reason = match failure {
                Failure::Network => "mất kết nối",
                Failure::Unavailable => "máy chủ tạm không phục vụ",
                Failure::Unauthenticated => "phiên đăng nhập không hợp lệ",
                Failure::Forbidden => "yêu cầu bị từ chối",
                Failure::NotFound => "không tìm thấy bản nháp",
                Failure::Invalid => "nội dung không hợp lệ",
                Failure::Unexpected => "lỗi không xác định",
            };
            format!("Chưa lưu được ({reason}). Nội dung vẫn còn trên trang; hãy thử lại.")
        }
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
