//! The UIS-03 state contract (`shared/states.md`) as plain data: separate axes with lawful
//! combinations, a safe problem projection, notice placement and the `LoadingGate` timing policy.
//! Nothing here fetches, schedules, navigates or decides permission; callers project their own
//! authorities (API errors, `EditorSession`, `CreateState`) into these types.

use std::fmt;

/// What a region holds right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Content {
    Absent,
    Empty(EmptyKind),
    /// `low`: little real content; `partial`: a part of the region could not be loaded.
    Present {
        low: bool,
        partial: bool,
    },
}

impl Content {
    pub const PRESENT: Self = Self::Present {
        low: false,
        partial: false,
    };

    pub fn id(self) -> String {
        match self {
            Self::Absent => "absent".into(),
            Self::Empty(kind) => format!("empty:{}", kind.id()),
            Self::Present { low, partial } => match (low, partial) {
                (false, false) => "present".into(),
                (true, false) => "present:low".into(),
                (false, true) => "present:partial".into(),
                (true, true) => "present:low+partial".into(),
            },
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Some(match id {
            "absent" => Self::Absent,
            "present" => Self::PRESENT,
            "present:low" => Self::Present {
                low: true,
                partial: false,
            },
            "present:partial" => Self::Present {
                low: false,
                partial: true,
            },
            "present:low+partial" => Self::Present {
                low: true,
                partial: true,
            },
            _ => Self::Empty(EmptyKind::parse(id.strip_prefix("empty:")?)?),
        })
    }
}

/// Empty is not an error: first use invites a start, no-results offers to clear the filter.
/// End-of-list is not empty; it is [`crate::ListEnd`] below real content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EmptyKind {
    FirstUse,
    NoResults,
}

impl EmptyKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::FirstUse => "first-use",
            Self::NoResults => "no-results",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        [Self::FirstUse, Self::NoResults]
            .into_iter()
            .find(|k| k.id() == id)
    }
}

/// The request phase of a region. `slow` comes from [`LoadingGate`]; it is not a timeout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    Idle,
    Loading { slow: bool },
    Failed(ProblemKind),
    Cancelled,
}

impl Phase {
    pub const LOADING: Self = Self::Loading { slow: false };

    pub fn id(self) -> String {
        match self {
            Self::Idle => "idle".into(),
            Self::Loading { slow: false } => "loading".into(),
            Self::Loading { slow: true } => "loading:slow".into(),
            Self::Failed(kind) => format!("failed:{}", kind.id()),
            Self::Cancelled => "cancelled".into(),
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Some(match id {
            "idle" => Self::Idle,
            "loading" => Self::LOADING,
            "loading:slow" => Self::Loading { slow: true },
            "cancelled" => Self::Cancelled,
            _ => Self::Failed(ProblemKind::parse(id.strip_prefix("failed:")?)?),
        })
    }
}

/// Presentation classes of a failure. The caller classifies; this enum only fixes what each
/// class may offer and where it is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProblemKind {
    Validation,
    Conflict,
    /// Fixture-only until the API answers 429.
    RateLimited,
    Timeout,
    Network,
    Unavailable,
    /// A write whose outcome is unknown: reconcile before a deliberate resend, never replay.
    Uncertain,
    /// An unclassified failure; copy must not promise that the operation certainly failed.
    Unknown,
    AuthRequired,
    /// Only with evidence from the session authority; a lone 401 does not prove it.
    SessionExpired,
    Forbidden,
    NotFound,
}

