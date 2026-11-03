package vink.ui.preview

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.TextFieldValue
import vink.ui.ActionPhase
import vink.ui.ActionState
import vink.ui.Announce
import vink.ui.ButtonVariant
import vink.ui.Content
import vink.ui.DialogKind
import vink.ui.DialogRole
import vink.ui.DismissPolicy
import vink.ui.Direction
import vink.ui.EmptyKind
import vink.ui.IndicatorSize
import vink.ui.PanelContent
import vink.ui.Phase
import vink.ui.Problem
import vink.ui.ProblemKind
import vink.ui.Progress
import vink.ui.Radius
import vink.ui.RecoveryAction
import vink.ui.RecoveryIntent
import vink.ui.RegionCopy
import vink.ui.RegionState
import vink.ui.SkeletonShape
import vink.ui.SnackbarMessage
import vink.ui.Space
import vink.ui.SurfaceTone
import vink.ui.TextTone
import vink.ui.Tone
import vink.ui.TypeRole
import vink.ui.VinkBanner
import vink.ui.VinkButton
import vink.ui.VinkContentRegion
import vink.ui.VinkDialog
import vink.ui.VinkEdgeProgress
import vink.ui.VinkFormSummary
import vink.ui.VinkInlineNotice
import vink.ui.VinkListEnd
import vink.ui.VinkLoadingIndicator
import vink.ui.VinkMotionActivity
import vink.ui.VinkSkeleton
import vink.ui.VinkSnackbar
import vink.ui.VinkStack
import vink.ui.VinkStatusBadge
import vink.ui.VinkSurface
import vink.ui.VinkText
import vink.ui.VinkTextField
import vink.ui.placementFor
import vink.ui.rememberLoadingGate
import vink.ui.rememberSnackbarTimeout

/**
 * UIS-03 specimens (shared/states.md) through the public `vink.ui` API. Works and people are
 * [Mẫu]; nothing is fetched, retried or sent: every intent goes to the preview's event log.
 */

private val works = listOf("Mưa trên phố Hàng Bạc [Mẫu]", "Khổ thơ cho mẹ [Mẫu]", "Chuyến tàu đêm Thống Nhất [Mẫu]")

@Composable
private fun Works(count: Int = works.size) = VinkStack(gap = Space.S3) {
    works.take(count).forEach { title ->
        VinkSurface(Modifier.fillMaxWidth(), SurfaceTone.Container, Radius.Md, Space.S4) {
            Column {
                VinkText(title, role = TypeRole.Title)
                VinkText("An Nhiên [Mẫu] · 5 phút đọc", tone = TextTone.Muted)
            }
        }
    }
}

@Composable
private fun Hint(text: String) = VinkText(text, role = TypeRole.Caption, tone = TextTone.Muted)

@Composable
private fun recover(what: String): (RecoveryIntent) -> Unit {
    val log = LocalPreviewLog.current
    return { log("$what → intent ${it.id}") }
}

private fun problem(kind: ProblemKind, vararg actions: Pair<RecoveryIntent, String>, code: String? = null): Problem {
    var p = Problem.fallback(kind)
    for ((intent, label) in actions) p = p.withAction(intent, label) ?: error("${kind.id} refuses ${intent.id}")
    return if (code != null) p.withCode(code) else p
}

private fun emptyPanel(kind: EmptyKind, title: String, message: String, vararg actions: Pair<RecoveryIntent, String>): PanelContent =
    (PanelContent.empty(kind, title, message, actions.map { RecoveryAction(it.first, it.second) }) as PanelContent.Result.Ok).content

/** Renders a UIS-03 `state.*` specimen; false when the ID is not a state entry. */
@Composable
fun StateSpecimen(id: String, scenario: String): Boolean {
    when (id) {
        "state.loading" -> StateLoading(scenario)
        "state.progress" -> if (scenario == "inactive") {
            Hint("Host báo không hoạt động (tab ẩn, ứng dụng ở nền): không vòng lặp nào chạy; nhãn vẫn còn.")
            VinkMotionActivity(false) { StateProgress(determinate = false) }
        } else {
            StateProgress(determinate = scenario == "determinate")
        }
        "state.empty" -> StateEmpty(scenario)
        "state.error" -> StateError(scenario)
        "state.notice" -> StateNotice(scenario)
        "state.status" -> StateStatus(scenario == "long-label")
        "state.dialog" -> StateDialog(scenario)
        "state.playground" -> Playground(scenario)
        else -> return false
    }
    return true
}

