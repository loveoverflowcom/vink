//! The publication region of Bàn viết (PUB-AUTHOR-WEB-01): the adapter between the editor, the
//! author's publication API and [`crate::publication_state`]. Layout:
//! [LayoutSpec](../../../../docs/design/publication-author.md).
//!
//! - It owns no save logic. "Lưu và gửi duyệt" calls the editor's own save and only *watches* the
//!   editor's [`DraftSession`]; the snapshot is taken for exactly what the server acknowledged.
//! - It owns no authority. What it says about the work comes from `GET …/publication/state`; the
//!   radio selection is the author's intent, never the work's state.
//! - Private data is read in the browser only (never in the server-rendered HTML), is bound to the
//!   tab's session scope and the route, and is never stored: not the manuscript, not a token.

use leptos::prelude::*;
use vink_ui::{
    ActionState, Announce, Button, ButtonVariant, ChoiceOption, Heading, HeadingLevel, IconName,
    InlineNotice, RadioGroup, Tone, TooltipIconButton, TypeRole,
};

use crate::contract::publication::{Audience, Decision};
use crate::desk_state::DraftSession;
use crate::publication_state::{
    Blocked, Conflict, Controller, Done, EditorFacts, Phase, ReadFailure, Refusal, Scope, Step,
    Summary, UncertainStage,
};

const PUBLIC: &str = "public";
const PRIVATE: &str = "private";

/// The one wording of a revision's decision, shared with the author's review page. A value this
/// build does not know is named as such, never read as pending or approved.
pub(crate) fn decision_label(decision: Decision) -> &'static str {
    match decision {
        Decision::Unsubmitted => "Đã chụp, chưa gửi duyệt",
        Decision::Pending => "Đang chờ kiểm duyệt",
        Decision::NeedsAdmin => "Đã chuyển cho người duyệt",
        Decision::Approved => "Đã được duyệt",
        Decision::Rejected => "Bị từ chối",
        Decision::Unknown => "Trạng thái chưa nhận diện được",
    }
}

/// What the page tells the author about the work, from server facts only.
fn summary_lines(summary: &Summary) -> Vec<(&'static str, String)> {
    let named = |title: &str| {
        if title.trim().is_empty() {
            "(chưa đặt tên)".to_string()
        } else {
            format!("«{title}»")
        }
    };
    let mut lines = Vec::new();
    lines.push((
        "publication-public",
        match (&summary.public, summary.audience) {
            (Some(public), _) => format!("Đang công khai: {}.", named(&public.title)),
            (None, Audience::Public) => "Chưa có bản nào đang công khai.".to_string(),
            (None, _) => "Chưa công khai. Chỉ bạn đọc được.".to_string(),
        },
    ));
    if let Some(requested) = &summary.requested {
        let mut text = format!(
            "Đang gửi: {} — {}.",
            named(&requested.title),
            decision_label(requested.decision)
        );
        if requested.decision == Decision::Approved && summary.public.is_none() {
            text.push_str(" Chưa là bản đang công khai.");
        }
        if let Some(reason) = requested.reason.as_deref().filter(|r| !r.trim().is_empty()) {
            text.push_str(&format!(" Lý do: {reason}"));
        }
        if summary.public.is_some() {
            text.push_str(" Người đọc vẫn đọc bản đang công khai.");
        }
        lines.push(("publication-requested", text));
    }
    if summary.unsent > 0 {
        lines.push((
            "publication-unsent",
            format!(
                "Có {} bản đã chụp nhưng chưa gửi duyệt. Gửi bản mới nếu bạn muốn công khai.",
                summary.unsent
            ),
        ));
    }
    lines
}

/// Why the primary action is disabled, in a short visible note. `None`: nothing to explain.
fn blocked_note(blocked: Blocked, phase: Phase) -> Option<&'static str> {
    match blocked {
        // While a lost answer waits to be checked, say why the action is greyed: the notice above
        // holds the one action that moves on.
        Blocked::Busy if matches!(phase, Phase::Uncertain(_)) => {
            Some("Kiểm tra lại lệnh trước rồi mới gửi tiếp.")
        }
        Blocked::NotRead => Some("Đang đọc trạng thái công khai."),
        Blocked::Unreadable => Some("Chưa hiểu trạng thái máy chủ trả về."),
        Blocked::NotActive => Some("Tác phẩm không còn hoạt động."),
        Blocked::TagsNotLoaded => Some("Thông tin tác phẩm chưa tải xong."),
        Blocked::TextConflict => Some("Bản nháp đang xung đột. Xử lý ở thanh lưu phía trên."),
        Blocked::TagsBlocked => Some("Thẻ chưa lưu được. Xử lý ở phần Thông tin tác phẩm."),
        Blocked::SaveFailing => Some("Bản nháp chưa lưu được. Thử lại ở thanh lưu phía trên."),
        Blocked::Busy | Blocked::NotPublic | Blocked::NotFresh => None,
    }
}

