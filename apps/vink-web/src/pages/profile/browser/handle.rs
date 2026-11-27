use leptos::{prelude::*, task::spawn_local};
use vink_ui::{
    ActionState, Button, ButtonVariant, Dialog, DismissPolicy, FieldMode, IconName, Link,
    TextField, TextInput,
};

use crate::{
    account::Session,
    api,
    contract::{
        ApiError,
        profile::{HandleCommand, HandleIntent, HandleReceipt, HandleRefusal, OwnHandle},
    },
};

const ROW: &str = "dia-chi-tac-gia";
const FIELD: &str = "author-handle";
const RULE: &str = "Dùng 1–40 chữ thường a–z, số và dấu gạch nối giữa các nhóm.";

#[derive(Clone, Copy)]
pub(super) struct Controller {
    session: Session,
    scope: StoredValue<String>,
    principal: StoredValue<String>,
    confirmed: RwSignal<Option<OwnHandle>>,
    pending: RwSignal<Option<HandleCommand>>,
    editing: RwSignal<Option<OwnHandle>>,
    draft: RwSignal<String>,
    composing: RwSignal<bool>,
    opened: RwSignal<bool>,
    busy: RwSignal<bool>,
    reread: RwSignal<bool>,
    unavailable: RwSignal<bool>,
    retryable: RwSignal<bool>,
    message: RwSignal<&'static str>,
    error: RwSignal<Option<String>>,
}

impl Controller {
    pub(super) fn new(session: Session, scope: StoredValue<String>, principal: String) -> Self {
        let pending = pending_for(&principal);
        Self {
            session,
            scope,
            principal: StoredValue::new(principal),
            confirmed: RwSignal::new(None),
            pending: RwSignal::new(pending),
            editing: RwSignal::new(None),
            draft: RwSignal::new(String::new()),
            composing: RwSignal::new(false),
            opened: RwSignal::new(false),
            busy: RwSignal::new(false),
            reread: RwSignal::new(false),
            unavailable: RwSignal::new(false),
            retryable: RwSignal::new(false),
            message: RwSignal::new(""),
            error: RwSignal::new(None),
        }
    }

    fn live(self) -> bool {
        self.scope.try_get_value().is_some_and(|scope| {
            self.session
                .profile_scope
                .try_get_untracked()
                .flatten()
                .as_ref()
                == Some(&scope)
                && self.session.principal.try_get_untracked().flatten()
                    == self.principal.try_get_value()
        })
    }

    /// All reads and writes share one in-flight slot. A broadcast/profile ACK while busy
    /// schedules another read, so it cannot replace a newer confirmation with a late answer.
    pub(super) fn refresh(self) {
        if !self.live() {
            return;
        }
        if self.busy.get_untracked() {
            self.reread.set(true);
            return;
        }
        self.busy.set(true);
        spawn_local(async move {
            let result = api::load_own_handle().await;
            if self.live() {
                let recovering = self.unavailable.get_untracked()
                    && self.pending.with_untracked(Option::is_none);
                if let Some(own) = self.accept_read(result) {
                    if recovering {
                        if self.opened.get_untracked() {
                            self.editing.set(Some(own.clone()));
                        }
                        self.message.set("");
                    }
                    self.confirmed.set(Some(own));
                }
                self.finished();
            }
        });
    }

    fn accept_read(self, result: Result<OwnHandle, ApiError>) -> Option<OwnHandle> {
        match result {
            Ok(own)
                if self
                    .principal
                    .with_value(|expected| *expected == own.principal_id)
                    && valid_handle(&own.handle)
                    && own.version >= 0
                    && (0..=2).contains(&own.remaining_changes)
                    && self.confirmed.with_untracked(|last| {
                        last.as_ref().is_none_or(|last| own.version >= last.version)
                    }) =>
            {
                self.unavailable.set(false);
                Some(own)
            }
            Ok(own)
                if self
                    .principal
                    .with_value(|expected| *expected != own.principal_id) =>
            {
                self.session.fence_profile();
                None
            }
            Err(ApiError::Unauthenticated | ApiError::Forbidden) => {
                self.session.fence_profile();
                None
            }
            _ => {
                self.unavailable.set(true);
                None
            }
        }
    }

    fn finished(self) {
        // Fencing a session can dispose this component on the same update.
        if !self.live() {
            return;
        }
        self.busy.set(false);
        if self.reread.get_untracked() {
            self.reread.set(false);
            self.refresh();
        }
    }