@Composable
private fun StateLoading(scenario: String) {
    val onRecover = recover("state.loading")
    val state = when (scenario) {
        "slow" -> RegionState(Content.Absent, Phase.Loading(slow = true))
        "refreshing" -> RegionState(Content.Present(), Phase.Loading())
        else -> RegionState(Content.Absent, Phase.Loading())
    }
    Box(Modifier.testTag("region")) {
        VinkContentRegion(state, "Tác phẩm [Mẫu]", onRecover) { Works() }
    }
    if (scenario == "default" || scenario == "reduced") {
        Hint("Hình dạng skeleton theo nội dung thật: đoạn văn, byline.")
        VinkSkeleton(SkeletonShape.Text(3))
        VinkSkeleton(SkeletonShape.Byline)
    }
    if (scenario == "refreshing") Hint("Làm mới giữ nguyên nội dung, focus và vị trí cuộn; chỉ thêm thanh ở mép trên.")
}

@Composable
private fun StateProgress(determinate: Boolean) {
    var value by remember { mutableFloatStateOf(0.42f) }
    val log = LocalPreviewLog.current
    if (determinate) {
        VinkEdgeProgress(Progress.Determinate(value), "Đang tải ảnh bìa lên: ${(value * 100).toInt()}%", Modifier.testTag("edge-progress"))
        VinkText("Tiến độ thật do caller cấp: ${(value * 100).toInt()}% (không tự tăng).", tone = TextTone.Muted)
        VinkButton("Nhận thêm 10% từ caller", {
            value = (value + 0.1f).coerceAtMost(1f)
            log("state.progress → caller báo ${(value * 100).toInt()}%")
        }, variant = ButtonVariant.Tonal)
        return
    }
    VinkStack(direction = Direction.Row, gap = Space.S4, wrap = true) {
        VinkLoadingIndicator(Modifier.testTag("micro-arc"), label = "Đang lưu bản nháp [Mẫu]", size = IndicatorSize.Medium)
        VinkText("MicroArc có nhãn (24dp)")
    }
    Box(Modifier.testTag("edge-progress")) { VinkEdgeProgress(Progress.Indeterminate, "Đang làm mới danh sách [Mẫu]") }
    Hint("EdgeProgress không giả phần trăm.")
    VinkStack(direction = Direction.Row, gap = Space.S4, wrap = true) {
        VinkButton("Lưu bản nháp", { log("state.progress → bấm khi pending (bị bỏ qua)") }, Modifier.testTag("pending-button"), state = ActionState.Pending("Đang lưu… (chờ xác nhận)"))
        VinkStatusBadge(ActionPhase.Pending, "Đang lưu…")
    }
}

@Composable
private fun StateEmpty(scenario: String) {
    val onRecover = recover("state.empty")
    when (scenario) {
        "no-results" -> VinkContentRegion(
            RegionState(Content.Empty(EmptyKind.NoResults), Phase.Idle), "Kết quả tìm kiếm [Mẫu]", onRecover,
            empty = emptyPanel(EmptyKind.NoResults, "Không có tác phẩm khớp “mưa phùn”", "Thử từ khóa khác hoặc bỏ lọc Thơ.", RecoveryIntent.ClearFilter to "Xóa bộ lọc"),
        ) {}
        "low-content" -> VinkContentRegion(
            RegionState(Content.Present(low = true), Phase.Idle), "Tác phẩm [Mẫu]", onRecover,
            copy = RegionCopy(lowContent = "Mới có 1 tác phẩm được đăng. Bạn có thể là người tiếp theo."),
        ) { Works(1) }
        "end-of-list" -> VinkContentRegion(RegionState(Content.Present(), Phase.Idle), "Tác phẩm [Mẫu]", onRecover) {
            Column {
                Works()
                VinkListEnd()
            }
        }
        else -> VinkContentRegion(
            RegionState(Content.Empty(EmptyKind.FirstUse), Phase.Idle), "Bàn viết [Mẫu]", onRecover,
            empty = emptyPanel(EmptyKind.FirstUse, "Chưa có bản nháp nào", "Bản nháp chỉ mình bạn thấy cho tới khi bạn gửi.", RecoveryIntent.Start to "Viết bản nháp đầu tiên"),
        ) {}
    }
    Hint("Trống không phải lỗi; không bịa bài hay số đếm để lấp chỗ.")
}