/// The sentence of the single polite status region: progress and confirmed outcomes only.
fn live_text(phase: Phase) -> &'static str {
    match phase {
        Phase::Reading => "Đang đọc trạng thái công khai…",
        Phase::Saving => "Đang lưu bản nháp để gửi duyệt…",
        Phase::Capturing => "Đang chụp bản đã lưu…",
        Phase::RequestingPublic => "Đang gửi yêu cầu công khai…",
        Phase::MakingPrivate => "Đang chuyển riêng tư…",
        Phase::Reconciled(Done::Requested) => {
            "Đã ghi nhận yêu cầu công khai. Trạng thái ở trên lấy từ máy chủ."
        }
        Phase::Reconciled(Done::MadePrivate) => {
            "Đã chuyển riêng tư. Trang công khai ngừng phục vụ nội dung này."
        }
        Phase::Ready
        | Phase::Unavailable(_)
        | Phase::Unreadable
        | Phase::Uncertain(_)
        | Phase::Conflict(_)
        | Phase::Failed(_) => "",
    }
}

fn phase_token(phase: Phase) -> &'static str {
    match phase {
        Phase::Reading => "reading",
        Phase::Unavailable(_) => "unavailable",
        Phase::Unreadable => "unreadable",
        Phase::Ready => "ready",
        Phase::Saving => "saving",
        Phase::Capturing => "capturing",
        Phase::RequestingPublic => "requesting_public",
        Phase::MakingPrivate => "making_private",
        Phase::Uncertain(_) => "uncertain",
        Phase::Conflict(_) => "conflict",
        Phase::Failed(_) => "failed",
        Phase::Reconciled(_) => "reconciled",
    }
}

/// A notice for a phase the author must act on: tone, title, message.
fn notice_text(phase: Phase, captured: bool) -> Option<(Tone, String, String)> {
    let warn =
        |title: &str, message: &str| Some((Tone::Warning, title.to_string(), message.to_string()));
    let error =
        |title: &str, message: &str| Some((Tone::Error, title.to_string(), message.to_string()));
    match phase {
        Phase::Uncertain(UncertainStage::Capturing) => warn(
            "Chưa xác nhận bản đã lưu được chụp để gửi.",
            "Máy chủ chưa phản hồi. Kiểm tra lại sẽ dùng đúng yêu cầu cũ, không tạo thêm bản chụp.",
        ),
        Phase::Uncertain(UncertainStage::Requesting) => warn(
            "Chưa xác nhận yêu cầu công khai.",
            "Bản đã chụp được giữ nguyên. Kiểm tra lại sẽ đọc trạng thái trước và chỉ gửi lại nếu chưa được ghi nhận.",
        ),
        Phase::Uncertain(UncertainStage::Making) => warn(
            "Chưa xác nhận việc chuyển riêng tư.",
            "Kiểm tra lại sẽ đọc trạng thái trước và chỉ gửi lại nếu chưa được ghi nhận. Chữ đang viết không bị ảnh hưởng.",
        ),
        Phase::Conflict(Conflict::Draft { current }) => Some((
            Tone::Warning,
            format!("Bản nháp trên máy chủ đã đổi ở nơi khác (phiên bản {current})."),
            "Chưa gửi gì. Lưu lại ở thanh phía trên để xử lý, rồi gửi lại.".to_string(),
        )),
        Phase::Conflict(Conflict::Metadata { current }) => Some((
            Tone::Warning,
            format!("Thông tin tác phẩm (thẻ) đã đổi ở nơi khác (phiên bản {current})."),
            "Chưa gửi gì. Xem lại phần Thông tin tác phẩm, lưu rồi gửi lại.".to_string(),
        )),
        Phase::Conflict(Conflict::Generation { .. }) => Some((
            Tone::Warning,
            "Trạng thái công khai đã đổi ở nơi khác.".to_string(),
            if captured {
                "Đã đọc lại trạng thái hiện tại ở trên. Chưa ghi đè gì; bản đã chụp vẫn giữ nguyên. Hãy kiểm tra rồi chọn lại."
            } else {
                "Đã đọc lại trạng thái hiện tại ở trên. Chưa ghi đè gì. Hãy kiểm tra rồi chọn lại."
            }
            .to_string(),
        )),
        Phase::Failed(Refusal::EditedWhileSaving) => warn(
            "Bạn đã gõ thêm khi đang lưu.",
            "Chưa gửi bản nào. Lưu rồi bấm gửi lại để gửi đúng bản bạn muốn.",
        ),
        Phase::Failed(Refusal::SaveNotConfirmed) => warn(
            "Chưa lưu được nên chưa gửi duyệt.",
            "Xử lý ở thanh lưu phía trên rồi gửi lại. Chữ vẫn còn trên trang.",
        ),
        Phase::Failed(Refusal::SessionEnded) | Phase::Unavailable(ReadFailure::SessionEnded) => error(
            "Phiên đăng nhập đã hết.",
            "Đăng nhập lại ở mục Tôi trong một thẻ khác rồi thử lại. Chữ vẫn còn trên trang.",
        ),
        Phase::Failed(Refusal::Forbidden) | Phase::Unavailable(ReadFailure::Forbidden) => error(
            "Yêu cầu bị từ chối.",
            "Tài khoản có thể đang bị hạn chế. Chưa có gì được ghi.",
        ),
        Phase::Failed(Refusal::NotFound) | Phase::Unavailable(ReadFailure::NotFound) => error(
            "Không tìm thấy bản nháp này.",
            "Bản nháp không tồn tại hoặc không thuộc về bạn.",
        ),
        Phase::Failed(Refusal::WorkUnavailable) => error(
            "Tác phẩm không còn hoạt động.",
            "Chưa có gì được ghi.",
        ),
        Phase::Failed(Refusal::RevisionRejected) => error(
            "Bản này đã bị từ chối.",
            "Sửa bản thảo, lưu rồi gửi bản mới. Lý do nằm ở trang kết quả kiểm duyệt.",
        ),
        Phase::Failed(Refusal::UnknownRevision) => error(
            "Không tìm thấy bản đã chụp trên tác phẩm này.",
            "Đã đọc lại trạng thái. Hãy gửi lại từ đầu.",
        ),
        Phase::Failed(Refusal::KeyReused) => error(
            "Mã lệnh không khớp với lần gửi trước.",
            "Máy chủ không ghi gì. Đã đọc lại trạng thái; hãy bắt đầu lại.",
        ),
        Phase::Failed(Refusal::Invalid) => error(
            "Yêu cầu chưa hợp lệ.",
            "Tải lại trang rồi thử lại.",
        ),
        Phase::Failed(Refusal::UnexpectedAnswer) => error(
            "Phản hồi không khớp với yêu cầu.",
            "Không gửi tiếp. Đã đọc lại trạng thái; hãy kiểm tra rồi thử lại.",
        ),
        Phase::Unreadable => error(
            "Chưa hiểu trạng thái máy chủ trả về.",
            "Không gửi gì. Hãy tải lại; nếu vẫn vậy, báo cho người vận hành.",
        ),
        Phase::Unavailable(ReadFailure::Unreachable) => error(
            "Chưa đọc được trạng thái công khai.",
            "Chưa kết nối được máy chủ. Chữ đang viết không bị ảnh hưởng; hãy tải lại.",
        ),
        Phase::Reading
        | Phase::Ready
        | Phase::Saving
        | Phase::Capturing
        | Phase::RequestingPublic
        | Phase::MakingPrivate
        | Phase::Reconciled(_) => None,
    }
}

