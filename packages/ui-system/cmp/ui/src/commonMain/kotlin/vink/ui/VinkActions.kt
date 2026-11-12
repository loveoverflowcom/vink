package vink.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.InlineTextContent
import androidx.compose.foundation.text.appendInlineContent
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.IconToggleButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.Placeholder
import androidx.compose.ui.text.PlaceholderVerticalAlign
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em

/**
 * Actions (shared/components.md). One target, one dispatch per activation, none while disabled
 * or pending. Native Compose has no forms, so there is no Submit kind; navigation is the host's
 * `onNavigate`, so there are no tabs or modifier-clicks.
 */
sealed interface ActionState {
    data object Enabled : ActionState

    data class Disabled(val reason: String? = null) : ActionState

    data class Pending(val status: String) : ActionState

    /** The visible text explaining a disabled or pending action. */
    val note: String?
        get() = when (this) {
            Enabled -> null
            is Disabled -> reason?.takeIf { it.isNotBlank() }
            is Pending -> status
        }
}

enum class ButtonVariant(val id: String) { Filled("filled"), Tonal("tonal"), Outlined("outlined"), Text("text"), Danger("danger") }

/** Minimum native target: Material's 48dp, never the Web compact 44. */
private val NativeControl = maxOf(48.dp, VinkDimens.DensityControlHeight)

@Composable
private fun Note(note: String?) {
    if (note != null) VinkText(note, role = TypeRole.Caption, tone = TextTone.Muted)
}

/**
 * Disabled uses `enabled = false` (still read by TalkBack/VoiceOver); Pending keeps the enabled,
 * focusable node, ignores clicks and states its status, so focus and width do not move.
 */
@Composable
fun VinkButton(
    label: String,
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    variant: ButtonVariant = ButtonVariant.Filled,
    state: ActionState = ActionState.Enabled,
    leading: IconName? = null,
) {
    val c = LocalVink.current.colors
    val (container, content) = when (variant) {
        ButtonVariant.Filled -> c[ColorRole.ActionPrimaryContainer] to c[ColorRole.ActionPrimaryContent]
        ButtonVariant.Tonal -> c[ColorRole.ActionTonalContainer] to c[ColorRole.ActionTonalContent]
        ButtonVariant.Outlined, ButtonVariant.Text -> androidx.compose.ui.graphics.Color.Transparent to c[ColorRole.Primary]
        ButtonVariant.Danger -> c[ColorRole.Error] to c[ColorRole.OnError]
    }
    val disabled = state is ActionState.Disabled
    val pending = state as? ActionState.Pending
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space1)) {
        Button(
            onClick = { if (state == ActionState.Enabled) onPress() },
            modifier = modifier.heightIn(min = NativeControl).semantics { if (pending != null) stateDescription = pending.status },
            enabled = !disabled,
            shape = RoundedCornerShape(VinkDimens.RadiusFull),
            colors = ButtonDefaults.buttonColors(
                containerColor = container,
                contentColor = content,
                disabledContainerColor = if (variant == ButtonVariant.Text) androidx.compose.ui.graphics.Color.Transparent else c[ColorRole.DisabledContainer],
                disabledContentColor = c[ColorRole.DisabledContent],
            ),
            border = if (variant == ButtonVariant.Outlined && !disabled) BorderStroke(1.dp, c[ColorRole.Outline]) else null,
            contentPadding = PaddingValues(
                start = if (variant == ButtonVariant.Text) VinkDimens.Space3 else VinkDimens.Space6,
                // Reserve the status slot before pending to keep every variant stable.
                end = VinkDimens.Space6,
                top = VinkDensityDimens.ControlPadding.at(LocalVink.current.density),
                bottom = VinkDensityDimens.ControlPadding.at(LocalVink.current.density),
            ),
        ) {
            Box(contentAlignment = Alignment.CenterStart) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (leading != null) {
                        VinkIcon(leading, size = IconSize.Small)
                        androidx.compose.foundation.layout.Spacer(Modifier.size(VinkDimens.Space2))
                    }
                    Text(label, style = LocalVink.current.text(VinkTextRoles.Label))
                }
                // UIS-03 status slot: the MicroArc sits in the end padding, so the label and the
                // button width do not move when the state turns pending.
                if (pending != null) {
                    VinkLoadingIndicator(Modifier.align(Alignment.CenterEnd).wrapContentSize(unbounded = true).offset(x = VinkDimens.SizeIndicator + 2.dp))
                }
            }
        }
        Note(state.note)
    }
}

