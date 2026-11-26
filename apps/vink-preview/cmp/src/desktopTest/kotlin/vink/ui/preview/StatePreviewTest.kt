package vink.ui.preview

import androidx.compose.ui.input.key.Key
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.runComposeUiTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue
import vink.ui.ActionPhaseId
import vink.ui.Content
import vink.ui.LoopRunning
import vink.ui.Phase
import vink.ui.RegionState
import vink.ui.VinkContentRegion
import vink.ui.VinkPreferences
import vink.ui.VinkTheme

/** Scroll the workbench to the node first: an off-screen click is not delivered. */
private fun SemanticsNodeInteraction.tap() = performScrollTo().performClick()

@OptIn(ExperimentalTestApi::class)
class StatePreviewTest {
    private val fonts = PreviewFonts.load()

    private fun ComposeUiTest.workbench(entry: String, scenario: String = "default", config: PreviewConfig = PreviewConfig(systemDark = false)): PreviewModel {
        val model = PreviewModel(config.copy(entry = entry, scenario = scenario))
        setContent { Workbench(model, fonts) }
        waitForIdle()
        model.log.clear()
        return model
    }

    private fun ComposeUiTest.loops(): List<Boolean> =
        onAllNodes(SemanticsMatcher.keyIsDefined(LoopRunning), useUnmergedTree = true).fetchSemanticsNodes().map { it.config[LoopRunning] }

    private fun ComposeUiTest.under(tag: String, text: String) =
        onNode(hasText(text, substring = true) and hasAnyAncestor(hasTestTag(tag)), useUnmergedTree = true)

    private fun ComposeUiTest.phase(): String =
        onNode(SemanticsMatcher.keyIsDefined(ActionPhaseId) and hasAnyAncestor(hasTestTag("playground-status")), useUnmergedTree = true)
            .fetchSemanticsNode().config[ActionPhaseId]

    @Test
    fun submissionRejectsDoubleBeginAndIgnoresLateAcks() {
        val s = Submission()
        val first = s.begin()!!
        assertNull(s.begin(), "a second activation while pending gets no ticket")
        s.cancel()
        assertTrue(!s.ack(first), "an ack after cancel is ignored")
        val second = s.begin()!!
        assertTrue(!s.ack(first), "a late ack of an older ticket does not confirm the new one")
        assertTrue(s.pending)
        assertTrue(s.ack(second))
        assertTrue(!s.pending)
    }

    @Test
    fun theLoadingIndicatorWaitsForTheGateDelayOnTheManualClock() = runComposeUiTest {
        val model = workbench("state.playground")
        // Before 300ms the skeleton keeps its space but is not drawn or animated.
        assertTrue(loops().none { it }, "${loops()}")
        repeat(2) { onNodeWithTag("playground-advance-100").tap() }
        waitForIdle()
        assertTrue(loops().none { it }, "still hidden at 200ms")
        onNodeWithTag("playground-advance-100").tap()
        waitForIdle()
        assertTrue(loops().any { it }, "skeleton pulses from 300ms: ${loops()}")
        onNodeWithTag("playground-step").tap()
        waitForIdle()
        under("playground-region", "Mưa trên phố Hàng Bạc [Mẫu]").assertExists()
        assertTrue(model.log.any { it.contains("+300ms") && it.contains("Present") }, model.log.joinToString())
    }

    @Test
    fun retryDispatchesOnceAndKeepsTheInput() = runComposeUiTest {
        val model = workbench("state.playground", "loading-error-retry")
        onNode(hasText("Ghi chú đang viết [Mẫu]") and hasSetTextAction()).performTextInput("Thêm ")
        onNodeWithTag("playground-step").tap()
        waitForIdle()
        under("playground-region", "Chưa kết nối được").assertExists()
        under("playground-region", "Mã: read.works.network").assertExists()
        under("playground-region", "Thử lại").tap()
        waitForIdle()
        assertEquals(1, model.log.count { it.contains("intent retry") }, model.log.joinToString())
        onNodeWithTag("playground-step").tap()
        waitForIdle()
        under("playground-region", "Khổ thơ cho mẹ [Mẫu]").assertExists()
        under("playground-input", "Thêm Khổ đầu chưa lưu [Mẫu]").assertExists()
    }

    @Test
    fun aConflictKeepsTheTextAndNeverShowsSaved() = runComposeUiTest {
        val model = workbench("state.playground", "save-conflict")
        onNode(hasText("Ghi chú đang viết [Mẫu]") and hasSetTextAction()).performTextInput("Sửa ")
        waitForIdle()
        assertEquals("dirty", phase())
        onNodeWithTag("playground-action").tap()
        waitForIdle()
        assertEquals("pending", phase())
        onNodeWithTag("playground-action").tap()
        waitForIdle()
        assertEquals(1, model.log.count { it.contains("gửi ticket") }, "a pending action ignores a second activation")
        onNodeWithTag("playground-step").tap()
        waitForIdle()
        assertEquals("failed", phase())
        onNode(hasText("Đối chiếu")).assertExists()
        onNode(hasText("Tải bản sao (.txt)")).assertExists()
        assertTrue(model.log.none { it.contains("Confirmed") }, model.log.joinToString())
        under("playground-input", "Sửa Khổ đầu chưa lưu [Mẫu]").assertExists()
    }