impl ProblemKind {
    pub const ALL: [Self; 12] = [
        Self::Validation,
        Self::Conflict,
        Self::RateLimited,
        Self::Timeout,
        Self::Network,
        Self::Unavailable,
        Self::Uncertain,
        Self::Unknown,
        Self::AuthRequired,
        Self::SessionExpired,
        Self::Forbidden,
        Self::NotFound,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::Conflict => "conflict",
            Self::RateLimited => "rate-limited",
            Self::Timeout => "timeout",
            Self::Network => "network",
            Self::Unavailable => "unavailable",
            Self::Uncertain => "uncertain",
            Self::Unknown => "unknown",
            Self::AuthRequired => "auth-required",
            Self::SessionExpired => "session-expired",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not-found",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    /// Failures of a write, shown beside the field or action rather than replacing content.
    pub const fn is_write(self) -> bool {
        matches!(self, Self::Validation | Self::Conflict | Self::Uncertain)
    }

    /// The content can no longer be shown; the caller drops it.
    pub const fn revokes_content(self) -> bool {
        matches!(self, Self::Forbidden | Self::NotFound | Self::AuthRequired)
    }

    pub const fn allows(self, intent: RecoveryIntent) -> bool {
        use RecoveryIntent as I;
        match self {
            Self::Validation => false,
            Self::Conflict | Self::Uncertain => {
                matches!(intent, I::Reconcile | I::Export | I::Return)
            }
            Self::RateLimited => matches!(intent, I::Retry | I::Return),
            Self::Timeout | Self::Network | Self::Unavailable | Self::Unknown => {
                matches!(intent, I::Retry | I::Export | I::Return)
            }
            Self::AuthRequired => matches!(intent, I::Reauthenticate | I::Return),
            Self::SessionExpired => matches!(intent, I::Reauthenticate | I::Export | I::Return),
            Self::Forbidden | Self::NotFound => matches!(intent, I::Return),
        }
    }

    /// Tone of the panel or notice that shows this kind.
    pub const fn tone(self) -> Tone {
        match self {
            Self::RateLimited | Self::Uncertain | Self::Conflict | Self::SessionExpired => {
                Tone::Warning
            }
            Self::AuthRequired | Self::Forbidden | Self::NotFound => Tone::Neutral,
            _ => Tone::Error,
        }
    }

    /// Default Vietnamese copy, for callers without their own; never claims a certain failure
    /// for `Unknown`/`Uncertain`.
    pub const fn fallback_copy(self) -> (&'static str, &'static str) {
        match self {
            Self::Validation => (
                "Cần sửa vài chỗ",
                "Kiểm tra các ô được đánh dấu; chữ bạn đã nhập vẫn còn.",
            ),
            Self::Conflict => (
                "Bản trên máy chủ đã thay đổi",
                "Chữ của bạn vẫn còn ở đây. Đối chiếu trước khi lưu để không ghi đè.",
            ),
            Self::RateLimited => ("Thao tác quá nhanh", "Vui lòng đợi một chút rồi thử lại."),
            Self::Timeout => (
                "Máy chủ chưa trả lời kịp",
                "Có thể thử lại; nội dung đang có không bị xóa.",
            ),
            Self::Network => (
                "Chưa kết nối được",
                "Không gửi hoặc nhận được dữ liệu. Kiểm tra kết nối rồi thử lại.",
            ),
            Self::Unavailable => (
                "Dịch vụ tạm thời không sẵn sàng",
                "Có thể thử lại sau ít phút.",
            ),
            Self::Uncertain => (
                "Chưa rõ thao tác đã hoàn tất hay chưa",
                "Kiểm tra lại trước khi gửi lần nữa để không tạo bản trùng.",
            ),
            Self::Unknown => ("Chưa tải được", "Chưa rõ nguyên nhân. Có thể thử lại."),
            Self::AuthRequired => (
                "Cần đăng nhập để xem phần này",
                "Nội dung công khai vẫn đọc được mà không cần đăng nhập.",
            ),
            Self::SessionExpired => (
                "Phiên đăng nhập đã hết hạn",
                "Chữ bạn đang viết vẫn còn trên trang này.",
            ),
            Self::Forbidden => (
                "Không có quyền xem nội dung này",
                "Nội dung này không dành cho tài khoản hiện tại.",
            ),
            Self::NotFound => (
                "Không tìm thấy nội dung",
                "Có thể nội dung đã được gỡ hoặc đường dẫn không đúng.",
            ),
        }
    }
}

/// Recovery the caller offers; the library only reports which one was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RecoveryIntent {
    Retry,
    Reconcile,
    Reauthenticate,
    Return,
    ClearFilter,
    Export,
    /// Only for an empty first-use panel: begin (write a first draft, explore).
    Start,
}

