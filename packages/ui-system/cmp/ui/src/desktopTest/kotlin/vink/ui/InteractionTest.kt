package vink.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.*
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import kotlin.test.Test
import kotlin.test.assertEquals

/** Real Compose targets. These tests do not substitute for an OS IME walkthrough. */
@OptIn(ExperimentalTestApi::class)
class InteractionTest {
    @Test
    fun searchClearReturnsFocusToTheEditableTargetAndDispatchesOnce() = runComposeUiTest {
        val value = mutableStateOf(TextFieldValue("Tiếng Việt", TextRange(3)))
        val focus = FocusRequester()
        var edits = 0
        var submits = 0
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false) {
                VinkSearchField("Tìm tác phẩm", value.value, { value.value = it; edits++ },
                    Modifier.focusRequester(focus), onSubmit = { submits++ })
            }
        }
        runOnIdle { focus.requestFocus() }
        onNode(hasSetTextAction()).assertIsFocused()
        // Keyboard activation of the clear button starts with focus on that button.
        onNodeWithContentDescription("Xóa chữ tìm").performSemanticsAction(
            androidx.compose.ui.semantics.SemanticsActions.RequestFocus,
        )
        onNodeWithContentDescription("Xóa chữ tìm").performClick()
        onNode(hasSetTextAction()).assertIsFocused()
        assertEquals(TextFieldValue(""), value.value)
        assertEquals(1, edits)
        assertEquals(0, submits)
    }

    @Test
    fun focusedFieldSurvivesThemeAndErrorWithoutChangingVietnameseValue() = runComposeUiTest {
        val initial = TextFieldValue("Tiếng Việt", TextRange(3), TextRange(0, 3))
        val value = mutableStateOf(initial)
        val dark = mutableStateOf(false)
        val error = mutableStateOf<String?>(null)
        val focus = FocusRequester()
        setContent {
            VinkTheme(VinkPreferences(), systemDark = dark.value) {
                Column {
                    VinkTextField("Tên bản thảo", value.value, { value.value = it },
                        Modifier.focusRequester(focus), error = error.value)
                    VinkButton("Lưu", {}, Modifier.testTag("save"))
                }
            }
        }
        runOnIdle { focus.requestFocus() }
        onNode(hasSetTextAction()).assertIsFocused()
        // A real focus target is reached through the existing wrapper; no claimed wrapper bug.
        runOnIdle { value.value = initial; dark.value = true; error.value = "Kiểm tra lại tên" }
        onNode(hasSetTextAction()).assertIsFocused()
        runOnIdle { assertEquals(initial, value.value) }
    }
}
