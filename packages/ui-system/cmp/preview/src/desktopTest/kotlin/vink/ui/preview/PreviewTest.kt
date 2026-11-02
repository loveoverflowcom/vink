package vink.ui.preview

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.ComposeUiTest
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.runComposeUiTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import vink.ui.ThemePreference
import vink.ui.VinkCatalog

@OptIn(ExperimentalTestApi::class)
class PreviewTest {
    /** A text node inside the tagged region (tags sit on containers, text on their children). */
    private fun ComposeUiTest.textUnder(tag: String, text: String) =
        onNode(hasText(text, substring = true) and hasAnyAncestor(hasTestTag(tag)), useUnmergedTree = true).assertExists()

    private val fonts = PreviewFonts.load()

    @Test
    fun everySharedCatalogEntryIsRegisteredHere() {
        assertEquals(VinkCatalog.entries.map { it.id }, REGISTERED)
        for (entry in VinkCatalog.entries) assertTrue(entry.cmp in VinkCatalog.statuses, entry.id)
    }

    @Test
    fun eachScenarioRendersItsSpecimen() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false))
        setContent { Workbench(model, fonts) }
        for (entry in VinkCatalog.entries) {
            for (scenario in entry.scenarios) {
                model.update("test") { it.copy(entry = entry.id, scenario = scenario.id) }
                waitForIdle()
                onNodeWithTag("specimen:${entry.id}:${scenario.id}").assertExists()
                textUnder("inspector-id", "${entry.id} / ${scenario.id}")
            }
        }
        textUnder("inspector-status", "implemented-unverified")
    }

    @Test
    fun controlsChangeTheEffectiveThemeAndLogEvents() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false))
        setContent { Workbench(model, fonts) }
        onNodeWithTag("theme:Tối").performClick()
        waitForIdle()
        assertEquals(ThemePreference.Dark, model.config.theme)
        textUnder("effective:color.surface", "#17121d")
        onNodeWithTag("theme:Hệ thống").performClick()
        waitForIdle()
        textUnder("effective:color.surface", "#fffaff")
        assertTrue(model.log.first().contains("giao diện → Hệ thống"))
        assertTrue(model.log.first().startsWith("2026-11-02T09:00:00+07:00"))
    }

    @Test
    fun reducedScenarioStopsTheLoopAndCompactKeepsTheHitTarget() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false, entry = "foundation.motion"))
        setContent { Workbench(model, fonts) }
        val running = SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "đang chạy")
        val stopped = SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "dừng")
        mainClock.autoAdvance = false
        onNodeWithTag("loop").assert(running)
        model.update("test") { it.copy(scenario = "reduced") }
        mainClock.advanceTimeByFrame()
        onNodeWithTag("loop").assert(stopped)
        model.update("test") { it.copy(scenario = "default", systemReducedMotion = true) }
        mainClock.advanceTimeByFrame()
        onNodeWithTag("loop").assert(stopped)
        mainClock.autoAdvance = true
        model.update("test") { it.copy(entry = "foundation.shape-spacing", scenario = "compact", systemReducedMotion = false) }
        waitForIdle()
        val height = onNodeWithTag("control:Lưu bản nháp").fetchSemanticsNode().size.height
        assertTrue(height >= 44, "compact control is $height px tall at density 1")
    }

    @Test
    fun searchIgnoresVietnameseMarksAndSaysWhenNothingMatches() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false))
        setContent { Workbench(model, fonts) }
        onNodeWithTag("catalog-search").performTextInput("chuyen dong")
        waitForIdle()
        onNodeWithTag("catalog-link:foundation.motion").assertExists()
        onNodeWithTag("catalog-link:foundation.typography").assertDoesNotExist()
        model.update("test") { it.copy(query = "zzz") }
        waitForIdle()
        onNodeWithTag("catalog-empty").assertExists()
        assertTrue(matches(VinkCatalog.entries[1], "DẤU TỔ HỢP"))
    }

    @Test
    fun selfHostedFacesDecodeOnDesktop() {
        assertEquals(3, fonts.loaded.size, fonts.status)
    }
}
