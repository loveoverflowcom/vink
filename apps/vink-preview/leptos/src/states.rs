//! UIS-03 specimens: state, feedback and loading components through the public `vink_ui` API.
//! Problems are projected the way an app adapter would; nothing is fetched. The playground runs
//! on a manual clock (advanced only by its buttons) and a ticket reducer, so every recipe is
//! reproducible and every intent lands in the event log.

use leptos::prelude::*;
use vink_ui::catalog::fixture;
use vink_ui::{
    ActionPhase, ActionState, Align, Announce, Banner, Button, ButtonVariant, Clock, Content,
    ContentRegion, Dialog, DialogKind, DialogRole, Direction, DismissPolicy, DismissReason,
    EdgeProgress, EmptyKind, FormSummary, IndicatorSize, InlineNotice, ListEnd, LoadingIndicator,
    MotionActivityScope, PanelContent, Phase, Problem, ProblemKind, Progress, RecoveryAction,
    RecoveryIntent, RegionState, SkeletonShape, Snackbar, SnackbarMessage, Space, Stack,
    StatePanel, StatusBadge, Text, TextArea, TextField, TextInput, TextTone, Tone, TypeRole,
    use_loading_gate, use_snackbar_timeout,
};

use crate::components::log;

/// [Mẫu] works shown as loaded content.
const WORKS: [&str; 3] = [
    "Mùa hoa sữa cuối cùng [Mẫu]",
    "Những con phố cũ của Hà Nội [Mẫu]",
    "Gửi người ở lại [Mẫu]",
];

fn action(intent: RecoveryIntent, label: &str) -> RecoveryAction {
    RecoveryAction {
        intent,
        label: label.into(),
    }
}

fn problem(kind: ProblemKind, actions: &[(RecoveryIntent, &str)]) -> Problem {
    actions
        .iter()
        .fold(Problem::fallback(kind), |p, &(intent, label)| {
            p.with_action(intent, label).expect("lawful preview action")
        })
}

fn note(text: &'static str) -> impl IntoView {
    view! { <p class="vkp-note">{text}</p> }
}

fn works(count: usize) -> impl IntoView {
    view! {
        <ul class="vkp-works">
            {WORKS
                .iter()
                .take(count)
                .map(|title| view! { <li><a class="vk-ui-link" href="#state.loading/refreshing">{*title}</a></li> })
                .collect_view()}
        </ul>
    }
}

fn recovery_log(entry: &'static str) -> Callback<RecoveryIntent> {
    Callback::new(move |intent: RecoveryIntent| log(format!("{entry} → intent {}", intent.id())))
}

/// The specimen body for a UIS-03 catalog entry, or `None` for an unknown ID.
pub fn specimen(id: &str, scenario: &str) -> Option<AnyView> {
    Some(match id {
        "state.loading" => loading(scenario),
        "state.progress" => progress(scenario),
        "state.empty" => empty(scenario),
        "state.error" => error(scenario),
        "state.notice" => notice(scenario),
        "state.status" => status(scenario == "long-label"),
        "state.dialog" => dialog(scenario),
        "state.playground" => view! { <Playground recipe=scenario.to_string() /> }.into_any(),
        _ => return None,
    })
}

fn loading(scenario: &str) -> AnyView {
    match scenario {
        "refreshing" => view! {
            {note("Làm mới giữ nội dung: danh sách vẫn mount, liên kết vẫn focus được; EdgeProgress nằm ở khoảng dành sẵn.")}
            <ContentRegion state=RegionState::new(Content::PRESENT, Phase::LOADING) label="Tác phẩm [Mẫu]" on_recover=recovery_log("state.loading")>
                {works(3)}
            </ContentRegion>
        }
        .into_any(),
        "slow" => view! {
            {note("Sau duration.slow-notice: gợi ý “vẫn đang tải”, không phải lỗi hay timeout.")}
            <ContentRegion state=RegionState::new(Content::Absent, Phase::Loading { slow: true }) label="Tác phẩm [Mẫu]">
                {works(3)}
            </ContentRegion>
        }
        .into_any(),
        _ => view! {
            {note("Skeleton cùng hình học với thẻ thật (tiêu đề → byline → đoạn trích); trang trí ẩn với AT, vùng có aria-busy và một nhãn.")}
            <Stack gap=Space::S6>
                <ContentRegion state=RegionState::new(Content::Absent, Phase::LOADING) label="Tác phẩm [Mẫu]">
                    {works(3)}
                </ContentRegion>
                <ContentRegion
                    state=RegionState::new(Content::Absent, Phase::LOADING)
                    label="Tác giả [Mẫu]"
                    skeleton=SkeletonShape::Byline
                >
                    {works(1)}
                </ContentRegion>
                <ContentRegion
                    state=RegionState::new(Content::Absent, Phase::LOADING)
                    label="Đoạn văn [Mẫu]"
                    skeleton=SkeletonShape::Text { lines: 4 }
                >
                    {works(1)}
                </ContentRegion>
            </Stack>
        }
        .into_any(),
    }
}