@Composable
private fun StateError(scenario: String) {
    val onRecover = recover("state.error")
    val back = RecoveryIntent.Return to "Về Khám phá"
    val (state, p) = when (scenario) {
        "partial" -> RegionState(Content.Present(partial = true), Phase.Idle) to problem(ProblemKind.Unavailable, RecoveryIntent.Retry to "Tải lại phần còn thiếu")
        "forbidden" -> RegionState(Content.Absent, Phase.Failed(ProblemKind.Forbidden)) to problem(ProblemKind.Forbidden, back)
        "not-found" -> RegionState(Content.Absent, Phase.Failed(ProblemKind.NotFound)) to problem(ProblemKind.NotFound, back, code = "read.work.not-found")
        "auth-required" -> RegionState(Content.Absent, Phase.Failed(ProblemKind.AuthRequired)) to problem(ProblemKind.AuthRequired, back)
        "rate-limited" -> RegionState(Content.Absent, Phase.Failed(ProblemKind.RateLimited)) to problem(ProblemKind.RateLimited, RecoveryIntent.Retry to "Thử lại", back)
        "unknown" -> RegionState(Content.Absent, Phase.Failed(ProblemKind.Unknown)) to problem(ProblemKind.Unknown, RecoveryIntent.Retry to "Thử lại", code = "e500")
        "long-label" -> RegionState(Content.Absent, Phase.Failed(ProblemKind.Network)) to Problem(
            ProblemKind.Network,
            "Chưa kết nối được tới thư viện tác phẩm của Vink để tải tập truyện ngắn về những con phố cũ",
            "Không gửi hoặc nhận được dữ liệu trong lúc tải danh sách. Nội dung bạn đang viết ở Bàn viết không bị ảnh hưởng; kiểm tra kết nối mạng của thiết bị rồi thử lại khi sẵn sàng.",
        ).withAction(RecoveryIntent.Retry, "Thử tải lại danh sách tác phẩm")!!.withAction(RecoveryIntent.Return, "Quay về trang Khám phá")!!.withCode("read.works.network")
        else -> RegionState(Content.Absent, Phase.Failed(ProblemKind.Network)) to problem(ProblemKind.Network, RecoveryIntent.Retry to "Thử lại", code = "read.works.network")
    }
    Box(Modifier.testTag("region")) {
        VinkContentRegion(state, "Tác phẩm [Mẫu]", onRecover, problem = p) { Works() }
    }
    when (scenario) {
        "auth-required" -> Hint("Chưa có đăng nhập trong Vink (auth chưa cấu hình): không có nút đăng nhập giả. Nội dung công khai không cần đăng nhập.")
        "rate-limited" -> Hint("Chỉ là fixture: API hiện không trả 429.")
        "unknown" -> Hint("Chưa rõ nguyên nhân: copy không khẳng định thao tác đã thất bại chắc chắn.")
        else -> {}
    }
}