impl RecoveryIntent {
    pub const ALL: [Self; 7] = [
        Self::Retry,
        Self::Reconcile,
        Self::Reauthenticate,
        Self::Return,
        Self::ClearFilter,
        Self::Export,
        Self::Start,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::Reconcile => "reconcile",
            Self::Reauthenticate => "reauthenticate",
            Self::Return => "return",
            Self::ClearFilter => "clear-filter",
            Self::Export => "export",
            Self::Start => "start",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|i| i.id() == id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RecoveryAction {
    pub intent: RecoveryIntent,
    pub label: String,
}

/// A stable diagnostic code (`[a-z0-9][a-z0-9._-]{0,63}`). Anything else is dropped rather than
/// shown, so a raw message, path or token cannot pass as a code.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DiagnosticCode(String);

impl DiagnosticCode {
    pub fn parse(code: &str) -> Option<Self> {
        let mut chars = code.chars();
        let first = chars.next()?;
        let valid = code.len() <= 64
            && (first.is_ascii_lowercase() || first.is_ascii_digit())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c));
        valid.then(|| Self(code.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemError {
    IntentNotAllowed {
        kind: ProblemKind,
        intent: RecoveryIntent,
    },
}

/// The safe projection of a failure: localized copy, an optional stable code and the recovery
/// actions the caller offers. There is no field for a payload, stack trace or token.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Problem {
    pub kind: ProblemKind,
    pub title: String,
    pub message: String,
    pub code: Option<DiagnosticCode>,
    pub actions: Vec<RecoveryAction>,
}

impl Problem {
    pub fn new(kind: ProblemKind, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            message: message.into(),
            code: None,
            actions: Vec::new(),
        }
    }

    /// The kind's fallback copy.
    pub fn fallback(kind: ProblemKind) -> Self {
        let (title, message) = kind.fallback_copy();
        Self::new(kind, title, message)
    }

    /// Keeps the code only when it is a valid [`DiagnosticCode`].
    pub fn with_code(mut self, code: &str) -> Self {
        self.code = DiagnosticCode::parse(code);
        self
    }

    /// Refuses an intent the kind does not allow (for example `Retry` after an uncertain write).
    pub fn with_action(
        mut self,
        intent: RecoveryIntent,
        label: impl Into<String>,
    ) -> Result<Self, ProblemError> {
        if !self.kind.allows(intent) {
            return Err(ProblemError::IntentNotAllowed {
                kind: self.kind,
                intent,
            });
        }
        self.actions.push(RecoveryAction {
            intent,
            label: label.into(),
        });
        Ok(self)
    }
}

impl EmptyKind {
    pub const fn allows(self, intent: RecoveryIntent) -> bool {
        match self {
            Self::FirstUse => matches!(intent, RecoveryIntent::Start | RecoveryIntent::Return),
            Self::NoResults => {
                matches!(intent, RecoveryIntent::ClearFilter | RecoveryIntent::Return)
            }
        }
    }
}

/// What a [`crate::StatePanel`] shows: symbol, heading, explanation, actions (primary first)
/// and an optional code. Build it with [`PanelContent::from_problem`], [`PanelContent::empty`] or
/// [`PanelContent::cancelled`] so the actions stay lawful for the panel's kind.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PanelContent {
    pub kind: PanelKind,
    pub title: String,
    pub message: String,
    pub actions: Vec<RecoveryAction>,
    pub code: Option<DiagnosticCode>,
}

impl PanelContent {
    pub fn from_problem(problem: &Problem) -> Self {
        Self {
            kind: PanelKind::Error(problem.kind),
            title: problem.title.clone(),
            message: problem.message.clone(),
            actions: problem.actions.clone(),
            code: problem.code.clone(),
        }
    }

    pub fn empty(
        kind: EmptyKind,
        title: impl Into<String>,
        message: impl Into<String>,
        actions: Vec<RecoveryAction>,
    ) -> Result<Self, PanelError> {
        if let Some(bad) = actions.iter().find(|a| !kind.allows(a.intent)) {
            return Err(PanelError::IntentNotAllowed(bad.intent));
        }
        Ok(Self {
            kind: PanelKind::Empty(kind),
            title: title.into(),
            message: message.into(),
            actions,
            code: None,
        })
    }

    /// A stopped load is neutral, never red; reloading is the caller's `Retry`.
    pub fn cancelled(
        title: impl Into<String>,
        message: impl Into<String>,
        retry: Option<String>,
    ) -> Self {
        Self {
            kind: PanelKind::Cancelled,
            title: title.into(),
            message: message.into(),
            actions: retry
                .map(|label| RecoveryAction {
                    intent: RecoveryIntent::Retry,
                    label,
                })
                .into_iter()
                .collect(),
            code: None,
        }
    }

    pub const fn tone(&self) -> Tone {
        match self.kind {
            PanelKind::Error(kind) => kind.tone(),
            PanelKind::Cancelled | PanelKind::Empty(_) => Tone::Neutral,
        }
    }

