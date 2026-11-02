package vink.ui

/**
 * The UIS-03 state contract (shared/states.md) as plain data, mirroring the Leptos `state` module:
 * separate axes with lawful combinations, a safe problem projection, notice placement and the
 * [LoadingGate] timing policy. Nothing here fetches, schedules, navigates or decides permission;
 * callers project their own authorities into these types. Ids equal the Rust ones.
 */

/** Empty is not an error: first use invites a start, no-results offers to clear the filter. */
enum class EmptyKind(val id: String) {
    FirstUse("first-use"),
    NoResults("no-results");

    fun allows(intent: RecoveryIntent): Boolean = when (this) {
        FirstUse -> intent == RecoveryIntent.Start || intent == RecoveryIntent.Return
        NoResults -> intent == RecoveryIntent.ClearFilter || intent == RecoveryIntent.Return
    }

    companion object {
        fun parse(id: String): EmptyKind? = entries.firstOrNull { it.id == id }
    }
}

/** What a region holds right now. `low`: little real content; `partial`: a part failed to load. */
sealed interface Content {
    data object Absent : Content
    data class Empty(val kind: EmptyKind) : Content
    data class Present(val low: Boolean = false, val partial: Boolean = false) : Content

    val id: String
        get() = when (this) {
            Absent -> "absent"
            is Empty -> "empty:${kind.id}"
            is Present -> when {
                low && partial -> "present:low+partial"
                low -> "present:low"
                partial -> "present:partial"
                else -> "present"
            }
        }

    companion object {
        fun parse(id: String): Content? = when (id) {
            "absent" -> Absent
            "present" -> Present()
            "present:low" -> Present(low = true)
            "present:partial" -> Present(partial = true)
            "present:low+partial" -> Present(low = true, partial = true)
            else -> id.removePrefix("empty:").takeIf { id.startsWith("empty:") }?.let(EmptyKind::parse)?.let(::Empty)
        }
    }
}

/** The request phase of a region. `slow` comes from [LoadingGate]; it is not a timeout. */
sealed interface Phase {
    data object Idle : Phase
    data class Loading(val slow: Boolean = false) : Phase
    data class Failed(val kind: ProblemKind) : Phase
    data object Cancelled : Phase

    val id: String
        get() = when (this) {
            Idle -> "idle"
            is Loading -> if (slow) "loading:slow" else "loading"
            is Failed -> "failed:${kind.id}"
            Cancelled -> "cancelled"
        }

    companion object {
        fun parse(id: String): Phase? = when (id) {
            "idle" -> Idle
            "loading" -> Loading()
            "loading:slow" -> Loading(slow = true)
            "cancelled" -> Cancelled
            else -> id.removePrefix("failed:").takeIf { id.startsWith("failed:") }?.let(ProblemKind::parse)?.let(::Failed)
        }
    }
}

enum class Tone(val id: String) {
    Neutral("neutral"),
    Info("info"),
    Success("success"),
    Warning("warning"),
    Error("error");

    val icon: IconName
        get() = when (this) {
            Neutral, Info -> IconName.Info
            Success -> IconName.Check
            Warning -> IconName.Warning
            Error -> IconName.Error
        }

    companion object {
        fun parse(id: String): Tone? = entries.firstOrNull { it.id == id }
    }
}

/** Recovery the caller offers; the library only reports which one was chosen. */
enum class RecoveryIntent(val id: String) {
    Retry("retry"),
    Reconcile("reconcile"),
    Reauthenticate("reauthenticate"),
    Return("return"),
    ClearFilter("clear-filter"),
    Export("export"),

    /** Only for an empty first-use panel: begin (write a first draft, explore). */
    Start("start");

    companion object {
        fun parse(id: String): RecoveryIntent? = entries.firstOrNull { it.id == id }
    }
}

data class RecoveryAction(val intent: RecoveryIntent, val label: String)

/**
 * Presentation classes of a failure. The caller classifies; this enum only fixes what each class
 * may offer and where it is shown. `RateLimited` is fixture-only until the API answers 429;
 * `Uncertain` is a write with an unknown outcome (reconcile, never replay); `SessionExpired` needs
 * evidence from the session authority — a lone 401 does not prove it.
 */
