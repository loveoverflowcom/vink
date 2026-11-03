package vink.ui.preview

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.painter.BitmapPainter
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import java.util.Base64
import org.jetbrains.skia.Image
import vink.ui.ActionState
import vink.ui.AspectRatio
import vink.ui.AvatarKind
import vink.ui.AvatarSize
import vink.ui.ButtonVariant
import vink.ui.ChoiceOption
import vink.ui.ColorRole
import vink.ui.Direction
import vink.ui.FieldMode
import vink.ui.GridMin
import vink.ui.HeadingLevel
import vink.ui.IconName
import vink.ui.ImageState
import vink.ui.LinkVariant
import vink.ui.LocalVink
import vink.ui.Measure
import vink.ui.Person
import vink.ui.Radius
import vink.ui.Space
import vink.ui.SurfaceTone
import vink.ui.TextKind
import vink.ui.TextTone
import vink.ui.ThemePreference
import vink.ui.TypeRole
import vink.ui.VinkAuthorByline
import vink.ui.VinkAvatar
import vink.ui.VinkAvatarGroup
import vink.ui.VinkButton
import vink.ui.VinkCatalog
import vink.ui.VinkCheckbox
import vink.ui.VinkContainer
import vink.ui.VinkDivider
import vink.ui.VinkFilterChip
import vink.ui.VinkFixture
import vink.ui.VinkFormSummary
import vink.ui.VinkGrid
import vink.ui.VinkHeading
import vink.ui.VinkIcon
import vink.ui.VinkIconButton
import vink.ui.VinkIconSource
import vink.ui.VinkIdentityVectors
import vink.ui.VinkImageFrame
import vink.ui.VinkLink
import vink.ui.VinkRadioGroup
import vink.ui.VinkScope
import vink.ui.VinkSearchField
import vink.ui.VinkSelect
import vink.ui.VinkStack
import vink.ui.VinkSurface
import vink.ui.VinkSwitch
import vink.ui.VinkText
import vink.ui.VinkTextArea
import vink.ui.VinkTextField

/** The preview's event log, provided by the workbench; specimens write intents into it. */
val LocalPreviewLog = staticCompositionLocalOf<(String) -> Unit> { {} }

/**
 * Preview adapter, not library code: decodes bytes with Skia. A decode failure (the undecodable
 * AVIF fixture) becomes [ImageState.Failed], so the component falls back without crashing.
 */
fun decodeImage(base64: String): ImageState = try {
    val bytes = Base64.getDecoder().decode(base64)
    ImageState.Ready(BitmapPainter(Image.makeFromEncoded(bytes).toComposeImageBitmap()))
} catch (e: Exception) {
    ImageState.Failed
}

private val sample by lazy { decodeImage(VinkIdentityVectors.SAMPLE_PNG_BASE64) }
private val broken by lazy { decodeImage(VinkIdentityVectors.BROKEN_AVIF_BASE64) }

private fun person(i: Int) = VinkIdentityVectors.people[i]

private fun people(n: Int) = VinkIdentityVectors.people.take(n).map { (key, name) -> Person(key, name) }

private val genres = listOf(
    ChoiceOption("tho", "Thơ"),
    ChoiceOption("van-xuoi", "Văn xuôi"),
    ChoiceOption("tuy-but", "Tùy bút"),
    ChoiceOption("kich", "Kịch (chưa hỗ trợ)", disabled = true),
)

/** UIS-02 catalog IDs registered by this preview. */
val COMPONENT_IDS: List<String> = VinkCatalog.entries.map { it.id }.filterNot { it.startsWith("foundation.") }