    pub const fn icon(&self) -> crate::IconName {
        use crate::IconName as I;
        match self.kind {
            PanelKind::Empty(EmptyKind::FirstUse) => I::Edit,
            PanelKind::Empty(EmptyKind::NoResults) => I::Search,
            PanelKind::Cancelled => I::Close,
            PanelKind::Error(kind) => kind.icon(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelError {
    IntentNotAllowed(RecoveryIntent),
}

impl ProblemKind {
    pub const fn icon(self) -> crate::IconName {
        use crate::IconName as I;
        match self {
            Self::Validation | Self::Unavailable | Self::Network | Self::Timeout => I::Error,
            Self::Conflict | Self::RateLimited => I::Warning,
            Self::Uncertain | Self::Unknown => I::Help,
            Self::AuthRequired | Self::SessionExpired => I::Lock,
            Self::Forbidden => I::Block,
            Self::NotFound => I::Search,
        }
    }
}

impl Tone {
    pub const fn icon(self) -> crate::IconName {
        use crate::IconName as I;
        match self {
            Self::Neutral | Self::Info => I::Info,
            Self::Success => I::Check,
            Self::Warning => I::Warning,
            Self::Error => I::Error,
        }
    }
}

/// Copy a [`crate::ContentRegion`] uses for its own notices; the defaults are Vietnamese.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RegionCopy {
    /// Visually hidden status while the skeleton shows.
    pub loading: String,
    pub slow: String,
    pub partial: String,
    pub low_content: String,
    pub cancelled_title: String,
    pub cancelled_message: String,
    pub reload: String,
}

impl Default for RegionCopy {
    fn default() -> Self {
        Self {
            loading: "Đang tải…".into(),
            slow: "Vẫn đang tải, lâu hơn thường lệ. Nội dung sẽ hiện khi xong.".into(),
            partial: "Một phần chưa tải được; phần còn lại vẫn dùng được.".into(),
            low_content: "Chưa có nhiều nội dung ở đây.".into(),
            cancelled_title: "Đã dừng tải".into(),
            cancelled_message: "Không có gì bị thay đổi.".into(),
            reload: "Tải lại".into(),
        }
    }
}

/// The geometry a skeleton imitates, in the reading order of the real content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkeletonShape {
    /// Lines of running text; the last one is shorter.
    Text { lines: u8 },
    /// A work card: title, byline, two lines of excerpt.
    Card,
    /// Avatar and two short lines.
    Byline,
}

impl SkeletonShape {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::Card => "card",
            Self::Byline => "byline",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum IndicatorSize {
    /// 20px, inside a button's status slot.
    #[default]
    Small,
    /// 24px, beside a status label.
    Medium,
}

/// One short message that needs no decision, with at most one action.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SnackbarMessage {
    pub text: String,
    pub action: Option<String>,
}

impl SnackbarMessage {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            action: None,
        }
    }

    pub fn with_action(mut self, label: impl Into<String>) -> Self {
        self.action = Some(label.into());
        self
    }

