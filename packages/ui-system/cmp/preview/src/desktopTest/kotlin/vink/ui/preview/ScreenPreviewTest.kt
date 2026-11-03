package vink.ui.preview

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.*
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
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
    fun meThemeIsLocalHasOneSelectionAndResetsOnlyOnRemount() = runComposeUiTest {
        var scenario by mutableStateOf("default")
        var mount by mutableStateOf(0)
        var dark by mutableStateOf(false)
        val preferences = VinkPreferences()
        val events = mutableListOf<String>()
        val radio = SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.RadioButton)
        fun assertSelection(label: String) {
            onAllNodes(radio).assertCountEquals(3)
            onAllNodes(radio and isSelected()).assertCountEquals(1)
            onNode(hasText(label) and radio).assertIsSelected()
        }
        setContent {
            CompositionLocalProvider(LocalPreviewLog provides { events.add(it) }) {
                VinkTheme(preferences, systemDark = dark, fonts = fonts.fonts) {
                    Column(Modifier.verticalScroll(rememberScrollState())) {
                        key(scenario, mount) { ScreenRecipe("screen.me", scenario) }
                    }
                }
            }
        }
        for (sample in listOf("default", "auth-required", "long")) {
            runOnIdle { scenario = sample }
            assertSelection("Hệ thống")
            for (label in listOf("Sáng", "Tối", "Hệ thống")) {
                onNode(hasText(label) and radio).performScrollTo().performClick()
                assertSelection(label)
            }
            for ((label, key) in listOf("Sáng" to Key.Enter, "Tối" to Key.Spacebar, "Hệ thống" to Key.Enter)) {
                onNode(hasText(label) and radio).performScrollTo()
                    .performSemanticsAction(SemanticsActions.RequestFocus) { it() }
                    .performKeyInput { pressKey(key) }
                    .assertIsFocused()
                assertSelection(label)
            }
            onNode(hasText("Tối") and radio).performClick()
                .performSemanticsAction(SemanticsActions.RequestFocus) { it() }
            runOnIdle { dark = !dark }
            assertSelection("Tối")
            onNode(hasText("Tối") and radio).assertIsFocused()
            runOnIdle { mount++ }
            assertSelection("Hệ thống")
        }
        runOnIdle {
            assertEquals(ThemePreference.System, preferences.theme)
            assertEquals(emptyList(), events)
        }
    }

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
    fun recipesAcknowledgeOnlyLocalIntentsAndKeepDrafts() = runComposeUiTest {
        var entry by mutableStateOf("screen.reader")
        var scenario by mutableStateOf("feedback")
        var dark by mutableStateOf(false)
        val events = mutableListOf<String>()
        val text = "Góp ý chưa gửi\n\n  giữ nguyên khổ thơ. Tiếng Việt: Ỗ Ữ Đường, Ỗ Ữ."
        setContent {
            CompositionLocalProvider(LocalPreviewLog provides { events.add(it) }) {
                VinkTheme(VinkPreferences(), systemDark = dark, fonts = fonts.fonts) {
                    Column(Modifier.verticalScroll(rememberScrollState())) {
                        key(entry, scenario) { ScreenRecipe(entry, scenario) }
                    }
                }
            }
        }
        for ((id, sample) in listOf("screen.reader" to "feedback", "composition.feedback" to "default", "composition.feedback" to "error")) {
            runOnIdle { entry = id; scenario = sample; events.clear() }
            val field = onNode(hasSetTextAction())
            field.performTextReplacement(text)
            field.performTextInputSelection(TextRange(4, 9))
            // Invoke the action without a pointer moving focus: this isolates the local
            // status update from the platform's normal selection collapse on input blur.
            onNodeWithText("Gửi nhận xét").performScrollTo().performSemanticsAction(SemanticsActions.OnClick) { it() }
            onNodeWithTag("sample-result").assertTextEquals("Mẫu · Intent gửi nhận xét, chưa gửi hoặc lưu.")
            runOnIdle { assertEquals(listOf("feedback → submit intent (sample)"), events); dark = !dark }
            field.assertTextContains(text).assertIsFocused()
            assertEquals(TextRange(4, 9), field.fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange])
            if (sample == "error") {
                onNodeWithText("Chưa tải/lưu được. Chữ đang viết được giữ nguyên; hãy thử lại.").assertExists()
                onNodeWithText("Thử lại").performScrollTo().performSemanticsAction(SemanticsActions.OnClick) { it() }
                onNodeWithTag("sample-result").assertTextEquals("Mẫu · Đã yêu cầu tải lại; không gọi dịch vụ.")
                field.assertTextContains(text).assertIsFocused()
                assertEquals(TextRange(4, 9), field.fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange])
            }
        }
    }

    @Test
    fun blockedRecipeStatesEmitNoIntentOrAcknowledgment() = runComposeUiTest {
        var scenario by mutableStateOf("pending")
        val events = mutableListOf<String>()
        setContent {
            CompositionLocalProvider(LocalPreviewLog provides { events.add(it) }) {
                VinkTheme(VinkPreferences(), fonts = fonts.fonts) {
                    Column(Modifier.verticalScroll(rememberScrollState())) {
                        key(scenario) { ScreenRecipe("composition.feedback", scenario) }
                    }
                }
            }
        }
        for (sample in listOf("pending", "auth-required", "uncertain")) {
            runOnIdle { scenario = sample }
            onNodeWithText("Gửi nhận xét").performScrollTo().performClick()
            onNodeWithTag("sample-result").assertTextEquals("")
            runOnIdle { assertEquals(emptyList(), events) }
        }
    }
}
