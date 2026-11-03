package vink.ui.preview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import vink.ui.ButtonVariant
import vink.ui.CloseReason
import vink.ui.DismissPolicy
import vink.ui.DismissReason
import vink.ui.IconName
import vink.ui.NavContext
import vink.ui.NavDestination
import vink.ui.NavLayout
import vink.ui.OpenResult
import vink.ui.OverlayLayer
import vink.ui.RailPreference
import vink.ui.TextTone
import vink.ui.TypeRole
import vink.ui.VinkAdaptiveScaffold
import vink.ui.VinkBottomNavigation
import vink.ui.VinkButton
import vink.ui.VinkDialog
import vink.ui.VinkDimens
import vink.ui.VinkFixture
import vink.ui.VinkIconButton
import vink.ui.VinkNavUtility
import vink.ui.VinkNavigationRail
import vink.ui.VinkText
import vink.ui.VinkTextArea
import vink.ui.VinkTopBar
import vink.ui.WindowClass

/**
 * UIS-04 specimens (shared/navigation.md) and the diagnostic navigation adapter. There is no
 * native Vink app: [DiagnosticNavigator] is a preview host that owns typed destinations, a back
 * stack, deep links and unsaved text, so the primitives can be checked without a nav controller.
 * Nothing here is native back, predictive back or iOS swipe evidence.
 */

/** Typed destinations mirroring the Web routes. */
sealed interface Route {
    /** The navigation destination this route belongs to; null for NotFound. */
    val key: String?

    data class Explore(val query: String = "") : Route {
        override val key = "explore"
    }

    /** Readers belong to Explore and carry the list's query as return context. */
    data class Reader(val workId: String, val query: String = "") : Route {
        override val key = "explore"
    }

    data object Desk : Route {
        override val key = "desk"
    }

    data class Editor(val draftId: String) : Route {
        override val key = "desk"
    }

    data object Me : Route {
        override val key = "me"
    }

    data class NotFound(val raw: String) : Route {
        override val key: String? = null
    }
}

/** Local deep links only: `/…` paths in the Web app's format; anything else is NotFound. */
object DiagnosticLinks {
    private val ID = Regex("[A-Za-z0-9-]{1,64}")
    private const val UNRESERVED = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~"

    fun encode(text: String): String = buildString {
        for (byte in text.encodeToByteArray()) {
            val c = (byte.toInt() and 0xff).toChar()
            if (c in UNRESERVED) append(c) else append('%').append((byte.toInt() and 0xff).toString(16).uppercase().padStart(2, '0'))
        }
    }

    /** Percent-decoding as a URL query is read (`+` is a space); null for malformed input. */
    fun decode(text: String): String? {
        val bytes = ArrayList<Byte>()
        var i = 0
        while (i < text.length) {
            val c = text[i]
            when {
                c == '%' -> {
                    val hex = text.substring(i + 1, minOf(i + 3, text.length))
                    bytes += (hex.takeIf { it.length == 2 }?.toIntOrNull(16) ?: return null).toByte()
                    i += 3
                }
                c == '+' -> { bytes += ' '.code.toByte(); i++ }
                else -> { bytes += c.toString().encodeToByteArray().toList(); i++ }
            }
        }
        val decoded = bytes.toByteArray().decodeToString()
        return decoded.takeUnless { '�' in it && '�' !in text }
    }

    private fun query(q: String) = if (q.isEmpty()) "" else "?q=${encode(q)}"

    fun link(route: Route): String = when (route) {
        is Route.Explore -> "/${query(route.query)}"
        is Route.Reader -> "/tac-pham/${encode(route.workId)}${query(route.query)}"
        Route.Desk -> "/ban-viet"
        is Route.Editor -> "/ban-viet/${encode(route.draftId)}"
        Route.Me -> "/toi"
        is Route.NotFound -> "/"
    }

    fun parse(raw: String): Route {
        // No scheme, host or scheme-relative link: a deep link never leaves the app.
        if (!raw.startsWith("/") || raw.startsWith("//") || raw.contains('\\')) return Route.NotFound(raw)
        val noHash = raw.substringBefore('#')
        val path = noHash.substringBefore('?')
        val params = noHash.substringAfter('?', "").split('&').filter { it.isNotEmpty() }.map { it.substringBefore('=') to it.substringAfter('=', "") }
        val q = params.firstOrNull { it.first == "q" }?.second?.let { decode(it) ?: return Route.NotFound(raw) } ?: ""
        val segments = path.split('/').filter { it.isNotEmpty() }.map { decode(it) ?: return Route.NotFound(raw) }
        return when {
            segments.isEmpty() -> Route.Explore(q)
            segments.size == 2 && segments[0] == "tac-pham" && ID.matches(segments[1]) -> Route.Reader(segments[1], q)
            segments == listOf("ban-viet") -> Route.Desk
            segments.size == 2 && segments[0] == "ban-viet" && ID.matches(segments[1]) -> Route.Editor(segments[1])
            segments == listOf("toi") -> Route.Me
            else -> Route.NotFound(raw)
        }
    }
}