@Composable
private fun StateNotice(scenario: String) {
    val log = LocalPreviewLog.current
    val onRecover = recover("state.notice")
    when (scenario) {
        "banner" -> {
            val (placement, announce) = placementFor(ProblemKind.SessionExpired, hasData = true, writing = false)
            VinkBanner(Tone.Warning, ProblemKind.SessionExpired.fallbackCopy.second, title = ProblemKind.SessionExpired.fallbackCopy.first, announce = announce)
            Hint("placementFor(SessionExpired, đang đọc) = ${placement.id}/${announce.id}; không có nút đăng nhập vì auth chưa cấu hình.")
            var shown by remember { mutableStateOf(true) }
            if (shown) {
                VinkBanner(Tone.Info, "Bản xem trước này dùng dữ liệu [Mẫu]; không gọi dịch vụ.", onDismiss = {
                    shown = false
                    log("state.notice → đóng banner")
                })
            }
        }
        "snackbar" -> {
            var message by remember { mutableStateOf<SnackbarMessage?>(SnackbarMessage("Đã lưu bản nháp [Mẫu].")) }
            rememberSnackbarTimeout(message) {
                message = null
                log("state.notice → snackbar tự đóng")
            }
            VinkStack(direction = Direction.Row, gap = Space.S3, wrap = true) {
                VinkButton("Hiện snackbar (tự đóng)", {
                    message = SnackbarMessage("Đã lưu bản nháp [Mẫu].")
                    log("state.notice → snackbar tự đóng sau 6s")
                }, Modifier.testTag("snackbar-auto"), variant = ButtonVariant.Tonal)
                VinkButton("Hiện snackbar có hành động", {
                    message = SnackbarMessage("Đã chuyển bản nháp vào thùng rác [Mẫu].", "Hoàn tác")
                    log("state.notice → snackbar chờ người dùng")
                }, Modifier.testTag("snackbar-action"), variant = ButtonVariant.Tonal)
            }
            Box(Modifier.testTag("snackbar")) {
                VinkSnackbar(message, onAction = {
                    log("state.notice → snackbar hành động Hoàn tác")
                    message = null
                }, onDismiss = {
                    log("state.notice → đóng snackbar")
                    message = null
                })
            }
            Hint("Snackbar chỉ cho xác nhận ngắn; lỗi, xung đột, hết phiên không bao giờ chỉ nằm ở đây.")
        }
        "field-summary" -> {
            var title by remember { mutableStateOf(TextFieldValue("")) }
            var body by remember { mutableStateOf(TextFieldValue("Khổ đầu chưa xong [Mẫu]\n\n")) }
            val focusTitle = remember { FocusRequester() }
            val focusBody = remember { FocusRequester() }
            VinkFormSummary(
                "Cần sửa 2 chỗ trước khi gửi",
                listOf("title" to "Tiêu đề: cần có tiêu đề.", "body" to "Nội dung: cần ít nhất một khổ hoàn chỉnh."),
                onSelect = {
                    log("state.notice → tóm tắt chọn $it")
                    (if (it == "title") focusTitle else focusBody).requestFocus()
                },
            )
            VinkTextField("Tiêu đề", title, { title = it }, Modifier.focusRequester(focusTitle).testTag("field:title"), error = "Cần có tiêu đề.", required = true)
            VinkTextField("Nội dung", body, { body = it }, Modifier.focusRequester(focusBody), error = "Cần ít nhất một khổ hoàn chỉnh.", required = true)
            val (p, a) = placementFor(ProblemKind.Validation, hasData = true, writing = true)
            Hint("Validation → ${p.id}/${a.id}: lỗi gắn với field, chữ thô được giữ.")
        }
        else -> {
            VinkInlineNotice(Tone.Neutral, "Bản nháp chỉ mình bạn thấy.", onRecover = onRecover)
            VinkInlineNotice(Tone.Info, "Vẫn đang tải, lâu hơn thường lệ. Nội dung sẽ hiện khi xong.", onRecover = onRecover)
            VinkInlineNotice(Tone.Success, "Đã lưu lúc 09:00 (máy chủ xác nhận).", onRecover = onRecover)
            VinkInlineNotice(
                Tone.Warning, ProblemKind.Conflict.fallbackCopy.second, title = ProblemKind.Conflict.fallbackCopy.first,
                action = RecoveryAction(RecoveryIntent.Reconcile, "Đối chiếu"), onRecover = onRecover,
            )
            var shown by remember { mutableStateOf(true) }
            if (shown) {
                VinkInlineNotice(
                    Tone.Error, "Chưa làm mới được danh sách; nội dung cũ vẫn hiển thị.", title = "Chưa kết nối được",
                    action = RecoveryAction(RecoveryIntent.Retry, "Thử lại"), onRecover = onRecover,
                    onDismiss = {
                        shown = false
                        log("state.notice → đóng thông báo lỗi")
                    },
                    modifier = Modifier.testTag("notice-error"),
                )
            }
            Hint("Tone luôn đi kèm icon và chữ; announce: Polite mặc định, không role=alert cho mọi thứ (${Announce.Polite.id}).")
        }
    }
}

