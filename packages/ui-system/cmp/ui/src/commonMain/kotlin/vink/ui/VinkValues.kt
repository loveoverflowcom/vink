package vink.ui

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.roundToInt

/** Value types the generated VinkTokens.kt and VinkCatalog.kt are written in. */

enum class NativeFamily { SansSerif, Serif, Monospace }

/** A self-hosted face; `minWeight..maxWeight` is a variable range when they differ. */
data class FontFace(val family: String, val file: String, val minWeight: Int, val maxWeight: Int)

/** A length that depends on density. */
data class DensityDimen(val comfortable: Dp, val compact: Dp) {
    fun at(density: Density): Dp = if (density == Density.Compact) compact else comfortable
}

/**
 * A text role. `lineHeight` is already `size × ratio` in sp, so the platform font scale applies
 * once to both. `maxSize` records the Web fluid cap; native renderers use `size`.
 */
data class VinkTextRole(
    val token: String,
    val font: FontRole,
    val size: TextUnit,
    val maxSize: TextUnit?,
    val weight: Int,
    val lineHeight: TextUnit,
    val trackingEm: Float,
)

data class ContrastPair(val fg: ColorRole, val bg: ColorRole, val min: Double, val usage: String)

data class Scenario(val id: String, val label: String)

/** A preview registration. A status never implies runtime, accessibility or design acceptance. */
data class CatalogEntry(
    val id: String,
    val family: String,
    val label: String,
    val summary: String,
    val scenarios: List<Scenario>,
    val leptos: String,
    val cmp: String,
)

/** WCAG 2.x contrast of ARGB `fg` (composited over `bg`) on opaque `bg`; token arithmetic only. */
fun contrast(fg: Long, bg: Long): Double {
    fun channel(argb: Long, shift: Int) = ((argb shr shift) and 0xFF).toInt()
    val alpha = channel(fg, 24) / 255.0
    fun mix(shift: Int) = (channel(fg, shift) * alpha + channel(bg, shift) * (1 - alpha)).roundToInt()
    fun linear(c: Int): Double {
        val v = c / 255.0
        return if (v <= 0.04045) v / 12.92 else ((v + 0.055) / 1.055).pow(2.4)
    }
    fun luminance(r: Int, g: Int, b: Int) = 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    val a = luminance(mix(16), mix(8), mix(0))
    val b = luminance(channel(bg, 16), channel(bg, 8), channel(bg, 0))
    return (max(a, b) + 0.05) / (min(a, b) + 0.05)
}