const val PROFILE = "nav-profile"

/** What a back press did. */
enum class BackResult { ClosedOverlay, Held, Popped, Exit }

/** What a navigation intent did. */
enum class NavResult { Pushed, Held, Ignored }

/**
 * The diagnostic host: one back stack, one overlay layer, unsaved text it owns. Back closes an
 * overlay before it pops; dirty editor text holds a pop or navigation until the person chooses;
 * an intent to the current route adds no duplicate entry.
 */
class DiagnosticNavigator(start: Route = Route.Explore()) {
    val stack = mutableStateListOf(start)
    val overlays = OverlayLayer()
    var draft by mutableStateOf(TextFieldValue(VinkFixture.READING, TextRange(5, 12)))
    var dirty by mutableStateOf(false)
    var pending by mutableStateOf<Route?>(null)
        private set
    var pendingPop by mutableStateOf(false)
        private set

    val current: Route get() = stack.last()

    private val holds: Boolean get() = current is Route.Editor && dirty

    fun navigate(route: Route): NavResult {
        overlays.close(PROFILE, CloseReason.Navigate)
        if (route == current) return NavResult.Ignored
        if (holds) {
            pending = route
            return NavResult.Held
        }
        stack.add(route)
        return NavResult.Pushed
    }

    fun back(): BackResult {
        if (overlays.back() != null) return BackResult.ClosedOverlay
        if (holds) {
            pendingPop = true
            return BackResult.Held
        }
        if (stack.size == 1) return BackResult.Exit
        stack.removeAt(stack.lastIndex)
        return BackResult.Popped
    }

    /** Stay with the text; the held intent is dropped and never runs later. */
    fun stay() {
        pending = null
        pendingPop = false
    }

    /** Leave after an explicit choice. The text stays in host memory; nothing is saved or sent. */
    fun leave() {
        val target = pending
        val pop = pendingPop
        stay()
        dirty = false
        when {
            target != null -> stack.add(target)
            pop && stack.size > 1 -> stack.removeAt(stack.lastIndex)
        }
    }

    fun edit(value: TextFieldValue) {
        if (value.text != draft.text) dirty = true
        draft = value
    }
}

private val explore = NavDestination("explore", "Khám phá", IconName.Explore)
private val desk = NavDestination("desk", "Bàn viết", IconName.Edit)
private val me = NavDestination("me", "Tôi", IconName.Person)
private val exploreLong = NavDestination("explore", "Khám phá những tác phẩm mới công khai", IconName.Explore)
private val deskLong = NavDestination("desk", "Bàn viết của tôi và các bản nháp", IconName.Edit)

private fun Route.label(): String = when (this) {
    is Route.Explore -> if (query.isEmpty()) "Khám phá" else "Khám phá · tìm “$query”"
    is Route.Reader -> "Đọc $workId"
    Route.Desk -> "Bàn viết"
    is Route.Editor -> "Đang viết $draftId"
    Route.Me -> "Tôi"
    is Route.NotFound -> "Không tìm thấy ($raw)"
}

@Composable
private fun Hint(text: String) = VinkText(text, role = TypeRole.Caption, tone = TextTone.Muted)

@Composable
private fun Brand() = VinkText("Vink", role = TypeRole.Headline)

/**
 * A scaffold in a fixed frame driven by [host]. The draft field reads the host's TextFieldValue,
 * so neither the toggle nor a layout change resets text, selection or composition.
 */