/** An icon-only action; [label] is its accessible name and [pressed] makes it a toggle. */
@Composable
fun VinkIconButton(
    icon: IconName,
    label: String,
    onPress: () -> Unit,
    modifier: Modifier = Modifier,
    state: ActionState = ActionState.Enabled,
    pressed: Boolean? = null,
) {
    val c = LocalVink.current.colors
    val pending = state as? ActionState.Pending
    val target = modifier.semantics { if (pending != null) stateDescription = pending.status }
    val enabled = state !is ActionState.Disabled
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space1)) {
        if (pressed == null) {
            IconButton(
                onClick = { if (state == ActionState.Enabled) onPress() },
                modifier = target,
                enabled = enabled,
                colors = IconButtonDefaults.iconButtonColors(contentColor = c[ColorRole.OnSurfaceVariant], disabledContentColor = c[ColorRole.DisabledContent]),
            ) { VinkIcon(icon, label = label) }
        } else {
            IconToggleButton(
                checked = pressed,
                onCheckedChange = { if (state == ActionState.Enabled) onPress() },
                modifier = target,
                enabled = enabled,
                colors = IconButtonDefaults.iconToggleButtonColors(
                    contentColor = c[ColorRole.OnSurfaceVariant],
                    checkedContainerColor = c[ColorRole.SecondaryContainer],
                    checkedContentColor = c[ColorRole.OnSecondaryContainer],
                    disabledContentColor = c[ColorRole.DisabledContent],
                ),
            ) { VinkIcon(icon, label = label) }
        }
        Note(state.note)
    }
}

enum class LinkVariant { Inline, Standalone }

/**
 * Link semantics through [LinkAnnotation]; the host performs navigation in [onNavigate]. A
 * refused [href] (see [safeHref]) renders plain text. [external] adds the registry icon.
 */
@Composable
fun VinkLink(
    text: String,
    href: String,
    onNavigate: (String) -> Unit,
    modifier: Modifier = Modifier,
    variant: LinkVariant = LinkVariant.Inline,
    external: Boolean = false,
) {
    val vink = LocalVink.current
    val target = safeHref(href)
    val style = vink.text(if (variant == LinkVariant.Standalone) VinkTextRoles.Label else VinkTextRoles.Body)
    val linkStyle = SpanStyle(
        color = vink.colors[ColorRole.Primary],
        textDecoration = if (variant == LinkVariant.Inline) TextDecoration.Underline else null,
        fontWeight = if (variant == LinkVariant.Standalone) FontWeight(VinkTextRoles.Label.weight) else null,
    )
    val annotated = buildAnnotatedString {
        if (target == null) {
            append(text)
        } else {
            withLink(LinkAnnotation.Clickable(target, TextLinkStyles(linkStyle)) { onNavigate(target) }) { append(text) }
        }
        if (external && target != null) {
            append(" ")
            appendInlineContent("external", "(mở ngoài)")
        }
    }
    val inline = mapOf(
        "external" to InlineTextContent(Placeholder(1.em, 1.em, PlaceholderVerticalAlign.TextCenter)) {
            VinkIcon(IconName.OpenInNew, size = IconSize.Small, tint = vink.colors[ColorRole.Primary])
        },
    )
    val box = if (variant == LinkVariant.Standalone) modifier.heightIn(min = NativeControl) else modifier
    Row(box, verticalAlignment = Alignment.CenterVertically) {
        Text(annotated, style = style, color = vink.colors[ColorRole.OnSurface], inlineContent = inline)
    }
}

/** A toggle chip with a check mark when selected (never colour alone); a disabled reason shows under it. */
@Composable
fun VinkFilterChip(
    label: String,
    selected: Boolean,
    onToggle: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    state: ActionState = ActionState.Enabled,
) {
    val c = LocalVink.current.colors
    val container by rememberEffectColor(c[if (selected) ColorRole.SecondaryContainer else ColorRole.SurfaceContainer])
    val labelColor by rememberEffectColor(c[if (selected) ColorRole.OnSecondaryContainer else ColorRole.OnSurfaceVariant])
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space1)) {
    FilterChip(
        selected = selected,
        onClick = { if (state == ActionState.Enabled) onToggle(!selected) },
        label = { Text(label, style = LocalVink.current.text(VinkTextRoles.Label)) },
        modifier = modifier.heightIn(min = NativeControl),
        enabled = state !is ActionState.Disabled,
        leadingIcon = {
            // Reserve the check slot so selecting a chip never shifts the row.
            androidx.compose.foundation.layout.Box(Modifier.size(VinkDimens.SizeIconSm)) {
                if (selected) VinkIcon(IconName.Check, size = IconSize.Small)
            }
        },
        shape = RoundedCornerShape(VinkDimens.RadiusSm),
        colors = FilterChipDefaults.filterChipColors(
            containerColor = container,
            labelColor = labelColor,
            selectedContainerColor = container,
            selectedLabelColor = labelColor,
            selectedLeadingIconColor = c[ColorRole.OnSecondaryContainer],
            disabledLabelColor = c[ColorRole.DisabledContent],
        ),
        border = null,
    )
        Note(state.note)
    }
}