/// The primary action for what the author chose against what the server confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Primary {
    Submit {
        /// Text or tags the editor has not saved yet: the save comes first.
        saves_first: bool,
        replaces: bool,
    },
    Withdraw,
    Nothing,
}

fn primary_for(choice: &str, audience: Option<Audience>, saves_first: bool) -> Primary {
    match (choice, audience) {
        (PUBLIC, Some(audience)) => Primary::Submit {
            saves_first,
            replaces: audience == Audience::Public,
        },
        (PRIVATE, Some(Audience::Public)) => Primary::Withdraw,
        _ => Primary::Nothing,
    }
}

fn primary_label(primary: Primary) -> &'static str {
    match primary {
        Primary::Submit {
            saves_first: true, ..
        } => "Lưu và gửi duyệt",
        Primary::Submit { replaces: true, .. } => "Gửi bản mới",
        Primary::Submit { .. } => "Gửi duyệt bản đã lưu",
        Primary::Withdraw => "Chuyển riêng tư",
        Primary::Nothing => "",
    }
}

fn pending_label(phase: Phase) -> &'static str {
    match phase {
        Phase::MakingPrivate => "Đang chuyển riêng tư…",
        Phase::Saving => "Đang lưu…",
        _ => "Đang gửi duyệt…",
    }
}

/// 16 random bytes for command keys; `None` when the browser offers none.
fn entropy() -> Option<[u8; 16]> {
    #[cfg(feature = "hydrate")]
    {
        let mut bytes = [0_u8; 16];
        web_sys::window()?
            .crypto()
            .ok()?
            .get_random_values_with_u8_array(&mut bytes)
            .ok()?;
        Some(bytes)
    }
    #[cfg(not(feature = "hydrate"))]
    {
        Some([0; 16])
    }
}

/// Performs a [`Step`]: the only place this page touches the transport. Every answer goes back
/// through the controller with its ticket, so an answer for anything but the live attempt of the
/// live scope changes nothing.
#[derive(Clone, Copy)]
struct Runner {
    controller: RwSignal<Controller>,
    work_id: StoredValue<String>,
    on_save: Callback<()>,
}