/** Renders a UIS-02 specimen; false when the ID is not a component entry. */
@Composable
fun ComponentSpecimen(id: String, scenario: String): Boolean {
    when (id) {
        "text.text" -> TextText(scenario == "long-label")
        "text.heading" -> TextHeading()
        "layout.surface" -> LayoutSurface(scenario == "nested-theme")
        "layout.container" -> LayoutContainer()
        "layout.stack" -> LayoutStack(scenario == "long-label")
        "layout.grid" -> LayoutGrid()
        "layout.divider" -> LayoutDivider()
        "action.button" -> ActionButton(scenario)
        "action.icon-button" -> ActionIconButton(scenario)
        "action.link" -> ActionLink(scenario == "long-label")
        "action.filter-chip" -> ActionChips(scenario)
        "field.text-field" -> FieldText(scenario)
        "field.text-area" -> FieldArea(scenario)
        "field.search-field" -> FieldSearch(scenario == "long-label")
        "field.checkbox" -> FieldCheckbox(scenario)
        "field.radio" -> FieldRadio(scenario)
        "field.switch" -> FieldSwitch(scenario == "disabled")
        "field.select" -> FieldSelect(scenario)
        "field.form" -> FieldForm(scenario == "invalid")
        "identity.icon" -> IdentityIcon()
        "identity.avatar" -> IdentityAvatar(scenario)
        "identity.avatar-group" -> IdentityGroup(scenario == "absent-identity")
        "identity.author-byline" -> IdentityByline(scenario == "long-label")
        "identity.image-frame" -> IdentityFrame(scenario == "image-error")
        "composition.nested" -> Composition(scenario == "narrow")
        else -> return StateSpecimen(id, scenario) || NavigationSpecimen(id, scenario)
    }
    return true
}

@Composable
private fun Note(text: String) = VinkSurface(Modifier.fillMaxWidth(), SurfaceTone.Container, Radius.Md, Space.S3) {
    VinkText(text, tone = TextTone.Muted)
}

@Composable
private fun Caption(text: String) = VinkText(text, role = TypeRole.Code, tone = TextTone.Muted)

@Composable
private fun TextText(long: Boolean) {
    if (long) {
        VinkStack {
            VinkText(VinkFixture.LONG_TITLE, role = TypeRole.Title)
            VinkText(VinkFixture.COMBINING)
            VinkText(VinkFixture.READING, role = TypeRole.Reading)
        }
        return
    }
    VinkStack(gap = Space.S3) {
        TypeRole.entries.forEach { role ->
            Column(Modifier.testTag("text:${role.id}")) {
                Caption(role.role.token)
                VinkText(
                    when (role) {
                        TypeRole.Reading -> VinkFixture.READING
                        TypeRole.Code -> VinkFixture.CODE
                        TypeRole.Display -> VinkFixture.DISPLAY
                        TypeRole.Title -> VinkFixture.TITLE
                        TypeRole.Label -> VinkFixture.LABEL
                        TypeRole.Caption -> VinkFixture.CAPTION
                        else -> VinkFixture.BODY
                    },
                    role = role,
                )
            }
        }
        VinkDivider(decorative = true)
        VinkStack(direction = Direction.Row, gap = Space.S4, wrap = true) {
            TextTone.entries.forEach { VinkText("Tông ${it.id}", tone = it) }
        }
    }
}

@Composable
private fun TextHeading() {
    Note("Compose chỉ có heading() không cấp; cấp chọn kiểu mặc định. Web giữ h1–h6.")
    VinkHeading("h2 · headline — Những trang viết đầu tiên", HeadingLevel.H2, role = TypeRole.Headline)
    VinkHeading("h3 · title (mặc định) — Ghi chép từ phố Hàng Đường", HeadingLevel.H3)
    VinkHeading("h4 · label (mặc định) — Khổ thơ chưa có tên", HeadingLevel.H4)
    VinkHeading("h5 · caption — Cập nhật 09:00", HeadingLevel.H5, role = TypeRole.Caption, tone = TextTone.Muted)
}

@Composable
private fun Tones() {
    VinkGrid(min = GridMin.Small, gap = Space.S3) {
        SurfaceTone.entries.forEach { tone ->
            VinkSurface(Modifier.testTag("surface:${tone.id}"), tone, Radius.Lg, Space.S4) {
                Column {
                    VinkText("Surface ${tone.id}", role = TypeRole.Label)
                    VinkText("Chữ trên nền", role = TypeRole.Caption)
                }
            }
        }
    }
}