    @Test
    fun anUncertainSubmitNeedsReconcileBeforeADeliberateResend() = runComposeUiTest {
        val model = workbench("state.playground", "submit-uncertain")
        onNodeWithTag("playground-action").tap()
        onNodeWithTag("playground-step").tap()
        waitForIdle()
        assertEquals("uncertain", phase())
        onNodeWithTag("playground-action").tap()
        waitForIdle()
        assertEquals(1, model.log.count { it.contains("gửi ticket") }, "disabled until reconciled: ${model.log}")
        onNode(hasText("Kiểm tra lại")).tap()
        waitForIdle()
        under("playground-action", "Gửi lại có chủ ý").assertExists()
        onNodeWithTag("playground-action").tap()
        waitForIdle()
        assertEquals(2, model.log.count { it.contains("gửi ticket") }, "the resend is a new, deliberate ticket")
    }

    @Test
    fun reducedInactiveAndHiddenHostsRunNoLoop() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false, entry = "state.progress"))
        setContent { Workbench(model, fonts) }
        mainClock.autoAdvance = false
        mainClock.advanceTimeByFrame()
        val running = loops()
        assertTrue(running.size >= 4 && running.all { it }, "default: arc, edge, button arc, badge arc run: $running")
        for (scenario in listOf("reduced", "inactive")) {
            model.update("test") { it.copy(scenario = scenario) }
            mainClock.advanceTimeByFrame()
            assertTrue(loops().isNotEmpty() && loops().none { it }, "$scenario: ${loops()}")
        }
        model.update("test") { it.copy(scenario = "default", hostActive = false) }
        mainClock.advanceTimeByFrame()
        assertTrue(loops().none { it }, "hidden window: ${loops()}")
        model.update("test") { it.copy(hostActive = true, systemReducedMotion = true) }
        mainClock.advanceTimeByFrame()
        assertTrue(loops().none { it }, "system reduced motion: ${loops()}")
        model.update("test") { it.copy(systemReducedMotion = false) }
        mainClock.advanceTimeByFrame()
        assertTrue(loops().all { it }, "foreground again: ${loops()}")
        onNodeWithTag("micro-arc", useUnmergedTree = true)
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "đang chạy"))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.ContentDescription, listOf("Đang lưu bản nháp [Mẫu]")))
    }

    @Test
    fun aDestructiveDialogFocusesTheSafeActionAndReturnsFocus() = runComposeUiTest {
        val model = workbench("state.dialog")
        onNodeWithTag("dialog-safe").assertIsFocused()
        onNodeWithTag("dialog-safe").performKeyInput { pressKey(Key.Escape) }
        waitForIdle()
        onNodeWithTag("dialog-safe").assertDoesNotExist()
        onNodeWithTag("dialog-trigger").assertIsFocused()
        assertTrue(model.log.any { it.contains("đóng (Escape)") }, model.log.joinToString())
        assertTrue(model.log.none { it.contains("xóa") }, "Escape keeps the draft")
    }

    @Test
    fun aSessionExpiredDialogRequiresAChoiceAndKeepsTheDraft() = runComposeUiTest {
        workbench("state.dialog", "session-expired")
        onNodeWithTag("dialog-safe").assertIsFocused().performKeyInput { pressKey(Key.Escape) }
        waitForIdle()
        onNodeWithTag("dialog-safe").assertExists()
        onNode(hasText("Đăng nhập", substring = true, ignoreCase = true) and hasClickAction()).assertDoesNotExist()
        onNodeWithTag("dialog-safe").performClick()
        waitForIdle()
        under("dialog-draft", "Khổ đầu đang viết dở [Mẫu]").assertExists()
    }

    @Test
    fun anActionlessSnackbarClosesAfterItsDurationAndOneWithAnActionWaits() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false, entry = "state.notice", scenario = "snackbar"))
        setContent { Workbench(model, fonts) }
        mainClock.autoAdvance = false
        mainClock.advanceTimeByFrame()
        under("snackbar", "Đã lưu bản nháp [Mẫu].").assertExists()
        mainClock.advanceTimeBy(5900)
        under("snackbar", "Đã lưu bản nháp [Mẫu].").assertExists()
        mainClock.advanceTimeBy(200)
        under("snackbar", "Đã lưu bản nháp [Mẫu].").assertDoesNotExist()
        onNodeWithTag("snackbar-action").tap()
        mainClock.advanceTimeByFrame()
        mainClock.advanceTimeBy(20_000)
        under("snackbar", "Hoàn tác").assertExists()
    }

    @Test
    fun anUnlawfulRegionShowsADiagnosticInsteadOfContent() = runComposeUiTest {
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false) {
                VinkContentRegion(RegionState(Content.Absent, Phase.Idle), "Tác phẩm", {}) { androidx.compose.material3.Text("nội dung") }
            }
        }
        onNodeWithTag("unlawful").assertExists()
        onNode(hasText("not-requested", substring = true)).assertExists()
        onNode(hasText("nội dung")).assertDoesNotExist()
    }

    @Test
    fun errorScenariosOfferOnlyTheirLawfulActions() = runComposeUiTest {
        val model = workbench("state.error", "auth-required")
        onNode(hasText("Đăng nhập", ignoreCase = true) and hasClickAction()).assertDoesNotExist()
        onNode(hasText("Về Khám phá")).tap()
        waitForIdle()
        assertEquals(listOf("state.error → intent return"), model.log.map { it.substringAfter(" · #1 ") })
        model.update("test") { it.copy(scenario = "partial") }
        waitForIdle()
        under("region", "Mưa trên phố Hàng Bạc [Mẫu]").assertExists()
        under("region", "Một phần chưa tải được").assertExists()
    }
}
