package vink.ui.preview

import androidx.compose.ui.graphics.toAwtImage
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.isRoot
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.runDesktopComposeUiTest
import java.awt.image.BufferedImage
import java.io.File
import javax.imageio.ImageIO
import kotlin.test.Test
import kotlin.test.assertTrue
import vink.ui.ThemePreference

/**
 * Captures playground recipes AFTER their steps (the static captures only show initial states).
 * Workbench without scroll at 834×2600dp (side-by-side nav), density 1, light; each PNG is listed in
 * `driven-contexts.json` with the steps that produced it. Not a visual baseline.
 */
@OptIn(ExperimentalTestApi::class)
class DrivenCaptureTest {
    private val out = File(System.getProperty("vink.captures") ?: "build/captures")
    private val fonts = PreviewFonts.load()
    private val contexts = mutableListOf<String>()

    private fun drive(recipe: String, steps: String, width: Int = 834, body: ComposeUiTest.() -> Unit) = runDesktopComposeUiTest(width, 2600) {
        val model = PreviewModel(PreviewConfig(theme = ThemePreference.Light, systemDark = false, entry = "state.playground", scenario = recipe))
        setContent { Workbench(model, fonts, scroll = false) }
        waitForIdle()
        body()
        waitForIdle()
        val name = "state.playground--$recipe--driven--light--$width"
        out.mkdirs()
        // A dialog is a second root (an overlay layer in the same window): draw the layers in order.
        val roots = onAllNodes(isRoot())
        val layers = roots.fetchSemanticsNodes().indices.map { roots[it].captureToImage().toAwtImage() }
        val image = BufferedImage(layers[0].getWidth(null), layers[0].getHeight(null), BufferedImage.TYPE_INT_ARGB)
        image.createGraphics().apply { layers.forEach { drawImage(it, 0, 0, null) }; dispose() }
        ImageIO.write(image, "png", File(out, "$name.png"))
        contexts += """"$name": {"widthDp": $width, "heightDp": 2600, "density": 1.0, "steps": "$steps", "log": "${model.log.firstOrNull()?.replace("\"", "'") ?: ""}"}"""
    }

    @Test
    fun capturesPlaygroundRecipesAfterTheirSteps() {
        drive("default", "+300ms (3×100) — skeleton visible") { repeat(3) { onNodeWithTag("playground-advance-100").performClick() } }
        drive("loading-error-retry", "step → Failed(network)") { onNodeWithTag("playground-step").performClick() }
        drive("refresh-partial", "step, step → Present(partial)") { repeat(2) { onNodeWithTag("playground-step").performClick(); waitForIdle() } }
        drive("pending-cancel", "action → Pending") { onNodeWithTag("playground-action").performClick() }
        drive("submit-uncertain", "action, step → Uncertain") {
            onNodeWithTag("playground-action").performClick()
            waitForIdle()
            onNodeWithTag("playground-step").performClick()
        }
        drive("save-conflict", "type, action, step → Failed(conflict)") {
            onNode(hasText("Ghi chú đang viết [Mẫu]") and hasSetTextAction()).performTextInput("Sửa ")
            onNodeWithTag("playground-action").performClick()
            waitForIdle()
            onNodeWithTag("playground-step").performClick()
        }
        drive("session-expired", "type, step → dialog") {
            onNode(hasText("Ghi chú đang viết [Mẫu]") and hasSetTextAction()).performTextInput("Sửa ")
            onNodeWithTag("playground-step").performClick()
        }
        File(out, "driven-contexts.json").writeText("{\n  \"captures\": {\n    ${contexts.joinToString(",\n    ")}\n  }\n}\n")
        assertTrue(contexts.size == 7, "${contexts.size} driven captures")
    }
}
