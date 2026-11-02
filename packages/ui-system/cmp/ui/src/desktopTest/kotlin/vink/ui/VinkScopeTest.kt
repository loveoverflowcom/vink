package vink.ui

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.runComposeUiTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/** Composition-level checks of VinkTheme/VinkScope precedence on the desktop JVM target. */
@OptIn(ExperimentalTestApi::class)
class VinkScopeTest {
    private fun capture(block: (MutableMap<String, VinkScopeState>) -> @androidx.compose.runtime.Composable () -> Unit): Map<String, VinkScopeState> {
        val seen = mutableMapOf<String, VinkScopeState>()
        runComposeUiTest {
            setContent(block(seen))
            waitForIdle()
        }
        return seen
    }

    @Test
    fun nestedScopesOverrideAndInheritLikeTheCssScopes() {
        val seen = capture { seen ->
            {
                VinkTheme(VinkPreferences(theme = ThemePreference.Dark), systemDark = false) {
                    seen["root"] = LocalVink.current
                    VinkScope {
                        seen["inherit"] = LocalVink.current
                    }
                    VinkScope(theme = ThemePreference.Light) {
                        seen["light"] = LocalVink.current
                    }
                    VinkScope(theme = ThemePreference.System) {
                        seen["system"] = LocalVink.current
                    }
                }
            }
        }
        assertEquals(ResolvedTheme.Dark, seen.getValue("root").theme)
        assertEquals(ResolvedTheme.Dark, seen.getValue("inherit").theme)
        assertEquals(ResolvedTheme.Light, seen.getValue("light").theme)
        assertEquals(ResolvedTheme.Light, seen.getValue("system").theme, "System inside Dark follows the light system")
        assertEquals(ColorRole.Surface.light, seen.getValue("light").colors.argb(ColorRole.Surface))
    }

    @Test
    fun reducedMotionIsAdditiveAndDensityTakesTheNearestValue() {
        val seen = capture { seen ->
            {
                VinkTheme(VinkPreferences(density = Density.Compact), systemDark = false, systemReducedMotion = false) {
                    VinkScope(reduceMotion = true) {
                        seen["reduced"] = LocalVink.current
                        VinkScope(density = Density.Comfortable) {
                            seen["inner"] = LocalVink.current
                        }
                    }
                }
            }
        }
        assertTrue(seen.getValue("reduced").motion.reduced)
        assertEquals(Density.Compact, seen.getValue("reduced").density)
        assertTrue(seen.getValue("inner").motion.reduced, "a descendant cannot restore motion")
        assertEquals(Density.Comfortable, seen.getValue("inner").density)
    }

    @Test
    fun overridesFollowTheScopeAndInvalidSetsAreReported() {
        val swap = listOf(RoleOverride(ColorRole.Primary, ColorRole.Tertiary))
        val seen = capture { seen ->
            {
                VinkTheme(VinkPreferences(theme = ThemePreference.Light), systemDark = false, overrides = swap) {
                    VinkScope {
                        seen["inherited"] = LocalVink.current
                    }
                    VinkScope(theme = ThemePreference.Dark) {
                        seen["rethemed"] = LocalVink.current
                    }
                    VinkScope(overrides = listOf(RoleOverride(ColorRole.Primary, ColorRole.Primary))) {
                        seen["invalid"] = LocalVink.current
                    }
                }
            }
        }
        assertEquals(ColorRole.Tertiary.light, seen.getValue("inherited").colors.argb(ColorRole.ActionPrimaryContainer))
        assertEquals(ColorRole.Primary.dark, seen.getValue("rethemed").colors.argb(ColorRole.Primary))
        assertNotNull(seen.getValue("invalid").overrideError)
        assertEquals(ColorRole.Tertiary.light, seen.getValue("invalid").colors.argb(ColorRole.Primary), "invalid own overrides are ignored")
    }
}
