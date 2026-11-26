package vink.ui.preview

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.*
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.unit.dp
import kotlin.math.abs
import kotlin.test.*
import vink.ui.*

@OptIn(ExperimentalTestApi::class)
class EmptyArtworkPreviewTest {
    @Test fun semanticStatesActionsAndInputIdentitySurviveChanges() = runComposeUiTest {
        var scenario by mutableStateOf("no-results")
        var id by mutableStateOf("screen.explore")
        var dark by mutableStateOf(false)
        var scale by mutableStateOf(1f)
        val events = mutableListOf<String>()
        setContent {
            CompositionLocalProvider(LocalPreviewLog provides { events.add(it) },
                androidx.compose.ui.platform.LocalDensity provides androidx.compose.ui.unit.Density(1f, scale)) {
                VinkTheme(VinkPreferences(), systemDark = dark, fonts = PreviewFonts.load().fonts) {
                    Column(Modifier.width(320.dp).height(600.dp).verticalScroll(rememberScrollState()).padding(16.dp)) {
                        ScreenRecipe(id, scenario)
                    }
                }
            }
        }
        for (screen in listOf("screen.explore", "screen.desk")) {
            runOnIdle { id = screen; scenario = "no-results" }
            val input = onNode(hasSetTextAction())
            input.performTextReplacement("Đường về Ỗ")
            input.performTextInputSelection(TextRange(2, 5))
            for (sample in listOf("empty", "no-results", "loading", "error", "auth-required", "forbidden", "pending", "refresh", "partial", "stale", "uncertain", "no-results")) {
                runOnIdle { scenario = sample; dark = !dark; scale = if (scale == 1f) 2f else 1f }
                input.assertTextContains("Đường về Ỗ").assertIsFocused()
                assertEquals(TextRange(2, 5), input.fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange])
                if (sample in listOf("empty", "no-results")) {
                    waitUntil(timeoutMillis = 10_000) { onAllNodesWithTag("empty-artwork-ready").fetchSemanticsNodes().isNotEmpty() }
                    val slot = onNodeWithTag("empty-artwork-slot").performScrollTo().getUnclippedBoundsInRoot()
                    assertTrue(abs((slot.right - slot.left).value - 176f) < 1)
                    assertTrue(abs((slot.right - slot.left).value / (slot.bottom - slot.top).value - 512f / 427f) < 0.02f)
                    onNodeWithTag("empty-artwork-ready").assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.ContentDescription))
                    val label = if (sample == "no-results") "Xóa tìm kiếm" else if (screen == "screen.desk") "Viết bản thảo đầu tiên" else "Bắt đầu viết"
                    onNodeWithText(label).performScrollTo().assertIsDisplayed()
                } else onNodeWithTag("empty-artwork-panel").assertDoesNotExist()
            }
            onNodeWithText("Xóa tìm kiếm").performSemanticsAction(androidx.compose.ui.semantics.SemanticsActions.OnClick) { it() }
            input.assert(SemanticsMatcher.expectValue(SemanticsProperties.EditableText, androidx.compose.ui.text.AnnotatedString("")))
            onNodeWithTag("sample-result").assertTextEquals("Mẫu · Đã xóa tìm kiếm; không gọi dịch vụ.")
            runOnIdle { scenario = "empty" }
            onNodeWithText(if (screen == "screen.desk") "Viết bản thảo đầu tiên" else "Bắt đầu viết")
                .performScrollTo().performClick()
            onNodeWithTag("sample-result").assertTextEquals("Mẫu · Đích #screen.editor/dirty")
        }
        assertEquals(2, events.count { it == "screen → clear search (sample)" })
        assertEquals(2, events.count { it == "screen → #screen.editor/dirty (sample)" })
    }

    @Test fun panelReflowsAtActualWidthsAndBothTextScales() = runComposeUiTest {
        var width by mutableStateOf(320)
        var scale by mutableStateOf(1f)
        setContent {
            CompositionLocalProvider(androidx.compose.ui.platform.LocalDensity provides androidx.compose.ui.unit.Density(1f, scale)) {
                VinkTheme(VinkPreferences()) {
                    Column(Modifier.width(width.dp).height(600.dp).verticalScroll(rememberScrollState()).padding(16.dp)) {
                        EmptyArtworkPanel(null, "Một trang dành riêng cho những điều bạn muốn viết.", "Viết bản thảo đầu tiên", {})
                    }
                }
            }
        }
        for (w in listOf(320, 390, 834)) for (s in listOf(1f, 2f)) {
            runOnIdle { width = w; scale = s }
            val slot = onNodeWithTag("empty-artwork-slot").performScrollTo().getUnclippedBoundsInRoot()
            assertTrue(abs((slot.right - slot.left).value - if (w < 500) 176f else 240f) < 1)
            assertTrue(abs((slot.right - slot.left).value / (slot.bottom - slot.top).value - 512f / 427f) < 0.02f)
            onNodeWithText("Viết bản thảo đầu tiên").performScrollTo().assertIsDisplayed()
            // One centered cluster (Web parity): artwork, wrapped sentence and CTA share the panel axis.
            val axis = onNodeWithTag("empty-artwork-panel").getUnclippedBoundsInRoot().let { (it.left + it.right).value / 2 }
            for (node in listOf(onNodeWithTag("empty-artwork-slot"), onNodeWithTag("empty-artwork-text"), onNodeWithText("Viết bản thảo đầu tiên"))) {
                val box = node.getUnclippedBoundsInRoot()
                assertTrue(abs((box.left + box.right).value / 2 - axis) <= 1f, "center delta at ${w}dp × $s")
            }
        }
    }

    @Test fun failedPictureKeepsSemanticTextAndAction() = runComposeUiTest {
        var calls = 0
        setContent {
            VinkTheme(VinkPreferences()) {
                EmptyArtworkPanel(null, "Chưa có tác phẩm công khai.", "Bắt đầu viết", { calls++ })
            }
        }
        onNodeWithText("Chưa có tác phẩm công khai.").assertExists()
        onNodeWithTag("empty-artwork-ready").assertDoesNotExist()
        onNodeWithText("Bắt đầu viết").performClick()
        assertEquals(1, calls)
    }
}