fn indicators() -> impl IntoView {
    view! {
        <Stack gap=Space::S6 align=Align::Start>
            <span data-testid="arc-labelled">
                <Stack direction=Direction::Row gap=Space::S3 align=Align::Center>
                    <LoadingIndicator label="Đang tải nhận xét" size=IndicatorSize::Medium />
                    <Text>"Đang tải nhận xét…"</Text>
                </Stack>
            </span>
            <div class="vkp-edge" data-testid="edge">
                <EdgeProgress label="Đang làm mới danh sách" />
            </div>
            <Stack direction=Direction::Row gap=Space::S4 wrap=true align=Align::Start>
                <span data-testid="button-enabled">
                    <Button on_press=move |_| log("state.progress → Lưu")>"Lưu bản nháp"</Button>
                </span>
                <span data-testid="button-pending">
                    <Button id="pending-progress" state=ActionState::pending("Đang lưu… (chờ xác nhận)")>"Lưu bản nháp"</Button>
                </span>
                <span data-testid="button-pending-text">
                    <Button variant=ButtonVariant::Text state=ActionState::pending("Đang tải…")>"Tải thêm"</Button>
                </span>
            </Stack>
        </Stack>
    }
}

fn progress(scenario: &str) -> AnyView {
    match scenario {
        "determinate" => view! {
            {note("Determinate chỉ khi caller có tiến độ thật (byte đã gửi / tổng); không tăng giả tới 99%.")}
            <div class="vkp-edge" data-testid="edge">
                <EdgeProgress label="Đang tải ảnh bìa lên: 42%" progress=Progress::Determinate(0.42) />
            </div>
        }
        .into_any(),
        "inactive" => view! {
            {note("Host báo không hoạt động (tab ẩn, app nền, ngoài màn hình): không vòng lặp nào chạy; chỉ báo tĩnh + nhãn.")}
            <MotionActivityScope active=false>{indicators()}</MotionActivityScope>
        }
        .into_any(),
        "reduced" => view! {
            {note("Giảm chuyển động: cung và thanh đứng yên, nhãn hiện thành chữ; không chạy vòng lặp 0ms.")}
            {indicators()}
        }
        .into_any(),
        _ => view! {
            {note("MicroArc trong slot status giữ nhãn và bề rộng nút; EdgeProgress không giả phần trăm.")}
            {indicators()}
        }
        .into_any(),
    }
}

fn empty(scenario: &str) -> AnyView {
    match scenario {
        "no-results" => {
            let panel = PanelContent::empty(
                EmptyKind::NoResults,
                "Không có tác phẩm khớp “hoa sữa” trong Thơ",
                "Thử từ khóa khác hoặc xóa bộ lọc thể loại.",
                vec![action(RecoveryIntent::ClearFilter, "Xóa bộ lọc")],
            )
            .expect("lawful");
            view! {
                <ContentRegion
                    state=RegionState::new(Content::Empty(EmptyKind::NoResults), Phase::Idle)
                    label="Kết quả tìm [Mẫu]"
                    empty=Some(panel)
                    on_recover=recovery_log("state.empty")
                >
                    {works(0)}
                </ContentRegion>
            }
            .into_any()
        }
        "low-content" => view! {
            {note("Ít nội dung: hiện đúng một tác phẩm thật, không nhân bản spotlight/feed để lấp chỗ.")}
            <ContentRegion state=RegionState::new(Content::Present { low: true, partial: false }, Phase::Idle) label="Tác phẩm mới [Mẫu]">
                {works(1)}
            </ContentRegion>
        }
        .into_any(),
        "end-of-list" => view! {
            <ContentRegion state=RegionState::new(Content::PRESENT, Phase::Idle) label="Tác phẩm [Mẫu]">
                {works(3)}
                <ListEnd />
            </ContentRegion>
        }
        .into_any(),
        _ => {
            let panel = PanelContent::empty(
                EmptyKind::FirstUse,
                "Bàn viết còn trống",
                "Bản nháp chỉ mình bạn thấy cho tới khi bạn gửi.",
                vec![action(RecoveryIntent::Start, "Viết bản nháp đầu tiên")],
            )
            .expect("lawful");
            view! {
                <StatePanel content=panel on_recover=recovery_log("state.empty") />
            }
            .into_any()
        }
    }
}