@Composable
private fun LayoutSurface(nested: Boolean) {
    if (!nested) return Tones()
    VinkScope(theme = ThemePreference.Dark) {
        Column(Modifier.fillMaxWidth().padding(16.dp).testTag("nested:dark"), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            VinkText("Tối (tường minh)", role = TypeRole.Label)
            Tones()
            VinkScope(theme = ThemePreference.Light) {
                Column(Modifier.fillMaxWidth().padding(16.dp).testTag("nested:light"), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    VinkText("Sáng bên trong Tối", role = TypeRole.Label)
                    Tones()
                }
            }
        }
    }
}

@Composable
private fun LayoutContainer() {
    val outline = LocalVink.current.colors[ColorRole.OutlineVariant]
    Measure.entries.forEach { m ->
        Box(Modifier.fillMaxWidth().border(1.dp, outline, RoundedCornerShape(12.dp)).padding(8.dp).testTag("container:${m.id}")) {
            VinkContainer(measure = m) {
                VinkSurface(Modifier.fillMaxWidth(), SurfaceTone.Container, padding = Space.S3) {
                    Column {
                        VinkText("Container ${m.id}", role = TypeRole.Caption)
                        VinkText(VinkFixture.BODY)
                    }
                }
            }
        }
    }
}

@Composable
private fun LayoutStack(long: Boolean) {
    val log = LocalPreviewLog.current
    val labels = if (long) {
        listOf("Truyện ngắn về mùa thu Hà Nội", "Tùy bút", "Thơ lục bát có dấu tổ hợp", "Phê bình văn học đương đại", "Ký")
    } else {
        listOf("Thơ", "Văn xuôi", "Tùy bút")
    }
    VinkStack(gap = Space.Density) {
        VinkText("Stack dọc · khoảng density.stack-gap", role = TypeRole.Label)
        VinkStack(direction = Direction.Row, gap = Space.S2, wrap = true) {
            labels.forEach { VinkSurface(tone = SurfaceTone.High, radius = Radius.Sm, padding = Space.S2) { VinkText(it) } }
        }
        VinkStack(direction = Direction.Row, gap = Space.S3, wrap = true) {
            VinkButton("Hủy", { log("action.button → Hủy") }, variant = ButtonVariant.Tonal)
            VinkButton(VinkFixture.LABEL, { log("action.button → Lưu bản nháp") })
        }
    }
}

@Composable
private fun LayoutGrid() {
    VinkText("GridMin.Small (160dp × font scale)", role = TypeRole.Label)
    VinkGrid(min = GridMin.Small, gap = Space.S3) {
        (1..6).forEach { VinkSurface(tone = SurfaceTone.Container, padding = Space.S3) { VinkText("Ô $it") } }
    }
    VinkText("GridMin.Medium (240dp × font scale)", role = TypeRole.Label)
    VinkGrid(gap = Space.S4) {
        listOf(VinkFixture.TITLE, VinkFixture.HEADLINE, VinkFixture.LONG_TITLE).forEach {
            VinkSurface(tone = SurfaceTone.Low, radius = Radius.Lg, padding = Space.S4) { VinkText(it, role = TypeRole.Title) }
        }
    }
}

@Composable
private fun LayoutDivider() {
    VinkText("Phần một: bản nháp.")
    VinkDivider()
    VinkText("Phần hai: bản đã gửi.")
    VinkDivider(decorative = true)
    VinkText("Divider trang trí bị bỏ khỏi cây semantics.", tone = TextTone.Muted)
}