@Composable
private fun Shell(
    host: DiagnosticNavigator,
    window: WindowClass,
    context: NavContext = NavContext(),
    long: Boolean = false,
    primary: List<NavDestination> = if (long) listOf(exploreLong, deskLong) else listOf(explore, desk),
    height: Int = 460,
    railOnly: Boolean = false,
    adaptive: Boolean = false,
    page: @Composable () -> Unit = {},
) {
    val log = LocalPreviewLog.current
    var preference by remember { mutableStateOf<RailPreference?>(null) }
    val navigate: (String) -> Unit = { key ->
        val route = when (key) {
            "explore" -> Route.Explore()
            "desk" -> Route.Desk
            else -> Route.Me
        }
        log("điều hướng → $key: ${host.navigate(route).name}")
    }
    BoxWithConstraints(Modifier.fillMaxWidth().testTag("nav-frame")) {
        val measured = maxWidth
        val effectiveWindow = if (adaptive) WindowClass.fromDp(measured.value.toDouble()) else window
        val layout = NavLayout.resolve(effectiveWindow, context, preference)
        val rail: @Composable () -> Unit = {
            VinkNavigationRail(
                expanded = layout.railExpanded,
                onToggle = {
                    layout.nextPreference?.let {
                        preference = it
                        log("toggle → ${it.value} (giữ ${host.current.label()})")
                    }
                },
                destinations = primary,
                selected = host.current.key,
                onNavigate = navigate,
                expandLabel = "Mở rộng điều hướng",
                collapseLabel = "Thu gọn điều hướng",
                modifier = Modifier.testTag("nav-rail"),
                brand = { Brand() },
                account = { compact ->
                    VinkNavUtility(
                        label = if (long) "Tôi và giao diện của tôi" else "Tôi",
                        icon = IconName.Person,
                        open = host.overlays.isOpen(PROFILE),
                        current = host.current is Route.Me,
                        compact = compact,
                        modifier = Modifier.testTag("nav-profile-trigger"),
                        onOpen = {
                            log(
                                when (val r = host.overlays.open(PROFILE)) {
                                    OpenResult.Ok -> "Tôi → mở lớp phủ"
                                    is OpenResult.Busy -> "Tôi → từ chối: ${r.active} đang mở"
                                },
                            )
                        },
                    )
                },
            )
        }
        val canvasWidth = if (railOnly) {
            if (layout.railExpanded) VinkDimens.SizeRailExpanded else VinkDimens.SizeRailCompact
        } else if (adaptive || window == WindowClass.Compact) measured
        else maxOf(measured, (if (window == WindowClass.Expanded) WindowClass.DP.second else WindowClass.DP.first).dp)
        Column {
            Hint("Yêu cầu: ${if (railOnly) "rail riêng" else if (adaptive) "adaptive" else window.id} · hiệu lực: ${layout.id} · frame: ${measured.value} dp")
            // The constrained viewport scrolls a fixed stress canvas; adaptive uses measured dp.
            Box(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).testTag("nav-canvas-scroll")) {
                Box(Modifier.width(canvasWidth).height(height.dp).testTag("nav-shell:${layout.id}")) {
                    if (railOnly) rail() else VinkAdaptiveScaffold(
                        layout = layout,
                        rail = rail,
                        bar = {
                            VinkBottomNavigation(primary + me, host.current.key, navigate, Modifier.testTag("nav-bottom"))
                        },
                        topBar = { VinkTopBar(brand = { Brand() }) },
                    ) {
                        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                            VinkText("Trang hiện tại: ${host.current.label()}", role = TypeRole.Title, modifier = Modifier.testTag("nav-route"))
                            page()
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun Draft(host: DiagnosticNavigator) {
    VinkTextArea("Chữ chưa lưu [Mẫu]", host.draft, host::edit, Modifier.testTag("nav-draft"), rows = 4)
}

@Composable
private fun Profile(host: DiagnosticNavigator) {
    val log = LocalPreviewLog.current
    VinkDialog(
        open = host.overlays.isOpen(PROFILE),
        title = "Tôi",
        context = "Chưa có thông tin tài khoản. Vink chưa mở đăng nhập.",
        dismiss = DismissPolicy.EscapeAndOutside,
        onDismiss = { reason ->
            val closed = host.overlays.close(PROFILE, if (reason == DismissReason.Escape) CloseReason.Escape else CloseReason.Outside)
            log("lớp phủ → đóng (${closed?.reason?.id ?: "muộn, bỏ qua"})")
        },
    ) {
        VinkButton("Mô phỏng Back", { log("Back → ${host.back().name}") }, Modifier.testTag("nav-back"), variant = ButtonVariant.Text)
        VinkButton(
            "Mở hộp thoại khác",
            {
                log(
                    when (val r = host.overlays.open("nav-leave")) {
                        OpenResult.Ok -> "mở hộp thoại khác → mở"
                        is OpenResult.Busy -> "mở hộp thoại khác → từ chối: ${r.active} đang mở"
                    },
                )
            },
            Modifier.testTag("nav-second"),
            variant = ButtonVariant.Text,
        )
        VinkButton("Đóng", { log("lớp phủ → đóng (${host.overlays.close(PROFILE, CloseReason.Dismiss)?.reason?.id})") }, Modifier.testTag("nav-close"))
    }
}

@Composable
private fun Held(host: DiagnosticNavigator) {
    val log = LocalPreviewLog.current
    if (host.pending == null && !host.pendingPop) return
    Column(Modifier.testTag("nav-held"), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        VinkText("Chữ chưa lưu. Ở lại hay rời (chữ vẫn giữ trong bộ nhớ của host)?", role = TypeRole.Body)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            VinkButton("Ở lại", { host.stay(); log("giữ → ở lại") }, Modifier.testTag("nav-stay"), variant = ButtonVariant.Tonal)
            VinkButton("Rời", { host.leave(); log("giữ → rời") }, Modifier.testTag("nav-leave"), variant = ButtonVariant.Text)
        }
    }
}

/** Renders a UIS-04 `navigation.*` specimen; false when the ID is not a navigation entry. */
@Composable
fun NavigationSpecimen(id: String, scenario: String): Boolean {
    val host = remember { DiagnosticNavigator() }
    val log = LocalPreviewLog.current
    when (id) {
        "navigation.rail" -> {
            when (scenario) {
                "reader" -> remember { host.stack[0] = Route.Reader("33333333-3333-4333-8333-333333333333") }
                else -> Unit
            }
            Hint(
                when (scenario) {
                    "compact" -> "Rail 96dp: icon trên, nhãn dưới; nút Mở rộng vẫn hiển thị thường trực."
                    "reader" -> "Trang đọc thuộc Khám phá: Khám phá được chọn, không có đích Trang đọc."
                    "long-labels" -> "Stress rail 96dp với nhãn dài: chữ lớn có thể tách từ; dùng Mở rộng để đọc trong rail 220dp. Không coi đây là PASS khả năng đọc."
                    "guest" -> "Host cấp danh sách theo capability thật; thư viện không tự thêm hay ẩn đích."
                    else -> "Rail 220dp: thương hiệu → nút thu/mở → Khám phá → Bàn viết; Tôi ở chân."
                },
            )
            Shell(
                host,
                window = if (scenario in setOf("compact", "long-labels")) WindowClass.Medium else WindowClass.Expanded,
                long = scenario == "long-labels",
                railOnly = true,
                primary = when (scenario) {
                    "guest" -> listOf(explore)
                    "long-labels" -> listOf(exploreLong, deskLong)
                    else -> listOf(explore, desk)
                },
            )
        }
        "navigation.bottom-bar" -> {
            remember {
                when (scenario) {
                    "desk" -> host.stack[0] = Route.Desk
                    "me" -> host.stack[0] = Route.Me
                    else -> Unit
                }
            }
            Hint("Cửa sổ hẹp: Khám phá → Bàn viết → Tôi; Tôi là màn, không phải lớp phủ.")
            Shell(host, WindowClass.Compact, long = scenario == "long-labels", height = 420)
        }
        "navigation.top-bar" -> {
            Hint("Header hẹp chỉ có thương hiệu và hành động của màn; đích chính ở thanh dưới.")
            VinkTopBar(brand = { Brand() }) {
                if (scenario == "with-action") VinkIconButton(IconName.Search, "Tìm trong Khám phá", { log("top bar → tìm") })
            }
            Shell(host, WindowClass.Compact, height = 360)
        }
        "navigation.scaffold" -> {
            val (window, context) = when (scenario) {
                "medium", "toggle" -> WindowClass.Medium to NavContext()
                "compact" -> WindowClass.Compact to NavContext()
                "editor", "adaptive-editor" -> WindowClass.Expanded to NavContext(compactRail = true, hideBar = true)
                else -> WindowClass.Expanded to NavContext()
            }
            if (scenario == "editor" || scenario == "adaptive-editor") remember { host.stack[0] = Route.Editor("ban-nhap-mau") }
            Hint(
                when (scenario) {
                    "adaptive", "adaptive-editor" -> "Khung thích ứng theo bề rộng frame dp; giữ chữ, vùng chọn và lớp phủ khi đổi cỡ."
                    "medium" -> "Cửa sổ vừa: mặc định rail 96dp."
                    "compact" -> "Cửa sổ hẹp: thanh dưới; preference rail không áp."
                    "editor" -> "Đang viết: rail 96dp trên cửa sổ rộng; trên cửa sổ hẹp thanh dưới ẩn. Bàn viết được chọn."
                    "toggle" -> "Bấm nút: 96 → 220 → 96. Trang, chữ và vùng chọn giữ nguyên; không lưu."
                    "short" -> "Thấp hoặc ngang: rail cuộn, nút thu/mở và Tôi vẫn tới được."
                    else -> "Cửa sổ rộng: rail 220dp."
                },
            )
            if (!scenario.startsWith("adaptive")) Hint("Khung ép cố định để kiểm variant; cuộn ngang để xem canvas khi không vừa. Dùng adaptive để kiểm reflow.")
            Shell(host, window, context, height = if (scenario == "short") 260 else 460, adaptive = scenario.startsWith("adaptive")) {
                if (scenario == "toggle" || scenario == "editor" || scenario.startsWith("adaptive")) Draft(host)
            }
            if (scenario.startsWith("adaptive")) Profile(host)
        }
        "navigation.overlay" -> {
            if (scenario != "default") remember { host.overlays.open(PROFILE) }
            Hint(
                when (scenario) {
                    "open" -> "Tôi đang mở: một modal; Escape/bấm ngoài/Đóng đóng và trả focus."
                    "back" -> "Back khi lớp phủ mở chỉ đóng lớp phủ; Back sau mới rời màn."
                    "busy" -> "Chỉ một modal: mở hộp thoại thứ hai khi Tôi đang mở bị từ chối."
                    else -> "Đóng: bấm Tôi ở chân rail để mở lớp phủ."
                },
            )
            Shell(host, WindowClass.Expanded) {
                VinkButton("Mô phỏng Back", { log("Back → ${host.back().name}") }, Modifier.testTag("nav-back-outside"), variant = ButtonVariant.Tonal)
            }
            Profile(host)
        }
        "navigation.adapter" -> Adapter(host, scenario)
        else -> return false
    }
    return true
}

@Composable
private fun Adapter(host: DiagnosticNavigator, scenario: String) {
    val log = LocalPreviewLog.current
    Hint("Adapter chẩn đoán của preview: đích có kiểu, back stack, deep link và chữ chưa lưu do host giữ. Không phải app native hay bằng chứng Android/iOS back.")
    when (scenario) {
        "deep-link" -> {
            val samples = listOf(
                "/tac-pham/33333333-3333-4333-8333-333333333333?q=%C4%90%C6%B0%E1%BB%9Dng%20v%E1%BB%81",
                "/ban-viet/ban-nhap-mau",
                "/ban-vietnam",
                "//outside.example/toi",
                "https://outside.example/",
                "/?q=%E0%A4%A",
            )
            Column(Modifier.testTag("nav-links"), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                for (raw in samples) {
                    val route = DiagnosticLinks.parse(raw)
                    VinkButton("$raw → ${route.label()}", { log("deep link → ${host.navigate(route).name}: ${route.label()}") }, variant = ButtonVariant.Text)
                }
            }
        }
        "dirty-back" -> {
            remember {
                host.stack.clear()
                host.stack.addAll(listOf(Route.Desk, Route.Editor("ban-nhap-mau")))
            }
            var compact by remember { mutableStateOf(false) }
            VinkButton(
                if (compact) "Đổi sang cửa sổ rộng" else "Đổi sang cửa sổ hẹp",
                { compact = !compact; log("cửa sổ → ${if (compact) "hẹp" else "rộng"}") },
                Modifier.testTag("nav-resize"),
                variant = ButtonVariant.Tonal,
            )
            Shell(host, if (compact) WindowClass.Compact else WindowClass.Expanded, NavContext(compactRail = true, hideBar = true)) {
                Draft(host)
                VinkButton("Mô phỏng Back", { log("Back → ${host.back().name}") }, Modifier.testTag("nav-back-outside"), variant = ButtonVariant.Tonal)
                Held(host)
            }
            return
        }
        else -> Unit
    }
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        VinkButton("Mô phỏng Back", { log("Back → ${host.back().name}") }, Modifier.testTag("nav-back-outside"), variant = ButtonVariant.Tonal)
    }
    VinkText("Back stack: " + host.stack.joinToString(" › ") { DiagnosticLinks.link(it) }, modifier = Modifier.testTag("nav-stack"))
    Shell(host, WindowClass.Expanded)
}
