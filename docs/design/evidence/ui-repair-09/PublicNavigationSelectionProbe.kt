package vink.ui.preview

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInputSelection
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.runComposeUiTest
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import kotlin.test.Test
import kotlin.test.assertEquals
import vink.ui.*

@OptIn(ExperimentalTestApi::class)
class PublicNavigationSelectionProbe {
    @Test
    fun untouchedPublicPrimitivesCompareToggleAndUnrelatedBlur() = runComposeUiTest {
        val fonts = PreviewFonts.load()
        var layout by mutableStateOf(NavLayout.RailCompact)
        var value by mutableStateOf(TextFieldValue("Chữ riêng chưa lưu: đường về, mưa và gió."))
        val changes = mutableListOf<String>()
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false, fonts = fonts.fonts) {
                Column {
                    VinkButton("Không đổi layout", {}, Modifier.testTag("unrelated"))
                    Box(Modifier.width(900.dp).height(460.dp)) {
                        VinkAdaptiveScaffold(
                            layout = layout,
                            rail = {
                                VinkNavigationRail(
                                    expanded = layout.railExpanded,
                                    onToggle = { changes += "toggle handler sees ${value.selection}"; layout = if (layout.railExpanded) NavLayout.RailCompact else NavLayout.RailExpanded },
                                    destinations = listOf(NavDestination("explore", "Khám phá", IconName.Explore)),
                                    selected = "explore", onNavigate = {},
                                    expandLabel = "Mở rộng điều hướng", collapseLabel = "Thu gọn điều hướng",
                                )
                            },
                            bar = {},
                        ) {
                            VinkTextArea("Chữ mẫu", value, {
                                changes += "field onValueChange ${value.selection} -> ${it.selection}"
                                value = it
                            }, Modifier.testTag("probe-draft"))
                        }
                    }
                }
            }
        }
        val field = onNode(hasSetTextAction() and hasAnyAncestor(hasTestTag("probe-draft")), useUnmergedTree = true)
        fun select() {
            field.performTextReplacement("Chữ riêng chưa lưu: đường về, mưa và gió.")
            field.performTextInputSelection(TextRange(3, 9))
            waitForIdle()
            assertEquals(TextRange(3, 9), field.fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange])
        }
        select()
        onNode(hasContentDescription("Mở rộng điều hướng")).performClick()
        waitForIdle()
        println("PUBLIC_PRIMITIVE_AFTER_TOGGLE=${value.selection}")
        println("PUBLIC_PRIMITIVE_EVENTS_TOGGLE=${changes.joinToString(" | ")}")
        changes.clear()
        select()
        onNodeWithTag("unrelated").performClick()
        waitForIdle()
        println("PUBLIC_PRIMITIVE_AFTER_UNRELATED_BLUR=${value.selection}")
        println("PUBLIC_PRIMITIVE_EVENTS_BLUR=${changes.joinToString(" | ")}")
        changes.clear()
        select()
        layout = NavLayout.RailCompact
        waitForIdle()
        println("PUBLIC_PRIMITIVE_AFTER_LAYOUT_ONLY=${value.selection}")
        println("PUBLIC_PRIMITIVE_EVENTS_LAYOUT_ONLY=${changes.joinToString(" | ")}")
        assertEquals(TextRange(3, 9), value.selection)
    }
}