@Composable
private fun ActionButton(scenario: String) {
    val log = LocalPreviewLog.current
    var presses by remember { mutableIntStateOf(0) }
    val press: (String) -> () -> Unit = { what ->
        {
            presses++
            log("action.button → $what (lần $presses)")
        }
    }
    Box(Modifier.testTag("press-count")) { VinkText("Đã nhận $presses lần kích hoạt", tone = TextTone.Muted) }
    when (scenario) {
        "disabled" -> VinkStack(direction = Direction.Row, gap = Space.S4, wrap = true) {
            ButtonVariant.entries.forEach {
                VinkButton("Lưu (${it.id})", press("disabled"), Modifier.testTag("disabled:${it.id}"), it, ActionState.Disabled("Chưa có thay đổi để lưu"))
            }
        }
        "pending" -> {
            var state by remember { mutableStateOf<ActionState>(ActionState.Enabled) }
            VinkStack(direction = Direction.Row, gap = Space.S4, wrap = true) {
                VinkButton("Lưu bản nháp", {
                    presses++
                    state = ActionState.Pending("Đang lưu… (chờ xác nhận)")
                    log("action.button → Lưu (lần $presses) → pending")
                }, Modifier.testTag("pending-save"), state = state)
                VinkButton("Mô phỏng hoàn tất", {
                    state = ActionState.Enabled
                    log("action.button → hoàn tất (caller trả Enabled)")
                }, variant = ButtonVariant.Text)
            }
        }
        "long-label" -> VinkStack {
            VinkButton("Tạo bản nháp mới cho tập truyện ngắn về những con phố cũ của Hà Nội", press("long"), leading = IconName.Add)
            VinkButton("Xem trước bài viết", press("long"), variant = ButtonVariant.Outlined)
        }
        else -> VinkStack(direction = Direction.Row, gap = Space.S3, wrap = true) {
            VinkButton("Lưu bản nháp", press("Filled"), Modifier.testTag("press-once"))
            VinkButton("Xem trước", press("Tonal"), variant = ButtonVariant.Tonal, leading = IconName.Visibility)
            VinkButton("Tải .txt", press("Outlined"), variant = ButtonVariant.Outlined)
            VinkButton("Hủy", press("Text"), variant = ButtonVariant.Text)
            VinkButton("Xóa bản nháp", press("Danger"), variant = ButtonVariant.Danger)
        }
    }
}

@Composable
private fun ActionIconButton(scenario: String) {
    val log = LocalPreviewLog.current
    when (scenario) {
        "disabled" -> VinkIconButton(IconName.Edit, "Sửa tiêu đề", { log("icon-button → Sửa (không được gọi)") }, Modifier.testTag("icon-disabled"), ActionState.Disabled("Bản đã gửi không sửa được"))
        "selected" -> {
            var on by remember { mutableStateOf(true) }
            VinkStack(direction = Direction.Row, gap = Space.S3) {
                VinkIconButton(IconName.Visibility, "Xem trước", {
                    on = !on
                    log("icon-button → Xem trước = $on")
                }, Modifier.testTag("icon-toggle"), pressed = on)
                VinkText(if (on) "Xem trước: bật" else "Xem trước: tắt")
            }
        }
        else -> VinkStack(direction = Direction.Row, gap = Space.S2) {
            listOf(IconName.ArrowBack to "Quay lại", IconName.Edit to "Sửa tiêu đề", IconName.Add to "Tạo bản nháp", IconName.Close to "Đóng").forEach { (icon, label) ->
                VinkIconButton(icon, label, { log("icon-button → $label") }, Modifier.testTag("icon:${icon.id}"))
            }
        }
    }
}

@Composable
private fun ActionLink(long: Boolean) {
    val log = LocalPreviewLog.current
    val go: (String) -> Unit = { log("link → điều hướng $it (host)") }
    if (long) {
        VinkContainer {
            VinkLink("Một tiêu đề rất dài để kiểm tra việc xuống dòng của đường link tiếng Việt có dấu ở màn hình hẹp", "/tac-pham/mau-1", go)
        }
        return
    }
    VinkLink("Ghi chép từ phố Hàng Đường", "/tac-pham/mau-1", go, Modifier.testTag("link-inline"))
    VinkLink("Về Bàn viết", "/ban-viet", go, variant = LinkVariant.Standalone)
    VinkLink("Nguồn icon (Apache-2.0)", "https://github.com/google/material-design-icons", go, variant = LinkVariant.Standalone, external = true)
    VinkLink("URL bị từ chối hiển thị như chữ", "javascript:alert(1)", go, Modifier.testTag("link-refused"))
}

