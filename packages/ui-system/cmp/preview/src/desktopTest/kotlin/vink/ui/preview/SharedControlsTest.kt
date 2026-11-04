package vink.ui.preview

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toAwtImage
import androidx.compose.ui.test.captureToImage
import java.io.File
import javax.imageio.ImageIO
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextInputSelection
import androidx.compose.ui.test.runComposeUiTest
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import vink.ui.ActionState
import vink.ui.ButtonVariant
import vink.ui.FieldMode
import vink.ui.FieldVariant
import vink.ui.VinkButton
import vink.ui.VinkFilterChip
import vink.ui.VinkPreferences
import vink.ui.VinkTextField
import vink.ui.VinkTheme

/** JVM Compose interaction evidence, not OS IME or native-device qualification. */
@OptIn(ExperimentalTestApi::class)
class SharedControlsTest {
    @Test
    fun floatingFieldKeepsCompositionSelectionAndReadonlyAcrossRecomposition() = runComposeUiTest {
        val initial = TextFieldValue("Tiếng Việt", TextRange(3), TextRange(0, 3))
        val value = mutableStateOf(initial)
        val dark = mutableStateOf(false)
        val mode = mutableStateOf(FieldMode.Editable)
        val error = mutableStateOf<String?>(null)
        val fonts = PreviewFonts.load()
        setContent {
            VinkTheme(VinkPreferences(), systemDark = dark.value, fonts = fonts.fonts) {
                VinkTextField("Tên bản thảo", value.value, { value.value = it }, Modifier.testTag("title"),
                    variant = FieldVariant.FloatingTonal, mode = mode.value, error = error.value, help = "Có thể sửa sau.")
            }
        }
        runOnIdle { dark.value = true }
        waitForIdle()
        assertEquals(initial, value.value, "theme recomposition preserves the original TextFieldValue")
        val field = onNode(hasText("Tên bản thảo") and hasAnyAncestor(hasTestTag("title")))
        assertEquals(initial.text, field.fetchSemanticsNode().config[SemanticsProperties.EditableText].text)
        field.assert(SemanticsMatcher.expectValue(SemanticsProperties.TextSelectionRange, TextRange(3)))
        field.performTextInputSelection(TextRange(3))
        field.performTextInput("x")
        waitForIdle()
        assertEquals("Tiex̂́ng Việt", value.value.text)
        assertEquals(TextRange(4), value.value.selection)
        runOnIdle { error.value = "Lỗi mẫu khi đang nhập" }
        waitForIdle()
        field.assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, "Lỗi mẫu khi đang nhập"))
        val out = File(System.getProperty("vink.captures") ?: "build/captures")
        out.mkdirs()
        ImageIO.write(field.captureToImage().toAwtImage(), "png", File(out, "shared-controls--error-focus--dark.png"))
        runOnIdle { error.value = null; mode.value = FieldMode.ReadOnly }
        waitForIdle()
        val readonly = value.value
        field.assert(SemanticsMatcher.keyNotDefined(androidx.compose.ui.semantics.SemanticsActions.SetText))
        field.performTextInputSelection(TextRange(0, 3))
        waitForIdle()
        assertEquals(readonly.text, value.value.text)
        runOnIdle { mode.value = FieldMode.Disabled }
        waitForIdle()
        field.assertIsNotEnabled()
    }

    @Test
    fun textActionAndChipKeepWidthWhenStatusChanges() = runComposeUiTest {
        val pending = mutableStateOf(false)
        val selected = mutableStateOf(false)
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false) {
                androidx.compose.foundation.layout.Column {
                    VinkButton("Xem trước", {}, Modifier.testTag("action"), variant = ButtonVariant.Text,
                        state = if (pending.value) ActionState.Pending("Đang xử lý…") else ActionState.Enabled)
                    VinkFilterChip("Thơ tiếng Việt", selected.value, {}, Modifier.testTag("chip"))
                }
            }
        }
        val button = onNodeWithTag("action").fetchSemanticsNode().boundsInRoot
        val chip = onNodeWithTag("chip").fetchSemanticsNode().boundsInRoot
        runOnIdle { pending.value = true; selected.value = true }
        waitForIdle()
        assertEquals(button.width, onNodeWithTag("action").fetchSemanticsNode().boundsInRoot.width)
        assertEquals(chip.width, onNodeWithTag("chip").fetchSemanticsNode().boundsInRoot.width)
        assertTrue(button.height >= 48f && chip.height >= 48f)
    }
}