enum class ProblemKind(val id: String) {
    Validation("validation"),
    Conflict("conflict"),
    RateLimited("rate-limited"),
    Timeout("timeout"),
    Network("network"),
    Unavailable("unavailable"),
    Uncertain("uncertain"),
    Unknown("unknown"),
    AuthRequired("auth-required"),
    SessionExpired("session-expired"),
    Forbidden("forbidden"),
    NotFound("not-found");

    /** Failures of a write, shown beside the field or action rather than replacing content. */
    val isWrite: Boolean get() = this == Validation || this == Conflict || this == Uncertain

    /** The content can no longer be shown; the caller drops it. */
    val revokesContent: Boolean get() = this == Forbidden || this == NotFound || this == AuthRequired

    fun allows(intent: RecoveryIntent): Boolean =
        when (this) {
            Validation -> false
            Conflict, Uncertain -> intent in setOf(RecoveryIntent.Reconcile, RecoveryIntent.Export, RecoveryIntent.Return)
            RateLimited -> intent == RecoveryIntent.Retry || intent == RecoveryIntent.Return
            Timeout, Network, Unavailable, Unknown -> intent in setOf(RecoveryIntent.Retry, RecoveryIntent.Export, RecoveryIntent.Return)
            AuthRequired -> intent == RecoveryIntent.Reauthenticate || intent == RecoveryIntent.Return
            SessionExpired -> intent in setOf(RecoveryIntent.Reauthenticate, RecoveryIntent.Export, RecoveryIntent.Return)
            Forbidden, NotFound -> intent == RecoveryIntent.Return
        }

    val tone: Tone
        get() = when (this) {
            RateLimited, Uncertain, Conflict, SessionExpired -> Tone.Warning
            AuthRequired, Forbidden, NotFound -> Tone.Neutral
            else -> Tone.Error
        }

    val icon: IconName
        get() = when (this) {
            Validation, Unavailable, Network, Timeout -> IconName.Error
            Conflict, RateLimited -> IconName.Warning
            Uncertain, Unknown -> IconName.Help
            AuthRequired, SessionExpired -> IconName.Lock
            Forbidden -> IconName.Block
            NotFound -> IconName.Search
        }

    /** Default Vietnamese copy; never claims a certain failure for `Unknown`/`Uncertain`. */
    val fallbackCopy: Pair<String, String>
        get() = when (this) {
            Validation -> "Cần sửa vài chỗ" to "Kiểm tra các ô được đánh dấu; chữ bạn đã nhập vẫn còn."
            Conflict -> "Bản trên máy chủ đã thay đổi" to "Chữ của bạn vẫn còn ở đây. Đối chiếu trước khi lưu để không ghi đè."
            RateLimited -> "Thao tác quá nhanh" to "Vui lòng đợi một chút rồi thử lại."
            Timeout -> "Máy chủ chưa trả lời kịp" to "Có thể thử lại; nội dung đang có không bị xóa."
            Network -> "Chưa kết nối được" to "Không gửi hoặc nhận được dữ liệu. Kiểm tra kết nối rồi thử lại."
            Unavailable -> "Dịch vụ tạm thời không sẵn sàng" to "Có thể thử lại sau ít phút."
            Uncertain -> "Chưa rõ thao tác đã hoàn tất hay chưa" to "Kiểm tra lại trước khi gửi lần nữa để không tạo bản trùng."
            Unknown -> "Chưa tải được" to "Chưa rõ nguyên nhân. Có thể thử lại."
            AuthRequired -> "Cần đăng nhập để xem phần này" to "Nội dung công khai vẫn đọc được mà không cần đăng nhập."
            SessionExpired -> "Phiên đăng nhập đã hết hạn" to "Chữ bạn đang viết vẫn còn trên trang này."
            Forbidden -> "Không có quyền xem nội dung này" to "Nội dung này không dành cho tài khoản hiện tại."
            NotFound -> "Không tìm thấy nội dung" to "Có thể nội dung đã được gỡ hoặc đường dẫn không đúng."
        }

    companion object {
        fun parse(id: String): ProblemKind? = entries.firstOrNull { it.id == id }
    }
}

/**
 * A stable diagnostic code (`[a-z0-9][a-z0-9._-]{0,63}`). Anything else is dropped rather than
 * shown, so a raw message, path or token cannot pass as a code.
 */