impl Runner {
    fn run(self, step: Step) {
        match step {
            Step::None => {}
            Step::Save => self.on_save.run(()),
            Step::Read(ticket) => self.read(ticket),
            Step::WithdrawalCommitted(ticket) => {
                // Shared copies of this work are stale the moment the server confirmed it: cached
                // publication, and its card in cached lists. A late older read cannot put them back.
                crate::public_reads::invalidate_withdrawn(&self.work_id.get_value());
                if let Some(ticket) = ticket {
                    self.read(ticket);
                }
            }
            #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
            Step::Capture(ticket, command) => {
                #[cfg(feature = "hydrate")]
                {
                    use crate::contract::publication::CaptureSnapshotRequest;
                    let id = self.work_id.get_value();
                    leptos::task::spawn_local(async move {
                        let request = CaptureSnapshotRequest {
                            command_key: command.command_key,
                            expected_draft_version: command.expected_draft_version,
                            expected_metadata_version: command.expected_metadata_version,
                        };
                        let result = crate::api::capture_snapshot(&id, &request).await;
                        self.finish(|c| c.capture_finished(ticket, result));
                    });
                }
            }
            #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
            Step::RequestPublic(ticket, command) => {
                #[cfg(feature = "hydrate")]
                {
                    use crate::contract::publication::RequestPublicRequest;
                    let id = self.work_id.get_value();
                    leptos::task::spawn_local(async move {
                        let request = RequestPublicRequest {
                            command_key: command.command_key,
                            revision_id: command.revision_id,
                            expected_generation: command.expected_generation,
                        };
                        let result = crate::api::request_public(&id, &request).await;
                        self.finish(|c| c.request_finished(ticket, result));
                    });
                }
            }
            #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
            Step::SetPrivate(ticket, command) => {
                #[cfg(feature = "hydrate")]
                {
                    use crate::contract::publication::SetPrivateRequest;
                    let id = self.work_id.get_value();
                    leptos::task::spawn_local(async move {
                        let request = SetPrivateRequest {
                            command_key: command.command_key,
                            expected_generation: command.expected_generation,
                        };
                        let result = crate::api::set_private(&id, &request).await;
                        self.finish(|c| c.private_finished(ticket, result));
                    });
                }
            }
        }
    }

    #[cfg(feature = "hydrate")]
    fn finish(self, apply: impl FnOnce(&mut Controller) -> Step) {
        // `None`: the editor was closed (the owner disposed the signal); nothing more to do.
        if let Some(step) = self.controller.try_update(apply) {
            self.run(step);
        }
    }