fn error(scenario: &str) -> AnyView {
    use ProblemKind as K;
    use RecoveryIntent as I;
    let (state, problem, extra): (RegionState, Problem, Option<&'static str>) = match scenario {
        "partial" => (
            RegionState::new(Content::Present { low: false, partial: true }, Phase::Idle),
            problem(K::Network, &[(I::Retry, "Tải lại phần thiếu")]),
            Some("Lỗi một phần không xóa trang: nội dung đã tải vẫn dùng được."),
        ),
        "forbidden" => (
            RegionState::new(Content::Absent, Phase::Failed(K::Forbidden)),
            problem(K::Forbidden, &[(I::Return, "Về Khám phá")]),
            None,
        ),
        "not-found" => (
            RegionState::new(Content::Absent, Phase::Failed(K::NotFound)),
            problem(K::NotFound, &[(I::Return, "Về Khám phá")]),
            None,
        ),
        "auth-required" => (
            RegionState::new(Content::Absent, Phase::Failed(K::AuthRequired)),
            problem(K::AuthRequired, &[(I::Return, "Về Khám phá")]),
            Some("Chưa có hệ thống đăng nhập (D-05 mở): không có nút đăng nhập giả."),
        ),
        "rate-limited" => (
            RegionState::new(Content::Absent, Phase::Failed(K::RateLimited)),
            problem(K::RateLimited, &[(I::Retry, "Thử lại")]),
            Some("Chỉ fixture: API hiện chưa trả 429; thời điểm thử lại do caller quyết."),
        ),
        "unknown" => (
            RegionState::new(Content::Absent, Phase::Failed(K::Unknown)),
            problem(K::Unknown, &[(I::Retry, "Thử lại")]).with_code("works.list.unexpected-500"),
            None,
        ),
        "long-label" => (
            RegionState::new(Content::Absent, Phase::Failed(K::Unavailable)),
            Problem::new(
                K::Unavailable,
                "Dịch vụ lưu trữ bản thảo đang được bảo trì định kỳ nên tạm thời chưa thể mở danh sách tác phẩm của bạn",
                "Không có chữ nào bị mất. Bạn có thể thử lại sau ít phút; nếu vẫn chưa được, hãy tải bản sao các bản nháp đang mở trên thiết bị này trước khi đóng trang.",
            )
            .with_code("drafts.list.maintenance-window")
            .with_action(I::Retry, "Thử lại")
            .and_then(|p| p.with_action(I::Return, "Về Khám phá"))
            .expect("lawful"),
            None,
        ),
        _ => (
            RegionState::new(Content::Absent, Phase::Failed(K::Network)),
            problem(K::Network, &[(I::Retry, "Thử lại"), (I::Return, "Về Khám phá")])
                .with_code("works.list.network"),
            None,
        ),
    };
    view! {
        {extra.map(note)}
        <ContentRegion state=state label="Tác phẩm [Mẫu]" problem=Some(problem) on_recover=recovery_log("state.error")>
            {works(2)}
        </ContentRegion>
    }
    .into_any()
}

