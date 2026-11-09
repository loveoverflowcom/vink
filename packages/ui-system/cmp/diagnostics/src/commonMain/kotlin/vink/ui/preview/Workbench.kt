package vink.ui.preview

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import vink.ui.nfc
import kotlin.math.roundToInt
import vink.ui.CatalogEntry
import vink.ui.ColorRole
import vink.ui.Density
import vink.ui.LocalMotionActive
import vink.ui.LocalVink
import vink.ui.MotionPreference
import vink.ui.ThemePreference
import vink.ui.VinkCatalog
import vink.ui.VinkContrastPairs
import vink.ui.VinkDensityDimens
import vink.ui.VinkDimens
import vink.ui.VinkFixture
import vink.ui.VinkMotionTokens
import vink.ui.VinkPreferences
import vink.ui.VinkScope
import vink.ui.VinkScopeState
import vink.ui.VinkTextRole
import vink.ui.VinkTextRoles
import vink.ui.VinkTheme
import vink.ui.VinkTokens
import vink.ui.contrast

/**
 * The CMP preview workbench. Region tags follow the shared LayoutSpec: `catalog-nav`,
 * `preview-controls`, `specimen`, `inspector`. State lives in memory; nothing is persisted and
 * no service is called. Specimens render through the library's public VinkTheme/VinkScope.
 */

/** Catalog IDs this preview renders; a test keeps it equal to the shared catalog. */
val REGISTERED = listOf("foundation.color-roles", "foundation.typography", "foundation.shape-spacing", "foundation.motion") + listOf(
    "text.text", "text.heading", "layout.surface", "layout.container", "layout.stack", "layout.grid", "layout.divider",
    "action.button", "action.icon-button", "action.link", "action.filter-chip", "action.toggle-action",
    "field.text-field", "field.text-area", "field.search-field", "field.checkbox", "field.radio", "field.switch", "field.select", "field.form",
    "identity.icon", "identity.avatar", "identity.avatar-group", "identity.author-byline", "identity.image-frame",
    "state.loading", "state.progress", "state.empty", "state.error", "state.notice", "state.status", "state.dialog", "state.playground",
    "navigation.rail", "navigation.bottom-bar", "navigation.top-bar", "navigation.scaffold", "navigation.overlay", "navigation.adapter",
    "composition.nested",
    "screen.explore", "screen.reader", "screen.desk", "screen.editor", "screen.me", "composition.feedback",
    "composition.endorsement", "composition.tag-selector",
)

enum class Frame(val label: String, val width: Dp?) {
    Fluid("Tự do", null),
    Narrow("Hẹp 360dp", 360.dp),
    Tablet("Tablet 834dp", 834.dp),
    Wide("Rộng 1280dp", 1280.dp),
}

data class PreviewConfig(
    val theme: ThemePreference = ThemePreference.System,
    val frame: Frame = Frame.Fluid,
    val textScale: Float = 1f,
    val motion: MotionPreference = MotionPreference.System,
    val query: String = "",
    val entry: String = VinkCatalog.entries.first().id,
    val scenario: String = "default",
    /** Null follows the platform; captures and tests fix it. */
    val systemDark: Boolean? = null,
    val systemReducedMotion: Boolean = false,
    /** Host activity: the desktop window is shown and not minimized (Main.kt maps it). */
    val hostActive: Boolean = true,
)

class PreviewModel(initial: PreviewConfig = PreviewConfig()) {
    var config by mutableStateOf(initial)
        private set
    val log = mutableStateListOf<String>()

    fun update(event: String, change: (PreviewConfig) -> PreviewConfig) {
        config = change(config)
        record(event)
    }

    /** An intent from a specimen; nothing is sent anywhere. */
    fun record(event: String) {
        log.add(0, "${VinkFixture.CLOCK} · #${log.size + 1} $event")
    }
}

/** Case- and diacritic-insensitive key: "Màu" matches "mau", Đ matches D. */
fun fold(text: String): String = buildString {
    val groups = listOf("àáảãạăằắẳẵặâầấẩẫậ", "èéẻẽẹêềếểễệ", "ìíỉĩị", "òóỏõọôồốổỗộơờớởỡợ", "ùúủũụưừứửữự", "ỳýỷỹỵ", "đ")
    for (c in nfc(text).lowercase()) {
        if (c in '\u0300'..'\u036f') continue
        val group = groups.indexOfFirst { c in it }
        append(if (group >= 0) "aeiouyd"[group] else c)
    }
}