@Composable
private fun ActionChips(scenario: String) {
    val log = LocalPreviewLog.current
    val genres = listOf("tho" to "Thơ", "van-xuoi" to "Văn xuôi", "tuy-but" to "Tùy bút", "ky" to "Ký")
    VinkStack(direction = Direction.Row, gap = Space.S2, wrap = true) {
        genres.forEachIndexed { i, (id, label) ->
            var selected by remember { mutableStateOf(scenario == "selected" && i % 2 == 0) }
            VinkFilterChip(
                label,
                selected,
                {
                    selected = it
                    log("filter-chip → $id = $it")
                },
                Modifier.testTag("chip:$id"),
                // One visible reason for the row; the other chips are disabled without repeating it.
                when {
                    scenario != "disabled" -> ActionState.Enabled
                    i == 0 -> ActionState.Disabled("Bộ lọc chưa dùng được")
                    else -> ActionState.Disabled()
                },
            )
        }
    }
}

@Composable
private fun textState(initial: String, field: String): Pair<TextFieldValue, (TextFieldValue) -> Unit> {
    val log = LocalPreviewLog.current
    var value by remember { mutableStateOf(TextFieldValue(initial)) }
    return value to { next ->
        value = next
        log("$field → \"${next.text}\" sel=${next.selection.start}..${next.selection.end}${if (next.composition != null) " (đang soạn IME)" else ""}")
    }
}

@Composable
private fun FieldText(scenario: String) {
    val (initial, mode, error, label) = when (scenario) {
        "invalid" -> listOf("", FieldMode.Editable, "Cần có tiêu đề trước khi gửi.", "Tiêu đề")
        "read-only" -> listOf(VinkFixture.TITLE, FieldMode.ReadOnly, null, "Tiêu đề (bản đã gửi)")
        "disabled" -> listOf(VinkFixture.TITLE, FieldMode.Disabled, null, "Tiêu đề")
        "long-label" -> listOf("0", FieldMode.Editable, null, "Số chương dự kiến cho tập truyện ngắn đầu tay của bạn (có thể để 0)")
        else -> listOf("", FieldMode.Editable, null, "Tiêu đề")
    }
    val (title, setTitle) = textState(initial as String, "title")
    val (email, setEmail) = textState("", "contact")
    VinkTextField(label as String, title, setTitle, Modifier.testTag("field:title"), help = "Có thể sửa sau; Vink không tự cắt khoảng trắng.", error = error as String?, required = true, mode = mode as FieldMode)
    VinkTextField("Email liên hệ", email, setEmail, Modifier.testTag("field:contact"), kind = TextKind.Email)
    Box(Modifier.testTag("title-value")) { VinkText("Giá trị caller giữ: \"${title.text}\" · chọn ${title.selection.start}..${title.selection.end}", tone = TextTone.Muted) }
}

@Composable
private fun FieldArea(scenario: String) {
    val (body, setBody) = textState(if (scenario == "multiline") VinkFixture.READING else "", "body")
    VinkTextArea("Nội dung", body, setBody, Modifier.testTag("field:body"), help = "Dòng trống giữa hai khổ được giữ nguyên.", error = if (scenario == "invalid") "Nội dung trống: thêm ít nhất một dòng." else null, required = true, rows = 6)
    VinkText("${body.text.split('\n').size} dòng", tone = TextTone.Muted)
}

@Composable
private fun FieldSearch(long: Boolean) {
    val log = LocalPreviewLog.current
    val (query, setQuery) = textState(if (long) "Hàng Đường" else "", "search")
    VinkSearchField(
        if (long) "Tìm trong các tác phẩm đã tải theo tiêu đề hoặc tên tác giả, có hoặc không dấu" else "Tìm tác phẩm",
        query,
        setQuery,
        Modifier.testTag("field:search"),
        onSubmit = { log("search → gửi \"$it\"") },
        help = "Phím tìm của bàn phím gửi; không gửi khi đang gõ dấu.",
        placeholder = "Ví dụ: mùa thu",
    )
}

@Composable
private fun FieldCheckbox(scenario: String) {
    val log = LocalPreviewLog.current
    var agree by remember { mutableStateOf(false) }
    var keep by remember { mutableStateOf(true) }
    VinkCheckbox("Tôi xác nhận đây là tác phẩm của mình", agree, {
        agree = it
        log("checkbox agree = $it")
    }, Modifier.testTag("check:agree"), help = "Không thay cho quy định cộng đồng.", error = if (scenario == "invalid") "Cần đồng ý trước khi gửi duyệt." else null, enabled = scenario != "disabled")
    VinkCheckbox("Giữ bản nháp sau khi gửi", keep, {
        keep = it
        log("checkbox keep-draft = $it")
    }, enabled = scenario != "disabled")
}