fn notice(scenario: &str) -> AnyView {
    match scenario {
        "banner" => view! {
            <Stack gap=Space::S4>
                <Banner
                    tone=Tone::Warning
                    title=ProblemKind::SessionExpired.fallback_copy().0
                    message="Bạn vẫn đọc được trang này; phần riêng tư cần phiên mới. (Chưa có đăng nhập: không có nút.)"
                    announce=Announce::Assertive
                />
                <Banner tone=Tone::Info message="Vink đang ở chế độ thử nghiệm [Mẫu]; dữ liệu có thể được đặt lại." on_dismiss=move |_| log("state.notice → đóng banner") />
            </Stack>
        }
        .into_any(),
        "snackbar" => {
            let clock = RwSignal::new(0u64);
            let message = RwSignal::new(None::<SnackbarMessage>);
            let dismiss = Callback::new(move |_| {
                message.set(None);
                log("state.notice → snackbar đóng");
            });
            use_snackbar_timeout(message.into(), Clock::Manual(clock.into()), dismiss);
            view! {
                <Stack gap=Space::S4>
                    {note("Đồng hồ thủ công: snackbar không có hành động tự đóng sau duration.snackbar (6000ms); có hành động thì chờ người dùng.")}
                    <Stack direction=Direction::Row gap=Space::S3 wrap=true>
                        <span data-testid="snackbar-show">
                            <Button variant=ButtonVariant::Tonal on_press=move |_| { message.set(Some(SnackbarMessage::new("Đã lưu bản nháp (máy chủ xác nhận)."))); log("state.notice → snackbar"); }>"Hiện snackbar"</Button>
                        </span>
                        <span data-testid="snackbar-show-action">
                            <Button variant=ButtonVariant::Tonal on_press=move |_| { message.set(Some(SnackbarMessage::new("Đã chuyển bản nháp vào thùng rác.").with_action("Hoàn tác"))); log("state.notice → snackbar có hành động"); }>"Hiện snackbar có hành động"</Button>
                        </span>
                        <span data-testid="snackbar-advance">
                            <Button variant=ButtonVariant::Text on_press=move |_| clock.update(|t| *t += 6000)>"Tiến 6s"</Button>
                        </span>
                    </Stack>
                    <p class="vkp-note" data-testid="snackbar-clock">{move || format!("Đồng hồ: +{}ms", clock.get())}</p>
                    <Snackbar message=message on_action=move |_| { message.set(None); log("state.notice → hoàn tác"); } on_dismiss=dismiss />
                </Stack>
            }
            .into_any()
        }
        "field-summary" => {
            let title = RwSignal::new("   ".to_string());
            let summary = Signal::derive(move || {
                if title.with(|t| t.trim().is_empty()) {
                    vec![("notice-title".to_string(), "Tiêu đề: cần có tiêu đề.".to_string())]
                } else {
                    Vec::new()
                }
            });
            let error = Signal::derive(move || summary.with(|s| (!s.is_empty()).then(|| "Cần có tiêu đề.".to_string())));
            view! {
                <Stack gap=Space::S4>
                    {note("Lỗi field giữ chữ thô (kể cả khoảng trắng); tóm tắt link tới đúng field; không chỉ bằng màu.")}
                    <FormSummary title="Cần sửa trước khi gửi" items=summary />
                    <TextField id="notice-title" label="Tiêu đề" value=title on_input=move |i: TextInput| title.set(i.value) required=true error=error />
                </Stack>
            }
            .into_any()
        }
        _ => view! {
            <Stack gap=Space::S3>
                <InlineNotice tone=Tone::Neutral message="Bản nháp riêng tư: chỉ bạn thấy." />
                <InlineNotice tone=Tone::Info title="Mẹo" message="Có thể dán từ Word/Docs; đoạn và khổ thơ được giữ." />
                <InlineNotice tone=Tone::Success message="Đã lưu lúc 09:00 (máy chủ xác nhận)." />
                <InlineNotice
                    tone=Tone::Warning
                    title=ProblemKind::Conflict.fallback_copy().0
                    message=ProblemKind::Conflict.fallback_copy().1
                    action=action(RecoveryIntent::Reconcile, "Đối chiếu")
                    on_recover=recovery_log("state.notice")
                />
                <InlineNotice
                    tone=Tone::Error
                    title=ProblemKind::Network.fallback_copy().0
                    message=ProblemKind::Network.fallback_copy().1
                    action=action(RecoveryIntent::Retry, "Thử lại")
                    on_recover=recovery_log("state.notice")
                    on_dismiss=move |_| log("state.notice → đóng")
                />
            </Stack>
        }
        .into_any(),
    }
}

fn status(long: bool) -> AnyView {
    let rows: Vec<(ActionPhase, &str)> = if long {
        vec![
            (
                ActionPhase::Uncertain,
                "Chưa rõ bản thảo “Những con phố cũ của Hà Nội” đã được gửi duyệt hay chưa; hãy kiểm tra lại trước khi gửi lần nữa",
            ),
            (
                ActionPhase::Failed(ProblemKind::Conflict),
                "Bản trên máy chủ đã đổi từ một thiết bị khác; chữ của bạn vẫn còn ở đây",
            ),
        ]
    } else {
        vec![
            (ActionPhase::Idle, "Chưa có thay đổi"),
            (ActionPhase::Dirty, "Chưa lưu"),
            (ActionPhase::Pending, "Đang lưu… (chờ xác nhận)"),
            (ActionPhase::Confirmed, "Đã lưu (máy chủ xác nhận)"),
            (
                ActionPhase::Failed(ProblemKind::Network),
                "Chưa lưu được — mạng",
            ),
            (ActionPhase::Cancelled, "Đã dừng — chưa lưu"),
            (ActionPhase::Uncertain, "Chưa rõ đã gửi hay chưa"),
        ]
    };
    view! {
        <Stack gap=Space::S3 align=Align::Start>
            {rows
                .into_iter()
                .map(|(phase, label)| view! {
                    <span data-testid=format!("status-{}", phase.id())>
                        <StatusBadge phase=phase label=label.to_string() />
                    </span>
                })
                .collect_view()}
        </Stack>
    }
    .into_any()
}

