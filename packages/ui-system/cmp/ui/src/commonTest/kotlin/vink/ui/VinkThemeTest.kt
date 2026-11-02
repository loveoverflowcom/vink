package vink.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class VinkThemeTest {
    @Test
    fun storedThemeValuesRoundTripAndUnknownFallsBackToSystem() {
        for (theme in ThemePreference.entries) assertEquals(theme, ThemePreference.parse(theme.value))
        assertEquals(ThemePreference.System, ThemePreference.parse("broken"))
        assertEquals(ThemePreference.System, ThemePreference.parse(null))
    }

    @Test
    fun explicitThemesWinOverTheSystem() {
        assertEquals(ResolvedTheme.Light, ThemePreference.Light.resolve(systemDark = true))
        assertEquals(ResolvedTheme.Dark, ThemePreference.Dark.resolve(systemDark = false))
        assertEquals(ResolvedTheme.Dark, ThemePreference.System.resolve(systemDark = true))
        assertEquals(ResolvedTheme.Light, ThemePreference.System.resolve(systemDark = false))
    }

    @Test
    fun everyDeclaredPairMeetsItsThresholdInBothThemes() {
        for (pair in VinkContrastPairs) {
            val light = contrast(pair.fg.light, pair.bg.light)
            val dark = contrast(pair.fg.dark, pair.bg.dark)
            assertTrue(light >= pair.min && dark >= pair.min, "$pair: $light / $dark")
        }
    }

    @Test
    fun contrastMatchesTheWebBaseline() {
        // docs/design/README.md measured primary on surface at 6.77 (light) and 10.83 (dark).
        val light = contrast(ColorRole.Primary.light, ColorRole.Surface.light)
        val dark = contrast(ColorRole.Primary.dark, ColorRole.Surface.dark)
        assertTrue(kotlin.math.abs(light - 6.77) < 0.01, "$light")
        assertTrue(kotlin.math.abs(dark - 10.83) < 0.01, "$dark")
    }

    @Test
    fun recipesFollowTheirRoleAndScopeOverrides() {
        val plain = VinkColors(ResolvedTheme.Dark)
        assertEquals(ColorRole.Primary.dark, plain.argb(ColorRole.ActionPrimaryContainer))
        val swapped = VinkColors(ResolvedTheme.Light, mapOf(ColorRole.Primary to ColorRole.Tertiary))
        assertEquals(ColorRole.Tertiary.light, swapped.argb(ColorRole.Primary))
        assertEquals(ColorRole.Tertiary.light, swapped.argb(ColorRole.ActionPrimaryContainer))
    }

    @Test
    fun overrideSetsRejectSelfReferenceDuplicatesAndCycles() {
        val swap = RoleOverride(ColorRole.Primary, ColorRole.Tertiary)
        assertNull(validate(listOf(swap)))
        assertEquals(OverrideError.SelfReference(ColorRole.Primary), validate(listOf(RoleOverride(ColorRole.Primary, ColorRole.Primary))))
        assertEquals(OverrideError.Duplicate(ColorRole.Primary), validate(listOf(swap, swap)))
        assertEquals(OverrideError.Cycle(ColorRole.Primary), validate(listOf(swap, RoleOverride(ColorRole.Tertiary, ColorRole.Primary))))
        assertEquals(OverrideError.Cycle(ColorRole.Primary), validate(listOf(RoleOverride(ColorRole.Primary, ColorRole.ActionPrimaryContainer))))
        assertNull(validate(listOf(RoleOverride(ColorRole.ActionPrimaryContainer, ColorRole.Tertiary))))
    }

    @Test
    fun nestedOverridesResolveAgainstInheritedValuesLikeCss() {
        // Parent swaps primary → tertiary; a child swapping tertiary → primary reads the parent's
        // already-resolved primary (= tertiary), not a cycle.
        val parent = VinkColors(ResolvedTheme.Light, mapOf(ColorRole.Primary to ColorRole.Tertiary))
        val child = VinkColors(ResolvedTheme.Light, mapOf(ColorRole.Tertiary to ColorRole.Primary), parent)
        assertEquals(ColorRole.Tertiary.light, child.argb(ColorRole.Tertiary))
        assertEquals(ColorRole.Tertiary.light, child.argb(ColorRole.ActionPrimaryContainer))
    }

    @Test
    fun reducedMotionMakesTransitionsInstantAndStopsLoops() {
        val full = VinkMotion(reduced = false)
        val reduced = VinkMotion(reduced = true)
        assertEquals(160, full.shortMs)
        assertEquals(0, reduced.mediumMs)
        assertTrue(full.loopsRun)
        assertTrue(!reduced.loopsRun)
        assertEquals(full.loopMs, reduced.loopMs, "the loop cycle is never shortened")
    }

    @Test
    fun lineHeightIsSizeTimesRatioInSp() {
        assertEquals(VinkTextRoles.Body.size.value * 1.6f, VinkTextRoles.Body.lineHeight.value, 0.001f)
        assertEquals(17f, VinkTextRoles.Reading.size.value)
        assertEquals(20f, VinkTextRoles.Reading.maxSize?.value)
    }

    @Test
    fun densityDimensKeepComfortableAndCompactValues() {
        assertEquals(VinkDensityDimens.ControlPadding.comfortable, VinkDensityDimens.ControlPadding.at(Density.Comfortable))
        assertTrue(VinkDensityDimens.ControlPadding.at(Density.Compact) < VinkDensityDimens.ControlPadding.comfortable)
    }
}
