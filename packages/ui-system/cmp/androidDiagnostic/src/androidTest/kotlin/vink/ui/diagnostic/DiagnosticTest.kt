package vink.ui.diagnostic

import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.lifecycle.Lifecycle
import androidx.core.view.WindowCompat
import org.junit.Rule
import org.junit.Test
import org.junit.Assert.*

class DiagnosticTest {
    @get:Rule val ui = createAndroidComposeRule<DiagnosticActivity>()

    @Test fun controlledVietnameseTextSurvivesThemeAndRecreation() {
        val input = ui.onAllNodes(hasSetTextAction()).onFirst()
        input.performTextReplacement("Đường về – một buổi chiều")
        ui.onNodeWithText("Đổi sáng/tối").performClick()
        input.assertTextContains("Đường về – một buổi chiều", substring = true)
        ui.runOnIdle {
            assertFalse(WindowCompat.getInsetsController(ui.activity.window, ui.activity.window.decorView).isAppearanceLightStatusBars)
        }
        ui.activityRule.scenario.recreate()
        ui.onAllNodes(hasSetTextAction()).onFirst().assertTextContains("Đường về – một buổi chiều", substring = true)
    }

    @Test fun hostStopsActivityInBackgroundAndResumes() {
        ui.activityRule.scenario.moveToState(Lifecycle.State.CREATED)
        assertFalse(ui.activity.model.config.hostActive)
        ui.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
        ui.runOnIdle { assertTrue(ui.activity.model.config.hostActive) }
    }

    @Test fun pendingAndDisabledDoNotDispatchAgain() {
        ui.runOnIdle {
            ui.activity.model.update("test") { it.copy(entry = "action.button", scenario = "pending") }
        }
        ui.onNodeWithTag("pending-save").performClick()
        ui.onNodeWithTag("pending-save").performClick()
        ui.onNodeWithText("Đã nhận 1 lần kích hoạt").assertExists()
    }
}
