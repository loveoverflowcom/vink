package vink.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.*
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import kotlin.test.*

@OptIn(ExperimentalTestApi::class)
class InteractionLifecycleTest {
    @Test fun imeDoneKeepsCompositionAndDispatchesOnlyCommittedEditableInput() = runComposeUiTest {
        val value = mutableStateOf(TextFieldValue("Tiếng Việt", TextRange(3), TextRange(0, 3)))
        val mode = mutableStateOf(FieldMode.Editable)
        val actions = mutableListOf<ImeAction>()
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false) {
                VinkTextField("Tên", value.value, { value.value = it }, mode = mode.value,
                    inputModifier = Modifier.testTag("input"), imeAction = ImeAction.Done,
                    onImeAction = { actions += it })
            }
        }
        val input = onNodeWithTag("input")
        input.assert(SemanticsMatcher.expectValue(SemanticsProperties.ImeAction, ImeAction.Done))
        // Use the IME action semantics without focusing first (focus itself can commit composition).
        input.performSemanticsAction(SemanticsActions.OnImeAction) { it() }
        assertTrue(actions.isEmpty())
        runOnIdle { value.value = value.value.copy(composition = null) }
        input.performImeAction()
        assertEquals(listOf(ImeAction.Done), actions)
        runOnIdle { mode.value = FieldMode.ReadOnly }
        input.performSemanticsAction(SemanticsActions.OnImeAction) { it() }
        assertEquals(listOf(ImeAction.Done), actions)
        assertEquals("Tiếng Việt", value.value.text)
    }

    @Test fun pressDraggedOutsideCancelsAndPendingDoesNotDispatch() = runComposeUiTest {
        var count = 0
        val pending = mutableStateOf(false)
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false) {
                VinkButton("Lưu", { count++; pending.value = true }, Modifier.testTag("save"),
                    state = if (pending.value) ActionState.Pending("Đang lưu…") else ActionState.Enabled)
            }
        }
        val button = onNodeWithTag("save")
        button.performTouchInput { down(center); moveTo(Offset(-100f, -100f)); up() }
        assertEquals(0, count)
        button.performClick()
        button.performClick()
        assertEquals(1, count)
    }

    @Test fun navigateCloseDoesNotRestoreOldTriggerAndDismissDoes() = runComposeUiTest {
        val open = mutableStateOf(false)
        val reason = mutableStateOf(CloseReason.Dismiss)
        val trigger = FocusRequester()
        val safe = FocusRequester()
        val destination = FocusRequester()
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false) {
                Column {
                    VinkButton("Mở", { open.value = true }, Modifier.focusRequester(trigger).testTag("trigger"))
                    VinkTextField("Đích mới", TextFieldValue(""), {}, inputModifier = Modifier.focusRequester(destination).testTag("destination"))
                    VinkDialog(open.value, "Mẫu", { open.value = false },
                        initialFocus = safe, returnFocus = trigger, closeReason = reason.value) {
                        VinkButton("Đóng", { open.value = false }, Modifier.focusRequester(safe).testTag("safe"))
                    }
                }
            }
        }
        onNodeWithTag("trigger").performClick()
        onNodeWithTag("safe").assertIsFocused().performKeyInput { pressKey(Key.Escape) }
        onNodeWithTag("trigger").assertIsFocused()
        onNodeWithTag("trigger").performClick()
        onNodeWithTag("safe").assertIsFocused()
        runOnIdle { reason.value = CloseReason.Navigate; open.value = false; destination.requestFocus() }
        onNodeWithTag("destination").assertIsFocused()
        onNodeWithTag("trigger").assertIsNotFocused()
    }

    @Test fun rejectedOutsideDismissKeepsSheetAndDisposedCallbackCannotCloseReopenedSheet() = runComposeUiTest {
        val open = mutableStateOf(true)
        var dismissals = 0
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false) {
                VinkDialog(open.value, "Mẫu", { dismissals++ }, kind = DialogKind.Sheet,
                    dismiss = DismissPolicy.EscapeAndOutside) { VinkButton("Giữ", {}) }
            }
        }
        val scrim = onNodeWithContentDescription("Đóng Mẫu")
        val oldDismiss = scrim.fetchSemanticsNode().config[SemanticsActions.OnClick].action!!
        scrim.performClick()
        onNodeWithText("Giữ").assertExists()
        assertEquals(1, dismissals)
        runOnIdle { open.value = false }
        onNodeWithText("Giữ").assertDoesNotExist()
        runOnIdle { open.value = true }
        onNodeWithText("Giữ").assertExists()
        runOnIdle { oldDismiss() }
        assertEquals(1, dismissals)
    }

    @Test fun motionRetargetsSnapsOnReductionAndStopsWithInactiveAncestorOrDisposal() = runComposeUiTest {
        val selected = mutableStateOf(false)
        val reduced = mutableStateOf(false)
        val active = mutableStateOf(true)
        val mounted = mutableStateOf(true)
        lateinit var color: State<Color>
        lateinit var entrance: State<Float>
        mainClock.autoAdvance = false
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false, systemReducedMotion = reduced.value) {
                VinkMotionActivity(active.value) {
                    VinkMotionActivity(true) {
                        if (mounted.value) {
                            color = rememberEffectColor(if (selected.value) Color.White else Color.Black)
                            entrance = rememberOverlayEntrance(spatial = true)
                        }
                    }
                }
            }
        }
        mainClock.advanceTimeByFrame()
        runOnIdle { selected.value = true }
        mainClock.advanceTimeBy(64)
        runOnIdle { assertTrue(color.value.red > 0f && color.value.red < 1f) }
        runOnIdle { selected.value = false }
        mainClock.advanceTimeBy(400)
        runOnIdle { assertEquals(Color.Black, color.value); assertEquals(1f, entrance.value) }
        runOnIdle { selected.value = true }
        mainClock.advanceTimeBy(48)
        runOnIdle { reduced.value = true }
        mainClock.advanceTimeByFrame()
        waitForIdle()
        runOnIdle { assertEquals(Color.White, color.value) }
        runOnIdle { reduced.value = false; selected.value = false }
        mainClock.advanceTimeBy(48)
        runOnIdle { active.value = false }
        mainClock.advanceTimeByFrame()
        waitForIdle()
        runOnIdle { assertEquals(Color.Black, color.value) }
        runOnIdle { active.value = true; selected.value = true }
        mainClock.advanceTimeBy(48)
        runOnIdle { mounted.value = false }
        mainClock.advanceTimeByFrame()
        val stopped = color.value
        mainClock.advanceTimeBy(1000)
        runOnIdle { assertEquals(stopped, color.value) }
    }
}