fn dialog(scenario: &str) -> AnyView {
    let open = RwSignal::new(false);
    let close = move |reason: &str| {
        open.set(false);
        log(format!("state.dialog → đóng ({reason})"));
    };
    let dismissed = Callback::new(move |reason: DismissReason| {
        close(match reason {
            DismissReason::Escape => "Escape",
            DismissReason::Outside => "bấm ngoài",
        });
    });
    match scenario {
        "session-expired" => {
            let text = RwSignal::new(fixture::READING.to_string());
            let (title, message) = ProblemKind::SessionExpired.fallback_copy();
            view! {
                <Stack gap=Space::S4>
                    {note("Hết phiên lúc đang viết: hộp thoại cần quyết định (Escape/bấm ngoài không đóng), chữ còn nguyên; chưa có auth nên không có nút đăng nhập.")}
                    <TextArea id="session-text" label="Bản nháp [Mẫu]" value=text on_input=move |i: TextInput| text.set(i.value) rows=6 />
                    <span data-testid="dialog-open">
                        <Button variant=ButtonVariant::Tonal on_press=move |_| { open.set(true); log("state.dialog → mô phỏng hết phiên"); }>"Mô phỏng hết phiên"</Button>
                    </span>
                    <Dialog
                        id="session-dialog"
                        open=open
                        title=title
                        context=message
                        role=DialogRole::AlertDialog
                        dismiss=DismissPolicy::Explicit
                        on_dismiss=dismissed
                        actions=move || view! {
                            <span data-testid="dialog-stay">
                                <Button variant=ButtonVariant::Text on_press=move |_| close("ở lại")>"Ở lại trang"</Button>
                            </span>
                            <Button on_press=move |_| log("state.dialog → tải bản sao .txt (chữ giữ nguyên)")>"Tải bản sao (.txt)"</Button>
                        }
                    >
                        <Text role=TypeRole::Caption tone=TextTone::Muted>"Đăng nhập chưa được cấu hình trong Vink; phiên mới sẽ có khi D-05 được chọn."</Text>
                    </Dialog>
                </Stack>
            }
            .into_any()
        }
        "sheet" => view! {
            <Stack gap=Space::S4>
                {note("Sheet: cùng anatomy, neo đáy ở màn hẹp; thông tin nên Escape và bấm ngoài đều đóng.")}
                <span data-testid="dialog-open">
                    <Button variant=ButtonVariant::Tonal on_press=move |_| { open.set(true); log("state.dialog → mở sheet"); }>"Sắp xếp danh sách"</Button>
                </span>
                <Dialog
                    id="sort-sheet"
                    open=open
                    title="Sắp xếp tác phẩm [Mẫu]"
                    kind=DialogKind::Sheet
                    dismiss=DismissPolicy::EscapeAndOutside
                    on_dismiss=dismissed
                    actions=move || view! {
                        <Button variant=ButtonVariant::Text on_press=move |_| close("đóng")>"Đóng"</Button>
                    }
                >
                    <Stack gap=Space::S2 align=Align::Start>
                        <Button variant=ButtonVariant::Outlined on_press=move |_| close("Mới nhất")>"Mới nhất"</Button>
                        <Button variant=ButtonVariant::Outlined on_press=move |_| close("Tên A–Z")>"Tên A–Z"</Button>
                    </Stack>
                </Dialog>
            </Stack>
        }
        .into_any(),
        _ => view! {
            <Stack gap=Space::S4>
                {note("Xác nhận phá hủy: focus ban đầu vào lựa chọn an toàn; Escape = giữ; đóng trả focus về nút mở.")}
                <span data-testid="dialog-open">
                    <Button variant=ButtonVariant::Danger on_press=move |_| { open.set(true); log("state.dialog → mở xác nhận"); }>"Xóa bản nháp…"</Button>
                </span>
                <Dialog
                    id="delete-dialog"
                    open=open
                    title="Xóa bản nháp “Gửi người ở lại [Mẫu]”?"
                    context="Bản nháp và lịch sử lưu của nó sẽ bị xóa khỏi Bàn viết. Không hoàn tác được."
                    role=DialogRole::AlertDialog
                    on_dismiss=dismissed
                    actions=move || view! {
                        <span data-testid="dialog-keep">
                            <Button variant=ButtonVariant::Tonal on_press=move |_| close("giữ")>"Giữ bản nháp"</Button>
                        </span>
                        <span data-testid="dialog-delete">
                            <Button variant=ButtonVariant::Danger on_press=move |_| close("xóa (mẫu, không gọi dịch vụ)")>"Xóa"</Button>
                        </span>
                    }
                />
            </Stack>
        }
        .into_any(),
    }
}

// Playground -------------------------------------------------------------------------------------