class DiagnosticCode private constructor(val value: String) {
    override fun equals(other: Any?) = other is DiagnosticCode && other.value == value
    override fun hashCode() = value.hashCode()
    override fun toString() = value

    companion object {
        private fun lowerOrDigit(c: Char) = c in 'a'..'z' || c in '0'..'9'

        fun parse(code: String): DiagnosticCode? {
            if (code.isEmpty() || code.length > 64 || !lowerOrDigit(code[0])) return null
            if (!code.all { lowerOrDigit(it) || it == '.' || it == '_' || it == '-' }) return null
            return DiagnosticCode(code)
        }
    }
}

/**
 * The safe projection of a failure: localized copy, an optional stable code and the recovery
 * actions the caller offers. There is no field for a payload, stack trace or token.
 */
data class Problem(
    val kind: ProblemKind,
    val title: String,
    val message: String,
    val code: DiagnosticCode? = null,
    val actions: List<RecoveryAction> = emptyList(),
) {
    /** Keeps the code only when it is a valid [DiagnosticCode]. */
    fun withCode(code: String): Problem = copy(code = DiagnosticCode.parse(code))

    /** Null when the kind does not allow [intent] (for example `Retry` after an uncertain write). */
    fun withAction(intent: RecoveryIntent, label: String): Problem? =
        if (kind.allows(intent)) copy(actions = actions + RecoveryAction(intent, label)) else null

    companion object {
        fun fallback(kind: ProblemKind): Problem = Problem(kind, kind.fallbackCopy.first, kind.fallbackCopy.second)
    }
}

/** Why a region state cannot be shown. */
enum class Unlawful(val id: String) {
    /** No content and no request: the caller has to start loading. */
    NotRequested("not-requested"),

    /** Validation, conflict and uncertain writes belong beside the field or action. */
    WriteProblem("write-problem"),

    /** Forbidden, not found or auth required with content still held: drop the content first. */
    Revoked("revoked"),
}

sealed interface PanelKind {
    data class Error(val kind: ProblemKind) : PanelKind
    data object Cancelled : PanelKind
    data class Empty(val kind: EmptyKind) : PanelKind
}

sealed interface Body {
    data object Skeleton : Body
    data class Panel(val kind: PanelKind) : Body

    /** The caller's content stays composed (focus, scroll, selection kept). */
    data object Content : Body
}

sealed interface RegionNotice {
    data object Slow : RegionNotice
    data class Refresh(val kind: ProblemKind) : RegionNotice
    data object Partial : RegionNotice
    data object LowContent : RegionNotice
}

data class RegionLayout(val body: Body, val progress: Boolean, val notice: RegionNotice?) {
    /** The vector form `body|progress|notice`. */
    val id: String
        get() {
            val b = when (body) {
                Body.Skeleton -> "skeleton"
                Body.Content -> "content"
                is Body.Panel -> when (val k = body.kind) {
                    PanelKind.Cancelled -> "panel:cancelled"
                    is PanelKind.Empty -> "panel:empty:${k.kind.id}"
                    is PanelKind.Error -> "panel:error:${k.kind.id}"
                }
            }
            val n = when (notice) {
                null -> "-"
                RegionNotice.Slow -> "slow"
                RegionNotice.Partial -> "partial"
                RegionNotice.LowContent -> "low-content"
                is RegionNotice.Refresh -> "refresh:${notice.kind.id}"
            }
            return "$b|${if (progress) "+" else "-"}|$n"
        }
}

/** A layout, or why the state is unlawful. */
sealed interface RegionResult {
    data class Lawful(val layout: RegionLayout) : RegionResult
    data class Rejected(val reason: Unlawful) : RegionResult
}

