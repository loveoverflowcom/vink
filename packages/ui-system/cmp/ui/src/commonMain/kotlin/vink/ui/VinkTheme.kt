package vink.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em

/**
 * Appearance preferences a caller injects. The library reads no storage, account or service.
 *
 * Precedence, nearest first: an explicit theme on the nearest [VinkScope], then enclosing
 * scopes, then [VinkTheme]'s preference, then the system. Density takes the nearest declared
 * value. Reduced motion is additive: the injected system setting, the root or any enclosing
 * scope can reduce motion and nothing below can restore it.
 */
enum class ThemePreference(val value: String) {
    Light("light"),
    Dark("dark"),
    System("system");

    fun resolve(systemDark: Boolean): ResolvedTheme = when (this) {
        Light -> ResolvedTheme.Light
        Dark -> ResolvedTheme.Dark
        System -> if (systemDark) ResolvedTheme.Dark else ResolvedTheme.Light
    }

    companion object {
        /** Unknown or missing stored values fall back to the system. */
        fun parse(value: String?): ThemePreference = entries.firstOrNull { it.value == value } ?: System
    }
}

enum class ResolvedTheme { Light, Dark }

enum class Density { Comfortable, Compact }

enum class MotionPreference { System, Reduce }

data class VinkPreferences(
    val theme: ThemePreference = ThemePreference.System,
    val density: Density = Density.Comfortable,
    val motion: MotionPreference = MotionPreference.System,
)

/** Points one colour role at another in a scope. Only roles are targets, never literal colours. */
data class RoleOverride(val role: ColorRole, val to: ColorRole)

sealed interface OverrideError {
    data class SelfReference(val role: ColorRole) : OverrideError
    data class Duplicate(val role: ColorRole) : OverrideError
    data class Cycle(val role: ColorRole) : OverrideError
}

/** The first problem in an override set, or null; mirrors the Leptos `RoleOverride::style`. */
fun validate(overrides: List<RoleOverride>): OverrideError? {
    val seen = mutableSetOf<ColorRole>()
    for (o in overrides) {
        if (o.role == o.to) return OverrideError.SelfReference(o.role)
        if (!seen.add(o.role)) return OverrideError.Duplicate(o.role)
    }
    // Follow overrides and recipe aliases on the same scope, as the CSS custom properties would.
    val map = overrides.associate { it.role to it.to }
    for (o in overrides) {
        var next: ColorRole? = o.to
        repeat(overrides.size + ColorRole.entries.size + 1) {
            if (next == o.role) return OverrideError.Cycle(o.role)
            next = next?.let { map[it] ?: it.alias } ?: return@repeat
        }
    }
    return null
}

/** Font families per role; native defaults until a licensed TTF/OTF pipeline exists. */
@Immutable
class VinkFonts(
    val ui: FontFamily = FontFamily.SansSerif,
    val reading: FontFamily = FontFamily.Serif,
    val code: FontFamily = FontFamily.Monospace,
) {
    fun of(role: FontRole): FontFamily = when (role) {
        FontRole.Ui -> ui
        FontRole.Reading -> reading
        FontRole.Code -> code
    }
}

/**
 * Resolved colours for one scope, mirroring CSS custom properties: this scope's overrides, then
 * recipe aliases (re-declared on every scope), then the value inherited from the enclosing scope
 * — already resolved there — or, at the root and on a re-themed scope, the theme value.
 */
@Immutable
class VinkColors(
    val theme: ResolvedTheme,
    val overrides: Map<ColorRole, ColorRole> = emptyMap(),
    val inherited: VinkColors? = null,
) {
    operator fun get(role: ColorRole): Color = Color(argb(role))

    /** Terminates because [validate] rejects cycles through overrides and aliases. */
    fun argb(role: ColorRole): Long {
        overrides[role]?.let { return argb(it) }
        role.alias?.let { return argb(it) }
        return inherited?.argb(role) ?: if (theme == ResolvedTheme.Dark) role.dark else role.light
    }
}

