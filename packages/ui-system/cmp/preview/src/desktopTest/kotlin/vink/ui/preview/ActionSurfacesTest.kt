package vink.ui.preview

import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.*
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.jetbrains.skia.EncodedImageFormat
import vink.ui.*

/** JVM/offscreen evidence only: does not qualify Android/iOS, OS IME or screen readers. */
@OptIn(ExperimentalTestApi::class)
class ActionSurfacesTest {
    @Test
    fun tonalIconAndToggleKeepFocusAndDispatchOnlyWhenAllowed() = runComposeUiTest {
        val state = mutableStateOf<ActionState>(ActionState.Enabled)
        val value = mutableStateOf(ToggleValue.Off)
        val phase = mutableStateOf<TogglePhase>(TogglePhase.Idle)
        val blocked = mutableStateOf<String?>(null)
        val surface = mutableStateOf(ActionSurface.Standard)
        var icons = 0
        var toggles = 0
        var recoveries = 0
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false) {
                Column {
                    VinkIconButton(IconName.Edit, "Sửa tiêu đề", { icons++; state.value = ActionState.Pending("Đang lưu…") },
                        Modifier.testTag("icon"), state = state.value, surface = surface.value)
                    vink.patterns.EndorsementAction(vink.patterns.EndorsementKind.Work, vink.patterns.EndorsementFacts(value = ToggleValue.Off), {}, {},
                        Modifier.testTag("endorsement"), surface = ActionSurface.Tonal)
                    VinkToggleAction(ToggleCopy("Ghi nhận", "Bỏ ghi nhận"), value.value,
                        { toggles++; phase.value = TogglePhase.Pending(it) }, { recoveries++ },
                        Modifier.testTag("toggle"), phase = phase.value, blocked = blocked.value,
                        icon = IconName.PenFlight, surface = surface.value)
                }
            }
        }
        onNodeWithTag("endorsement").assertContentDescriptionEquals("Thấm")
        val icon = onNodeWithTag("icon")
        val toggle = onNodeWithTag("toggle")
        val initial = icon.fetchSemanticsNode().boundsInRoot
        icon.performSemanticsAction(SemanticsActions.RequestFocus)
        runOnIdle { surface.value = ActionSurface.Tonal }
        icon.assertIsFocused()
        assertEquals(initial, icon.fetchSemanticsNode().boundsInRoot)
        for (key in listOf(Key.Enter, Key.Spacebar)) {
            runOnIdle { state.value = ActionState.Enabled }
            icon.performKeyInput { pressKey(key) }
            icon.performKeyInput { pressKey(key) }
            icon.assertIsFocused()
        }
        assertEquals(2, icons)
        runOnIdle { state.value = ActionState.Enabled }
        icon.performTouchInput { click(center) }; icon.performTouchInput { click(center) }
        assertEquals(3, icons)
        icon.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Đang lưu…"))
        assertEquals(initial.width, icon.fetchSemanticsNode().boundsInRoot.width)
        val touch = icon.fetchSemanticsNode().touchBoundsInRoot
        assertTrue(touch.width >= 48f && touch.height >= 48f, "Material touch target: $touch (drawn $initial)")
        runOnIdle { state.value = ActionState.Disabled("Mẫu · Không khả dụng.") }
        icon.assertIsNotEnabled(); icon.performTouchInput { click(center) }
        assertEquals(3, icons)

        toggle.performSemanticsAction(SemanticsActions.RequestFocus)
        val before = toggle.fetchSemanticsNode().boundsInRoot
        toggle.performKeyInput { pressKey(Key.Enter) }
        toggle.performKeyInput { pressKey(Key.Spacebar) }
        toggle.performClick()
        assertEquals(1, toggles)
        toggle.assertIsFocused()
        assertEquals(before.width, toggle.fetchSemanticsNode().boundsInRoot.width)
        toggle.assertContentDescriptionEquals("Ghi nhận")
        runOnIdle { phase.value = TogglePhase.Uncertain(true) }
        toggle.performClick()
        assertEquals(1, toggles)
        onNodeWithText("Kiểm tra lại").performClick()
        assertEquals(1, recoveries)
        runOnIdle { value.value = ToggleValue.On; phase.value = TogglePhase.Failed }
        toggle.assertContentDescriptionEquals("Bỏ ghi nhận")
        toggle.performSemanticsAction(SemanticsActions.RequestFocus)
        toggle.performKeyInput { pressKey(Key.Spacebar) }
        assertEquals(2, toggles)
        runOnIdle { value.value = ToggleValue.Unknown; phase.value = TogglePhase.Idle }
        toggle.performClick()
        assertEquals(2, toggles)
        runOnIdle { blocked.value = "Mẫu · Không khả dụng." }
        toggle.assertIsNotEnabled(); toggle.performClick()
        assertEquals(2, toggles)
        assertTrue(before.width >= 48f && before.height >= 48f)
    }

    @Test
    fun capturePairedSurfacesAndLongVietnameseLabels() {
        val out = File(System.getProperty("vink.captures") ?: "build/captures").apply { mkdirs() }
        val fonts = PreviewFonts.load()
        for (theme in listOf(ThemePreference.Light, ThemePreference.Dark)) {
            for (scale in listOf(1f, 2f)) {
                for (scenario in listOf("hierarchy", "icons", "toggles", "long-label")) {
                    val height = if (scenario == "toggles") 2200 else 1300
                    val scene = ImageComposeScene(390, height, Density(1f, 1f)) {
                        CompositionLocalProvider(LocalDensity provides Density(1f, scale)) {
                            VinkTheme(VinkPreferences(theme = theme, motion = MotionPreference.Reduce), systemDark = false, fonts = fonts.fonts) {
                                VinkSurface(Modifier.fillMaxSize(), tone = SurfaceTone.Low) {
                                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                                        VinkText("Mẫu · Hành động dùng chung", role = TypeRole.Title)
                                        when (scenario) {
                                            "hierarchy" -> {
                                                VinkButton("Lưu bản nháp", {}, leading = IconName.Edit)
                                                VinkButton("Xem trước", {}, variant = ButtonVariant.Tonal, leading = IconName.Visibility)
                                                VinkButton("Hủy", {}, variant = ButtonVariant.Text)
                                                VinkButton("Tạo bản nháp mới cho những con phố cũ của Hà Nội", {}, leading = IconName.Add, state = ActionState.Pending("Đang lưu…"))
                                            }
                                            "icons" -> ComponentSpecimen("action.icon-button", "surfaces")
                                            "toggles" -> EndorsementSpecimen("action.toggle-action", "surfaces")
                                            else -> EndorsementSpecimen("action.toggle-action", "tonal-long-label")
                                        }
                                    }
                                }
                            }
                        }
                    }
                    try {
                        scene.render() // settle Text.onTextLayout before the clean frame
                        scene.render().encodeToData(EncodedImageFormat.PNG)!!.use {
                            File(out, "action-surfaces--$scenario--${theme.value}--390--text${(scale * 100).toInt()}.png").writeBytes(it.bytes)
                        }
                    } finally { scene.close() }
                }
            }
        }
        File(out, "action-surfaces-context.json").writeText("""{"host":"desktop-offscreen","width_dp":390,"density":1,"os_font_scale":1,"preview_scales":[1,2],"themes":["light","dark"],"motion":"reduce","font":"PreviewFonts self-hosted TTF","data_mode":"synthetic","native_runtime":"NOT_RUN"}""")
    }
}
