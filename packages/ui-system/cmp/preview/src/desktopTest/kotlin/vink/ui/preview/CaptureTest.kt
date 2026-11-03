package vink.ui.preview

import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.unit.Density
import java.io.File
import kotlin.test.Test
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import vink.ui.ThemePreference
import vink.ui.VinkCatalog

/**
 * Renders the real workbench off-screen and writes PNGs (plus contexts.json) for a person to
 * open. It asserts only that each capture was produced; it is not a visual baseline.
 * Density 1 (1dp = 1px) with font scale 1 or 2; the system theme is fixed to light.
 */
class CaptureTest {
    private val out = File(System.getProperty("vink.captures") ?: "build/captures")
    private val fonts = PreviewFonts.load()
    private val contexts = mutableListOf<String>()

    private fun capture(name: String, width: Int, height: Int, config: PreviewConfig) {
        val model = PreviewModel(config.copy(systemDark = false))
        val scene = ImageComposeScene(width, height, Density(1f, 1f)) { Workbench(model, fonts, scroll = false) }
        try {
            val image = scene.render()
            val png = image.encodeToData(EncodedImageFormat.PNG) ?: error("no PNG for $name")
            out.mkdirs()
            File(out, "$name.png").writeBytes(png.bytes)
            contexts += """"$name": {"widthDp": $width, "heightDp": $height, "density": 1.0, "fontScale": ${config.textScale}, "theme": "${config.theme.value}", "systemDark": false, "entry": "${config.entry}", "scenario": "${config.scenario}", "fonts": "${fonts.status}"}"""
        } finally {
            scene.close()
        }
    }

    @Test
    fun capturesCatalogSpecimens() {
        val sizes = listOf("narrow" to (360 to 4200), "tablet" to (834 to 3000), "wide" to (1280 to 2600))
        for (theme in listOf(ThemePreference.Light, ThemePreference.Dark)) {
            for ((label, size) in sizes) {
                for (entry in VinkCatalog.entries.filter { it.family == "foundation" }) {
                    capture("${entry.id}--default--${theme.value}--$label", size.first, size.second, PreviewConfig(theme = theme, entry = entry.id))
                }
            }
        }
        val extra = listOf(
            Triple("foundation.color-roles", "nested-theme", 1280 to 2600),
            Triple("foundation.typography", "combining-marks", 360 to 4200),
            Triple("foundation.shape-spacing", "compact", 834 to 3000),
            Triple("foundation.motion", "reduced", 834 to 3000),
        )
        for ((entry, scenario, size) in extra) {
            capture("$entry--$scenario--light--${size.first}", size.first, size.second, PreviewConfig(theme = ThemePreference.Light, entry = entry, scenario = scenario))
        }
        for ((label, size) in listOf("narrow" to (360 to 7000), "wide" to (1280 to 4200))) {
            capture("foundation.typography--combining-marks--light--$label--text200", size.first, size.second,
                PreviewConfig(theme = ThemePreference.Light, entry = "foundation.typography", scenario = "combining-marks", textScale = 2f))
        }
        // UIS-02: every component entry in both themes, narrow and wide, plus hard states.
        for (theme in listOf(ThemePreference.Light, ThemePreference.Dark)) {
            for ((label, size) in listOf("narrow" to (360 to 4200), "wide" to (1280 to 2600))) {
                for (entry in COMPONENT_IDS) {
                    capture("$entry--default--${theme.value}--$label", size.first, size.second, PreviewConfig(theme = theme, entry = entry))
                }
            }
        }
        val states = listOf(
            "action.button" to "disabled", "action.button" to "pending", "field.text-field" to "invalid",
            "field.text-field" to "read-only", "field.text-area" to "multiline", "field.form" to "invalid",
            "identity.avatar" to "image-error", "identity.avatar" to "absent-identity", "identity.avatar" to "long-label",
            "identity.avatar-group" to "absent-identity", "identity.image-frame" to "image-error", "composition.nested" to "narrow",
        )
        for ((entry, scenario) in states) {
            capture("$entry--$scenario--light--narrow", 360, 4200, PreviewConfig(theme = ThemePreference.Light, entry = entry, scenario = scenario))
        }
        for (entry in listOf("field.form", "identity.author-byline", "action.button")) {
            capture("$entry--default--light--narrow--text200", 360, 7000, PreviewConfig(theme = ThemePreference.Light, entry = entry, textScale = 2f))
        }
        // UIS-03: every non-default state scenario (light, narrow) and three at 200% text.
        val stateScenarios = VinkCatalog.entries.filter { it.family == "state" }.flatMap { e -> e.scenarios.drop(1).map { e.id to it.id } }
        for ((entry, scenario) in stateScenarios) {
            capture("$entry--$scenario--light--narrow", 360, 4200, PreviewConfig(theme = ThemePreference.Light, entry = entry, scenario = scenario))
        }
        val stateText200 = listOf("state.error" to "long-label", "state.playground" to "save-conflict", "state.notice" to "default")
        for ((entry, scenario) in stateText200) {
            capture("$entry--$scenario--light--narrow--text200", 360, 12000, PreviewConfig(theme = ThemePreference.Light, entry = entry, scenario = scenario, textScale = 2f))
        }
        // UIS-04: every navigation scenario light at narrow and wide, defaults in dark, 200% text.
        val navigation = VinkCatalog.entries.filter { it.family == "navigation" }.flatMap { e -> e.scenarios.map { e.id to it.id } }
        for ((entry, scenario) in navigation) {
            for ((label, size) in listOf("narrow" to (360 to 5200), "wide" to (1280 to 1600))) {
                capture("$entry--$scenario--light--$label", size.first, size.second, PreviewConfig(theme = ThemePreference.Light, entry = entry, scenario = scenario))
            }
        }
        for (entry in navigation.map { it.first }.distinct()) {
            capture("$entry--default--dark--wide", 1280, 1600, PreviewConfig(theme = ThemePreference.Dark, entry = entry))
        }
        val navigationText200 = listOf("navigation.rail" to "long-labels", "navigation.bottom-bar" to "long-labels", "navigation.scaffold" to "short")
        for ((entry, scenario) in navigationText200) {
            capture("$entry--$scenario--light--narrow--text200", 360, 9000, PreviewConfig(theme = ThemePreference.Light, entry = entry, scenario = scenario, textScale = 2f))
        }
        File(out, "contexts.json").writeText(
            "{\n  \"renderer\": \"Compose Multiplatform 1.9.0 desktop (Skia, offscreen ImageComposeScene)\",\n" +
                "  \"os\": \"${System.getProperty("os.name")} ${System.getProperty("os.version")} ${System.getProperty("os.arch")}\",\n" +
                "  \"jvm\": \"${System.getProperty("java.vendor")} ${System.getProperty("java.version")}\",\n" +
                "  \"captures\": {\n    ${contexts.joinToString(",\n    ")}\n  }\n}\n",
        )
        assertTrue(
            contexts.size == 30 + COMPONENT_IDS.size * 4 + 15 + stateScenarios.size + stateText200.size +
                navigation.size * 2 + navigation.map { it.first }.distinct().size + navigationText200.size,
            "${contexts.size} captures",
        )
    }
}
