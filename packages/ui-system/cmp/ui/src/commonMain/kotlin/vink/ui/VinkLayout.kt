package vink.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.max

/** Text and layout primitives (shared/components.md); the same names and variants as Leptos. */

enum class TypeRole(val id: String, val role: VinkTextRole) {
    Display("display", VinkTextRoles.Display),
    Headline("headline", VinkTextRoles.Headline),
    Title("title", VinkTextRoles.Title),
    Body("body", VinkTextRoles.Body),
    Label("label", VinkTextRoles.Label),
    Caption("caption", VinkTextRoles.Caption),
    Reading("reading", VinkTextRoles.Reading),
    Code("code", VinkTextRoles.Code),
}

/** Text colours that keep 4.5:1; `Default` inherits the surrounding content colour. */
enum class TextTone(val id: String, val role: ColorRole?) {
    Default("default", null),
    Muted("muted", ColorRole.OnSurfaceVariant),
    Primary("primary", ColorRole.Primary),
    Error("error", ColorRole.Error),
    Warning("warning", ColorRole.Warning),
}

@Composable
fun VinkText(text: String, modifier: Modifier = Modifier, role: TypeRole = TypeRole.Body, tone: TextTone = TextTone.Default) {
    val vink = LocalVink.current
    Text(text, modifier, color = tone.role?.let { vink.colors[it] } ?: LocalContentColor.current, style = vink.text(role.role))
}

/** Compose has only a boolean heading; the level chooses the default look (Web keeps h1–h6). */
enum class HeadingLevel(val defaultRole: TypeRole) { H1(TypeRole.Display), H2(TypeRole.Headline), H3(TypeRole.Title), H4(TypeRole.Label), H5(TypeRole.Label), H6(TypeRole.Label) }

@Composable
fun VinkHeading(text: String, level: HeadingLevel, modifier: Modifier = Modifier, role: TypeRole? = null, tone: TextTone = TextTone.Default) =
    VinkText(text, modifier.semantics { heading() }, role ?: level.defaultRole, tone)

enum class Space(val id: String) { None("none"), S1("1"), S2("2"), S3("3"), S4("4"), S6("6"), S8("8"), S12("12"), Density("density") }

/** The dp of a named space in the current scope (Density follows `density.stack-gap`). */
@Composable
fun Space.dp(): Dp = when (this) {
    Space.None -> 0.dp
    Space.S1 -> VinkDimens.Space1
    Space.S2 -> VinkDimens.Space2
    Space.S3 -> VinkDimens.Space3
    Space.S4 -> VinkDimens.Space4
    Space.S6 -> VinkDimens.Space6
    Space.S8 -> VinkDimens.Space8
    Space.S12 -> VinkDimens.Space12
    Space.Density -> VinkDensityDimens.StackGap.at(LocalVink.current.density)
}

enum class SurfaceTone(val id: String, val bg: ColorRole, val fg: ColorRole) {
    Base("base", ColorRole.Surface, ColorRole.OnSurface),
    Low("low", ColorRole.SurfaceContainerLow, ColorRole.OnSurface),
    Container("container", ColorRole.SurfaceContainer, ColorRole.OnSurface),
    High("high", ColorRole.SurfaceContainerHigh, ColorRole.OnSurface),
    Highest("highest", ColorRole.SurfaceContainerHighest, ColorRole.OnSurface),
    Primary("primary", ColorRole.PrimaryContainer, ColorRole.OnPrimaryContainer),
}

enum class Radius(val dp: Dp) { None(0.dp), Sm(VinkDimens.RadiusSm), Md(VinkDimens.RadiusMd), Lg(VinkDimens.RadiusLg), Xl(VinkDimens.RadiusXl) }

