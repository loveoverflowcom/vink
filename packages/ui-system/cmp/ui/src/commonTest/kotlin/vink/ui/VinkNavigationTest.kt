package vink.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

/** The same assertions as the Leptos `navigation` tests, against the generated shared vectors. */
class VinkNavigationTest {
    private fun context(id: String) = NavContext(compactRail = "compact-rail" in id, hideBar = "hide-bar" in id)

    @Test
    fun windowClassesMatchTheSharedBreakpoints() {
        assertEquals(VinkNavigationVectors.cssPx, WindowClass.CSS_PX.first.toInt() to WindowClass.CSS_PX.second.toInt())
        assertEquals(VinkNavigationVectors.dp, WindowClass.DP.first.toInt() to WindowClass.DP.second.toInt())
        for ((unit, width, expected) in VinkNavigationVectors.windows) {
            val actual = if (unit == "px") WindowClass.fromCssPx(width.toDouble()) else WindowClass.fromDp(width.toDouble())
            assertEquals(expected, actual.id, "$unit $width")
        }
    }

    @Test
    fun layoutsFollowPreferenceOverContextAndNeverRailOnCompact() {
        assertEquals(36, VinkNavigationVectors.layouts.size)
        for ((window, context, preference, expected) in VinkNavigationVectors.layouts) {
            val w = WindowClass.entries.first { it.id == window }
            val p = if (preference == "-") null else RailPreference.parse(preference) ?: error(preference)
            assertEquals(expected, NavLayout.resolve(w, context(context), p).id, "($window, $context, $preference)")
        }
    }

    @Test
    fun theToggleFlipsTheRailAsShown() {
        for ((shown, next) in VinkNavigationVectors.toggles) {
            val layout = NavLayout.entries.first { it.id == shown }
            assertEquals(next, layout.nextPreference?.value ?: "-", shown)
        }
        assertNull(RailPreference.parse("220"))
    }

    private fun step(layer: OverlayLayer, op: String): String {
        fun closed(c: Closed?, none: String) = c?.let { "closed:${it.id}:${it.reason.id}" } ?: none
        val parts = op.split(':')
        return when (parts[0]) {
            "open" -> when (val r = layer.open(parts[1])) {
                OpenResult.Ok -> "ok"
                is OpenResult.Busy -> "busy:${r.active}"
            }
            "close" -> closed(layer.close(parts[1], CloseReason.parse(parts[2]) ?: error(op)), "stale")
            "back" -> closed(layer.back(), "pass")
            "escape" -> closed(layer.escape(), "pass")
            else -> error(op)
        }
    }

    @Test
    fun oneModalIsActiveAndBackClosesItBeforeLeaving() {
        for ((name, steps) in VinkNavigationVectors.overlay) {
            val layer = OverlayLayer()
            for ((op, expected) in steps) assertEquals(expected, step(layer, op), "$name: $op")
        }
    }

    @Test
    fun focusReturnsToAVisibleTriggerOrMainAndNavigationLeavesIt() {
        for ((reason, visible, expected) in VinkNavigationVectors.focus) {
            assertEquals(expected, (CloseReason.parse(reason) ?: error(reason)).focusReturn(visible).id, "$reason $visible")
        }
    }
}