/// The preview's request tickets: one in flight at a time, and only the current ticket may
/// settle. It stands in for the app's owners (`EditorSession`, `CreateState`), not for them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Submission {
    issued: u32,
    current: Option<u32>,
}

impl Submission {
    /// A new ticket, or `None` while one is in flight (a double click sends nothing).
    pub fn begin(&mut self) -> Option<u32> {
        if self.current.is_some() {
            return None;
        }
        self.issued += 1;
        self.current = Some(self.issued);
        self.current
    }

    /// Whether `ticket` is the request in flight; a late or cancelled ticket settles nothing.
    pub fn settle(&mut self, ticket: u32) -> bool {
        let current = self.current == Some(ticket);
        if current {
            self.current = None;
        }
        current
    }

    pub fn cancel(&mut self) {
        self.current = None;
    }

    pub const fn last(&self) -> u32 {
        self.issued
    }
}

fn phase_label(phase: ActionPhase) -> &'static str {
    match phase {
        ActionPhase::Idle => "Chưa có thay đổi",
        ActionPhase::Dirty => "Chưa lưu",
        ActionPhase::Pending => "Đang gửi… (chờ xác nhận)",
        ActionPhase::Confirmed => "Đã lưu (máy chủ xác nhận)",
        ActionPhase::Failed(ProblemKind::Conflict) => "Xung đột phiên bản — chưa lưu",
        ActionPhase::Failed(_) => "Chưa lưu được",
        ActionPhase::Cancelled => "Đã dừng — chưa lưu",
        ActionPhase::Uncertain => "Chưa rõ đã gửi hay chưa",
    }
}

type NoticeView = (Tone, String, String, Vec<RecoveryAction>);

