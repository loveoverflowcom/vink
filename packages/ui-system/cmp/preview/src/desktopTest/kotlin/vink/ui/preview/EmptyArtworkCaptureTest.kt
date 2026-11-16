package vink.ui.preview

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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

/** Same scenario/viewport before and after #261, separate from workbench gallery captures. */
class EmptyArtworkCaptureTest {
    @Test fun emptyStateMatrix() {
        val fonts = PreviewFonts.load()
        val out = File(System.getenv("VINK_EMPTY_CAPTURE_DIR") ?: "build/empty-artwork-captures")
        out.mkdirs()
        for (id in listOf("screen.explore", "screen.desk")) {
            for (scenario in listOf("empty", "no-results")) {
                for (width in listOf(320, 390, 834)) {
                    for (theme in listOf(ThemePreference.Light, ThemePreference.Dark)) {
                        for (scale in listOf(1f, 2f)) {
                            val scene = ImageComposeScene(width, 1000, Density(1f, 1f)) {
                                CompositionLocalProvider(LocalDensity provides Density(1f, scale)) {
                                    VinkTheme(VinkPreferences(theme = theme), fonts = fonts.fonts) {
                                        Column(Modifier.fillMaxSize().background(LocalVink.current.colors[ColorRole.Surface])
                                            .verticalScroll(rememberScrollState()).padding(16.dp)) {
                                            ScreenRecipe(id, scenario)
                                        }
                                    }
                                }
                            }
                            try {
                                // Let the bounded asynchronous decoder publish pixels before capture.
                                repeat(30) { scene.render(); Thread.sleep(10) }
                                val png = scene.render().encodeToData(EncodedImageFormat.PNG) ?: error("no image")
                                File(out, "$id--$scenario--$width--${theme.value}--${(scale * 100).toInt()}.png").writeBytes(png.bytes)
                            } finally { scene.close() }
                        }
                    }
                }
            }
        }
        File(out, "context.json").writeText("""{"renderer":"CMP JVM ImageComposeScene","density":1,"viewportHeightDp":1000,"paddingDp":16,"osInsetsDp":0,"widthsDp":[320,390,834],"fontScale":[1,2],"themes":["light","dark"],"font":"PreviewFonts pinned TTF","captureMode":"offscreen","native":false}""")
    }
}