data class RegionState(val content: Content, val phase: Phase) {
    /** The lawful combinations of shared/states.md. */
    fun layout(): RegionResult {
        val absent = content == Content.Absent
        val (body, quiet) = when (val c = content) {
            Content.Absent -> Body.Skeleton to null
            is Content.Empty -> Body.Panel(PanelKind.Empty(c.kind)) to null
            is Content.Present -> Body.Content to when {
                c.partial -> RegionNotice.Partial
                c.low -> RegionNotice.LowContent
                else -> null
            }
        }
        fun ok(body: Body, progress: Boolean, notice: RegionNotice?) = RegionResult.Lawful(RegionLayout(body, progress, notice))
        return when (val p = phase) {
            Phase.Idle -> if (absent) RegionResult.Rejected(Unlawful.NotRequested) else ok(body, false, quiet)
            is Phase.Loading -> {
                val slow = if (p.slow) RegionNotice.Slow else null
                if (absent) ok(Body.Skeleton, false, slow) else ok(body, true, slow ?: quiet)
            }
            Phase.Cancelled -> if (absent) ok(Body.Panel(PanelKind.Cancelled), false, null) else ok(body, false, quiet)
            is Phase.Failed -> when {
                p.kind.isWrite -> RegionResult.Rejected(Unlawful.WriteProblem)
                absent -> ok(Body.Panel(PanelKind.Error(p.kind)), false, null)
                p.kind.revokesContent -> RegionResult.Rejected(Unlawful.Revoked)
                else -> ok(body, false, RegionNotice.Refresh(p.kind))
            }
        }
    }
}

enum class Placement(val id: String) {
    Field("field"),
    Inline("inline"),
    Region("region"),
    Banner("banner"),
    Snackbar("snackbar"),
    Dialog("dialog");

    companion object {
        fun parse(id: String): Placement? = entries.firstOrNull { it.id == id }
    }
}

/** How a live region announces a change. Explicit: there is no "alert everything". */
enum class Announce(val id: String) {
    Off("off"),
    Polite("polite"),
    Assertive("assertive");

    companion object {
        fun parse(id: String): Announce? = entries.firstOrNull { it.id == id }
    }
}

/** What ends a notice: the owning state changing, the person, or (snackbar only) a timeout. */
enum class Dismiss(val id: String) {
    State("state"),
    Manual("manual"),
    Auto("auto");

    companion object {
        fun parse(id: String): Dismiss? = entries.firstOrNull { it.id == id }
    }
}

/** Where a problem is shown and how it is announced (shared/states.md). */
fun placementFor(kind: ProblemKind, hasData: Boolean, writing: Boolean): Pair<Placement, Announce> = when {
    kind == ProblemKind.Validation -> Placement.Field to Announce.Off
    kind == ProblemKind.Conflict || kind == ProblemKind.Uncertain -> Placement.Inline to Announce.Polite
    kind == ProblemKind.SessionExpired && writing -> Placement.Dialog to Announce.Off
    kind == ProblemKind.SessionExpired -> Placement.Banner to Announce.Assertive
    kind.revokesContent -> Placement.Region to Announce.Polite
    hasData || writing -> Placement.Inline to Announce.Polite
    else -> Placement.Region to Announce.Polite
}

enum class NoticeError(val id: String) {
    /** A snackbar disappears; it cannot carry an error or warning. */
    SnackbarTone("snackbar-tone"),
    Announce("announce"),
    Dismiss("dismiss"),
}

/** A validated combination of placement, tone, announcement and dismissal. */
@ConsistentCopyVisibility
data class NoticeSpec private constructor(val placement: Placement, val tone: Tone, val announce: Announce, val dismiss: Dismiss) {
    sealed interface Result {
        data class Ok(val spec: NoticeSpec) : Result
        data class Rejected(val error: NoticeError) : Result
    }

    companion object {
        fun create(placement: Placement, tone: Tone, announce: Announce, dismiss: Dismiss): Result {
            if (placement == Placement.Snackbar && (tone == Tone.Error || tone == Tone.Warning)) {
                return Result.Rejected(NoticeError.SnackbarTone)
            }
            val announceOk = when (placement) {
                Placement.Field, Placement.Dialog -> announce == vink.ui.Announce.Off
                Placement.Snackbar -> announce == vink.ui.Announce.Polite
                Placement.Inline, Placement.Region, Placement.Banner -> announce != vink.ui.Announce.Off
            }
            if (!announceOk) return Result.Rejected(NoticeError.Announce)
            val dismissOk = when (placement) {
                Placement.Snackbar -> dismiss != vink.ui.Dismiss.State
                Placement.Field, Placement.Region -> dismiss == vink.ui.Dismiss.State
                Placement.Inline, Placement.Banner, Placement.Dialog -> dismiss != vink.ui.Dismiss.Auto
            }
            if (!dismissOk) return Result.Rejected(NoticeError.Dismiss)
            return Result.Ok(NoticeSpec(placement, tone, announce, dismiss))
        }
    }
}