private val phases = listOf(
    ActionPhase.Idle to "Chưa có thay đổi",
    ActionPhase.Dirty to "Có thay đổi chưa lưu",
    ActionPhase.Pending to "Đang lưu… (chờ xác nhận)",
    ActionPhase.Confirmed to "Đã lưu (máy chủ xác nhận)",
    ActionPhase.Failed(ProblemKind.Network) to "Lưu chưa được",
    ActionPhase.Cancelled to "Đã dừng; chưa lưu",
    ActionPhase.Uncertain to "Chưa rõ đã gửi chưa",
)

@Composable
private fun StateStatus(long: Boolean) {
    if (long) {
        VinkStatusBadge(ActionPhase.Uncertain, "Chưa rõ bản thảo “Mưa trên phố Hàng Bạc [Mẫu]” đã được gửi tới ban biên tập hay chưa; kiểm tra lại trước khi gửi lần nữa")
        VinkStatusBadge(ActionPhase.Failed(ProblemKind.Conflict), "Bản trên máy chủ đã thay đổi từ thiết bị khác; chữ của bạn vẫn còn ở đây")
        return
    }
    VinkStack(gap = Space.S3) {
        phases.forEach { (phase, label) -> VinkStatusBadge(phase, label, Modifier.testTag("status:${phase.id}")) }
    }
    Hint("Đã xác nhận chỉ hiện sau khi máy chủ xác nhận đúng yêu cầu hiện hành.")
}

@Composable
private fun StateDialog(scenario: String) {
    val log = LocalPreviewLog.current
    var open by remember { mutableStateOf(true) }
    val trigger = remember { FocusRequester() }
    val safe = remember { FocusRequester() }
    var draft by remember { mutableStateOf(TextFieldValue("Khổ đầu đang viết dở [Mẫu]")) }
    if (scenario == "session-expired") {
        VinkTextField("Đang viết [Mẫu]", draft, { draft = it }, Modifier.testTag("dialog-draft"))
    }
    VinkButton("Mở hộp thoại", {
        open = true
        log("state.dialog → mở")
    }, Modifier.focusRequester(trigger).testTag("dialog-trigger"), variant = ButtonVariant.Tonal)
    val close: (String) -> Unit = {
        open = false
        log("state.dialog → $it")
    }
    when (scenario) {
        "session-expired" -> {
            val (title, message) = ProblemKind.SessionExpired.fallbackCopy
            VinkDialog(
                open, title, { close("đóng (${it.name})") },
                role = DialogRole.AlertDialog, dismiss = DismissPolicy.Explicit, context = message,
                initialFocus = safe, returnFocus = trigger,
                content = { Hint("Chưa có đăng nhập trong Vink (auth chưa cấu hình) nên không có nút đăng nhập; trang không tự tải lại.") },
            ) {
                VinkButton("Tải bản sao (.txt)", { log("state.dialog → intent export") }, variant = ButtonVariant.Text)
                VinkButton("Ở lại trang", { close("intent return") }, Modifier.focusRequester(safe).testTag("dialog-safe"), variant = ButtonVariant.Filled)
            }
        }
        "sheet" -> VinkDialog(
            open, "Chia sẻ bản nháp [Mẫu]", { close("đóng (${it.name})") },
            kind = DialogKind.Sheet, dismiss = DismissPolicy.EscapeAndOutside,
            context = "Bản nháp chỉ mình bạn thấy cho tới khi bạn gửi.", initialFocus = safe, returnFocus = trigger,
            content = { Hint("Sheet cùng anatomy với dialog, neo đáy.") },
        ) {
            VinkButton("Đóng", { close("đóng") }, Modifier.focusRequester(safe).testTag("dialog-safe"), variant = ButtonVariant.Outlined)
        }
        else -> VinkDialog(
            open, "Xóa bản nháp [Mẫu]?", { close("đóng (${it.name})") },
            role = DialogRole.AlertDialog, dismiss = DismissPolicy.EscapeOnly,
            context = "“Mưa trên phố Hàng Bạc” sẽ bị xóa khỏi Bàn viết. Không hoàn tác được.",
            initialFocus = safe, returnFocus = trigger,
        ) {
            VinkButton("Giữ bản nháp", { close("giữ") }, Modifier.focusRequester(safe).testTag("dialog-safe"), variant = ButtonVariant.Outlined)
            VinkButton("Xóa", { close("xóa (caller thực hiện)") }, Modifier.testTag("dialog-danger"), variant = ButtonVariant.Danger)
        }
    }
}

