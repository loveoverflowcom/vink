package vink.ui.preview

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertWidthIsEqualTo
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.runComposeUiTest
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.unit.dp
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import vink.ui.IconName
import vink.ui.NavDestination
import vink.ui.NavLayout
import vink.ui.VinkAdaptiveScaffold
import vink.ui.VinkBottomNavigation
import vink.ui.VinkButton
import vink.ui.VinkFixture
import vink.ui.VinkNavigationRail
import vink.ui.VinkPreferences
import vink.ui.VinkTheme

private fun SemanticsNodeInteraction.tap() = performScrollTo().performClick()

/**
 * The diagnostic adapter and the CMP navigation primitives on the JVM desktop. This is preview
 * evidence: no Android/iOS back, predictive back, lifecycle or IME is exercised here.
 */
@OptIn(ExperimentalTestApi::class)
class NavigationPreviewTest {
    private val fonts = PreviewFonts.load()

    @Test
    fun deepLinksRoundTripAndNeverLeaveTheApp() {
        val routes = listOf(
            Route.Explore(), Route.Explore("Đường về & 100% + #?/"), Route.Reader("33333333-3333-4333-8333-333333333333", "a=b&q=c"),
            Route.Desk, Route.Editor("ban-nhap-mau"), Route.Me,
        )
        for (route in routes) assertEquals(route, DiagnosticLinks.parse(DiagnosticLinks.link(route)), DiagnosticLinks.link(route))
        for (raw in listOf("//outside.example/toi", "https://outside.example/", "toi", "/ban-vietnam", "/tac-pham/a/b", "/toi/khac", "/?q=%E0%A4%A", "/?q=%", "/tac-pham/%2F..%2F")) {
            assertTrue(DiagnosticLinks.parse(raw) is Route.NotFound, raw)
        }
        assertEquals(Route.Explore("returnTo=https://x"), DiagnosticLinks.parse("/?q=returnTo%3Dhttps%3A%2F%2Fx&returnTo=https://outside.example"))
    }

    @Test
    fun backClosesTheOverlayFirstAndIntentsAddNoDuplicateEntries() {
        val nav = DiagnosticNavigator()
        assertEquals(NavResult.Pushed, nav.navigate(Route.Desk))
        assertEquals(NavResult.Ignored, nav.navigate(Route.Desk))
        nav.overlays.open(PROFILE)
        assertEquals(BackResult.ClosedOverlay, nav.back())
        assertEquals(Route.Desk, nav.current)
        assertEquals(BackResult.Popped, nav.back())
        assertEquals(BackResult.Exit, nav.back())
        nav.overlays.open(PROFILE)
        assertEquals(NavResult.Pushed, nav.navigate(Route.Me), "a navigation closes the overlay first")
        assertEquals(null, nav.overlays.active)
    }

    @Test
    fun dirtyTextHoldsBackAndNavigationUntilThePersonChooses() {
        val nav = DiagnosticNavigator(Route.Editor("ban-nhap-mau"))
        nav.edit(nav.draft.copy(text = nav.draft.text + " Thêm một dòng."))
        assertEquals(BackResult.Held, nav.back())
        assertEquals(Route.Editor("ban-nhap-mau"), nav.current)
        nav.stay()
        assertEquals(NavResult.Held, nav.navigate(Route.Explore()))
        assertEquals(1, nav.stack.size, "a held intent changes nothing")
        nav.stay()
        assertEquals(BackResult.Held, nav.back(), "a stale held intent never runs later")
        nav.leave()
        assertEquals(1, nav.stack.size, "leaving the only screen does not invent one")
        assertTrue(nav.draft.text.endsWith("Thêm một dòng."), "the host still holds the text")
    }