#[component]
fn Playground(recipe: String) -> impl IntoView {
    let recipe = StoredValue::new(recipe);
    let with_actions = matches!(
        recipe.get_value().as_str(),
        "pending-cancel" | "submit-uncertain" | "save-conflict" | "session-expired"
    );
    let starts_loaded = with_actions || recipe.get_value() == "refresh-partial";
    let clock = RwSignal::new(0u64);
    let step = RwSignal::new(0u32);
    let content = RwSignal::new(if starts_loaded {
        Content::PRESENT
    } else {
        Content::Absent
    });
    let phase = RwSignal::new(if starts_loaded {
        Phase::Idle
    } else {
        Phase::LOADING
    });
    let loading = Signal::derive(move || matches!(phase.get(), Phase::Loading { .. }));
    let gate = use_loading_gate(loading, Clock::Manual(clock.into()));
    let region = Signal::derive(move || {
        let phase = match phase.get() {
            Phase::Loading { .. } => Phase::Loading {
                slow: gate.slow.get(),
            },
            other => other,
        };
        RegionState::new(content.get(), phase)
    });
    let input = RwSignal::new(String::from("Phố cũ mưa phùn, [Mẫu]"));
    let submission = StoredValue::new(Submission::default());
    let action_state = RwSignal::new(ActionState::Enabled);
    let action_phase = RwSignal::new(ActionPhase::Idle);
    let resend = RwSignal::new(false);
    let notice = RwSignal::new(None::<NoticeView>);
    let snackbar = RwSignal::new(None::<SnackbarMessage>);
    let dialog_open = RwSignal::new(false);
    let empty_panel = PanelContent::empty(
        EmptyKind::FirstUse,
        "Chưa có tác phẩm nào",
        "Tác phẩm đã đăng sẽ hiện ở đây.",
        vec![action(RecoveryIntent::Start, "Viết bản nháp đầu tiên")],
    )
    .expect("lawful");
    let region_problem = Signal::derive(move || match phase.get() {
        Phase::Failed(ProblemKind::Network) => Some(problem(
            ProblemKind::Network,
            &[(RecoveryIntent::Retry, "Thử lại")],
        )),
        _ => content
            .get()
            .eq(&Content::Present {
                low: false,
                partial: true,
            })
            .then(|| {
                problem(
                    ProblemKind::Network,
                    &[(RecoveryIntent::Retry, "Tải lại phần thiếu")],
                )
            }),
    });
    let dismiss = Callback::new(move |_| snackbar.set(None));
    use_snackbar_timeout(snackbar.into(), Clock::Manual(clock.into()), dismiss);

    let warn = move |kind: ProblemKind, actions: Vec<RecoveryAction>| {
        let (title, message) = kind.fallback_copy();
        notice.set(Some((kind.tone(), title.into(), message.into(), actions)));
    };
    let press = move |_| {
        let Some(ticket) = submission.try_update_value(Submission::begin).flatten() else {
            log("playground → bỏ qua kích hoạt thứ hai (đang chờ)");
            return;
        };
        action_state.set(ActionState::pending("Đang gửi… (chờ xác nhận)"));
        action_phase.set(ActionPhase::Pending);
        notice.set(None);
        log(format!(
            "playground → gửi vé #{ticket}{}",
            if resend.get_untracked() {
                " (gửi lại có chủ ý)"
            } else {
                ""
            }
        ));
    };
    let cancel = move |_| {
        if action_phase.get_untracked() != ActionPhase::Pending {
            return;
        }
        submission.update_value(Submission::cancel);
        action_state.set(ActionState::Enabled);
        action_phase.set(ActionPhase::Cancelled);
        snackbar.set(Some(SnackbarMessage::new("Đã dừng lưu; chữ vẫn còn.")));
        log("playground → hủy (không lỗi đỏ)");
    };
    let next = move |_| {
        let n = step.get_untracked();
        step.set(n + 1);
        let now = clock.get_untracked();
        let last = submission.with_value(Submission::last);
        match recipe.get_value().as_str() {
            "loading-empty" if loading.get_untracked() => {
                content.set(Content::Empty(EmptyKind::FirstUse));
                phase.set(Phase::Idle);
            }
            "loading-error-retry" if loading.get_untracked() && n == 0 => {
                phase.set(Phase::Failed(ProblemKind::Network))
            }
            "refresh-partial" if n == 0 => phase.set(Phase::LOADING),
            "refresh-partial" if loading.get_untracked() => {
                content.set(Content::Present {
                    low: false,
                    partial: true,
                });
                phase.set(Phase::Idle);
            }
            "pending-cancel" => {
                // A late acknowledgment of a cancelled ticket must not show "saved".
                if submission
                    .try_update_value(|s| s.settle(last))
                    .unwrap_or(false)
                {
                    action_state.set(ActionState::Enabled);
                    action_phase.set(ActionPhase::Confirmed);
                    log(format!("playground → ack vé #{last} → đã xác nhận"));
                } else {
                    log(format!("playground → ack muộn của vé #{last} bị bỏ qua"));
                }
            }
            "submit-uncertain" if action_phase.get_untracked() == ActionPhase::Pending => {
                submission.update_value(|s| {
                    s.settle(last);
                });
                action_state.set(ActionState::disabled("Kiểm tra lại trước khi gửi lần nữa"));
                action_phase.set(ActionPhase::Uncertain);
                warn(
                    ProblemKind::Uncertain,
                    vec![action(RecoveryIntent::Reconcile, "Kiểm tra lại")],
                );
            }
            "save-conflict" if action_phase.get_untracked() == ActionPhase::Pending => {
                submission.update_value(|s| {
                    s.settle(last);
                });
                action_state.set(ActionState::Enabled);
                action_phase.set(ActionPhase::Failed(ProblemKind::Conflict));
                warn(
                    ProblemKind::Conflict,
                    vec![
                        action(RecoveryIntent::Reconcile, "Đối chiếu"),
                        action(RecoveryIntent::Export, "Tải bản sao (.txt)"),
                    ],
                );
            }
            "session-expired" => dialog_open.set(true),
            _ if loading.get_untracked() => {
                content.set(Content::PRESENT);
                phase.set(Phase::Idle);
            }
            _ => {
                log("playground → không còn bước");
                return;
            }
        }
        log(format!("playground → bước {} @+{now}ms", n + 1));
    };
    let on_region = Callback::new(move |intent: RecoveryIntent| {
        log(format!("playground → intent {}", intent.id()));
        if intent == RecoveryIntent::Retry {
            if content.get_untracked() == Content::Absent {
                phase.set(Phase::LOADING);
            } else {
                content.set(Content::PRESENT);
            }
        }
    });
    let on_notice = Callback::new(move |intent: RecoveryIntent| {
        log(format!("playground → intent {}", intent.id()));
        match intent {
            RecoveryIntent::Reconcile if action_phase.get_untracked() == ActionPhase::Uncertain => {
                notice.set(None);
                resend.set(true);
                action_state.set(ActionState::Enabled);
                action_phase.set(ActionPhase::Idle);
                log("playground → đối chiếu xong (GET); chỉ gửi lại khi người dùng bấm");
            }
            RecoveryIntent::Export => log("playground → tải bản sao .txt (chữ giữ nguyên)"),
            _ => {}
        }
    });
    let on_input = move |i: TextInput| {
        input.set(i.value);
        if !matches!(
            action_phase.get_untracked(),
            ActionPhase::Pending | ActionPhase::Uncertain
        ) {
            action_phase.set(ActionPhase::Dirty);
        }
    };
    let action_label = move || match recipe.get_value().as_str() {
        "submit-uncertain" if resend.get() => "Gửi lại có chủ ý",
        "submit-uncertain" => "Gửi bản thảo",
        "save-conflict" => "Lưu",
        _ => "Lưu bản nháp",
    };
    let (session_title, session_message) = ProblemKind::SessionExpired.fallback_copy();
    view! {
        <Stack gap=Space::S4>
            <div class="vkp-playground-bar">
                <p class="vkp-note" data-testid="playground-clock">{move || format!("+{}ms", clock.get())}</p>
                <Stack direction=Direction::Row gap=Space::S2 wrap=true>
                    <span data-testid="playground-advance-100">
                        <Button variant=ButtonVariant::Outlined on_press=move |_| clock.update(|t| *t += 100)>"Tiến 100ms"</Button>
                    </span>
                    <span data-testid="playground-advance-1000">
                        <Button variant=ButtonVariant::Outlined on_press=move |_| clock.update(|t| *t += 1000)>"Tiến 1s"</Button>
                    </span>
                    <span data-testid="playground-advance-8000">
                        <Button variant=ButtonVariant::Outlined on_press=move |_| clock.update(|t| *t += 8000)>"Tiến 8s"</Button>
                    </span>
                    <span data-testid="playground-step">
                        <Button variant=ButtonVariant::Tonal on_press=next>"Bước tiếp"</Button>
                    </span>
                </Stack>
            </div>
            <TextField id="playground-input" label="Ghi chú đang viết [Mẫu]" value=input on_input=on_input />
            {with_actions.then(|| view! {
                <Stack direction=Direction::Row gap=Space::S3 wrap=true align=Align::Start>
                    <span data-testid="playground-action">
                        <Button id="playground-action-button" state=action_state on_press=press>{action_label}</Button>
                    </span>
                    {(recipe.get_value() == "pending-cancel").then(|| view! {
                        <span data-testid="playground-cancel">
                            <Button variant=ButtonVariant::Text on_press=cancel>"Dừng"</Button>
                        </span>
                    })}
                    <span data-testid="playground-status">
                        <StatusBadge phase=action_phase label=Signal::derive(move || phase_label(action_phase.get()).to_string()) />
                    </span>
                </Stack>
            })}
            {move || notice.get().map(|(tone, title, message, actions)| {
                let actions = actions
                    .into_iter()
                    .map(|a| {
                        let intent = a.intent;
                        view! {
                            <span data-intent=intent.id()>
                                <Button variant=ButtonVariant::Text on_press=move |_| on_notice.run(intent)>{a.label}</Button>
                            </span>
                        }
                    })
                    .collect_view();
                view! {
                    <div data-testid="playground-notice">
                        <InlineNotice tone=tone title=title message=message />
                        <Stack direction=Direction::Row gap=Space::S2 wrap=true>{actions}</Stack>
                    </div>
                }
            })}
            <div data-testid="playground-region">
                <ContentRegion
                    state=region
                    label="Tác phẩm [Mẫu]"
                    problem=region_problem
                    empty=Some(empty_panel)
                    indicator_visible=gate.visible
                    on_recover=on_region
                >
                    {works(3)}
                </ContentRegion>
            </div>
            <Snackbar message=snackbar on_dismiss=dismiss />
            <Dialog
                id="playground-session"
                open=dialog_open
                title=session_title
                context=session_message
                role=DialogRole::AlertDialog
                dismiss=DismissPolicy::Explicit
                on_dismiss=Callback::new(|_| {})
                actions=move || view! {
                    <span data-testid="playground-stay">
                        <Button variant=ButtonVariant::Text on_press=move |_| { dialog_open.set(false); log("playground → ở lại trang (chữ còn)"); }>"Ở lại trang"</Button>
                    </span>
                    <Button on_press=move |_| log("playground → tải bản sao .txt (chữ giữ nguyên)")>"Tải bản sao (.txt)"</Button>
                }
            >
                <Text role=TypeRole::Caption tone=TextTone::Muted>"Chưa có đăng nhập trong Vink: không có nút đăng nhập."</Text>
            </Dialog>
        </Stack>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submissions_ignore_double_clicks_and_late_acknowledgments() {
        let mut s = Submission::default();
        let first = s.begin().expect("first ticket");
        assert_eq!(
            s.begin(),
            None,
            "a second activation while pending sends nothing"
        );
        s.cancel();
        let second = s.begin().expect("a new ticket after cancel");
        assert_ne!(first, second);
        assert!(
            !s.settle(first),
            "the cancelled ticket's late ack settles nothing"
        );
        assert!(s.settle(second));
        assert!(!s.settle(second), "a ticket settles once");
        s.begin();
        s.cancel();
        assert!(!s.settle(s.last()), "an ack after cancel is ignored");
    }
}