    fn remember(self, command: Option<HandleCommand>) {
        if let Ok(Some(storage)) = window().session_storage() {
            let key = self
                .principal
                .with_value(|principal| storage_key(principal));
            match &command {
                Some(command) => {
                    if let Ok(json) = serde_json::to_string(command) {
                        let _ = storage.set_item(&key, &json);
                    }
                }
                None => {
                    let _ = storage.remove_item(&key);
                }
            }
        }
        self.pending.set(command);
        self.retryable.set(false);
    }

    fn open(self) {
        if self.busy.get_untracked() || !self.live() {
            return;
        }
        let pending = self.pending.get_untracked();
        let own = self.confirmed.get_untracked();
        let editable = own
            .as_ref()
            .is_some_and(|own| !own.setup_complete || own.remaining_changes > 0);
        if pending.is_none() && (!editable || self.unavailable.get_untracked()) {
            return;
        }
        self.draft.set(
            pending
                .as_ref()
                .map(|command| command.handle.clone())
                .or_else(|| own.as_ref().map(|own| own.handle.clone()))
                .unwrap_or_default(),
        );
        self.editing.set(own);
        self.error.set(None);
        self.composing.set(false);
        self.message.set(if pending.is_some() {
            "Chưa nhận được xác nhận. Hãy kiểm tra kết quả trước khi đổi tiếp."
        } else {
            ""
        });
        self.opened.set(true);
    }

    fn close(self) {
        self.opened.set(false);
        self.composing.set(false);
        request_animation_frame(move || {
            if self.live()
                && document()
                    .active_element()
                    .is_none_or(|active| active.tag_name() == "BODY")
            {
                super::super::super::focus_recovery(ROW);
            }
        });
    }

    fn submit(self) {
        if !self.opened.get_untracked()
            || !self.live()
            || self.busy.get_untracked()
            || self.unavailable.get_untracked()
            || self.composing.get_untracked()
            || self.pending.with_untracked(Option::is_some)
        {
            return;
        }
        let Some(own) = self.editing.get_untracked() else {
            return;
        };
        let handle = self.draft.get_untracked();
        let intent = if own.setup_complete {
            if own.remaining_changes == 0 {
                self.field_error("Bạn đã dùng hết hai lượt đổi.");
                return;
            }
            if handle == own.handle {
                self.field_error("Đây là địa chỉ hiện tại của bạn.");
                return;
            }
            HandleIntent::Rename
        } else {
            HandleIntent::Setup
        };
        if !valid_handle(&handle) {
            self.field_error(RULE);
            return;
        }
        let Some(command_id) = command_id() else {
            self.message
                .set("Trình duyệt chưa tạo được mã lệnh. Hãy thử lại.");
            return;
        };
        self.error.set(None);
        self.remember(Some(HandleCommand {
            principal_id: self.principal.get_value(),
            command_id,
            expected_version: own.version,
            intent,
            handle,
        }));
        self.dispatch(false);
    }

    fn field_error(self, message: &'static str) {
        self.error.set(Some(message.to_string()));
        if self.opened.get_untracked() {
            super::super::super::focus_recovery(FIELD);
        }
    }

