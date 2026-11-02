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
    fun capturesFoundationSpecimens() {
        val sizes = listOf("narrow" to (360 to 4200), "tablet" to (834 to 3000), "wide" to (1280 to 2600))
        for (theme in listOf(ThemePreference.Light, ThemePreference.Dark)) {
            for ((label, size) in sizes) {
                for (entry in VinkCatalog.entries) {
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
        File(out, "contexts.json").writeText(
            "{\n  \"renderer\": \"Compose Multiplatform 1.9.0 desktop (Skia, offscreen ImageComposeScene)\",\n" +
                "  \"os\": \"${System.getProperty("os.name")} ${System.getProperty("os.version")} ${System.getProperty("os.arch")}\",\n" +
                "  \"jvm\": \"${System.getProperty("java.vendor")} ${System.getProperty("java.version")}\",\n" +
                "  \"captures\": {\n    ${contexts.joinToString(",\n    ")}\n  }\n}\n",
        )
        assertTrue(contexts.size == 30, "${contexts.size} captures")
    }
}