/** Motion for one scope. Reduced transitions are instant; loops stop rather than spin at 0 ms. */
@Immutable
class VinkMotion(val reduced: Boolean) {
    val shortMs: Int get() = if (reduced) VinkMotionTokens.DurationShortReducedMs else VinkMotionTokens.DurationShortMs
    val mediumMs: Int get() = if (reduced) VinkMotionTokens.DurationMediumReducedMs else VinkMotionTokens.DurationMediumMs
    val loopMs: Int get() = VinkMotionTokens.DurationLoopMs
    val loopsRun: Boolean get() = !reduced
}

/** Everything a descendant may read about its scope. */
@Immutable
class VinkScopeState(
    val preference: ThemePreference,
    val theme: ResolvedTheme,
    val systemDark: Boolean,
    val systemReducedMotion: Boolean,
    val density: Density,
    val colors: VinkColors,
    val motion: VinkMotion,
    val fonts: VinkFonts,
    val overrideError: OverrideError?,
) {
    fun text(role: VinkTextRole): TextStyle = TextStyle(
        fontFamily = fonts.of(role.font),
        fontSize = role.size,
        fontWeight = FontWeight(role.weight),
        lineHeight = role.lineHeight,
        letterSpacing = role.trackingEm.em,
    )
}

val LocalVink = staticCompositionLocalOf {
    VinkScopeState(
        preference = ThemePreference.System,
        theme = ResolvedTheme.Light,
        systemDark = false,
        systemReducedMotion = false,
        density = Density.Comfortable,
        colors = VinkColors(ResolvedTheme.Light),
        motion = VinkMotion(reduced = false),
        fonts = VinkFonts(),
        overrideError = null,
    )
}

/**
 * The root of a Vink UI subtree. `preferences` come from the caller (which owns persistence);
 * `systemDark` defaults to the platform; Compose Multiplatform 1.9 has no common reduced-motion
 * API, so `systemReducedMotion` is injected by the host. An invalid override set is ignored and
 * reported through [VinkScopeState.overrideError].
 */
@Composable
fun VinkTheme(
    preferences: VinkPreferences,
    systemDark: Boolean = isSystemInDarkTheme(),
    systemReducedMotion: Boolean = false,
    fonts: VinkFonts = VinkFonts(),
    overrides: List<RoleOverride> = emptyList(),
    content: @Composable () -> Unit,
) {
    val error = validate(overrides)
    val theme = preferences.theme.resolve(systemDark)
    val state = VinkScopeState(
        preference = preferences.theme,
        theme = theme,
        systemDark = systemDark,
        systemReducedMotion = systemReducedMotion,
        density = preferences.density,
        colors = VinkColors(theme, if (error == null) overrides.associate { it.role to it.to } else emptyMap()),
        motion = VinkMotion(reduced = systemReducedMotion || preferences.motion == MotionPreference.Reduce),
        fonts = fonts,
        overrideError = error,
    )
    Provide(state, paint = true, content)
}

/**
 * A nested scope. `null` inherits; an explicit theme re-themes the subtree and, like the CSS
 * scope that re-declares every role, drops overrides inherited from outside it.
 */
@Composable
fun VinkScope(
    theme: ThemePreference? = null,
    density: Density? = null,
    reduceMotion: Boolean = false,
    overrides: List<RoleOverride> = emptyList(),
    content: @Composable () -> Unit,
) {
    val parent = LocalVink.current
    val error = validate(overrides)
    val resolved = theme?.resolve(parent.systemDark) ?: parent.theme
    val inherited = if (theme == null) parent.colors else null
    val own = if (error == null) overrides.associate { it.role to it.to } else emptyMap()
    val state = VinkScopeState(
        preference = theme ?: parent.preference,
        theme = resolved,
        systemDark = parent.systemDark,
        systemReducedMotion = parent.systemReducedMotion,
        density = density ?: parent.density,
        colors = VinkColors(resolved, own, inherited),
        motion = VinkMotion(reduced = parent.motion.reduced || reduceMotion),
        fonts = parent.fonts,
        overrideError = error,
    )
    Provide(state, paint = theme != null, content)
}

