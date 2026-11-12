package vink.ui.preview

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toAwtImage
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.*
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import java.awt.image.BufferedImage
import java.io.File
import javax.imageio.ImageIO
import kotlin.test.Test
import vink.ui.*

@OptIn(ExperimentalTestApi::class)
class InteractionCaptureTest {
    @Test fun capturesRealFieldsAndOverlayAtNarrowLargeText() {
        val out = File(System.getProperty("vink.captures") ?: "build/captures").apply { mkdirs() }
        val fonts = PreviewFonts.load()
        val contexts = mutableListOf<String>()
        for (theme in listOf(ThemePreference.Light, ThemePreference.Dark)) {
            for (scale in listOf(1f, 2f)) {
                for (scenario in listOf("default", "sheet")) {
                    runDesktopComposeUiTest(360, 800) {
                        setContent {
                            CompositionLocalProvider(LocalDensity provides Density(1f, scale)) {
                                VinkTheme(VinkPreferences(theme = theme, motion = MotionPreference.Reduce),
                                    systemDark = false, fonts = fonts.fonts) {
                                    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp)) {
                                        InteractionSpecimen(scenario)
                                    }
                                }
                            }
                        }
                        onNodeWithTag("interaction-trigger").performScrollTo().performClick()
                        onNodeWithTag("interaction-close").performScrollTo().assertIsDisplayed()
                        waitForIdle()
                        val roots = onAllNodes(isRoot())
                        val layers = roots.fetchSemanticsNodes().indices.map { roots[it].captureToImage().toAwtImage() }
                        val capture = BufferedImage(360, 800, BufferedImage.TYPE_INT_ARGB)
                        capture.createGraphics().apply { layers.forEach { drawImage(it, 0, 0, null) }; dispose() }
                        val name = "interaction--$scenario--${theme.value}--360--text${(scale * 100).toInt()}"
                        ImageIO.write(capture, "png", File(out, "$name.png"))
                        contexts += """{"file":"$name.png","entry":"composition.interaction","scenario":"$scenario","theme":"${theme.value}","effectiveTheme":"${theme.value}","widthDp":360,"heightDp":800,"density":1,"osFontScale":1,"previewTextScale":$scale,"motion":"reduce","host":"desktop-offscreen","fonts":"Be Vietnam Pro/Literata pinned TTF","insets":"no OS window or keyboard","state":"open, close action scrolled into view"}"""
                        onNodeWithTag("interaction-close").performClick()
                        onNodeWithTag("interaction-trigger").assertIsFocused()
                    }
                }
            }
        }
        File(out, "interaction-contexts.json").writeText("[${contexts.joinToString(",\n")} ]\n")
    }
}