/**
 * The interaction phase of one action, as its owner (for example `EditorSession`) reports it.
 * `Confirmed` is only for an acknowledgment of the current request.
 */
sealed interface ActionPhase {
    data object Idle : ActionPhase
    data object Dirty : ActionPhase
    data object Pending : ActionPhase
    data object Confirmed : ActionPhase
    data class Failed(val kind: ProblemKind) : ActionPhase
    data object Cancelled : ActionPhase
    data object Uncertain : ActionPhase

    val id: String
        get() = when (this) {
            Idle -> "idle"
            Dirty -> "dirty"
            Pending -> "pending"
            Confirmed -> "confirmed"
            is Failed -> "failed"
            Cancelled -> "cancelled"
            Uncertain -> "uncertain"
        }

    val tone: Tone
        get() = when (this) {
            Idle, Pending, Cancelled -> Tone.Neutral
            Dirty -> Tone.Info
            Confirmed -> Tone.Success
            is Failed -> kind.tone
            Uncertain -> Tone.Warning
        }
}

/** Real progress from the caller, or none. Never a fake percentage. */
sealed interface Progress {
    data object Indeterminate : Progress

    /** 0..1, clamped. */
    data class Determinate(val fraction: Float) : Progress

    val value: Float?
        get() = when (this) {
            Indeterminate -> null
            is Determinate -> if (fraction.isNaN()) 0f else fraction.coerceIn(0f, 1f)
        }
}

/** Delay, minimum-visible and slow thresholds in milliseconds; defaults are the duration tokens. */
data class LoadingPolicy(
    val delay: Long = VinkMotionTokens.DurationLoadingDelayMs.toLong(),
    val minVisible: Long = VinkMotionTokens.DurationLoadingMinVisibleMs.toLong(),
    val slow: Long = VinkMotionTokens.DurationSlowNoticeMs.toLong(),
)

/**
 * When a loading indicator is visible: only after `delay`, then at least `minVisible`, and a
 * `slow` notice after `slow`. Pure: the adapter passes the clock in and schedules one wake-up at
 * [nextChange], so tests drive it with a fixed clock. Mutable, like the Rust gate.
 */
class LoadingGate(val policy: LoadingPolicy = LoadingPolicy()) {
    private sealed interface State {
        data object Idle : State

        /** `shown`: carried over when a new load starts while the indicator still lingers. */
        data class Running(val started: Long, val shown: Long?) : State

        /** Finished, but the indicator has not been visible for `minVisible` yet. */
        data class Lingering(val shown: Long) : State
    }

    private var state: State = State.Idle

    fun start(now: Long) {
        state = when (val s = state) {
            State.Idle -> State.Running(now, null)
            is State.Running -> s
            is State.Lingering -> State.Running(now, s.shown.takeIf { now < it + policy.minVisible })
        }
    }

    fun finish(now: Long) {
        val s = state as? State.Running ?: return
        val shown = s.shown ?: (s.started + policy.delay).takeIf { now >= it }
        state = if (shown != null && now < shown + policy.minVisible) State.Lingering(shown) else State.Idle
    }

    val running: Boolean get() = state is State.Running

    fun visible(now: Long): Boolean = when (val s = state) {
        State.Idle -> false
        is State.Running -> s.shown != null || now >= s.started + policy.delay
        is State.Lingering -> now < s.shown + policy.minVisible
    }

    fun slow(now: Long): Boolean = (state as? State.Running)?.let { now >= it.started + policy.slow } ?: false

    /** The next time [visible] or [slow] can change, if any. */
    fun nextChange(now: Long): Long? = when (val s = state) {
        State.Idle -> null
        is State.Running -> {
            val show = s.started + policy.delay
            val slow = s.started + policy.slow
            if (s.shown == null && now < show) show else slow.takeIf { now < it }
        }
        is State.Lingering -> (s.shown + policy.minVisible).takeIf { now < it }
    }
}

sealed interface PanelError {
    data class IntentNotAllowed(val intent: RecoveryIntent) : PanelError
}