    fn dispatch(self, receipt_only: bool) {
        let Some(command) = self.pending.get_untracked() else {
            return;
        };
        if self.busy.get_untracked() || !self.live() {
            return;
        }
        self.retryable.set(false);
        self.busy.set(true);
        self.message.set(if receipt_only {
            "Đang kiểm tra kết quả…"
        } else {
            "Đang lưu địa chỉ…"
        });
        spawn_local(async move {
            let result = if receipt_only {
                api::load_handle_receipt(&command.command_id).await
            } else {
                api::save_handle(&command).await
            };
            // The committed public address may have changed even if this owner's form is gone.
            if result.is_ok()
                || (!receipt_only
                    && matches!(
                        &result,
                        Err(ApiError::Network
                            | ApiError::Unavailable
                            | ApiError::Unexpected { .. })
                    ))
            {
                super::announce_change();
            }
            if !self.live() {
                return;
            }
            match result {
                Ok(receipt) if matches_receipt(&command, &receipt) => {
                    let result = api::load_own_handle().await;
                    if !self.live() {
                        return;
                    }
                    match self.accept_read(result) {
                        Some(own) if own.version >= receipt.accepted_version => {
                            self.confirmed.set(Some(own));
                            self.remember(None);
                            self.message.set("Đã lưu địa chỉ tác giả.");
                            self.close();
                        }
                        _ if self.live() => {
                            self.message.set("Đã nhận biên nhận. Chưa đọc được địa chỉ hiện tại, hãy kiểm tra lại.");
                        }
                        _ => {}
                    }
                }
                Err(ApiError::Unauthenticated | ApiError::Forbidden) => {
                    self.session.fence_profile();
                }
                Err(ApiError::NotFound) if receipt_only => {
                    self.retryable.set(true);
                    self.message
                        .set("Chưa tìm thấy biên nhận. Bạn có thể gửi lại đúng yêu cầu này.");
                }
                Err(ApiError::NotFound) => {
                    self.remember(None);
                    self.confirmed.set(None);
                    self.editing.set(None);
                    self.unavailable.set(true);
                    self.message
                        .set("Không tìm thấy địa chỉ tác giả. Hãy tải lại để kiểm tra.");
                }
                Err(refused) if !receipt_only && refusal_message(&refused).is_some() => {
                    self.remember(None);
                    let result = api::load_own_handle().await;
                    if !self.live() {
                        return;
                    }
                    if let Some(own) = self.accept_read(result) {
                        self.editing.set(Some(own.clone()));
                        self.confirmed.set(Some(own));
                    }
                    if self.live() {
                        let message = refusal_message(&refused).unwrap_or_default();
                        self.message.set("");
                        self.field_error(message);
                    }
                }
                _ => self
                    .message
                    .set("Chưa nhận được xác nhận. Hãy kiểm tra kết quả trước khi đổi tiếp."),
            }
            self.finished();
            if self.live()
                && self.opened.get_untracked()
                && self.pending.with_untracked(Option::is_some)
                && document()
                    .active_element()
                    .is_none_or(|active| active.tag_name() == "BODY")
            {
                super::super::super::focus_recovery("author-handle-receipt");
            }
        });
    }
}

fn refusal_message(error: &ApiError) -> Option<&'static str> {
    Some(match error {
        ApiError::Invalid { .. } => "Địa chỉ không hợp lệ hoặc được dành riêng. Hãy chọn tên khác.",
        ApiError::VersionConflict { .. } => {
            "Địa chỉ đã thay đổi ở phiên khác. Hãy kiểm tra lại trước khi lưu."
        }
        ApiError::HandleRefused(HandleRefusal::Taken) => {
            "Địa chỉ này đã có người dùng. Hãy chọn tên khác."
        }
        ApiError::HandleRefused(HandleRefusal::OwnAlias) => {
            "Đây là địa chỉ cũ của bạn. Hiện chưa thể dùng lại."
        }
        ApiError::HandleRefused(HandleRefusal::Exhausted) => "Bạn đã dùng hết hai lượt đổi.",
        ApiError::HandleRefused(HandleRefusal::KeyReused) => {
            "Yêu cầu này đã dùng. Hãy kiểm tra địa chỉ hiện tại."
        }
        _ => return None,
    })
}

fn valid_handle(text: &str) -> bool {
    text.len() <= 40
        && text.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        })
}

fn command_id() -> Option<String> {
    let mut bytes = [0; 16];
    window()
        .crypto()
        .ok()?
        .get_random_values_with_u8_array(&mut bytes)
        .ok()?;
    Some(crate::endorsement_state::uuid_v4(bytes))
}

fn storage_key(principal: &str) -> String {
    format!("vink.handle.pending.{principal}")
}

fn pending_for(principal: &str) -> Option<HandleCommand> {
    let storage = window().session_storage().ok()??;
    let raw = storage.get_item(&storage_key(principal)).ok()??;
    let command: HandleCommand = serde_json::from_str(&raw).ok()?;
    (command.principal_id == principal
        && command.expected_version >= 0
        && valid_handle(&command.handle)
        && command.command_id.len() == 36
        && command.command_id.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        }))
    .then_some(command)
}

fn matches_receipt(command: &HandleCommand, receipt: &HandleReceipt) -> bool {
    receipt.command_id == command.command_id
        && receipt.handle == command.handle
        && (command.expected_version.checked_add(1) == Some(receipt.accepted_version)
            || (command.intent == HandleIntent::Rename
                && command.expected_version == receipt.accepted_version))
}

