package vink.ui.preview

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.toAwtImage
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.*
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import java.awt.image.BufferedImage
import java.awt.Color
import java.io.File
import java.security.MessageDigest
import javax.imageio.ImageIO
import kotlin.test.Test
import kotlin.test.assertTrue
import vink.ui.*

@OptIn(ExperimentalTestApi::class)
class InteractionCaptureTest {
    @Test fun capturesRealFieldsAndOverlayAtNarrowLargeText() {
        val out = File(System.getProperty("vink.captures") ?: "build/captures").apply { mkdirs() }
        val fonts = PreviewFonts.load()
        val contexts = mutableListOf<String>()
        val title = "Bản thảo đang viết [Mẫu]"
        val repo = generateSequence(File(".").canonicalFile) { it.parentFile }.first { File(it, ".git").exists() }
        fun git(vararg args: String): String {
            val process = ProcessBuilder("git", *args).directory(repo).start()
            val result = process.inputStream.bufferedReader().readText().trim()
            check(process.waitFor() == 0) { "git ${args.first()} failed" }
            return result
        }
        fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
        fun sources() = git("ls-files", "--cached", "--others", "--exclude-standard", "--",
            "packages/ui-system/shared", "packages/ui-system/cmp", "packages/vink-patterns/cmp")
            .lines().sorted().associateWith { sha(File(repo, it).readBytes()) }
        val sourceFiles = sources()
        val sourceFingerprint = sha(sourceFiles.entries.joinToString("\n") { "${it.key}=${it.value}" }.toByteArray())
        File(out, "interaction-run.json").writeText("""{"git_sha":"${git("rev-parse", "HEAD")}","dirty":${git("status", "--porcelain").isNotEmpty()},"source_fingerprint":"$sourceFingerprint","source_files":{${sourceFiles.entries.joinToString(",") { "\"${it.key}\":\"${it.value}\"" }}},"host":"desktop-offscreen","data_mode":"synthetic"}
""")
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
                        // A synthetic click can leave hover over a control that appears at the
                        // same point in the overlay. Exit the pointer before every clean capture.
                        onAllNodes(isRoot()).onLast().performMouseInput {
                            moveTo(Offset.Zero)
                            exit(Offset(-1f, -1f))
                        }
                        val name = "interaction--$scenario--${theme.value}--360--text${(scale * 100).toInt()}"
                        val scroller = onNode(hasScrollAction() and hasAnyDescendant(hasText(title)), useUnmergedTree = true)
                        fun scrollRange() = scroller.fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
                        fun scrollViewport() = scroller.fetchSemanticsNode().boundsInRoot.height
                        fun bounds(node: SemanticsNodeInteraction): Rect {
                            val coords = node.fetchSemanticsNode().layoutInfo.coordinates
                            val start = coords.localToWindow(Offset.Zero)
                            return Rect(start.x, start.y, start.x + coords.size.width, start.y + coords.size.height)
                        }
                        fun json(r: Rect) = """{"x":${r.left},"y":${r.top},"width":${r.width},"height":${r.height}}"""
                        fun capture(fileName: String, state: String) {
                            waitForIdle()
                            val nodes = linkedMapOf("panel" to onNodeWithTag("interaction-panel"),
                                "title" to onNodeWithText(title, useUnmergedTree = true),
                                "field-label" to onNodeWithText("Nội dung chưa lưu [Mẫu]", useUnmergedTree = true),
                                "actions" to onNodeWithTag("interaction-close"))
                            val measured = nodes.mapValues { bounds(it.value) }
                            val clipped = nodes.mapValues { it.value.fetchSemanticsNode().layoutInfo.coordinates.boundsInWindow() }
                            val scroll = scrollRange().value()
                            val focused = onAllNodes(isFocused()).fetchSemanticsNodes().map { it.id }
                            val roots = onAllNodes(isRoot())
                            val layers = roots.fetchSemanticsNodes().indices.map { roots[it].captureToImage().toAwtImage() }
                            val clean = BufferedImage(360, 800, BufferedImage.TYPE_INT_ARGB)
                            clean.createGraphics().apply { layers.forEach { drawImage(it, 0, 0, null) }; dispose() }
                            val file = File(out, "$fileName.png")
                            ImageIO.write(clean, "png", file)
                            val overlay = BufferedImage(360, 800, BufferedImage.TYPE_INT_ARGB)
                            overlay.createGraphics().apply {
                                drawImage(clean, 0, 0, null)
                                measured.entries.forEachIndexed { i, (id, r) ->
                                    color = listOf(Color.BLUE, Color.MAGENTA, Color.ORANGE, Color(0, 128, 74))[i]
                                    drawRect(r.left.toInt(), r.top.toInt(), r.width.toInt(), r.height.toInt())
                                    drawString("#$id", r.left.toInt() + 2, (r.top.toInt() + 14).coerceIn(14, 795))
                                }
                                dispose()
                            }
                            val overlayFile = File(out, "$fileName--overlay.png")
                            ImageIO.write(overlay, "png", overlayFile)
                            assertTrue(nodes.mapValues { bounds(it.value) } == measured && scrollRange().value() == scroll
                                && onAllNodes(isFocused()).fetchSemanticsNodes().map { it.id } == focused,
                                "capture must not change bounds, scroll or focus")
                            val anchors = measured.entries.joinToString(",") { (id, r) ->
                                """{"anchor_id":"$id","coordinate_space":"window-px","unit":"px","provenance":"measured: LayoutCoordinates, density=1","bounds":${json(r)},"visible_bounds":${json(clipped.getValue(id))}}"""
                            }
                            contexts += """{"file":"$fileName.png","sha256":"${sha(file.readBytes())}","captured_at":"${java.time.Instant.now()}","entry":"composition.interaction","scenario":"$scenario","theme":"${theme.value}","effectiveTheme":"${theme.value}","widthDp":360,"heightDp":800,"density":1,"osFontScale":1,"previewTextScale":$scale,"motion":"reduce","host":"desktop-offscreen","fonts":"Be Vietnam Pro/Literata pinned TTF","insets":"no OS window or keyboard","state":"$state","inspected":false,"pointer":"exit(-1,-1) before capture","scrollOffsetPx":$scroll,"scrollMaxPx":${scrollRange().maxValue()},"scrollViewportPx":${scrollViewport()},"anchors":[$anchors],"glyph_metrics":{"provenance":"unavailable","reason":"semantics/layout bounds are not glyph bounds"},"overlay":{"file":"$fileName--overlay.png","derived_from":"$fileName.png","sha256":"${sha(overlayFile.readBytes())}","provenance":"derived: bounds drawn on clean PNG"}}"""
                            File(out, "interaction-contexts.json").writeText("[${contexts.joinToString(",\n")} ]\n")
                        }
                        capture("$name--initial", "just opened; no corrective scroll")
                        scroller.performScrollToNode(hasText(title))
                        onNodeWithText(title, useUnmergedTree = true).assertIsDisplayed()
                        assertTrue(scrollRange().value() == 0f, "first tile must start at the top")
                        var tile = 1
                        capture("$name--part$tile", "scroll tile $tile, starts at top")
                        while (scrollRange().value() < scrollRange().maxValue()) {
                            val before = scrollRange().value()
                            val visibleHeight = scrollViewport()
                            val step = visibleHeight * .75f
                            scroller.performSemanticsAction(SemanticsActions.ScrollBy) { it(0f, step) }
                            waitForIdle()
                            assertTrue(scrollRange().value() > before && ++tile <= 12, "bounded tiles must advance")
                            assertTrue(scrollRange().value() <= before + visibleHeight, "tiles must not skip content")
                            capture("$name--part$tile", "scroll tile $tile, overlap 25 percent")
                        }
                        onNodeWithTag("interaction-close").performScrollTo().assertIsDisplayed()
                        capture(name, "open, close action scrolled into view")
                        onNodeWithTag("interaction-close").performClick()
                        onNodeWithTag("interaction-trigger").assertIsFocused()
                    }
                }
            }
        }
        assertTrue(sourceFiles == sources(), "capture source must stay unchanged")
        File(out, "interaction-contexts.json").writeText("[${contexts.joinToString(",\n")} ]\n")
    }
}