/**
 * What a [VinkStatePanel] shows: symbol, heading, explanation, actions (primary first) and an
 * optional code. Build it with [fromProblem], [empty] or [cancelled] so the actions stay lawful.
 */
@ConsistentCopyVisibility
data class PanelContent internal constructor(
    val kind: PanelKind,
    val title: String,
    val message: String,
    val actions: List<RecoveryAction>,
    val code: DiagnosticCode?,
) {
    sealed interface Result {
        data class Ok(val content: PanelContent) : Result
        data class Rejected(val error: PanelError) : Result
    }

    val tone: Tone
        get() = when (kind) {
            is PanelKind.Error -> kind.kind.tone
            PanelKind.Cancelled, is PanelKind.Empty -> Tone.Neutral
        }

    val icon: IconName
        get() = when (kind) {
            is PanelKind.Empty -> if (kind.kind == EmptyKind.FirstUse) IconName.Edit else IconName.Search
            PanelKind.Cancelled -> IconName.Close
            is PanelKind.Error -> kind.kind.icon
        }

    companion object {
        fun fromProblem(problem: Problem): PanelContent =
            PanelContent(PanelKind.Error(problem.kind), problem.title, problem.message, problem.actions, problem.code)

        fun empty(kind: EmptyKind, title: String, message: String, actions: List<RecoveryAction> = emptyList()): Result {
            actions.firstOrNull { !kind.allows(it.intent) }?.let { return Result.Rejected(PanelError.IntentNotAllowed(it.intent)) }
            return Result.Ok(PanelContent(PanelKind.Empty(kind), title, message, actions, null))
        }

        /** A stopped load is neutral, never red; reloading is the caller's `Retry`. */
        fun cancelled(title: String, message: String, retry: String? = null): PanelContent =
            PanelContent(PanelKind.Cancelled, title, message, listOfNotNull(retry?.let { RecoveryAction(RecoveryIntent.Retry, it) }), null)
    }
}

/** Copy a [VinkContentRegion] uses for its own notices; the defaults are Vietnamese. */
data class RegionCopy(
    /** Status while the skeleton shows. */
    val loading: String = "Đang tải…",
    val slow: String = "Vẫn đang tải, lâu hơn thường lệ. Nội dung sẽ hiện khi xong.",
    val partial: String = "Một phần chưa tải được; phần còn lại vẫn dùng được.",
    val lowContent: String = "Chưa có nhiều nội dung ở đây.",
    val cancelledTitle: String = "Đã dừng tải",
    val cancelledMessage: String = "Không có gì bị thay đổi.",
    val reload: String = "Tải lại",
)

/** The geometry a skeleton imitates, in the reading order of the real content. */
sealed interface SkeletonShape {
    val id: String

    /** Lines of running text; the last one is shorter. */
    data class Text(val lines: Int) : SkeletonShape {
        override val id get() = "text"
    }

    /** A work card: title, byline, two lines of excerpt. */
    data object Card : SkeletonShape {
        override val id get() = "card"
    }

    /** Avatar and two short lines. */
    data object Byline : SkeletonShape {
        override val id get() = "byline"
    }
}

enum class IndicatorSize {
    /** 20dp, inside a button's status slot. */
    Small,

    /** 24dp, beside a status label. */
    Medium,
}

/** One short message that needs no decision, with at most one action. */
data class SnackbarMessage(val text: String, val action: String? = null) {
    /** Without an action it closes after `duration.snackbar`; with one it waits for the person. */
    val dismiss: Dismiss get() = if (action != null) Dismiss.Manual else Dismiss.Auto
}

enum class DialogRole {
    Dialog,

    /** Interrupts for a decision (destructive confirmation, session expired while writing). */
    AlertDialog,
}

enum class DialogKind {
    Dialog,

    /** Same anatomy, anchored to the bottom edge. */
    Sheet,
}

enum class DismissReason { Escape, Outside }

/** Which gestures may close a dialog without choosing an action. */
enum class DismissPolicy {
    /** Only the dialog's own actions: a decision is required. */
    Explicit,
    EscapeOnly,
    EscapeAndOutside;

    fun allows(reason: DismissReason): Boolean = when (reason) {
        DismissReason.Escape -> this != Explicit
        DismissReason.Outside -> this == EscapeAndOutside
    }
}