/** Host reports its actual font boundary rather than claiming desktop fonts on mobile. */
data class LoadedFonts(val fonts: vink.ui.VinkFonts, val status: String, val loaded: List<String>)
private fun decimal(value: Double): String {
    val n = (value * 100).roundToInt()
    return "${n / 100}.${(n % 100).toString().padStart(2, '0')}"
}

fun matches(entry: CatalogEntry, query: String): Boolean {
    val q = fold(query.trim())
    if (q.isEmpty()) return true
    val fields = listOf(entry.id, entry.family, entry.label, entry.summary) + entry.scenarios.flatMap { listOf(it.id, it.label) }
    return fields.any { fold(it).contains(q) }
}

@Composable
fun Workbench(model: PreviewModel, fonts: LoadedFonts, scroll: Boolean = true) {
    val config = model.config
    val base = LocalDensity.current
    val preferences = VinkPreferences(theme = config.theme, motion = config.motion)
    val themed: @Composable (@Composable () -> Unit) -> Unit = { body ->
        if (config.systemDark == null) {
            VinkTheme(preferences, systemReducedMotion = config.systemReducedMotion, fonts = fonts.fonts, content = body)
        } else {
            VinkTheme(preferences, systemDark = config.systemDark, systemReducedMotion = config.systemReducedMotion, fonts = fonts.fonts, content = body)
        }
    }
    themed {
        CompositionLocalProvider(
            LocalDensity provides androidx.compose.ui.unit.Density(base.density, base.fontScale * config.textScale),
            LocalPreviewLog provides model::record,
            LocalMotionActive provides config.hostActive,
        ) {
            val vink = LocalVink.current
            // Fill the window (or the off-screen capture) so no area is left unpainted.
            val page = Modifier.fillMaxSize().background(vink.colors[ColorRole.Surface]).padding(16.dp)
            Column(
                if (scroll) page.verticalScroll(rememberScrollState()) else page,
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Header(fonts)
                BoxWithConstraints(Modifier.fillMaxWidth()) {
                    val side = maxWidth >= 760.dp * config.textScale
                    if (side) {
                        Row(horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                            Box(Modifier.width(280.dp * config.textScale.coerceAtMost(1.25f))) { CatalogNav(model) }
                            Box(Modifier.weight(1f)) { Stage(model, fonts) }
                        }
                    } else {
                        Column(verticalArrangement = Arrangement.spacedBy(24.dp)) {
                            CatalogNav(model)
                            Stage(model, fonts)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun TextIn(text: String, role: VinkTextRole, color: ColorRole = ColorRole.OnSurface, modifier: Modifier = Modifier) {
    val vink = LocalVink.current
    Text(text, modifier, color = vink.colors[color], style = vink.text(role))
}

@Composable
private fun Code(text: String, color: ColorRole = ColorRole.OnSurfaceVariant) = TextIn(text, VinkTextRoles.Code, color)

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Header(fonts: LoadedFonts) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        TextIn("Vink UI · preview CMP", VinkTextRoles.Title, modifier = Modifier.semantics { heading() })
        TextIn(
            "tokens.json ${VinkTokens.VERSION} · catalog ${VinkCatalog.VERSION} · Compose Multiplatform 1.9 · diagnostic",
            VinkTextRoles.Caption,
            ColorRole.OnSurfaceVariant,
        )
        Box(
            Modifier.background(LocalVink.current.colors[ColorRole.InfoContainer], RoundedCornerShape(VinkDimens.RadiusMd))
                .padding(horizontal = 12.dp, vertical = 4.dp).testTag("offline-note"),
        ) {
            TextIn(
                "Không kết nối dịch vụ · dữ liệu [Mẫu] · đồng hồ cố định ${VinkFixture.CLOCK} · ${fonts.status}",
                VinkTextRoles.Caption,
                ColorRole.OnInfoContainer,
            )
        }
    }
}

@Composable
private fun CatalogNav(model: PreviewModel) {
    val vink = LocalVink.current
    val visible = VinkCatalog.entries.filter { matches(it, model.config.query) }
    Column(
        Modifier.fillMaxWidth().testTag("catalog-nav")
            .background(vink.colors[ColorRole.SurfaceContainerLow], RoundedCornerShape(VinkDimens.RadiusLg)).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        OutlinedTextField(
            value = model.config.query,
            onValueChange = { q -> model.update("tìm → \"$q\"") { it.copy(query = q) } },
            label = { Text("Tìm họ, thành phần, kịch bản") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().testTag("catalog-search"),
        )
        if (visible.isEmpty()) {
            TextIn("Không có mục khớp.", VinkTextRoles.Body, ColorRole.OnSurfaceVariant, Modifier.testTag("catalog-empty"))
        }
        visible.forEach { entry ->
            val selected = entry.id == model.config.entry
            val bg = if (selected) ColorRole.PrimaryContainer else ColorRole.SurfaceContainerLow
            val fg = if (selected) ColorRole.OnPrimaryContainer else ColorRole.OnSurface
            Column(
                Modifier.fillMaxWidth().heightIn(min = VinkDimens.SizeHitTarget)
                    .background(vink.colors[bg], RoundedCornerShape(VinkDimens.RadiusMd))
                    .selectable(selected, role = Role.Tab) {
                        model.update("chọn → ${entry.id}") { it.copy(entry = entry.id, scenario = "default") }
                    }
                    .padding(horizontal = 12.dp, vertical = 8.dp).testTag("catalog-link:${entry.id}"),
            ) {
                TextIn(entry.label, VinkTextRoles.Body, fg)
                Code(entry.id, fg)
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun <T> Choice(legend: String, tag: String, options: List<Pair<T, String>>, value: T, pick: (T, String) -> Unit) {
    val vink = LocalVink.current
    Column(
        Modifier.border(1.dp, vink.colors[ColorRole.OutlineVariant], RoundedCornerShape(VinkDimens.RadiusLg))
            .padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
        TextIn(legend, VinkTextRoles.Label)
        FlowRow(Modifier.selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            options.forEach { (option, label) ->
                Row(
                    Modifier.heightIn(min = VinkDimens.SizeHitTarget)
                        .selectable(option == value, role = Role.RadioButton) { pick(option, label) }
                        .padding(end = 8.dp)
                        .testTag("$tag:$label"),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    RadioButton(selected = option == value, onClick = null)
                    TextIn(label, VinkTextRoles.Body)
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Stage(model: PreviewModel, fonts: LoadedFonts) {
    val config = model.config
    val entry = VinkCatalog.entries.first { it.id == config.entry }
    Column(verticalArrangement = Arrangement.spacedBy(24.dp)) {
        FlowRow(Modifier.testTag("preview-controls"), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Choice("Giao diện", "theme", listOf(ThemePreference.Light to "Sáng", ThemePreference.Dark to "Tối", ThemePreference.System to "Hệ thống"), config.theme) { v, l ->
                model.update("giao diện → $l") { it.copy(theme = v) }
            }
            Choice("Khung mẫu", "frame", Frame.entries.map { it to it.label }, config.frame) { v, l ->
                model.update("khung → $l") { it.copy(frame = v) }
            }
            Choice("Cỡ chữ", "scale", listOf(1f to "100%", 2f to "200%"), config.textScale) { v, l ->
                model.update("cỡ chữ → $l") { it.copy(textScale = v) }
            }
            Choice("Chuyển động", "motion", listOf(MotionPreference.System to "Hệ thống", MotionPreference.Reduce to "Giảm"), config.motion) { v, l ->
                model.update("chuyển động → $l") { it.copy(motion = v) }
            }
        }
        Column(Modifier.testTag("specimen"), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            TextIn(entry.label, VinkTextRoles.Headline, modifier = Modifier.semantics { heading() })
            TextIn(entry.summary, VinkTextRoles.Body, ColorRole.OnSurfaceVariant)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                entry.scenarios.forEach { s ->
                    val here = s.id == config.scenario
                    val vink = LocalVink.current
                    Box(
                        Modifier.heightIn(min = VinkDimens.SizeHitTarget)
                            .background(vink.colors[if (here) ColorRole.PrimaryContainer else ColorRole.Surface], RoundedCornerShape(VinkDimens.RadiusFull))
                            .border(1.dp, vink.colors[ColorRole.OutlineVariant], RoundedCornerShape(VinkDimens.RadiusFull))
                            .selectable(here, role = Role.Tab) { model.update("kịch bản → ${s.id}") { it.copy(scenario = s.id) } }
                            .padding(horizontal = 16.dp, vertical = 10.dp).testTag("scenario:${s.id}"),
                        contentAlignment = Alignment.Center,
                    ) {
                        TextIn(s.label, VinkTextRoles.Body, if (here) ColorRole.OnPrimaryContainer else ColorRole.OnSurface)
                    }
                }
            }
            Box(config.frame.width?.let { Modifier.widthIn(max = it) } ?: Modifier.fillMaxWidth()) {
                Specimen(entry.id, config.scenario)
            }
        }
        Inspector(model, entry, fonts)
    }
}

/** The specimen for an entry and scenario, inside the scope the inspector mirrors. */
@Composable
fun Specimen(id: String, scenario: String) {
    VinkScope(density = if (scenario == "compact") Density.Compact else null, reduceMotion = scenario == "reduced") {
        val vink = LocalVink.current
        Column(
            Modifier.fillMaxWidth().testTag("specimen:$id:$scenario")
                .background(vink.colors[ColorRole.SurfaceContainerLow], RoundedCornerShape(VinkDimens.RadiusXl))
                .padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(VinkDensityDimens.StackGap.at(vink.density)),
        ) {
            // A new scenario starts from its own initial state.
            key(id, scenario) {
            when (id) {
                "foundation.color-roles" -> ColorRoles(nested = scenario == "nested-theme")
                "foundation.typography" -> Typography(combining = scenario == "combining-marks")
                "foundation.shape-spacing" -> ShapeSpacing()
                "foundation.motion" -> Motion()
                else -> if (id.startsWith("screen.") || id == "composition.feedback") ScreenRecipe(id, scenario) else if (!EndorsementSpecimen(id, scenario) && !TagSpecimen(id, scenario) && !ComponentSpecimen(id, scenario)) TextIn("Chưa đăng ký trong preview CMP.", VinkTextRoles.Body)
            }
            }
        }
    }
}

private val swatchPairs = listOf(
    ColorRole.Surface to ColorRole.OnSurface,
    ColorRole.SurfaceContainer to ColorRole.OnSurfaceVariant,
    ColorRole.Primary to ColorRole.OnPrimary,
    ColorRole.PrimaryContainer to ColorRole.OnPrimaryContainer,
    ColorRole.SecondaryContainer to ColorRole.OnSecondaryContainer,
    ColorRole.TertiaryContainer to ColorRole.OnTertiaryContainer,
    ColorRole.ErrorContainer to ColorRole.OnErrorContainer,
    ColorRole.WarningContainer to ColorRole.OnWarningContainer,
    ColorRole.SuccessContainer to ColorRole.OnSuccessContainer,
    ColorRole.InfoContainer to ColorRole.OnInfoContainer,
    ColorRole.InverseSurface to ColorRole.InverseOnSurface,
    ColorRole.ActionPrimaryContainer to ColorRole.ActionPrimaryContent,
)

/** Equal-width swatches: as many ~180dp columns as fit. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Swatches() {
    val vink = LocalVink.current
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val gap = 12.dp
        val columns = ((maxWidth + gap) / (180.dp + gap)).toInt().coerceAtLeast(1)
        val width = (maxWidth - gap * (columns - 1)) / columns
        FlowRow(horizontalArrangement = Arrangement.spacedBy(gap), verticalArrangement = Arrangement.spacedBy(gap)) {
            swatchPairs.forEach { (bg, fg) ->
                Column(
                    Modifier.width(width)
                        .background(vink.colors[bg], RoundedCornerShape(VinkDimens.RadiusLg)).padding(16.dp).testTag("swatch:${bg.token}"),
                ) {
                    TextIn("Aa Chữ mẫu", VinkTextRoles.Title, fg)
                    Code(bg.token, fg)
                    Code(fg.token, fg)
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ColorRoles(nested: Boolean) {
    if (nested) {
        Note("Hai scope đặt theme tường minh, đè giao diện của preview; scope trong cùng đặt Hệ thống bên trong scope Tối.")
        val panes = listOf(ThemePreference.Light to "Sáng (tường minh)", ThemePreference.Dark to "Tối (tường minh)")
        val pane: @Composable (ThemePreference, String, Modifier) -> Unit = { theme, label, modifier ->
            Box(modifier) {
                VinkScope(theme = theme) {
                    Column(Modifier.fillMaxWidth().padding(16.dp).testTag("nested:${theme.value}"), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        TextIn(label, VinkTextRoles.Title)
                        Swatches()
                        if (theme == ThemePreference.Dark) {
                            VinkScope(theme = ThemePreference.System) {
                                Column(Modifier.fillMaxWidth().padding(16.dp).testTag("nested:system")) {
                                    TextIn("Hệ thống bên trong Tối", VinkTextRoles.Label)
                                    TextIn("Theo giao diện của máy (được inject vào VinkTheme).", VinkTextRoles.Body)
                                }
                            }
                        }
                    }
                }
            }
        }
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            if (maxWidth >= 640.dp) {
                Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    panes.forEach { (theme, label) -> pane(theme, label, Modifier.weight(1f)) }
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    panes.forEach { (theme, label) -> pane(theme, label, Modifier.fillMaxWidth()) }
                }
            }
        }
        return
    }
    Swatches()
    Column(Modifier.testTag("contrast-table"), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        TextIn("Tỉ lệ tương phản tính từ token (WCAG 2.x); không phải chứng nhận màn hình.", VinkTextRoles.Caption, ColorRole.OnSurfaceVariant)
        VinkContrastPairs.forEach { pair ->
            val light = contrast(pair.fg.light, pair.bg.light)
            val dark = contrast(pair.fg.dark, pair.bg.dark)
            Code("${pair.fg.token} / ${pair.bg.token} · ${pair.usage} · sáng ${decimal(light)} · tối ${decimal(dark)} · ngưỡng ${pair.min}", ColorRole.OnSurface)
        }
    }
}

@Composable
private fun Note(text: String) {
    Box(Modifier.fillMaxWidth().background(LocalVink.current.colors[ColorRole.SurfaceContainer], RoundedCornerShape(VinkDimens.RadiusMd)).padding(12.dp)) {
        TextIn(text, VinkTextRoles.Body, ColorRole.OnSurfaceVariant)
    }
}

@Composable
private fun Typography(combining: Boolean) {
    if (combining) {
        val nfc = nfc(VinkFixture.COMBINING)
        Note("Chuỗi dấu tổ hợp lưu ở dạng NFD: ${VinkFixture.COMBINING.length} điểm mã, ${nfc.length} khi chuẩn hóa NFC.")
    }
    val roles = listOf(
        VinkTextRoles.Display to (if (combining) VinkFixture.LONG_TITLE else VinkFixture.DISPLAY),
        VinkTextRoles.Headline to VinkFixture.HEADLINE,
        VinkTextRoles.Title to (if (combining) VinkFixture.COMBINING else VinkFixture.TITLE),
        VinkTextRoles.Body to VinkFixture.BODY,
        VinkTextRoles.Label to VinkFixture.LABEL,
        VinkTextRoles.Caption to VinkFixture.CAPTION,
        VinkTextRoles.Reading to (if (combining) VinkFixture.COMBINING else VinkFixture.READING),
        VinkTextRoles.Code to VinkFixture.CODE,
    )
    roles.forEach { (role, sample) ->
        Column(Modifier.testTag("type:${role.token}")) {
            Code("${role.token} · ${role.size.value}sp${role.maxSize?.let { " (Web tới ${it.value})" } ?: ""} · ${role.weight} · ${role.lineHeight.value}sp · ${role.trackingEm}em")
            Box(Modifier.widthIn(max = VinkDimens.Measure)) { TextIn(sample, role) }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ShapeSpacing() {
    val vink = LocalVink.current
    TextIn("Radius", VinkTextRoles.Label, ColorRole.OnSurfaceVariant)
    FlowRow(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        listOf("radius.sm" to VinkDimens.RadiusSm, "radius.md" to VinkDimens.RadiusMd, "radius.lg" to VinkDimens.RadiusLg,
            "radius.xl" to VinkDimens.RadiusXl, "radius.hero" to VinkDimens.RadiusHero, "radius.full" to VinkDimens.RadiusFull).forEach { (token, r) ->
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.size(64.dp).background(vink.colors[ColorRole.PrimaryContainer], RoundedCornerShape(r)).border(BorderStroke(2.dp, vink.colors[ColorRole.Primary]), RoundedCornerShape(r)))
                Code("$token · ${r.value}dp")
            }
        }
    }
    TextIn("Khoảng cách", VinkTextRoles.Label, ColorRole.OnSurfaceVariant)
    listOf("space.1" to VinkDimens.Space1, "space.2" to VinkDimens.Space2, "space.3" to VinkDimens.Space3, "space.4" to VinkDimens.Space4,
        "space.6" to VinkDimens.Space6, "space.8" to VinkDimens.Space8, "space.12" to VinkDimens.Space12).forEach { (token, d) ->
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Box(Modifier.width(d).heightIn(min = 16.dp).background(vink.colors[ColorRole.Tertiary], RoundedCornerShape(VinkDimens.RadiusSm)))
            Code("$token · ${d.value}dp")
        }
    }
    TextIn("Kích thước chạm và mật độ", VinkTextRoles.Label, ColorRole.OnSurfaceVariant)
    FlowRow(Modifier.testTag("density-sample"), horizontalArrangement = Arrangement.spacedBy(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        listOf("Lưu bản nháp", "Xem trước").forEach { Control(it) }
        Code("size.hit-target · ${VinkDimens.SizeHitTarget.value}dp · padding ${VinkDensityDimens.ControlPadding.at(vink.density).value}dp (${vink.density})", ColorRole.OnSurface)
    }
}

@Composable
private fun Control(label: String, onClick: () -> Unit = {}) {
    val vink = LocalVink.current
    Surface(
        onClick = onClick,
        shape = RoundedCornerShape(VinkDimens.RadiusFull),
        color = vink.colors[ColorRole.ActionPrimaryContainer],
        modifier = Modifier.heightIn(min = VinkDimens.SizeHitTarget).testTag("control:$label"),
    ) {
        Box(Modifier.padding(horizontal = 24.dp, vertical = VinkDensityDimens.ControlPadding.at(vink.density)), contentAlignment = Alignment.Center) {
            TextIn(label, VinkTextRoles.Label, ColorRole.ActionPrimaryContent)
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Motion() {
    val vink = LocalVink.current
    var moved by remember { mutableStateOf(false) }
    val offset by animateDpAsState(if (moved) 160.dp else 0.dp, tween(vink.motion.mediumMs, easing = VinkMotionTokens.EaseSpatial))
    FlowRow(horizontalArrangement = Arrangement.spacedBy(24.dp), verticalArrangement = Arrangement.spacedBy(24.dp)) {
        Column(Modifier.widthIn(max = 360.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            TextIn("Chuyển tiếp hữu hạn", VinkTextRoles.Label, ColorRole.OnSurfaceVariant)
            Control("Chạy chuyển tiếp") { moved = !moved }
            Box(Modifier.width(220.dp).heightIn(min = 48.dp).background(vink.colors[ColorRole.SurfaceContainerHigh], RoundedCornerShape(VinkDimens.RadiusFull))) {
                Box(Modifier.padding(4.dp).offset(x = offset).size(40.dp).background(vink.colors[ColorRole.Primary], RoundedCornerShape(VinkDimens.RadiusFull)))
            }
            Code("duration.medium · ease.spatial — ${vink.motion.mediumMs}ms")
        }
        Column(Modifier.widthIn(max = 360.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            TextIn("Vòng lặp tải", VinkTextRoles.Label, ColorRole.OnSurfaceVariant)
            val angle = if (vink.motion.loopsRun) {
                val transition = rememberInfiniteTransition()
                val a by transition.animateFloat(0f, 360f, infiniteRepeatable(tween(vink.motion.loopMs, easing = LinearEasing)))
                a
            } else {
                0f
            }
            val track = vink.colors[ColorRole.PrimaryContainer]
            val arc = vink.colors[ColorRole.Primary]
            Canvas(
                Modifier.size(48.dp).rotate(angle)
                    .semantics {
                        contentDescription = "Chỉ báo đang tải (mẫu)"
                        stateDescription = if (vink.motion.loopsRun) "đang chạy" else "dừng"
                    }
                    .testTag("loop"),
            ) {
                val stroke = Stroke(width = 6.dp.toPx())
                val inset = stroke.width / 2
                val box = Size(size.width - stroke.width, size.height - stroke.width)
                drawArc(track, 0f, 360f, useCenter = false, topLeft = Offset(inset, inset), size = box, style = stroke)
                drawArc(arc, -90f, 90f, useCenter = false, topLeft = Offset(inset, inset), size = box, style = stroke)
            }
            Code("duration.loop — ${if (vink.motion.loopsRun) "chạy" else "dừng (giảm chuyển động)"}")
        }
    }
}

@Composable
private fun Inspector(model: PreviewModel, entry: CatalogEntry, fonts: LoadedFonts) {
    val config = model.config
    val outer = LocalVink.current
    Column(
        Modifier.fillMaxWidth().testTag("inspector")
            .background(outer.colors[ColorRole.SurfaceContainer], RoundedCornerShape(VinkDimens.RadiusLg)).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        TextIn("Inspector", VinkTextRoles.Title, modifier = Modifier.semantics { heading() })
        Fact("Catalog ID", "${entry.id} / ${config.scenario}", "inspector-id")
        Fact("Nguồn", "${VinkTokens.SOURCE} @ ${VinkTokens.VERSION}")
        Fact("Trạng thái", "Leptos: ${entry.leptos} · CMP: ${entry.cmp}", "inspector-status")
        Fact("Font", fonts.status)
        TextIn("Giá trị hiệu lực trong specimen", VinkTextRoles.Label)
        // Same parent and parameters as Specimen, so this resolves exactly what it renders.
        VinkScope(density = if (config.scenario == "compact") Density.Compact else null, reduceMotion = config.scenario == "reduced") {
            Effective(LocalVink.current, LocalDensity.current.fontScale)
        }
        TextIn("Nhật ký sự kiện", VinkTextRoles.Label)
        Column(Modifier.testTag("event-log")) {
            if (model.log.isEmpty()) TextIn("Chưa có thao tác.", VinkTextRoles.Caption)
            model.log.forEach { TextIn(it, VinkTextRoles.Caption) }
        }
    }
}

@Composable
private fun Fact(name: String, value: String, tag: String? = null) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        TextIn(name, VinkTextRoles.Body, ColorRole.OnSurfaceVariant, Modifier.widthIn(min = 96.dp))
        Box(tag?.let { Modifier.testTag(it) } ?: Modifier) { Code(value, ColorRole.OnSurface) }
    }
}

private fun hex(argb: Long) = "#" + (argb and 0xFFFFFF).toString(16).padStart(6, '0')

@Composable
private fun Effective(state: VinkScopeState, fontScale: Float) {
    val rows = listOf(
        "theme" to "${state.preference.value} → ${state.theme.name.lowercase()}",
        "color.surface" to hex(state.colors.argb(ColorRole.Surface)),
        "color.on-surface" to hex(state.colors.argb(ColorRole.OnSurface)),
        "color.primary" to hex(state.colors.argb(ColorRole.Primary)),
        "action.primary.container" to hex(state.colors.argb(ColorRole.ActionPrimaryContainer)),
        "type.body" to "${VinkTextRoles.Body.size.value}sp × fontScale ${decimal(fontScale.toDouble())}",
        "density.control-padding" to "${VinkDensityDimens.ControlPadding.at(state.density).value}dp",
        "duration.short" to "${state.motion.shortMs}ms",
        "loop" to if (state.motion.loopsRun) "chạy" else "dừng",
    )
    Column(Modifier.testTag("effective")) {
        rows.forEach { (name, value) ->
            Row(Modifier.fillMaxWidth().testTag("effective:$name"), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Box(Modifier.weight(1f)) { Code(name, ColorRole.OnSurface) }
                Box(Modifier.weight(1f)) { Code(value, ColorRole.OnSurface) }
            }
        }
    }
}

@Composable
fun WorkbenchInspector(model: PreviewModel) = Inspector(model, VinkCatalog.entries.first { it.id == model.config.entry }, LoadedFonts(vink.ui.VinkFonts(), "Font hệ thống · native diagnostic", emptyList()))
