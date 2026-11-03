package vink.ui.preview

import androidx.compose.runtime.*
import androidx.compose.ui.test.*
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import kotlin.test.Test
import kotlin.test.assertEquals
import vink.patterns.FeedbackComposer
import vink.patterns.FeedbackIntent
import vink.ui.*

@OptIn(ExperimentalTestApi::class)
class ScreenPreviewTest {
    private val fonts = PreviewFonts.load()

    @Test
    fun feedbackKeepsTextAndSelectionAcrossThemePendingErrorAndUncertain() = runComposeUiTest {
        val original = TextFieldValue("Chữ đang viết\n\n  một khổ thơ", TextRange(4, 9))
        var value by mutableStateOf(original)
        var state by mutableStateOf<ActionState>(ActionState.Enabled)
        var dark by mutableStateOf(false)
        var presses = 0
        setContent {
            VinkTheme(VinkPreferences(), systemDark = dark, fonts = fonts.fonts) {
                FeedbackComposer(FeedbackIntent.Impressions, value, { value = it }, state, { presses++ })
            }
        }
        onNodeWithText("Gửi nhận xét").performClick()
        runOnIdle { assertEquals(1, presses); assertEquals(original, value); dark = true; state = ActionState.Pending("Đang chờ") }
        onNodeWithText("Gửi nhận xét").performClick()
        runOnIdle { assertEquals(1, presses); assertEquals(original, value); state = ActionState.Disabled("Chưa rõ kết quả; đối chiếu trước") }
        onNodeWithText("Gửi nhận xét").assertIsNotEnabled()
        runOnIdle { assertEquals(original, value); state = ActionState.Enabled }
        onNodeWithText("Gửi nhận xét").performClick()
        runOnIdle { assertEquals(2, presses); assertEquals(original, value) }
    }

    @Test
    fun sharingDoesNotAskForCriticism() = runComposeUiTest {
        setContent {
            VinkTheme(VinkPreferences(), fonts = fonts.fonts) {
                FeedbackComposer(FeedbackIntent.Sharing, TextFieldValue("giữ riêng"), {}, ActionState.Enabled, {})
            }
        }
        onNodeWithText("Tác giả chỉ chia sẻ bài viết này.").assertExists()
        onNodeWithText("Gửi nhận xét").assertDoesNotExist()
        onAllNodes(hasSetTextAction()).assertCountEquals(0)
    }
}