/**
 * The preview's ticket reducer for one action (not `EditorSession`): one request at a time, and
 * an acknowledgment or failure only applies to the current ticket.
 */
class Submission {
    private var counter = 0
    var current: Int? = null
        private set

    val pending: Boolean get() = current != null

    /** A new ticket, or null while one is pending (double activation). */
    fun begin(): Int? {
        if (pending) return null
        counter += 1
        current = counter
        return counter
    }

    /** True only for the current ticket; a late answer to an older one is ignored. */
    fun ack(ticket: Int): Boolean = settle(ticket)

    fun fail(ticket: Int): Boolean = settle(ticket)

    fun cancel() {
        current = null
    }

    private fun settle(ticket: Int): Boolean {
        if (current != ticket) return false
        current = null
        return true
    }
}

@Composable
private fun Playground(recipe: String) {
    var generation by remember { mutableIntStateOf(0) }
    key(generation) { PlaygroundRun(recipe) { generation++ } }
}

private fun phaseLabel(phase: ActionPhase): String = when (phase) {
    is ActionPhase.Failed -> if (phase.kind == ProblemKind.Conflict) "Xung đột: chưa lưu" else "Lưu chưa được"
    else -> phases.first { it.first == phase }.second
}

@Composable
private fun PlaygroundRun(recipe: String, reset: () -> Unit) {
    val log = LocalPreviewLog.current
    var clock by remember { mutableLongStateOf(0L) }
    val loadingRecipe = recipe in setOf("default", "loading-empty", "loading-error-retry")
    val actionRecipe = recipe in setOf("pending-cancel", "submit-uncertain", "save-conflict")
    var content by remember { mutableStateOf<Content>(if (loadingRecipe) Content.Absent else Content.Present()) }
    var loading by remember { mutableStateOf(loadingRecipe) }
    var failed by remember { mutableStateOf<ProblemKind?>(null) }
    var failures by remember { mutableIntStateOf(0) }
    var refreshes by remember { mutableIntStateOf(0) }
    var input by remember { mutableStateOf(TextFieldValue("Khổ đầu chưa lưu [Mẫu]")) }
    val submission = remember { Submission() }
    var phase by remember { mutableStateOf<ActionPhase>(ActionPhase.Idle) }
    var reconciled by remember { mutableStateOf(false) }
    var snackbar by remember { mutableStateOf<SnackbarMessage?>(null) }
    var dialog by remember { mutableStateOf(false) }
    val gate = rememberLoadingGate(loading, now = { clock })
    fun note(event: String) = log("+${clock}ms playground/$recipe → $event")

    val phaseNow = when {
        loading -> Phase.Loading(gate.slow)
        failed != null -> Phase.Failed(failed!!)
        else -> Phase.Idle
    }
    val problem = when (failed) {
        ProblemKind.Network -> problem(ProblemKind.Network, RecoveryIntent.Retry to "Thử lại", code = "read.works.network")
        else -> null
    } ?: if (recipe == "refresh-partial") problem(ProblemKind.Unavailable, RecoveryIntent.Retry to "Tải lại phần còn thiếu") else null

    fun startLoading() {
        failed = null
        loading = true
        note("bắt đầu tải")
    }

    fun step() {
        when (recipe) {
            "default" -> if (loading) {
                loading = false
                content = Content.Present()
                note("đã tải xong → Present")
            }
            "loading-empty" -> if (loading) {
                loading = false
                content = Content.Empty(EmptyKind.FirstUse)
                note("đã tải xong → Empty(first-use)")
            }
            "loading-error-retry" -> if (loading) {
                loading = false
                if (failures == 0) {
                    failures++
                    failed = ProblemKind.Network
                    note("tải lỗi → Failed(network)")
                } else {
                    content = Content.Present()
                    note("đã tải xong → Present")
                }
            }
            "refresh-partial" -> when {
                !loading && refreshes == 0 -> {
                    refreshes++
                    loading = true
                    note("làm mới (giữ nội dung)")
                }
                loading -> {
                    loading = false
                    content = if (refreshes == 1) Content.Present(partial = true) else Content.Present()
                    note(if (refreshes == 1) "làm mới xong một phần → Present(partial)" else "đã tải đủ → Present")
                }
            }
            "pending-cancel", "submit-uncertain", "save-conflict" -> {
                val ticket = submission.current ?: return note("bước: không có yêu cầu đang chờ")
                when (recipe) {
                    "pending-cancel" -> if (submission.ack(ticket)) {
                        phase = ActionPhase.Confirmed
                        note("ack ticket #$ticket → Confirmed")
                    }
                    "submit-uncertain" -> if (submission.fail(ticket)) {
                        if (reconciled) {
                            phase = ActionPhase.Confirmed
                            note("ticket #$ticket được xác nhận → Confirmed")
                        } else {
                            phase = ActionPhase.Uncertain
                            note("ticket #$ticket không rõ kết quả → Uncertain (không tự gửi lại)")
                        }
                    }
                    else -> if (submission.fail(ticket)) {
                        phase = ActionPhase.Failed(ProblemKind.Conflict)
                        note("ticket #$ticket → Failed(conflict); chữ giữ nguyên")
                    }
                }
            }
            "session-expired" -> {
                dialog = true
                note("phiên hết hạn (fixture) → dialog")
            }
        }
    }

    val onRecover: (RecoveryIntent) -> Unit = { intent ->
        note("intent ${intent.id}")
        when (intent) {
            RecoveryIntent.Retry -> if (failed != null || content == Content.Present(partial = true)) {
                if (content == Content.Present(partial = true)) refreshes++
                startLoading()
            }
            RecoveryIntent.Reconcile -> if (phase == ActionPhase.Uncertain) {
                reconciled = true
                note("đối chiếu xong: cho phép gửi lại có chủ ý")
            }
            else -> {}
        }
    }

    VinkStack(direction = Direction.Row, gap = Space.S2, wrap = true) {
        Box(Modifier.testTag("playground-clock")) { VinkText("+${clock}ms", role = TypeRole.Code) }
        for (ms in listOf(100L, 1000L, 8000L)) {
            VinkButton("+${ms}ms", {
                clock += ms
                log("+${clock}ms playground/$recipe → đồng hồ +${ms}ms")
            }, Modifier.testTag("playground-advance-$ms"), variant = ButtonVariant.Outlined)
        }
        VinkButton("Bước tiếp", { step() }, Modifier.testTag("playground-step"), variant = ButtonVariant.Tonal)
        VinkButton("Đặt lại", {
            log("playground/$recipe → đặt lại")
            reset()
        }, Modifier.testTag("playground-reset"), variant = ButtonVariant.Text)
    }
    VinkTextField("Ghi chú đang viết [Mẫu]", input, {
        input = it
        if (recipe == "save-conflict" && (phase == ActionPhase.Idle || phase == ActionPhase.Confirmed)) phase = ActionPhase.Dirty
    }, Modifier.testTag("playground-input"))

    if (actionRecipe) {
        val (label, buttonState) = when {
            submission.pending -> (if (recipe == "save-conflict") "Lưu" else if (recipe == "pending-cancel") "Lưu bản nháp" else "Gửi bản thảo") to
                ActionState.Pending("Đang chờ máy chủ xác nhận…")
            recipe == "submit-uncertain" && phase == ActionPhase.Uncertain && !reconciled ->
                "Gửi bản thảo" to ActionState.Disabled("Kiểm tra lại trước khi gửi lần nữa")
            recipe == "submit-uncertain" && reconciled && phase == ActionPhase.Uncertain -> "Gửi lại có chủ ý" to ActionState.Enabled
            recipe == "save-conflict" -> "Lưu" to ActionState.Enabled
            recipe == "pending-cancel" -> "Lưu bản nháp" to ActionState.Enabled
            else -> "Gửi bản thảo" to ActionState.Enabled
        }
        VinkStack(direction = Direction.Row, gap = Space.S3, wrap = true) {
            VinkButton(label, {
                val ticket = submission.begin()
                if (ticket == null) {
                    note("bỏ qua kích hoạt lặp khi đang chờ")
                } else {
                    phase = ActionPhase.Pending
                    note("gửi ticket #$ticket → Pending")
                }
            }, Modifier.testTag("playground-action"), state = buttonState)
            if (recipe == "pending-cancel") {
                VinkButton("Dừng", {
                    if (submission.pending) {
                        submission.cancel()
                        phase = ActionPhase.Cancelled
                        snackbar = SnackbarMessage("Đã dừng lưu; chữ vẫn còn.")
                        note("hủy → Cancelled")
                    }
                }, Modifier.testTag("playground-cancel"), variant = ButtonVariant.Text, state = if (submission.pending) ActionState.Enabled else ActionState.Disabled("Không có thao tác đang chờ"))
            }
            Box(Modifier.testTag("playground-status")) { VinkStatusBadge(phase, phaseLabel(phase)) }
        }
        if (phase == ActionPhase.Uncertain) {
            val (title, message) = ProblemKind.Uncertain.fallbackCopy
            VinkInlineNotice(Tone.Warning, message, title = title, action = if (reconciled) null else RecoveryAction(RecoveryIntent.Reconcile, "Kiểm tra lại"), onRecover = onRecover)
        }
        if (phase == ActionPhase.Failed(ProblemKind.Conflict)) {
            val (title, message) = ProblemKind.Conflict.fallbackCopy
            VinkInlineNotice(Tone.Warning, message, title = title, action = RecoveryAction(RecoveryIntent.Reconcile, "Đối chiếu"), onRecover = onRecover)
            VinkButton("Tải bản sao (.txt)", { onRecover(RecoveryIntent.Export) }, variant = ButtonVariant.Text)
        }
    }
    rememberSnackbarTimeout(snackbar) {
        snackbar = null
        note("snackbar tự đóng")
    }
    Box(Modifier.testTag("playground-snackbar")) { VinkSnackbar(snackbar, onAction = {}, onDismiss = { snackbar = null }) }

    Box(Modifier.testTag("playground-region")) {
        VinkContentRegion(
            RegionState(content, phaseNow), "Tác phẩm [Mẫu]", onRecover,
            problem = problem, indicatorVisible = gate.visible,
            empty = emptyPanel(EmptyKind.FirstUse, "Chưa có tác phẩm nào [Mẫu]", "Tác phẩm được đăng sẽ xuất hiện ở đây.", RecoveryIntent.Start to "Viết bản nháp đầu tiên"),
        ) { Works() }
    }
    Hint("Đồng hồ cố định: chỉ tiến khi bấm. Chỉ báo tải hiện từ 300ms, giữ ≥500ms, gợi ý chậm từ 8000ms.")

    if (recipe == "session-expired") {
        val (title, message) = ProblemKind.SessionExpired.fallbackCopy
        VinkDialog(
            dialog, title, { note("đóng dialog (${it.name})") },
            role = DialogRole.AlertDialog, dismiss = DismissPolicy.Explicit, context = message,
            content = { Hint("Chưa có đăng nhập trong Vink (auth chưa cấu hình): không có nút đăng nhập, không tự tải lại.") },
        ) {
            VinkButton("Tải bản sao (.txt)", { onRecover(RecoveryIntent.Export) }, Modifier.testTag("playground-export"), variant = ButtonVariant.Text)
            VinkButton("Ở lại trang", {
                dialog = false
                onRecover(RecoveryIntent.Return)
            }, Modifier.testTag("playground-stay"))
        }
    }
}
