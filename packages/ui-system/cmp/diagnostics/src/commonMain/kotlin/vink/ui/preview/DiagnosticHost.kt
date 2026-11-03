package vink.ui.preview

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import vink.ui.*

/** Bounded launch recipe. Unknown IDs/options fail visibly; no arbitrary screen schema or I/O. */
fun replay(args: List<String>): PreviewConfig {
    val options = args.filter { it.startsWith("--") }.associate {
        val pair = it.removePrefix("--").split('=', limit = 2)
        require(pair.size == 2) { "Expected --key=value" }
        pair[0] to pair[1]
    }
    require(options.keys.all { it in setOf("entry", "scenario", "theme", "motion", "scale") }) { "Unknown replay option" }
    val entry = options["entry"] ?: "field.text-field"
    val scenario = options["scenario"] ?: "default"
    require(VinkCatalog.entries.any { it.id == entry && it.scenarios.any { s -> s.id == scenario } }) { "Unknown catalog scenario" }
    val theme = when (options["theme"] ?: "system") {
        "light" -> ThemePreference.Light
        "dark" -> ThemePreference.Dark
        "system" -> ThemePreference.System
        else -> error("Unknown theme")
    }
    val motion = when (options["motion"] ?: "system") {
        "system" -> MotionPreference.System
        "reduce" -> MotionPreference.Reduce
        else -> error("Unknown motion")
    }
    val scale = when (options["scale"] ?: "100") {
        "100" -> 1f
        "200" -> 2f
        else -> error("Scale must be 100 or 200; separate from OS text scaling")
    }
    return PreviewConfig(entry = entry, scenario = scenario, theme = theme, motion = motion, textScale = scale)
}

/** Compact native host. Uses exactly the desktop specimens and exported library components. */
@Composable
fun DiagnosticHost(model: PreviewModel) {
    val config = model.config
    val density = LocalDensity.current
    VinkTheme(VinkPreferences(theme = config.theme, motion = config.motion), systemReducedMotion = config.systemReducedMotion) {
        CompositionLocalProvider(
            LocalDensity provides androidx.compose.ui.unit.Density(density.density, density.fontScale * config.textScale),
            LocalMotionActive provides config.hostActive,
            LocalPreviewLog provides model::record,
        ) {
            var controls by remember { mutableStateOf(false) }
            val entry = VinkCatalog.entries.first { it.id == config.entry }
            Column(
                Modifier.fillMaxSize().background(LocalVink.current.colors[ColorRole.Surface])
                    .safeDrawingPadding().imePadding().verticalScroll(rememberScrollState())
                    .padding(VinkDimens.Space4).testTag("diagnostic-root"),
                verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3),
            ) {
                VinkHeading("Vink UI · Mẫu", HeadingLevel.H1)
                VinkText("${entry.id}/${config.scenario} · tokens ${VinkTokens.VERSION} · catalog ${VinkCatalog.VERSION}", role = TypeRole.Caption)
                VinkText("Font hệ thống · Không kết nối dịch vụ", role = TypeRole.Caption)
                VinkButton("Điều khiển", { controls = !controls }, variant = ButtonVariant.Tonal)
                if (controls) {
                    VinkSelect("Thành phần", VinkCatalog.entries.map { ChoiceOption(it.id, it.label) }, config.entry, { id ->
                        if (id != null) model.update("entry → $id") { it.copy(entry = id, scenario = "default") }
                    })
                    VinkSelect("Kịch bản", entry.scenarios.map { ChoiceOption(it.id, it.label) }, config.scenario, { id ->
                        if (id != null) model.update("scenario → $id") { it.copy(scenario = id) }
                    })
                }
                VinkButton("Đổi sáng/tối", {
                    model.update("theme") { it.copy(theme = if (it.theme == ThemePreference.Dark) ThemePreference.Light else ThemePreference.Dark) }
                }, variant = ButtonVariant.Text)
                // One call site: utility/theme changes never replace an input's composition owner.
                Specimen(entry.id, config.scenario)
                VinkText("Hoạt động: ${config.hostActive} · Giảm chuyển động hệ thống: ${config.systemReducedMotion}", role = TypeRole.Caption, modifier = Modifier.testTag("host-activity"))
                VinkText("Đồng hồ ${VinkFixture.CLOCK} · seed ${VinkFixture.SEED}", role = TypeRole.Caption)
                if (controls) WorkbenchInspector(model)
            }
        }
    }
}
