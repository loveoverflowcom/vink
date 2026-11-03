package vink.ui.preview

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import java.io.File
import kotlin.test.Test
import org.jetbrains.skia.EncodedImageFormat
import vink.ui.*

/**
 * The same sample consumer without the workbench around it. Host widths are real specimen dp,
 * density 1, fontScale 1/2, 16dp padding, no OS safe-area/keyboard insets. JVM off-screen only.
 * Gallery captures remain separate; these do not crop or transform the gallery's images.
 */
class ScreenCaptureTest {
    @Test
    fun capturesScreenConsumerAtItsActualWidth() {
        val fonts = PreviewFonts.load()
        val out = File(System.getProperty("vink.captures") ?: "build/captures")
        val contexts = mutableListOf<String>()
        val entries = VinkCatalog.entries.filter { it.family == "screen" || it.id == "composition.feedback" }
        for (entry in entries) {
            for (width in listOf(360, 834, 1280)) {
                for (theme in listOf(ThemePreference.Light, ThemePreference.Dark)) {
                    for (scale in listOf(1f, 2f)) {
                        val scenario = if (scale == 2f && entry.scenarios.any { it.id == "long" }) "long" else "default"
                        val name = "consumer--${entry.id}--$scenario--${theme.value}--$width--text${(scale * 100).toInt()}"
                        val height = if (scale == 2f) 5200 else 2400
                        val scene = ImageComposeScene(width, height, Density(1f, 1f)) {
                            CompositionLocalProvider(LocalDensity provides Density(1f, scale)) {
                                VinkTheme(VinkPreferences(theme = theme), systemDark = false, fonts = fonts.fonts) {
                                    Box(Modifier.fillMaxSize().background(LocalVink.current.colors[ColorRole.Surface]).padding(16.dp)) {
                                        ScreenRecipe(entry.id, scenario)
                                    }
                                }
                            }
                        }
                        try {
                            out.mkdirs()
                            val png = scene.render().encodeToData(EncodedImageFormat.PNG) ?: error("no image for $name")
                            File(out, "$name.png").writeBytes(png.bytes)
                            contexts += """"$name":{"widthDp":$width,"heightDp":$height,"density":1,"fontScale":$scale,"theme":"${theme.value}","entry":"${entry.id}","scenario":"$scenario","paddingDp":16,"osInsetsDp":0}"""
                        } finally { scene.close() }
                    }
                }
            }
        }
        File(out, "screen-contexts.json").writeText("{\"renderer\":\"CMP JVM ImageComposeScene\",\"captures\":{${contexts.joinToString(",")}}}")
    }
}