@Composable
private fun FieldRadio(scenario: String) {
    val log = LocalPreviewLog.current
    var value by remember { mutableStateOf(if (scenario == "default") "tho" else null) }
    VinkRadioGroup("Thể loại", genres, value, {
        value = it
        log("radio genre = $it")
    }, Modifier.testTag("radio:genre"), help = "Giúp người đọc tìm thấy tác phẩm.", error = if (scenario == "invalid") "Chọn một thể loại." else null, required = true, enabled = scenario != "disabled")
}

@Composable
private fun FieldSwitch(disabled: Boolean) {
    val log = LocalPreviewLog.current
    var on by remember { mutableStateOf(false) }
    VinkSwitch("Báo khi có nhận xét mới", on, {
        on = it
        log("switch notify = $it")
    }, Modifier.testTag("switch:notify"), help = "Chỉ là tùy chọn hiển thị trong preview.", enabled = !disabled)
}

@Composable
private fun FieldSelect(scenario: String) {
    val log = LocalPreviewLog.current
    var value by remember { mutableStateOf<String?>(null) }
    VinkSelect("Thể loại", genres, value, {
        value = it
        log("select genre = $it")
    }, Modifier.testTag("select:genre"), placeholder = "Chọn thể loại…", error = if (scenario == "invalid") "Chọn thể loại trước khi gửi." else null, required = true, enabled = scenario != "disabled")
}

@Composable
private fun FieldForm(startInvalid: Boolean) {
    val log = LocalPreviewLog.current
    val (title, setTitle) = textState("", "form-title")
    var genre by remember { mutableStateOf<String?>(null) }
    var agree by remember { mutableStateOf(false) }
    var submitted by remember { mutableStateOf(startInvalid) }
    val errors = buildList {
        if (submitted && title.text.isBlank()) add("form-title" to "Tiêu đề: cần có tiêu đề.")
        if (submitted && genre == null) add("form-genre" to "Thể loại: chọn một thể loại.")
        if (submitted && !agree) add("form-agree" to "Xác nhận: cần đồng ý.")
    }
    fun errorFor(id: String) = errors.firstOrNull { it.first == id }?.second?.substringAfter(": ")
    VinkFormSummary("Cần sửa trước khi gửi", errors, { log("summary → tới $it") }, Modifier.testTag("form-summary"))
    VinkTextField("Tiêu đề", title, setTitle, required = true, error = errorFor("form-title"))
    VinkSelect("Thể loại", genres, genre, { genre = it }, required = true, error = errorFor("form-genre"))
    VinkCheckbox("Tôi xác nhận đây là tác phẩm của mình", agree, { agree = it }, error = errorFor("form-agree"))
    VinkStack(direction = Direction.Row, gap = Space.S3, wrap = true) {
        VinkButton("Gửi duyệt (mẫu)", {
            submitted = true
            log("form → submit")
        }, Modifier.testTag("form-submit"))
        VinkButton("Bỏ kiểm", {
            submitted = false
            log("form → bỏ kiểm")
        }, variant = ButtonVariant.Text)
    }
}

@Composable
private fun IdentityIcon() {
    Note("Nguồn: ${VinkIconSource.SOURCE}")
    VinkGrid(min = GridMin.Small, gap = Space.S2) {
        IconName.entries.forEach { icon ->
            VinkStack(Modifier.testTag("icon-cell:${icon.id}"), Direction.Row, Space.S2) {
                VinkIcon(icon, label = icon.id)
                Caption(icon.id)
            }
        }
    }
}

@Composable
private fun Figure(caption: String, tag: String, content: @Composable () -> Unit) {
    Column(Modifier.widthIn(max = 144.dp).testTag(tag), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        content()
        Caption(caption)
    }
}

