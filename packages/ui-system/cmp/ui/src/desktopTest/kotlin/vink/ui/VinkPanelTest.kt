package vink.ui

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.runComposeUiTest
import kotlin.test.Test
import kotlin.test.assertEquals

@OptIn(ExperimentalTestApi::class)
class VinkPanelTest {
    @Test
    fun callerActionSlotAndTypedRecoveryDispatchSeparately() = runComposeUiTest {
        var retries = 0
        var returns = 0
        val content = PanelContent.fromProblem(
            Problem.fallback(ProblemKind.Unavailable)
                .withAction(RecoveryIntent.Retry, "Thử lại")!!,
        )
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false) {
                VinkStatePanel(content, { intent ->
                    assertEquals(RecoveryIntent.Retry, intent)
                    retries++
                }, actions = {
                    VinkButton("Về Bàn viết", { returns++ }, variant = ButtonVariant.Text)
                })
            }
        }
        onNodeWithText("Thử lại").performClick()
        assertEquals(1, retries)
        assertEquals(0, returns)
        onNodeWithText("Về Bàn viết").performClick()
        assertEquals(1, retries)
        assertEquals(1, returns)
    }
}