    fn read(self, ticket: crate::publication_state::ReadTicket) {
        #[cfg(feature = "hydrate")]
        {
            let id = self.work_id.get_value();
            leptos::task::spawn_local(async move {
                let result = crate::api::load_publication_state(&id).await;
                self.finish(|c| c.read_finished(ticket, result));
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = ticket;
    }

    /// A read of the state unless one is running; the answer to a running one asks again.
    fn refresh(self) {
        if let Some(ticket) = self.controller.try_update(Controller::begin_read).flatten() {
            self.read(ticket);
        }
    }
}

/// Moves focus to `id` after the next frame (the node may be re-rendered first).
fn focus_after_frame(id: &'static str) {
    super::focus_recovery(id);
}

#[component]
pub fn PublicationPanel(
    work_id: String,
    session: RwSignal<DraftSession>,
    #[prop(into)] on_save: Callback<()>,
    /// Whether an input method is composing in the body. A press then waits for the commit, like
    /// the save shortcut: half a syllable is never the text that gets frozen and sent.
    #[prop(into)]
    is_composing: Callback<(), bool>,
) -> impl IntoView {
    let tab = crate::account::Session::current();
    let keys = entropy();
    let work = StoredValue::new(work_id.clone());
    let controller = RwSignal::new(Controller::new(
        Scope {
            work_id,
            session: tab.scope.get_untracked(),
        },
        keys.unwrap_or([0; 16]),
    ));
    let runner = Runner {
        controller,
        work_id: work,
        on_save,
    };
    let choice = RwSignal::new(String::new());

    // What the editor owner says, read only. The memo notifies on each edit (the counters move);
    // the labels below depend on the coarser `unsaved` memo.
    let editor = Memo::new(move |_| session.with(EditorFacts::of));
    let unsaved = Memo::new(move |_| editor.with(EditorFacts::has_unsaved));

    // A submission waiting for the editor moves on only through this effect. The controller is
    // touched only while one waits, so typing does not re-render the region.
    Effect::new(move |_| {
        let facts = editor.get();
        if !controller.with_untracked(Controller::waiting_for_editor) {
            return;
        }
        if let Some(step) = controller.try_update(|c| c.editor_changed(&facts)) {
            runner.run(step);
        }
    });

    // The first read, and a new one for every other session of the tab: nothing private carries
    // over (`change_scope`).
    Effect::new(move |previous: Option<Option<String>>| {
        let scope = tab.scope.get();
        match previous {
            None => runner.refresh(),
            Some(before) if before != scope => {
                controller.update(|c| {
                    let work_id = c.scope().work_id.clone();
                    c.change_scope(Scope {
                        work_id,
                        session: scope.clone(),
                    });
                });
                choice.set(String::new());
                runner.refresh();
            }
            Some(_) => {}
        }
        scope
    });

    // Returning to the tab or window: read again, at most every two seconds, never mid-command.
    #[cfg(feature = "hydrate")]
    {
        let last = StoredValue::new(0.0_f64);
        let on_return = move || {
            let now = js_sys::Date::now();
            let idle = controller
                .try_with_untracked(|c| !c.working())
                .unwrap_or(false);
            if idle && !document().hidden() && now - last.get_value() > 2000.0 {
                last.set_value(now);
                runner.refresh();
            }
        };
        let focus = window_event_listener(leptos::ev::focus, move |_| on_return());
        let visible = window_event_listener(leptos::ev::visibilitychange, move |_| on_return());
        on_cleanup(move || {
            focus.remove();
            visible.remove();
        });
    }

    let phase = Memo::new(move |_| controller.with(Controller::phase));
    let facts_audience = Memo::new(move |_| controller.with(|c| c.facts().map(|f| f.audience)));
    let summary = Memo::new(move |_| controller.with(|c| c.facts().map(Summary::of)));
    let pointer = Memo::new(move |_| {
        controller.with(|c| {
            c.facts()
                .map(|f| f.pointer.as_ref().map(|p| p.revision_id.clone()))
        })
    });

    // The radio follows the server when the server's audience changes (another tab, a command),
    // and starts at it; the author's own pending selection is never reset by a plain refresh.
    Effect::new(move |previous: Option<Option<Audience>>| {
        let now = facts_audience.get();
        if previous.is_none_or(|before| before != now) {
            match now {
                Some(Audience::Public) => choice.set(PUBLIC.into()),
                Some(Audience::Private) => choice.set(PRIVATE.into()),
                _ => {}
            }
        }
        now
    });

    // A public revision appearing or changing (an approval seen on a read) makes cached lists and
    // the cached publication of this work stale in this tab.
    Effect::new(move |previous: Option<Option<Option<String>>>| {
        let now = pointer.get();
        if let (Some(Some(before)), Some(after)) = (previous.as_ref(), now.as_ref())
            && before != after
        {
            crate::public_reads::invalidate_publication(&work.get_value());
        }
        now
    });

    let primary =
        Memo::new(move |_| primary_for(&choice.get(), facts_audience.get(), unsaved.get()));
    let gate = Memo::new(move |_| match primary.get() {
        Primary::Submit { .. } => {
            let facts = editor.get();
            controller.with(|c| c.can_submit(&facts))
        }
        Primary::Withdraw => controller.with(Controller::can_withdraw),
        Primary::Nothing => Ok(()),
    });
    let working = Memo::new(move |_| controller.with(Controller::working));
    let primary_state = Signal::derive(move || {
        if working.get() {
            return ActionState::pending(pending_label(phase.get()));
        }
        match gate.get() {
            Ok(()) if keys.is_some() => ActionState::Enabled,
            Ok(()) => ActionState::disabled("Trình duyệt chưa tạo được mã lệnh an toàn."),
            Err(blocked) => match blocked_note(blocked, phase.get()) {
                Some(note) => ActionState::disabled(note),
                None => ActionState::Disabled { reason: None },
            },
        }
    });
    let press_primary = Callback::new(move |()| {
        if is_composing.run(()) {
            return;
        }
        let step = match primary.get_untracked() {
            Primary::Submit { .. } => {
                let facts = editor.get_untracked();
                controller.try_update(|c| c.start_submit(&facts))
            }
            Primary::Withdraw => controller.try_update(Controller::start_private),
            Primary::Nothing => return,
        };
        // A press the controller refuses (double press, a blocked editor) starts nothing.
        if let Some(Ok(step)) = step {
            runner.run(step);
        }
    });
    let retry = Callback::new(move |()| {
        if let Some(step) = controller.try_update(Controller::retry) {
            runner.run(step);
        }
    });
    let dismiss = Callback::new(move |()| {
        controller.update(Controller::dismiss);
        focus_after_frame("publication-primary");
    });
    let resume = Callback::new(move |()| {
        if let Some(Ok(step)) = controller.try_update(Controller::resume_captured) {
            runner.run(step);
        }
    });
    let reload = Callback::new(move |()| runner.refresh());

    let notice = move || {
        let phase = phase.get();
        let captured = controller.with(|c| c.captured().is_some());
        let (tone, title, message) = notice_text(phase, captured)?;
        let uncertain = matches!(phase, Phase::Uncertain(_));
        let conflict = matches!(phase, Phase::Conflict(_));
        let reads = matches!(phase, Phase::Unreadable | Phase::Unavailable(_));
        let checking = controller.with(Controller::checking);
        let can_resume = controller.with(Controller::can_resume_captured);
        Some(view! {
            <section class="vk-editor-recovery" id="publication-recovery" data-testid="publication-recovery"
                data-tone=if tone == Tone::Error { "error" } else { "warning" } aria-label="Phục hồi công khai">
                <InlineNotice tone announce=Announce::Polite title message />
                <div class="vk-notice__actions">
                    {uncertain.then(|| view! {
                        <Button variant=ButtonVariant::Text id="publication-retry" leading=IconName::Refresh
                            state=Signal::derive(move || if checking { ActionState::pending("Đang kiểm tra…") } else { ActionState::Enabled })
                            on_press=move |_| retry.run(())>"Kiểm tra lại"</Button>
                    })}
                    {(conflict && can_resume).then(|| view! {
                        <Button variant=ButtonVariant::Text id="publication-resume"
                            on_press=move |_| resume.run(())>"Gửi lại bản đã chụp"</Button>
                    })}
                    {reads.then(|| view! {
                        <Button variant=ButtonVariant::Text id="publication-reload" leading=IconName::Refresh
                            on_press=move |_| reload.run(())>"Tải lại"</Button>
                    })}
                    {(!uncertain && !reads).then(|| view! {
                        <Button variant=ButtonVariant::Text id="publication-dismiss"
                            on_press=move |_| dismiss.run(())>"Đã hiểu"</Button>
                    })}
                </div>
            </section>
        })
    };

    // The last read failed but older facts are still shown: say so without hiding them.
    let stale_note = move || {
        let kept = matches!(phase.get(), Phase::Ready | Phase::Unreadable)
            && controller.with(|c| c.read_failure() == Some(ReadFailure::Unreachable));
        kept.then(|| {
            view! {
                <p class="vk-publication__muted" data-testid="publication-stale">
                    "Chưa làm mới được trạng thái; đang giữ lần đọc gần nhất."
                </p>
            }
        })
    };

    let review_label = move || {
        let appeal = summary.get().is_some_and(|s| {
            s.requested
                .as_ref()
                .or(s.public.as_ref())
                .is_some_and(|r| r.can_appeal)
        });
        if appeal {
            "Xem lý do và gửi phản đối"
        } else {
            "Xem kết quả kiểm duyệt và phản đối"
        }
    };

    view! {
        <section id="publication" class="vk-publication" aria-labelledby="publication-title"
            data-testid="publication" data-phase=move || phase_token(phase.get())>
            <div class="vk-publication__heading">
                <Heading level=HeadingLevel::H2 role=TypeRole::Title id="publication-title">"Công khai"</Heading>
                <TooltipIconButton icon=IconName::Refresh label="Tải lại trạng thái công khai" id="publication-refresh"
                    state=Signal::derive(move || if controller.with(Controller::reading) { ActionState::Disabled { reason: None } } else { ActionState::Enabled })
                    on_press=move |_| reload.run(()) />
            </div>
            <p class="vk-publication__live" id="publication-live" role="status" aria-atomic="true" data-testid="publication-live">
                {move || live_text(phase.get())}
            </p>
            <div class="vk-publication__summary" id="publication-summary" data-testid="publication-summary">
                {move || summary.get().map(|summary| {
                    summary_lines(&summary)
                        .into_iter()
                        .map(|(id, text)| view! { <p id=id data-testid=id>{text}</p> })
                        .collect_view()
                })}
                <Show when=move || unsaved.get() && summary.with(Option::is_some)>
                    <p class="vk-publication__muted" id="publication-draft" data-testid="publication-draft">
                        "Chữ hoặc thẻ chưa lưu sẽ không nằm trong bản được gửi."
                    </p>
                </Show>
                <Show when=move || summary.with(|s| s.as_ref().is_some_and(|s| s.public.is_some())) && !unsaved.get()>
                    <p class="vk-publication__muted" data-testid="publication-draft-note">
                        "Sửa bản nháp không đổi bản đang công khai cho tới khi bạn gửi bản mới và được duyệt."
                    </p>
                </Show>
                {stale_note}
            </div>
            {notice}
            <div class="vk-publication__choice" id="audience-choice">
                <RadioGroup id="audience" legend="Ai đọc được tác phẩm"
                    options=vec![ChoiceOption::new(PRIVATE, "Chỉ mình tôi"), ChoiceOption::new(PUBLIC, "Công khai")]
                    value=Signal::derive(move || {
                        let value = choice.get();
                        (!value.is_empty()).then_some(value)
                    })
                    disabled=Signal::derive(move || working.get() || facts_audience.get().is_none())
                    on_change=move |value: String| choice.set(value) />
                <p class="vk-publication__muted" id="audience-applied" data-testid="audience-applied">
                    {move || match facts_audience.get() {
                        Some(Audience::Public) => "Máy chủ đang áp dụng: Công khai.",
                        Some(Audience::Private) => "Máy chủ đang áp dụng: Chỉ mình tôi.",
                        _ => "Chưa đọc được lựa chọn đã áp dụng.",
                    }}
                </p>
            </div>
            <div class="vk-publication__actions" id="publication-actions">
                <Show when=move || primary.get() != Primary::Nothing
                    fallback=move || view! {
                        <p class="vk-publication__muted" data-testid="publication-nothing">
                            {move || if facts_audience.get() == Some(Audience::Private) {
                                "Chỉ bạn đọc được tác phẩm này. Chọn Công khai khi muốn gửi duyệt."
                            } else {
                                ""
                            }}
                        </p>
                    }>
                    <Button variant=ButtonVariant::Filled id="publication-primary" state=primary_state
                        on_press=move |_| press_primary.run(())>
                        {move || primary_label(primary.get())}
                    </Button>
                </Show>
            </div>
            <Show when=move || matches!(primary.get(), Primary::Withdraw)>
                <p class="vk-publication__muted" data-testid="publication-private-note">
                    "Chuyển riêng tư không xóa bài và không xóa chữ đang viết. Trang công khai ngừng phục vụ nội dung; bản sao ngoài Vink hoặc ở thẻ chưa tải lại có thể còn."
                </p>
            </Show>
            <Show when=move || matches!(primary.get(), Primary::Submit { .. })>
                <p class="vk-publication__muted" data-testid="publication-submit-note">
                    "Gửi duyệt chụp đúng bản đã lưu. Phần gõ thêm sau đó không nằm trong bản đã gửi."
                </p>
            </Show>
            <vink_ui::Link variant=vink_ui::LinkVariant::Standalone attr:id="editor-review-status" href=format!("/ban-viet/{}/kiem-duyet", work.get_value())>
                {review_label}
            </vink_ui::Link>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::publication::{
        AuthorPublicationState, AuthorRevision, Lifecycle, PublicPointer, PublishIntent,
    };

    fn revision(
        id: &str,
        title: &str,
        moderation: Decision,
        reason: Option<&str>,
    ) -> AuthorRevision {
        AuthorRevision {
            can_appeal: false,
            decision_reason: reason.map(str::to_string),
            revision_id: id.into(),
            title: title.into(),
            content_hash: "h".into(),
            moderation,
            decision_version: Some(1),
            metadata_snapshot_id: None,
            created_at: "2026-11-13T08:00:00Z".into(),
        }
    }

    fn state(audience: Audience) -> AuthorPublicationState {
        AuthorPublicationState {
            work_id: "w".into(),
            audience,
            lifecycle: Lifecycle::Active,
            generation: 3,
            intent: None,
            pointer: None,
            revisions: vec![],
            open_ticket: None,
        }
    }

    fn text(summary: &Summary) -> String {
        summary_lines(summary)
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join(" | ")
    }

    #[test]
    fn a_private_work_is_not_public_and_an_empty_audience_is_not_a_verdict() {
        let summary = Summary::of(&state(Audience::Private));
        assert_eq!(text(&summary), "Chưa công khai. Chỉ bạn đọc được.");
        // Public audience with no public revision: waiting, not "Đang công khai".
        let waiting = Summary::of(&state(Audience::Public));
        assert_eq!(text(&waiting), "Chưa có bản nào đang công khai.");
    }

    #[test]
    fn r1_stays_public_while_r2_waits_and_the_copy_says_readers_still_read_r1() {
        let mut s = state(Audience::Public);
        s.pointer = Some(PublicPointer {
            revision_id: "r1".into(),
            intent_generation: 2,
        });
        s.intent = Some(PublishIntent {
            revision_id: "r2".into(),
            generation: 3,
        });
        s.revisions = vec![
            revision("r2", "Bản hai", Decision::Pending, None),
            revision("r1", "Bản một", Decision::Approved, None),
        ];
        let said = text(&Summary::of(&s));
        assert!(said.contains("Đang công khai: «Bản một»."), "{said}");
        assert!(
            said.contains("Đang gửi: «Bản hai» — Đang chờ kiểm duyệt."),
            "{said}"
        );
        assert!(
            said.contains("Người đọc vẫn đọc bản đang công khai."),
            "{said}"
        );
        s.revisions[0] = revision("r2", "Bản hai", Decision::Rejected, Some("Chưa phù hợp."));
        let said = text(&Summary::of(&s));
        assert!(said.contains("Bị từ chối. Lý do: Chưa phù hợp."), "{said}");
        assert!(said.contains("Đang công khai: «Bản một»."), "{said}");
    }

    #[test]
    fn approved_without_a_pointer_is_not_called_public() {
        let mut s = state(Audience::Public);
        s.intent = Some(PublishIntent {
            revision_id: "r1".into(),
            generation: 3,
        });
        s.revisions = vec![revision("r1", "Bản một", Decision::Approved, None)];
        let said = text(&Summary::of(&s));
        assert!(
            said.contains("Đã được duyệt. Chưa là bản đang công khai."),
            "{said}"
        );
        assert!(!said.contains("Đang công khai"), "{said}");
    }

    #[test]
    fn every_decision_has_its_own_words_and_none_borrows_another() {
        let labels: Vec<_> = [
            Decision::Unsubmitted,
            Decision::Pending,
            Decision::NeedsAdmin,
            Decision::Approved,
            Decision::Rejected,
            Decision::Unknown,
        ]
        .into_iter()
        .map(decision_label)
        .collect();
        let mut unique = labels.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), labels.len(), "{labels:?}");
        // needs_admin is neither "AI is reviewing" nor a rejection.
        assert!(!decision_label(Decision::NeedsAdmin).contains("AI"));
        assert!(!decision_label(Decision::NeedsAdmin).contains("từ chối"));
    }

    #[test]
    fn the_primary_action_follows_the_choice_against_what_the_server_confirmed() {
        use Audience::{Private, Public};
        assert_eq!(
            primary_for(PUBLIC, Some(Private), false),
            Primary::Submit {
                saves_first: false,
                replaces: false
            }
        );
        assert_eq!(
            primary_label(primary_for(PUBLIC, Some(Private), false)),
            "Gửi duyệt bản đã lưu"
        );
        assert_eq!(
            primary_label(primary_for(PUBLIC, Some(Private), true)),
            "Lưu và gửi duyệt"
        );
        assert_eq!(
            primary_label(primary_for(PUBLIC, Some(Public), false)),
            "Gửi bản mới"
        );
        assert_eq!(primary_for(PRIVATE, Some(Public), true), Primary::Withdraw);
        assert_eq!(primary_label(Primary::Withdraw), "Chuyển riêng tư");
        // Choosing what the server already applies, or nothing yet, offers no action.
        assert_eq!(primary_for(PRIVATE, Some(Private), false), Primary::Nothing);
        assert_eq!(primary_for(PUBLIC, None, false), Primary::Nothing);
        assert_eq!(primary_for("", Some(Public), false), Primary::Nothing);
    }

    #[test]
    fn every_phase_the_author_must_act_on_has_a_notice_and_the_others_have_none() {
        for phase in [
            Phase::Uncertain(UncertainStage::Capturing),
            Phase::Uncertain(UncertainStage::Requesting),
            Phase::Uncertain(UncertainStage::Making),
            Phase::Conflict(Conflict::Draft { current: 8 }),
            Phase::Conflict(Conflict::Metadata { current: 6 }),
            Phase::Conflict(Conflict::Generation { current: 5 }),
            Phase::Failed(Refusal::EditedWhileSaving),
            Phase::Failed(Refusal::SaveNotConfirmed),
            Phase::Failed(Refusal::SessionEnded),
            Phase::Failed(Refusal::Forbidden),
            Phase::Failed(Refusal::NotFound),
            Phase::Failed(Refusal::WorkUnavailable),
            Phase::Failed(Refusal::RevisionRejected),
            Phase::Failed(Refusal::UnknownRevision),
            Phase::Failed(Refusal::KeyReused),
            Phase::Failed(Refusal::Invalid),
            Phase::Failed(Refusal::UnexpectedAnswer),
            Phase::Unreadable,
            Phase::Unavailable(ReadFailure::SessionEnded),
            Phase::Unavailable(ReadFailure::Forbidden),
            Phase::Unavailable(ReadFailure::NotFound),
            Phase::Unavailable(ReadFailure::Unreachable),
        ] {
            let (_, title, message) =
                notice_text(phase, false).unwrap_or_else(|| panic!("{phase:?}"));
            assert!(!title.is_empty() && !message.is_empty(), "{phase:?}");
        }
        for phase in [
            Phase::Reading,
            Phase::Ready,
            Phase::Saving,
            Phase::Capturing,
            Phase::RequestingPublic,
            Phase::MakingPrivate,
            Phase::Reconciled(Done::Requested),
            Phase::Reconciled(Done::MadePrivate),
        ] {
            assert!(notice_text(phase, false).is_none(), "{phase:?}");
        }
    }

    /// A lost answer never reads as success, and a generation conflict never reads as a failure of
    /// the author's text: the copy names what was and was not written.
    #[test]
    fn uncertain_and_conflict_copy_never_claims_an_outcome() {
        for stage in [
            UncertainStage::Capturing,
            UncertainStage::Requesting,
            UncertainStage::Making,
        ] {
            let (_, title, _) = notice_text(Phase::Uncertain(stage), false).unwrap();
            assert!(title.starts_with("Chưa xác nhận"), "{title}");
            assert!(!title.contains("Đã "), "{title}");
        }
        let (_, _, with) =
            notice_text(Phase::Conflict(Conflict::Generation { current: 5 }), true).unwrap();
        let (_, _, without) =
            notice_text(Phase::Conflict(Conflict::Generation { current: 5 }), false).unwrap();
        assert!(with.contains("bản đã chụp vẫn giữ nguyên") && !without.contains("đã chụp"));
        let (_, title, message) =
            notice_text(Phase::Conflict(Conflict::Draft { current: 8 }), false).unwrap();
        assert!(
            title.contains("phiên bản 8") && message.starts_with("Chưa gửi gì."),
            "{title} {message}"
        );
    }

    #[test]
    fn the_live_region_speaks_only_of_progress_and_confirmed_outcomes() {
        assert!(live_text(Phase::Ready).is_empty());
        assert!(live_text(Phase::Uncertain(UncertainStage::Requesting)).is_empty());
        assert!(live_text(Phase::Failed(Refusal::Forbidden)).is_empty());
        assert!(
            live_text(Phase::Reconciled(Done::Requested)).contains("Đã ghi nhận yêu cầu công khai")
        );
        assert!(!live_text(Phase::RequestingPublic).contains("Đã "));
    }

    #[test]
    fn a_blocked_press_names_its_reason_unless_nothing_needs_explaining() {
        for blocked in [
            Blocked::NotRead,
            Blocked::Unreadable,
            Blocked::NotActive,
            Blocked::TagsNotLoaded,
            Blocked::TextConflict,
            Blocked::TagsBlocked,
            Blocked::SaveFailing,
        ] {
            assert!(blocked_note(blocked, Phase::Ready).is_some(), "{blocked:?}");
        }
        for blocked in [Blocked::Busy, Blocked::NotPublic, Blocked::NotFresh] {
            assert!(
                blocked_note(blocked, Phase::RequestingPublic).is_none(),
                "{blocked:?}"
            );
        }
        // A greyed action next to a lost answer says why.
        assert!(
            blocked_note(Blocked::Busy, Phase::Uncertain(UncertainStage::Requesting)).is_some()
        );
    }
}