    /// Without an action it closes after `duration.snackbar`; with one it waits for the person.
    pub const fn dismiss(&self) -> Dismiss {
        if self.action.is_some() {
            Dismiss::Manual
        } else {
            Dismiss::Auto
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DialogRole {
    #[default]
    Dialog,
    /// Interrupts for a decision (destructive confirmation, session expired while writing).
    AlertDialog,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DialogKind {
    #[default]
    Dialog,
    /// Same anatomy, anchored to the bottom edge on narrow screens.
    Sheet,
}

/// Which gestures may close a dialog without choosing an action.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DismissPolicy {
    /// Only the dialog's own actions: a decision is required.
    Explicit,
    #[default]
    EscapeOnly,
    EscapeAndOutside,
}

impl DismissPolicy {
    pub const fn allows(self, reason: DismissReason) -> bool {
        match reason {
            DismissReason::Escape => !matches!(self, Self::Explicit),
            DismissReason::Outside => matches!(self, Self::EscapeAndOutside),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DismissReason {
    Escape,
    Outside,
}

/// Why a region state cannot be shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unlawful {
    /// No content and no request: the caller has to start loading.
    NotRequested,
    /// Validation, conflict and uncertain writes belong beside the field or action.
    WriteProblem,
    /// Forbidden, not found or auth required with content still held: drop the content first.
    Revoked,
}

impl Unlawful {
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotRequested => "not-requested",
            Self::WriteProblem => "write-problem",
            Self::Revoked => "revoked",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelKind {
    Error(ProblemKind),
    Cancelled,
    Empty(EmptyKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Body {
    Skeleton,
    Panel(PanelKind),
    /// The caller's content stays mounted (focus, scroll, selection kept).
    Content,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegionNotice {
    Slow,
    Refresh(ProblemKind),
    Partial,
    LowContent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RegionLayout {
    pub body: Body,
    /// EdgeProgress over the region while it refreshes.
    pub progress: bool,
    pub notice: Option<RegionNotice>,
}

impl RegionLayout {
    /// The vector form `body|progress|notice`.
    pub fn id(self) -> String {
        let body = match self.body {
            Body::Skeleton => "skeleton".to_string(),
            Body::Content => "content".to_string(),
            Body::Panel(PanelKind::Cancelled) => "panel:cancelled".to_string(),
            Body::Panel(PanelKind::Empty(e)) => format!("panel:empty:{}", e.id()),
            Body::Panel(PanelKind::Error(k)) => format!("panel:error:{}", k.id()),
        };
        let notice = match self.notice {
            None => "-".to_string(),
            Some(RegionNotice::Slow) => "slow".into(),
            Some(RegionNotice::Partial) => "partial".into(),
            Some(RegionNotice::LowContent) => "low-content".into(),
            Some(RegionNotice::Refresh(k)) => format!("refresh:{}", k.id()),
        };
        format!("{body}|{}|{notice}", if self.progress { "+" } else { "-" })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RegionState {
    pub content: Content,
    pub phase: Phase,
}

impl RegionState {
    pub const fn new(content: Content, phase: Phase) -> Self {
        Self { content, phase }
    }

    /// The lawful combinations of `shared/states.md`.
    pub fn layout(self) -> Result<RegionLayout, Unlawful> {
        let (body, quiet) = match self.content {
            Content::Absent => (Body::Skeleton, None),
            Content::Empty(kind) => (Body::Panel(PanelKind::Empty(kind)), None),
            Content::Present { low, partial } => (
                Body::Content,
                if partial {
                    Some(RegionNotice::Partial)
                } else if low {
                    Some(RegionNotice::LowContent)
                } else {
                    None
                },
            ),
        };
        let absent = self.content == Content::Absent;
        let layout = |body, progress, notice| RegionLayout {
            body,
            progress,
            notice,
        };
        Ok(match self.phase {
            Phase::Idle if absent => return Err(Unlawful::NotRequested),
            Phase::Idle => layout(body, false, quiet),
            Phase::Loading { slow } => {
                let slow = slow.then_some(RegionNotice::Slow);
                if absent {
                    layout(Body::Skeleton, false, slow)
                } else {
                    layout(body, true, slow.or(quiet))
                }
            }
            Phase::Cancelled if absent => layout(Body::Panel(PanelKind::Cancelled), false, None),
            Phase::Cancelled => layout(body, false, quiet),
            Phase::Failed(kind) if kind.is_write() => return Err(Unlawful::WriteProblem),
            Phase::Failed(kind) if absent => {
                layout(Body::Panel(PanelKind::Error(kind)), false, None)
            }
            Phase::Failed(kind) if kind.revokes_content() => return Err(Unlawful::Revoked),
            Phase::Failed(kind) => layout(body, false, Some(RegionNotice::Refresh(kind))),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Placement {
    Field,
    Inline,
    Region,
    Banner,
    Snackbar,
    Dialog,
}

impl Placement {
    pub const ALL: [Self; 6] = [
        Self::Field,
        Self::Inline,
        Self::Region,
        Self::Banner,
        Self::Snackbar,
        Self::Dialog,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Inline => "inline",
            Self::Region => "region",
            Self::Banner => "banner",
            Self::Snackbar => "snackbar",
            Self::Dialog => "dialog",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tone {
    #[default]
    Neutral,
    Info,
    Success,
    Warning,
    Error,
}

impl Tone {
    pub const ALL: [Self; 5] = [
        Self::Neutral,
        Self::Info,
        Self::Success,
        Self::Warning,
        Self::Error,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.id() == id)
    }
}

/// How a live region announces a change. Explicit: there is no "alert everything".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Announce {
    Off,
    #[default]
    Polite,
    Assertive,
}

impl Announce {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Polite => "polite",
            Self::Assertive => "assertive",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        [Self::Off, Self::Polite, Self::Assertive]
            .into_iter()
            .find(|a| a.id() == id)
    }
}

/// What ends a notice: the owning state changing, the person, or (snackbar only) a timeout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Dismiss {
    #[default]
    State,
    Manual,
    Auto,
}

impl Dismiss {
    pub const fn id(self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Manual => "manual",
            Self::Auto => "auto",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        [Self::State, Self::Manual, Self::Auto]
            .into_iter()
            .find(|d| d.id() == id)
    }
}

/// Where a problem is shown and how it is announced (`shared/states.md`).
pub const fn placement_for(
    kind: ProblemKind,
    has_data: bool,
    writing: bool,
) -> (Placement, Announce) {
    use ProblemKind as K;
    match kind {
        K::Validation => (Placement::Field, Announce::Off),
        K::Conflict | K::Uncertain => (Placement::Inline, Announce::Polite),
        K::SessionExpired if writing => (Placement::Dialog, Announce::Off),
        K::SessionExpired => (Placement::Banner, Announce::Assertive),
        K::AuthRequired | K::Forbidden | K::NotFound => (Placement::Region, Announce::Polite),
        _ if has_data || writing => (Placement::Inline, Announce::Polite),
        _ => (Placement::Region, Announce::Polite),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeError {
    /// A snackbar disappears; it cannot carry an error or warning.
    SnackbarTone,
    Announce,
    Dismiss,
}

impl NoticeError {
    pub const fn id(self) -> &'static str {
        match self {
            Self::SnackbarTone => "snackbar-tone",
            Self::Announce => "announce",
            Self::Dismiss => "dismiss",
        }
    }
}

/// A validated combination of placement, tone, announcement and dismissal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NoticeSpec {
    pub placement: Placement,
    pub tone: Tone,
    pub announce: Announce,
    pub dismiss: Dismiss,
}

impl NoticeSpec {
    pub fn new(
        placement: Placement,
        tone: Tone,
        announce: Announce,
        dismiss: Dismiss,
    ) -> Result<Self, NoticeError> {
        use Placement as P;
        if placement == P::Snackbar && matches!(tone, Tone::Error | Tone::Warning) {
            return Err(NoticeError::SnackbarTone);
        }
        let announce_ok = match placement {
            P::Field | P::Dialog => announce == Announce::Off,
            P::Snackbar => announce == Announce::Polite,
            P::Inline | P::Region | P::Banner => announce != Announce::Off,
        };
        if !announce_ok {
            return Err(NoticeError::Announce);
        }
        let dismiss_ok = match placement {
            P::Snackbar => dismiss != Dismiss::State,
            P::Field | P::Region => dismiss == Dismiss::State,
            P::Inline | P::Banner | P::Dialog => dismiss != Dismiss::Auto,
        };
        if !dismiss_ok {
            return Err(NoticeError::Dismiss);
        }
        Ok(Self {
            placement,
            tone,
            announce,
            dismiss,
        })
    }
}

/// The interaction phase of one action, as its owner (for example `EditorSession`) reports it.
/// `Confirmed` is only for an acknowledgment of the current request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ActionPhase {
    #[default]
    Idle,
    Dirty,
    Pending,
    Confirmed,
    Failed(ProblemKind),
    Cancelled,
    Uncertain,
}

impl ActionPhase {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Dirty => "dirty",
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Failed(_) => "failed",
            Self::Cancelled => "cancelled",
            Self::Uncertain => "uncertain",
        }
    }

    pub const fn tone(self) -> Tone {
        match self {
            Self::Idle | Self::Pending | Self::Cancelled => Tone::Neutral,
            Self::Dirty => Tone::Info,
            Self::Confirmed => Tone::Success,
            Self::Failed(kind) => kind.tone(),
            Self::Uncertain => Tone::Warning,
        }
    }
}

/// Real progress from the caller, or none. Never a fake percentage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Progress {
    #[default]
    Indeterminate,
    /// 0.0..=1.0, clamped.
    Determinate(f32),
}

impl Progress {
    pub fn value(self) -> Option<f32> {
        match self {
            Self::Indeterminate => None,
            Self::Determinate(v) if v.is_nan() => Some(0.0),
            Self::Determinate(v) => Some(v.clamp(0.0, 1.0)),
        }
    }
}

/// Delay, minimum-visible and slow thresholds in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadingPolicy {
    pub delay: u64,
    pub min_visible: u64,
    pub slow: u64,
}

impl Default for LoadingPolicy {
    /// The `duration.loading-delay`, `duration.loading-min-visible` and `duration.slow-notice`
    /// tokens.
    fn default() -> Self {
        use crate::tokens::motion;
        Self {
            delay: u64::from(motion::DURATION_LOADING_DELAY.ms),
            min_visible: u64::from(motion::DURATION_LOADING_MIN_VISIBLE.ms),
            slow: u64::from(motion::DURATION_SLOW_NOTICE.ms),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GateState {
    Idle,
    /// `shown`: carried over when a new load starts while the indicator still lingers.
    Running {
        started: u64,
        shown: Option<u64>,
    },
    /// Finished, but the indicator has not been visible for `min_visible` yet.
    Lingering {
        shown: u64,
    },
}

/// When a loading indicator is visible: only after `delay`, then at least `min_visible`, and a
/// `slow` notice after `slow`. Pure: the adapter passes the clock in and schedules one wake-up
/// at [`LoadingGate::next_change`], so tests drive it with a fixed clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadingGate {
    policy: LoadingPolicy,
    state: GateState,
}

impl LoadingGate {
    pub const fn new(policy: LoadingPolicy) -> Self {
        Self {
            policy,
            state: GateState::Idle,
        }
    }

    pub fn start(&mut self, now: u64) {
        self.state = match self.state {
            GateState::Idle => GateState::Running {
                started: now,
                shown: None,
            },
            running @ GateState::Running { .. } => running,
            GateState::Lingering { shown } => GateState::Running {
                started: now,
                shown: (now < shown + self.policy.min_visible).then_some(shown),
            },
        };
    }

    pub fn finish(&mut self, now: u64) {
        if let GateState::Running { started, shown } = self.state {
            let shown = shown.or_else(|| {
                let at = started + self.policy.delay;
                (now >= at).then_some(at)
            });
            self.state = match shown {
                Some(shown) if now < shown + self.policy.min_visible => {
                    GateState::Lingering { shown }
                }
                _ => GateState::Idle,
            };
        }
    }

    pub const fn running(&self) -> bool {
        matches!(self.state, GateState::Running { .. })
    }

    pub fn visible(&self, now: u64) -> bool {
        match self.state {
            GateState::Idle => false,
            GateState::Running { started, shown } => {
                shown.is_some() || now >= started + self.policy.delay
            }
            GateState::Lingering { shown } => now < shown + self.policy.min_visible,
        }
    }

    pub fn slow(&self, now: u64) -> bool {
        matches!(self.state, GateState::Running { started, .. } if now >= started + self.policy.slow)
    }

    /// The next time `visible` or `slow` can change, if any.
    pub fn next_change(&self, now: u64) -> Option<u64> {
        match self.state {
            GateState::Idle => None,
            GateState::Running { started, shown } => {
                let show = started + self.policy.delay;
                let slow = started + self.policy.slow;
                if shown.is_none() && now < show {
                    Some(show)
                } else {
                    (now < slow).then_some(slow)
                }
            }
            GateState::Lingering { shown } => {
                let hide = shown + self.policy.min_visible;
                (now < hide).then_some(hide)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_vectors as v;

    fn parse_layout(id: &str) -> Result<String, String> {
        id.strip_prefix('!')
            .map_or(Ok(id.to_string()), |r| Err(r.to_string()))
    }

    #[test]
    fn region_layouts_match_the_shared_vectors() {
        assert_eq!(v::REGIONS.len(), 7 * 16);
        for (content, phase, expected) in v::REGIONS {
            let state = RegionState::new(
                Content::parse(content).expect(content),
                Phase::parse(phase).expect(phase),
            );
            assert_eq!(Content::parse(content).unwrap().id(), *content);
            assert_eq!(Phase::parse(phase).unwrap().id(), *phase);
            let actual = state
                .layout()
                .map(RegionLayout::id)
                .map_err(|u| u.id().to_string());
            assert_eq!(actual, parse_layout(expected), "{content} × {phase}");
        }
    }

    #[test]
    fn intents_match_the_shared_vectors_and_unknown_writes_never_retry() {
        for (kind, allowed) in v::INTENTS {
            let kind = ProblemKind::parse(kind).expect(kind);
            for intent in RecoveryIntent::ALL {
                assert_eq!(
                    kind.allows(intent),
                    allowed.contains(&intent.id()),
                    "{kind:?} {intent:?}"
                );
            }
        }
        let uncertain = Problem::fallback(ProblemKind::Uncertain);
        assert_eq!(
            uncertain
                .clone()
                .with_action(RecoveryIntent::Retry, "Gửi lại"),
            Err(ProblemError::IntentNotAllowed {
                kind: ProblemKind::Uncertain,
                intent: RecoveryIntent::Retry,
            })
        );
        let ok = uncertain
            .with_action(RecoveryIntent::Reconcile, "Kiểm tra lại")
            .unwrap();
        assert_eq!(ok.actions.len(), 1);
    }

    #[test]
    fn placements_and_notice_specs_match_the_shared_vectors() {
        for (kind, has_data, writing, expected) in v::PLACEMENTS {
            let (p, a) = placement_for(ProblemKind::parse(kind).unwrap(), *has_data, *writing);
            assert_eq!(
                format!("{}/{}", p.id(), a.id()),
                *expected,
                "{kind} {has_data} {writing}"
            );
        }
        for (placement, tone, announce, dismiss, expected) in v::NOTICES {
            let spec = NoticeSpec::new(
                Placement::parse(placement).unwrap(),
                Tone::parse(tone).unwrap(),
                Announce::parse(announce).unwrap(),
                Dismiss::parse(dismiss).unwrap(),
            );
            let actual = spec.map_or_else(|e| e.id(), |_| "ok");
            assert_eq!(actual, *expected, "{placement} {tone} {announce} {dismiss}");
        }
    }

    #[test]
    fn the_loading_gate_matches_the_shared_vectors() {
        let (delay, min_visible, slow) = v::GATE_POLICY;
        let policy = LoadingPolicy {
            delay,
            min_visible,
            slow,
        };
        assert_eq!(policy, LoadingPolicy::default(), "vectors use the tokens");
        for case in v::GATE {
            let mut gate = LoadingGate::new(policy);
            let mut events = case.events.iter().peekable();
            for &(at, visible, is_slow, next) in case.checks {
                while let Some(&&(what, t)) = events.peek() {
                    if t > at {
                        break;
                    }
                    match what {
                        "start" => gate.start(t),
                        _ => gate.finish(t),
                    }
                    events.next();
                }
                assert_eq!(gate.visible(at), visible, "{} visible @{at}", case.name);
                assert_eq!(gate.slow(at), is_slow, "{} slow @{at}", case.name);
                assert_eq!(gate.next_change(at), next, "{} next @{at}", case.name);
            }
        }
    }

    #[test]
    fn codes_are_stable_identifiers_only() {
        for ok in ["draft.save.conflict", "e503", "net-timeout_2"] {
            assert!(DiagnosticCode::parse(ok).is_some(), "{ok}");
        }
        for bad in [
            "",
            "Draft",
            "at vink::api::save (api.rs:10)",
            "Bearer eyJhbGciOi",
            "-x",
            &"a".repeat(65),
        ] {
            assert!(DiagnosticCode::parse(bad).is_none(), "{bad}");
        }
        let p = Problem::fallback(ProblemKind::Unknown).with_code("{\"error\":\"raw\"}");
        assert_eq!(p.code, None);
    }

    #[test]
    fn panels_only_offer_lawful_actions() {
        let start = RecoveryAction {
            intent: RecoveryIntent::Start,
            label: "Viết bản nháp đầu tiên".into(),
        };
        let clear = RecoveryAction {
            intent: RecoveryIntent::ClearFilter,
            label: "Xóa bộ lọc".into(),
        };
        assert!(PanelContent::empty(EmptyKind::FirstUse, "t", "m", vec![start.clone()]).is_ok());
        assert_eq!(
            PanelContent::empty(EmptyKind::FirstUse, "t", "m", vec![clear.clone()]),
            Err(PanelError::IntentNotAllowed(RecoveryIntent::ClearFilter))
        );
        assert!(PanelContent::empty(EmptyKind::NoResults, "t", "m", vec![clear]).is_ok());
        for kind in ProblemKind::ALL {
            assert!(
                !kind.allows(RecoveryIntent::Start) && !kind.allows(RecoveryIntent::ClearFilter)
            );
        }
        let cancelled = PanelContent::cancelled("Đã dừng", "", Some("Tải lại".into()));
        assert_eq!(cancelled.tone(), Tone::Neutral, "cancelled is never red");
        assert!(!DismissPolicy::Explicit.allows(DismissReason::Escape));
        assert!(DismissPolicy::EscapeOnly.allows(DismissReason::Escape));
        assert!(!DismissPolicy::EscapeOnly.allows(DismissReason::Outside));
        assert!(DismissPolicy::EscapeAndOutside.allows(DismissReason::Outside));
        assert_eq!(SnackbarMessage::new("Đã lưu").dismiss(), Dismiss::Auto);
        assert_eq!(
            SnackbarMessage::new("Đã xóa")
                .with_action("Hoàn tác")
                .dismiss(),
            Dismiss::Manual
        );
    }

    #[test]
    fn progress_is_clamped_and_never_invented() {
        assert_eq!(Progress::Indeterminate.value(), None);
        assert_eq!(Progress::Determinate(1.4).value(), Some(1.0));
        assert_eq!(Progress::Determinate(f32::NAN).value(), Some(0.0));
    }
}