#[component]
pub(super) fn HandleRow(controller: Controller) -> impl IntoView {
    let row = NodeRef::<leptos::html::Section>::new();
    Effect::new(move |_| {
        if let Some(row) = row.get()
            && window()
                .location()
                .hash()
                .is_ok_and(|hash| hash == "#dia-chi-tac-gia")
            && document().active_element().is_none_or(|active| {
                active.tag_name() == "BODY"
                    || matches!(active.id().as_str(), "noi-dung" | "edit-profile-heading")
            })
        {
            row.scroll_into_view();
            let _ = row.focus();
        }
    });
    let can_open = move || {
        controller.pending.with(Option::is_some)
            || (!controller.unavailable.get()
                && controller.confirmed.with(|own| {
                    own.as_ref()
                        .is_some_and(|own| !own.setup_complete || own.remaining_changes > 0)
                }))
    };
    view! {
        <section id=ROW node_ref=row tabindex="-1" aria-labelledby="author-handle-heading"
            class="grid min-w-0 gap-vk-2 rounded-vk-md border border-vk-outline-variant p-vk-3 wrap-anywhere focus-visible:[outline:var(--vk-size-focus-ring)_solid_var(--vk-focus-ring)]">
            <div class="flex flex-wrap items-center justify-between gap-vk-3 min-w-0">
                <div class="grid min-w-0 gap-vk-1">
                    <h2 id="author-handle-heading" class="m-0 text-[length:var(--vk-type-body-size)] font-vk-role-label">"Địa chỉ tác giả"</h2>
                    {move || controller.confirmed.get().map(|own| view! {
                        <p class="m-0" id="handle-saved"
                            data-setup=own.setup_complete.to_string()>{format!("@{}", own.handle)}</p>
                        <p class="m-0 vk-muted">{if !own.setup_complete {
                            "Địa chỉ gợi ý".to_string()
                        } else if own.remaining_changes == 0 {
                            "Đã hết lượt đổi.".to_string()
                        } else {
                            format!("Còn {} lượt đổi.", own.remaining_changes)
                        }}</p>
                    })}
                </div>
                <Show when=can_open>
                    <Button id="author-handle-open" variant=ButtonVariant::Tonal leading=IconName::EditOutline
                        state=Signal::derive(move || button_state(controller.busy.get()))
                        on_press=Callback::new(move |()| controller.open())>
                        {move || if controller.pending.with(Option::is_some) {
                            "Kiểm tra kết quả"
                        } else if controller.confirmed.with(|own| own.as_ref().is_some_and(|own| own.setup_complete)) {
                            "Đổi địa chỉ"
                        } else {
                            "Thiết lập"
                        }}
                    </Button>
                </Show>
            </div>
            <p id="author-handle-status" role="status" aria-atomic="true" class="m-0 empty:hidden">{move || {
                if controller.opened.get() {
                    ""
                } else if controller.unavailable.get() {
                    "Chưa tải được địa chỉ tác giả."
                } else if controller.confirmed.with(Option::is_none) && controller.pending.with(Option::is_none) {
                    "Đang tải địa chỉ tác giả…"
                } else if controller.pending.with(Option::is_some) && controller.message.get().is_empty() {
                    "Có yêu cầu địa chỉ đang chờ xác nhận."
                } else {
                    controller.message.get()
                }
            }}</p>
            <Show when=move || controller.unavailable.get() && !controller.opened.get()>
                <Button id="author-handle-refresh" variant=ButtonVariant::Text
                    state=Signal::derive(move || button_state(controller.busy.get()))
                    on_press=Callback::new(move |()| controller.refresh())>"Thử lại"</Button>
            </Show>
            {move || controller.confirmed.get().filter(|own| own.setup_complete).map(|own| view! {
                <div class="flex flex-wrap items-start gap-vk-2 min-w-0">
                    <Link attr:id="own-handle-public-link" attr:data-profile-return="true"
                        href=format!("/@{}", own.handle)>"Xem trang tác giả"</Link>
                    {(!own.aliases.is_empty()).then(|| view! {
                        <vink_ui::SupportingDetails summary=format!("Liên kết cũ ({})", own.aliases.len()) id="author-handle-history">
                            <ul class="m-0 list-none p-0">
                                {own.aliases.into_iter().filter(|handle| valid_handle(handle)).map(|name| {
                                    let href = format!("/@{name}");
                                    view! {
                                        <li><Link attr:id=format!("own-handle-alias-{name}")
                                            attr:data-profile-return="true" href=href.clone()>{href}</Link></li>
                                    }
                                }).collect_view()}
                            </ul>
                        </vink_ui::SupportingDetails>
                    })}
                </div>
            })}
        </section>
    }
}

fn button_state(disabled: bool) -> ActionState {
    if disabled {
        ActionState::Disabled { reason: None }
    } else {
        ActionState::Enabled
    }
}