@Composable
private fun Provide(state: VinkScopeState, paint: Boolean, content: @Composable () -> Unit) {
    val colors = state.colors
    CompositionLocalProvider(LocalVink provides state, LocalContentColor provides colors[ColorRole.OnSurface]) {
        MaterialTheme(
            colorScheme = colorScheme(colors),
            typography = typography(state),
            shapes = Shapes(
                small = RoundedCornerShape(VinkDimens.RadiusSm),
                medium = RoundedCornerShape(VinkDimens.RadiusMd),
                large = RoundedCornerShape(VinkDimens.RadiusLg),
                extraLarge = RoundedCornerShape(VinkDimens.RadiusXl),
            ),
        ) {
            if (paint) {
                Box(Modifier.background(colors[ColorRole.Surface])) { content() }
            } else {
                content()
            }
        }
    }
}

private fun colorScheme(c: VinkColors): ColorScheme {
    val base = if (c.theme == ResolvedTheme.Dark) darkColorScheme() else lightColorScheme()
    return base.copy(
        primary = c[ColorRole.Primary],
        onPrimary = c[ColorRole.OnPrimary],
        primaryContainer = c[ColorRole.PrimaryContainer],
        onPrimaryContainer = c[ColorRole.OnPrimaryContainer],
        secondary = c[ColorRole.Secondary],
        onSecondary = c[ColorRole.OnSecondary],
        secondaryContainer = c[ColorRole.SecondaryContainer],
        onSecondaryContainer = c[ColorRole.OnSecondaryContainer],
        tertiary = c[ColorRole.Tertiary],
        onTertiary = c[ColorRole.OnTertiary],
        tertiaryContainer = c[ColorRole.TertiaryContainer],
        onTertiaryContainer = c[ColorRole.OnTertiaryContainer],
        background = c[ColorRole.Surface],
        onBackground = c[ColorRole.OnSurface],
        surface = c[ColorRole.Surface],
        onSurface = c[ColorRole.OnSurface],
        surfaceVariant = c[ColorRole.SurfaceContainerHighest],
        onSurfaceVariant = c[ColorRole.OnSurfaceVariant],
        surfaceTint = c[ColorRole.Primary],
        inverseSurface = c[ColorRole.InverseSurface],
        inverseOnSurface = c[ColorRole.InverseOnSurface],
        error = c[ColorRole.Error],
        onError = c[ColorRole.OnError],
        errorContainer = c[ColorRole.ErrorContainer],
        onErrorContainer = c[ColorRole.OnErrorContainer],
        outline = c[ColorRole.Outline],
        outlineVariant = c[ColorRole.OutlineVariant],
        scrim = c[ColorRole.Scrim],
        surfaceBright = c[ColorRole.Surface],
        surfaceDim = c[ColorRole.SurfaceContainerHigh],
        surfaceContainerLowest = c[ColorRole.Surface],
        surfaceContainerLow = c[ColorRole.SurfaceContainerLow],
        surfaceContainer = c[ColorRole.SurfaceContainer],
        surfaceContainerHigh = c[ColorRole.SurfaceContainerHigh],
        surfaceContainerHighest = c[ColorRole.SurfaceContainerHighest],
    )
}

private fun typography(state: VinkScopeState) = Typography(
    displaySmall = state.text(VinkTextRoles.Display),
    headlineMedium = state.text(VinkTextRoles.Headline),
    titleLarge = state.text(VinkTextRoles.Title),
    bodyLarge = state.text(VinkTextRoles.Body),
    bodyMedium = state.text(VinkTextRoles.Body),
    labelLarge = state.text(VinkTextRoles.Label),
    bodySmall = state.text(VinkTextRoles.Caption),
)