    @Test
    fun layoutChangesKeepTheContentMounted() = runComposeUiTest {
        var layout by mutableStateOf(NavLayout.RailExpanded)
        val destinations = listOf(NavDestination("explore", "Khám phá", IconName.Explore), NavDestination("desk", "Bàn viết", IconName.Edit))
        setContent {
            VinkTheme(VinkPreferences(), systemDark = false, fonts = fonts.fonts) {
                VinkAdaptiveScaffold(
                    layout = layout,
                    rail = { VinkNavigationRail(layout.railExpanded, {}, destinations, "explore", {}, "Mở rộng điều hướng", "Thu gọn điều hướng", Modifier.testTag("rail")) },
                    bar = { VinkBottomNavigation(destinations, "explore", {}, Modifier.testTag("bar")) },
                ) {
                    var count by remember { mutableIntStateOf(0) }
                    Column {
                        VinkButton("Đếm", { count++ }, Modifier.testTag("count"))
                        Text("n=$count", Modifier.testTag("value"))
                    }
                }
            }
        }
        onNodeWithTag("count").performClick()
        onNodeWithTag("rail").assertWidthIsEqualTo(220.dp)
        for (next in listOf(NavLayout.Bar, NavLayout.RailCompact, NavLayout.BarHidden, NavLayout.RailExpanded)) {
            layout = next
            waitForIdle()
            onNodeWithText("n=1").assertExists("remembered state survives ${next.id}")
            assertEquals(next.isRail, onAllNodes(hasTestTag("rail")).fetchSemanticsNodes().isNotEmpty(), next.id)
            assertEquals(next == NavLayout.Bar, onAllNodes(hasTestTag("bar")).fetchSemanticsNodes().isNotEmpty(), next.id)
        }
        layout = NavLayout.RailCompact
        waitForIdle()
        onNodeWithTag("rail").assertWidthIsEqualTo(96.dp)
    }

    @Test
    fun toggleExpandsAndCollapsesWithoutLosingThePageOrSelection() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false, entry = "navigation.scaffold", scenario = "toggle"))
        setContent { Workbench(model, fonts) }
        waitForIdle()
        val draft = onNode(hasSetTextActionTag("nav-draft"), useUnmergedTree = true)
        onNodeWithTag("nav-rail").assertWidthIsEqualTo(96.dp)
        onNode(hasContentDescription("Mở rộng điều hướng")).tap()
        onNodeWithTag("nav-rail").assertWidthIsEqualTo(220.dp)
        onNode(hasContentDescription("Thu gọn điều hướng")).tap()
        onNodeWithTag("nav-rail").assertWidthIsEqualTo(96.dp)
        val config = draft.fetchSemanticsNode().config
        assertEquals(VinkFixture.READING, config[SemanticsProperties.EditableText].text)
        assertEquals(TextRange(5, 12), config[SemanticsProperties.TextSelectionRange])
        onNodeWithTag("nav-route").assertExists()
        assertTrue(model.log.any { "toggle → expanded" in it } && model.log.any { "toggle → compact" in it }, model.log.toString())
    }

    @Test
    fun bottomBarKeepsTheLockedOrderAndNavigatesByKey() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false, entry = "navigation.bottom-bar"))
        setContent { Workbench(model, fonts) }
        waitForIdle()
        val x = listOf("Khám phá", "Bàn viết", "Tôi").map { label ->
            onNode(hasText(label) and hasAnyAncestor(hasTestTag("nav-bottom")), useUnmergedTree = true).fetchSemanticsNode().boundsInRoot.left
        }
        assertEquals(x.sorted(), x, "Khám phá → Bàn viết → Tôi")
        onNode(hasText("Bàn viết") and hasAnyAncestor(hasTestTag("nav-bottom")), useUnmergedTree = true).tap()
        onNode(hasText("Trang hiện tại: Bàn viết")).assertExists()
    }

    @Test
    fun overlayIsTheOnlyModalAndBackClosesItBeforeLeaving() = runComposeUiTest {
        val model = PreviewModel(PreviewConfig(systemDark = false, entry = "navigation.overlay", scenario = "busy"))
        setContent { Workbench(model, fonts) }
        waitForIdle()
        onNodeWithTag("nav-second").performClick()
        assertTrue(model.log.first().contains("từ chối: nav-profile đang mở"), model.log.first())
        onNodeWithTag("nav-back").performClick()
        waitForIdle()
        assertTrue(model.log.first().endsWith("Back → ClosedOverlay"), model.log.first())
        onNodeWithTag("nav-back").assertDoesNotExist()
        onNode(hasText("Trang hiện tại: Khám phá")).assertExists()
        onNodeWithTag("nav-back-outside").tap()
        assertTrue(model.log.first().endsWith("Back → Exit"), model.log.first())
    }

    private fun hasSetTextActionTag(tag: String) = androidx.compose.ui.test.hasSetTextAction() and (hasTestTag(tag) or hasAnyAncestor(hasTestTag(tag)))
}
