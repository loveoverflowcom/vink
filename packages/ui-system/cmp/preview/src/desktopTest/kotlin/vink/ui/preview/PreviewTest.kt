package vink.ui.preview

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextInputSelection
import androidx.compose.ui.test.runComposeUiTest
import androidx.compose.ui.text.TextRange
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import vink.ui.ImageState
import vink.ui.ThemePreference
import vink.ui.VinkCatalog
import vink.ui.VinkIdentityVectors

@OptIn(ExperimentalTestApi::class)
class PreviewTest {
    /** A text node inside the tagged region (tags sit on containers, text on their children). */
    private fun ComposeUiTest.textUnder(tag: String, text: String) =
        onNode(hasText(text, substring = true) and hasAnyAncestor(hasTestTag(tag)), useUnmergedTree = true).assertExists()

    private val fonts = PreviewFonts.load()

    @Test
    fun everySharedCatalogEntryIsRegisteredHere() {
        assertEquals(VinkCatalog.entries.map { it.id }, REGISTERED)
        for (entry in VinkCatalog.entries) assertTrue(entry.cmp in VinkCatalog.statuses, entry.id)
    }

    @Test
    fun eachScenarioRendersItsSpecimen() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false))
        setContent { Workbench(model, fonts) }
        for (entry in VinkCatalog.entries) {
            for (scenario in entry.scenarios) {
                model.update("test") { it.copy(entry = entry.id, scenario = scenario.id) }
                waitForIdle()
                onNodeWithTag("specimen:${entry.id}:${scenario.id}").assertExists()
                onNode(hasText("Chưa đăng ký", substring = true)).assertDoesNotExist()
                textUnder("inspector-id", "${entry.id} / ${scenario.id}")
            }
        }
        textUnder("inspector-status", "implemented-unverified")
    }

    @Test
    fun controlsChangeTheEffectiveThemeAndLogEvents() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false))
        setContent { Workbench(model, fonts) }
        onNodeWithTag("theme:Tối").performClick()
        waitForIdle()
        assertEquals(ThemePreference.Dark, model.config.theme)
        textUnder("effective:color.surface", "#17121d")
        onNodeWithTag("theme:Hệ thống").performClick()
        waitForIdle()
        textUnder("effective:color.surface", "#fffaff")
        assertTrue(model.log.first().contains("giao diện → Hệ thống"))
        assertTrue(model.log.first().startsWith("2026-11-02T09:00:00+07:00"))
    }

    @Test
    fun reducedScenarioStopsTheLoopAndCompactKeepsTheHitTarget() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false, entry = "foundation.motion"))
        setContent { Workbench(model, fonts) }
        val running = SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "đang chạy")
        val stopped = SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "dừng")
        mainClock.autoAdvance = false
        onNodeWithTag("loop").assert(running)
        model.update("test") { it.copy(scenario = "reduced") }
        mainClock.advanceTimeByFrame()
        onNodeWithTag("loop").assert(stopped)
        model.update("test") { it.copy(scenario = "default", systemReducedMotion = true) }
        mainClock.advanceTimeByFrame()
        onNodeWithTag("loop").assert(stopped)
        mainClock.autoAdvance = true
        model.update("test") { it.copy(entry = "foundation.shape-spacing", scenario = "compact", systemReducedMotion = false) }
        waitForIdle()
        val height = onNodeWithTag("control:Lưu bản nháp").fetchSemanticsNode().size.height
        assertTrue(height >= 44, "compact control is $height px tall at density 1")
    }

    @Test
    fun searchIgnoresVietnameseMarksAndSaysWhenNothingMatches() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false))
        setContent { Workbench(model, fonts) }
        onNodeWithTag("catalog-search").performTextInput("chuyen dong")
        waitForIdle()
        onNodeWithTag("catalog-link:foundation.motion").assertExists()
        onNodeWithTag("catalog-link:foundation.typography").assertDoesNotExist()
        model.update("test") { it.copy(query = "zzz") }
        waitForIdle()
        onNodeWithTag("catalog-empty").assertExists()
        assertTrue(matches(VinkCatalog.entries[1], "DẤU TỔ HỢP"))
    }

    private fun ComposeUiTest.open(model: PreviewModel, entry: String, scenario: String = "default") {
        model.update("test") { it.copy(entry = entry, scenario = scenario) }
        waitForIdle()
        model.log.clear()
    }

    @Test
    fun buttonsDispatchOnceAndNeverWhileDisabledOrPending() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false))
        setContent { Workbench(model, fonts) }
        open(model, "action.button")
        onNodeWithTag("press-once").performClick()
        waitForIdle()
        textUnder("press-count", "Đã nhận 1 lần")
        assertEquals(1, model.log.size, model.log.joinToString())
        open(model, "action.button", "disabled")
        onNodeWithTag("disabled:filled").assertIsNotEnabled().performClick()
        waitForIdle()
        textUnder("press-count", "Đã nhận 0 lần")
        open(model, "action.button", "pending")
        onNodeWithTag("pending-save").performClick()
        waitForIdle()
        val pending = SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Đang lưu… (chờ xác nhận)")
        onNodeWithTag("pending-save").assert(pending).assertIsEnabled().performClick()
        waitForIdle()
        textUnder("press-count", "Đã nhận 1 lần")
        assertEquals(1, model.log.size, "a pending button ignores a second activation: ${model.log}")
    }

    @Test
    fun textFieldsKeepSelectionAndExposeTheirLabelAndError() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false))
        setContent { Workbench(model, fonts) }
        open(model, "field.text-field")
        val title = onNode(hasText("Tiêu đề (bắt buộc)") and hasSetTextAction())
        title.performTextInput("Bàn viết")
        title.performTextInputSelection(TextRange(3))
        title.performTextInput("x")
        waitForIdle()
        textUnder("title-value", "\"Bànx viết\" · chọn 4..4")
        assertTrue(model.log.none { it.contains("đang soạn IME") }, "no composition was opened")
        open(model, "field.text-field", "invalid")
        onNode(hasText("Tiêu đề (bắt buộc)") and hasSetTextAction())
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, "Cần có tiêu đề trước khi gửi."))
        open(model, "field.text-field", "long-label")
        textUnder("title-value", "\"0\"")
    }

    @Test
    fun choicesAreOneTargetWithOneDispatch() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false))
        setContent { Workbench(model, fonts) }
        open(model, "field.checkbox")
        onNode(hasText("Tôi xác nhận đây là tác phẩm của mình") and hasAnyAncestor(hasTestTag("check:agree")), useUnmergedTree = true).performClick()
        waitForIdle()
        assertEquals(listOf("checkbox agree = true"), model.log.map { it.substringAfter(" · #1 ") })
        open(model, "action.filter-chip")
        onNodeWithTag("chip:tho").performClick()
        waitForIdle()
        assertEquals(1, model.log.size, model.log.joinToString())
    }

    @Test
    fun avatarsFallBackAndGroupsNeverInventCounts() = runComposeUiTest {
        assertTrue(decodeImage(VinkIdentityVectors.SAMPLE_WEBP_BASE64) is ImageState.Ready)
        assertEquals(ImageState.Failed, decodeImage(VinkIdentityVectors.BROKEN_WEBP_BASE64), "undecodable WebP falls back")
        val model = PreviewModel(PreviewConfig(systemDark = false))
        setContent { Workbench(model, fonts) }
        open(model, "identity.avatar")
        onNodeWithTag("avatar:author:an-nhien:reader").assert(SemanticsMatcher.expectValue(SemanticsProperties.ContentDescription, listOf("An Nhiên")))
        open(model, "identity.avatar-group", "absent-identity")
        val unknown = onNodeWithTag("group-unknown").fetchSemanticsNode().config[SemanticsProperties.ContentDescription].joinToString()
        assertTrue("người khác" !in unknown && "+" !in unknown, unknown)
        open(model, "identity.avatar-group")
        onNodeWithContentDescription("Người đã nhận xét: An Nhiên, Trần Linh, Trần Linh và 9 người khác").assertExists()
        open(model, "identity.author-byline")
        onNode(hasText("An Nhiên") and hasAnyAncestor(hasTestTag("byline:feed")), useUnmergedTree = true).assertExists()
        onAllNodesWithContentDescription("An Nhiên").assertCountEquals(0)
    }

    @Test
    fun selfHostedFacesDecodeOnDesktop() {
        assertEquals(3, fonts.loaded.size, fonts.status)
    }
}