/** A toned, non-interactive area; it sets the content colour its Default text inherits. */
@Composable
fun VinkSurface(
    modifier: Modifier = Modifier,
    tone: SurfaceTone = SurfaceTone.Base,
    radius: Radius = Radius.Md,
    padding: Space = Space.S4,
    content: @Composable () -> Unit,
) {
    val vink = LocalVink.current
    Box(modifier.background(vink.colors[tone.bg], RoundedCornerShape(radius.dp)).padding(padding.dp())) {
        CompositionLocalProvider(LocalContentColor provides vink.colors[tone.fg]) { content() }
    }
}

enum class Measure(val id: String, val max: Dp?) { Reading("reading", VinkDimens.Measure), Wide("wide", VinkDimens.MeasureWide), Full("full", null) }

/** Centres content at a named measure; adds no side padding. */
@Composable
fun VinkContainer(modifier: Modifier = Modifier, measure: Measure = Measure.Reading, content: @Composable () -> Unit) {
    Box(modifier.fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
        Box((measure.max?.let { Modifier.widthIn(max = it) } ?: Modifier).fillMaxWidth()) { content() }
    }
}

enum class Direction { Column, Row }

enum class Align { Stretch, Start, Center, End }

/** Children in order with a named gap; a wrapping row flows onto new lines. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun VinkStack(
    modifier: Modifier = Modifier,
    direction: Direction = Direction.Column,
    gap: Space = Space.S4,
    align: Align = Align.Stretch,
    wrap: Boolean = false,
    content: @Composable () -> Unit,
) {
    val g = gap.dp()
    when {
        direction == Direction.Column -> Column(
            modifier,
            verticalArrangement = Arrangement.spacedBy(g),
            horizontalAlignment = when (align) {
                Align.Center -> Alignment.CenterHorizontally
                Align.End -> Alignment.End
                else -> Alignment.Start
            },
        ) { content() }
        wrap -> FlowRow(modifier, horizontalArrangement = Arrangement.spacedBy(g), verticalArrangement = Arrangement.spacedBy(g)) { content() }
        else -> Row(
            modifier,
            horizontalArrangement = Arrangement.spacedBy(g),
            verticalAlignment = when (align) {
                Align.Start -> Alignment.Top
                Align.End -> Alignment.Bottom
                else -> Alignment.CenterVertically
            },
        ) { content() }
    }
}

enum class GridMin(val dp: Dp) { Small(VinkDimens.SizeGridMinSm), Medium(VinkDimens.SizeGridMinMd) }

/**
 * As many equal columns of at least [min] as fit (one when none fit). Like the Web rem value,
 * the minimum grows with the font scale.
 */
@Composable
fun VinkGrid(modifier: Modifier = Modifier, min: GridMin = GridMin.Medium, gap: Space = Space.S4, content: @Composable () -> Unit) {
    val g = gap.dp()
    val scale = LocalDensity.current.fontScale
    Layout(content, modifier.fillMaxWidth()) { measurables, constraints ->
        val gapPx = g.roundToPx()
        val width = constraints.maxWidth
        val minPx = (min.dp * scale).roundToPx()
        val columns = max(1, (width + gapPx) / (minPx + gapPx))
        val column = (width - gapPx * (columns - 1)) / columns
        val placeables = measurables.map { it.measure(Constraints(minWidth = column, maxWidth = column)) }
        val rows = placeables.chunked(columns)
        val heights = rows.map { row -> row.maxOf { it.height } }
        val height = heights.sum() + gapPx * max(0, rows.size - 1)
        layout(width, height) {
            var y = 0
            rows.forEachIndexed { r, row ->
                row.forEachIndexed { c, p -> p.placeRelative(c * (column + gapPx), y) }
                y += heights[r] + gapPx
            }
        }
    }
}

/** A separator; decorative dividers are removed from the semantics tree. */
@Composable
fun VinkDivider(modifier: Modifier = Modifier, decorative: Boolean = false) {
    HorizontalDivider(
        if (decorative) modifier.clearAndSetSemantics {} else modifier,
        color = LocalVink.current.colors[ColorRole.OutlineVariant],
    )
}