@Composable
private fun Avatar(i: Int, size: AvatarSize, image: ImageState = ImageState.Absent, kind: AvatarKind = AvatarKind.Author) {
    val (key, name) = person(i)
    VinkAvatar(name, key, Modifier.testTag("avatar:${key ?: "none"}:${size.id}"), image = image, size = size, kind = kind)
}

@Composable
private fun IdentityAvatar(scenario: String) {
    when (scenario) {
        "image-error" -> {
            Note("Ảnh hợp lệ hiện trên initials; AVIF giải mã lỗi, URL bị từ chối và Failed đều quay về initials.")
            VinkStack(direction = Direction.Row, gap = Space.S6, wrap = true) {
                Figure("Ready PNG", "case:ready") { Avatar(0, AvatarSize.Reader, sample) }
                Figure("AVIF lỗi", "case:broken") { Avatar(1, AvatarSize.Reader, broken) }
                Figure("Failed", "case:failed") { Avatar(5, AvatarSize.Reader, ImageState.Failed) }
                Figure("Loading", "case:loading") { Avatar(0, AvatarSize.Reader, ImageState.Loading) }
            }
        }
        "absent-identity" -> {
            Note("Thiếu tên → dấu người trung tính; thiếu key → tông trung tính. Không suy ra tài khoản từ tên.")
            VinkStack(direction = Direction.Row, gap = Space.S6, wrap = true) {
                Figure("Không tên, không key", "case:none") { VinkAvatar(null, null, size = AvatarSize.Feed) }
                Figure("Có tên, thiếu key", "case:name-only") { VinkAvatar("An Nhiên", null, size = AvatarSize.Feed) }
                Figure("Có key, thiếu tên", "case:key-only") { VinkAvatar(null, "acct-003", size = AvatarSize.Feed, kind = AvatarKind.Account) }
                Figure("Tên chỉ có emoji", "case:emoji") { VinkAvatar("🌸🌸", "acct-001", size = AvatarSize.Feed) }
            }
        }
        "long-label" -> {
            val log = LocalPreviewLog.current
            val order = remember { mutableStateListOf(2, 3, 0, 1) }
            Note("Trần Linh và Trần Lâm cùng initials TL nhưng khác key nên khác tông; đảo thứ tự không đổi màu.")
            VinkButton("Đảo thứ tự", {
                order.reverse()
                log("avatar → đảo thứ tự")
            }, Modifier.testTag("reorder"), variant = ButtonVariant.Tonal)
            VinkStack(direction = Direction.Row, gap = Space.S6, wrap = true) {
                order.forEach { i ->
                    val (key, name) = person(i)
                    Figure("$name · $key", "order:$key") { Avatar(i, AvatarSize.Feed) }
                }
            }
        }
        else -> {
            VinkStack(direction = Direction.Row, gap = Space.S6, wrap = true) {
                listOf(AvatarSize.Small, AvatarSize.Medium, AvatarSize.Spotlight, AvatarSize.Feed, AvatarSize.Reader).forEach {
                    Figure(it.id, "size:${it.id}") { Avatar(0, it) }
                }
            }
            VinkStack(direction = Direction.Row, gap = Space.S4, wrap = true) {
                (0 until 6).forEach { Avatar(it, AvatarSize.Feed) }
            }
        }
    }
}

@Composable
private fun IdentityGroup(unknown: Boolean) {
    if (unknown) {
        Note("Chưa biết tổng: không hiện +N, không hiện 0.")
        VinkAvatarGroup(people(5), "Người đã nhận xét", Modifier.testTag("group-unknown"))
        return
    }
    Note("Tổng 12 do caller cấp: hiện 3 avatar + 9. Cả nhóm có một nhãn, không có điểm dừng focus.")
    VinkAvatarGroup(people(5), "Người đã nhận xét", Modifier.testTag("group-known"), total = 12)
    VinkAvatarGroup(people(4), "Người theo dõi", maxVisible = 4, total = 4, size = AvatarSize.Medium)
}