#[component]
pub(super) fn HandleDialog(controller: Controller) -> impl IntoView {
    let guard = expect_context::<crate::navigation::NavigationGuard>();
    let registration = guard.set_overlay(move |reason| {
        if reason != vink_ui::CloseReason::Back
            || controller.opened.try_get_untracked() != Some(true)
        {
            return false;
        }
        controller.close();
        true
    });
    on_cleanup(move || guard.clear_overlay(registration));
    let setup = move || {
        controller
            .editing
            .with(|own| own.as_ref().is_some_and(|own| !own.setup_complete))
    };
    let pending = move || controller.pending.with(Option::is_some);
    let busy = move || controller.busy.get();
    // A background read can block a submit, but must not interrupt editing or composition.
    let locked = move || controller.pending.with_untracked(Option::is_some);
    view! {
        <Dialog id="author-handle-dialog"
            open=controller.opened
            title=Signal::derive(move || if pending() {
                "Kiểm tra địa chỉ tác giả"
            } else if setup() {
                "Thiết lập địa chỉ"
            } else {
                "Đổi địa chỉ"
            }.to_string())
            dismiss=DismissPolicy::EscapeOnly
            on_dismiss=move |_| controller.close()
            actions=move || view! {
                <Button id="author-handle-cancel" variant=ButtonVariant::Text
                    on_press=Callback::new(move |()| controller.close())>
                    {move || if pending() { "Đóng" } else { "Hủy" }}
                </Button>
                <Show when=pending fallback=move || view! {
                    <Button id="author-handle-submit"
                        state=Signal::derive(move || button_state(
                            busy() || controller.unavailable.get() || controller.editing.with(|own|
                                own.as_ref().is_none_or(|own| own.setup_complete && own.remaining_changes == 0))))
                        on_press=Callback::new(move |()| controller.submit())>
                        {move || if setup() { "Dùng địa chỉ này" } else { "Xác nhận đổi" }}
                    </Button>
                }>
                    <Button id="author-handle-receipt"
                        state=Signal::derive(move || button_state(busy()))
                        on_press=Callback::new(move |()| controller.dispatch(true))>"Kiểm tra kết quả"</Button>
                    <Show when=move || controller.retryable.get()>
                        <Button id="author-handle-resend" variant=ButtonVariant::Tonal
                            state=Signal::derive(move || button_state(busy()))
                            on_press=Callback::new(move |()| controller.dispatch(false))>"Gửi lại yêu cầu"</Button>
                    </Show>
                </Show>
            }
        >
            <form autocomplete="off" novalidate class="grid min-w-0 gap-vk-3 wrap-anywhere"
                on:compositionstart=move |_| controller.composing.set(true)
                on:compositionend=move |_| controller.composing.set(false)
                on:submit=move |ev| {
                    ev.prevent_default();
                    controller.submit();
                }
            >
                <TextField id=FIELD label="Tên trong liên kết" value=controller.draft
                    autocomplete="off" help=RULE error=Signal::derive(move || controller.error.get())
                    mode=Signal::derive(move || if pending() { FieldMode::ReadOnly } else { FieldMode::Editable })
                    on_input=move |input: TextInput| {
                        controller.composing.set(input.composing);
                        if !locked() {
                            controller.draft.set(input.value);
                            controller.error.set(None);
                        }
                    } />
                {move || controller.editing.get().filter(|_| !pending()).map(|own| view! {
                    <p class="m-0 vk-muted">{if own.setup_complete {
                        format!("Hiện tại @{}. Còn {} lượt đổi. Các liên kết cũ vẫn dẫn đến bạn.", own.handle, own.remaining_changes)
                    } else {
                        "Giữ gợi ý hoặc chọn tên riêng. Thiết lập không trừ lượt đổi.".to_string()
                    }}</p>
                })}
                <p class="m-0" id="author-handle-proposed">{move || format!("@{}", controller.draft.get())}</p>
                <p id="author-handle-dialog-status" role="status" aria-atomic="true" class="m-0"
                    class:vk-ui-visually-hidden=move || controller.error.with(Option::is_some) || controller.message.get().is_empty()>
                    {move || controller.error.get().unwrap_or_else(|| controller.message.get().to_string())}
                </p>
                <Show when=move || controller.unavailable.get() && !pending()>
                    <Button variant=ButtonVariant::Text state=Signal::derive(move || button_state(busy()))
                        on_press=Callback::new(move |()| controller.refresh())>"Kiểm tra địa chỉ"</Button>
                </Show>
            </form>
        </Dialog>
    }
}