@Composable
private fun IdentityByline(long: Boolean) {
    val log = LocalPreviewLog.current
    if (long) {
        val (key, name) = person(6)
        VinkContainer { VinkAuthorByline(name!!, key, meta = listOf("Cập nhật 09:00 · 02/11/2026", "Truyện ngắn", "12 phút đọc")) }
        return
    }
    Note("Tên đậm trước metadata; avatar bị bỏ khỏi semantics. Chỉ dòng reader mở hồ sơ (giả định đã xác minh).")
    val (key, name) = person(0)
    VinkAuthorByline(name!!, key, Modifier.testTag("byline:feed"), meta = listOf(VinkFixture.CAPTION, "byline feed"))
    VinkAuthorByline(name, key, Modifier.testTag("byline:spotlight"), meta = listOf(VinkFixture.CAPTION, "byline spotlight"), size = AvatarSize.Spotlight)
    VinkAuthorByline(name, key, Modifier.testTag("byline:reader"), meta = listOf(VinkFixture.CAPTION, "byline reader"), size = AvatarSize.Reader) { log("byline → mở hồ sơ (host)") }
    VinkAuthorByline("  ", null, Modifier.testTag("byline:blank"), meta = listOf("Tên trống được nói rõ"))
}

@Composable
private fun IdentityFrame(errors: Boolean) {
    if (errors) {
        VinkGrid(min = GridMin.Small, gap = Space.S4) {
            VinkImageFrame(broken, "[Mẫu] Bìa tập thơ", Modifier.testTag("frame:broken"))
            VinkImageFrame(ImageState.Failed, "[Mẫu] Bìa tập thơ", Modifier.testTag("frame:failed"))
            VinkImageFrame(ImageState.Absent, "", Modifier.testTag("frame:absent"), AspectRatio.Square)
            VinkImageFrame(ImageState.Loading, "[Mẫu] Ảnh minh họa", Modifier.testTag("frame:loading"), AspectRatio.Wide)
        }
        return
    }
    VinkGrid(gap = Space.S4) {
        VinkImageFrame(sample, "[Mẫu] Ảnh nền lavender", Modifier.testTag("frame:ready"))
        VinkImageFrame(sample, "", ratio = AspectRatio.Wide)
    }
}

@Composable
private fun Composition(narrow: Boolean) {
    val log = LocalPreviewLog.current
    val (title, setTitle) = textState(VinkFixture.TITLE, "compose-title")
    var genre by remember { mutableStateOf<String?>("van-xuoi") }
    var state by remember { mutableStateOf<ActionState>(ActionState.Enabled) }
    val (key, name) = person(0)
    val (otherKey, otherName) = person(1)
    Box(if (narrow) Modifier.widthIn(max = 360.dp).testTag("narrow-frame") else Modifier.fillMaxWidth()) {
        VinkScope(theme = ThemePreference.Dark) {
            Column(Modifier.fillMaxWidth().padding(16.dp).testTag("nested:dark")) {
                VinkSurface(Modifier.fillMaxWidth(), SurfaceTone.Low, Radius.Xl, Space.S6) {
                    VinkStack {
                        VinkAuthorByline(name!!, key, meta = listOf(VinkFixture.CAPTION), narrow = narrow)
                        VinkTextField("Tiêu đề", title, setTitle, required = true)
                        VinkRadioGroup("Thể loại", genres, genre, { genre = it })
                        VinkStack(direction = Direction.Row, gap = Space.S3, wrap = true) {
                            VinkButton("Lưu bản nháp", {
                                state = ActionState.Pending("Đang lưu…")
                                log("composition → Lưu")
                            }, state = state)
                            VinkButton("Đặt lại", { state = ActionState.Enabled }, variant = ButtonVariant.Text)
                        }
                        VinkScope(theme = ThemePreference.Light) {
                            Column(Modifier.fillMaxWidth().padding(16.dp).testTag("nested:light")) {
                                VinkSurface(Modifier.fillMaxWidth(), SurfaceTone.Base, Radius.Lg, Space.S4) {
                                    VinkStack(gap = Space.S2) {
                                        VinkAuthorByline(otherName!!, otherKey, meta = listOf("Thơ"), size = AvatarSize.Spotlight, narrow = narrow)
                                        VinkText(VinkFixture.READING, role = TypeRole.Reading)
                                        VinkAvatarGroup(people(3), "Người đã nhận xét")
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
