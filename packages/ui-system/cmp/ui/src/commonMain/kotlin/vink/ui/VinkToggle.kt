package vink.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.material3.LocalContentColor
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

/**
 * ToggleAction (shared/components.md, `toggle-contract-v1`): a compact on/off action that shows
 * what its owner **confirmed**, never an optimistic guess. A press emits the desired state (the
 * opposite of the confirmed value); nothing while pending, uncertain, blocked or unknown. An
 * uncertain write offers Reconcile, never a blind replay. A count is a fact: unavailable is a
 * mark, never 0. Request identity, ordering fences and stale acknowledgements stay with the
 * caller's reducer.
 */
enum class ToggleValue(val id: String) {
    Off("off"),
    On("on"),
    Unknown("unknown"),
    ;

    val known: Boolean?
        get() = when (this) {
            Off -> false
            On -> true
            Unknown -> null
        }

    companion object {
        fun fromSelected(selected: Boolean): ToggleValue = if (selected) On else Off
    }
}

/** The caller's latest intent. [Failed] is a known refusal; a lost answer is [Uncertain]. */
sealed interface TogglePhase {
    val id: String

    data object Idle : TogglePhase {
        override val id = "idle"
    }

    data class Pending(val desired: Boolean) : TogglePhase {
        override val id get() = if (desired) "pending:on" else "pending:off"
    }

    data class Uncertain(val desired: Boolean) : TogglePhase {
        override val id get() = if (desired) "uncertain:on" else "uncertain:off"
    }

    data object Failed : TogglePhase {
        override val id = "failed"
    }
}

/** An aggregate the caller may show: [Hidden] without capability, [Unavailable] never as 0. */
sealed interface CountFact {
    data object Hidden : CountFact

    data object Unavailable : CountFact

    data class Known(val count: Long) : CountFact {
        init {
            require(count >= 0) { "a count is never negative" }
        }
    }

    /** Digits grouped with `.` (vi-VN), `–` when unavailable, null when hidden. */
    val text: String?
        get() = when (this) {
            Hidden -> null
            Unavailable -> "–"
            is Known -> count.toString().reversed().chunked(3).joinToString(".").reversed()
        }

    companion object {
        /** A server count (`bigint ≥ 0`); a negative value is an integrity error, shown unavailable. */
        fun fromLong(count: Long): CountFact = if (count < 0) Unavailable else Known(count)
    }
}

enum class ToggleNote(val id: String) {
    PendingOn("pending-on"),
    PendingOff("pending-off"),
    Uncertain("uncertain"),
    Failed("failed"),
    Unknown("unknown"),
    Blocked("disabled"),
}

/** The resolved projection of value × phase × availability. */
data class ToggleView(
    /** The confirmed state shown; null = unknown, no mark and no claim. */
    val pressed: Boolean?,
    val busy: Boolean,
    /** The desired state a press emits; null ignores the press. */
    val dispatch: Boolean?,
    val note: ToggleNote?,
    val reconcile: Boolean,
) {
    companion object {
        /** Precedence: pending → uncertain → blocked → unknown → failed → idle. */
        fun resolve(value: ToggleValue, phase: TogglePhase, enabled: Boolean): ToggleView {
            val pressed = value.known
            return when {
                phase is TogglePhase.Pending ->
                    ToggleView(pressed, true, null, if (phase.desired) ToggleNote.PendingOn else ToggleNote.PendingOff, false)
                phase is TogglePhase.Uncertain -> ToggleView(pressed, false, null, ToggleNote.Uncertain, true)
                !enabled -> ToggleView(pressed, false, null, ToggleNote.Blocked, false)
                pressed == null -> ToggleView(null, false, null, ToggleNote.Unknown, true)
                else -> ToggleView(pressed, false, !pressed, if (phase == TogglePhase.Failed) ToggleNote.Failed else null, false)
            }
        }
    }
}

/**
 * Every string the control shows or speaks. [label] stays visible in every state; [selectedName]
 * names the action once On is confirmed and contains the label's words (WCAG 2.5.3).
 */
data class ToggleCopy(
    val label: String,
    val selectedName: String,
    val countUnit: String = "lượt",
    val countUnavailable: String = "chưa có số liệu",
    val pendingOn: String = "Đang gửi…",
    val pendingOff: String = "Đang gửi…",
    val uncertain: String = "Chưa xác nhận được.",
    val failed: String = "Chưa thực hiện được. Bạn có thể thử lại.",
    val unknown: String = "Chưa rõ trạng thái.",
    val reconcile: String = "Kiểm tra lại",
) {
    fun note(note: ToggleNote, reason: String?): String? = when (note) {
        ToggleNote.PendingOn -> pendingOn
        ToggleNote.PendingOff -> pendingOff
        ToggleNote.Uncertain -> uncertain
        ToggleNote.Failed -> failed
        ToggleNote.Unknown -> unknown
        ToggleNote.Blocked -> reason?.takeIf { it.isNotBlank() }
    }

    fun name(pressed: Boolean?): String = if (pressed == true) selectedName else label

    /** Spoken with the name: "12 lượt đánh giá cao"; null when hidden. */
    fun countDescription(count: CountFact): String? = when (count) {
        CountFact.Hidden -> null
        CountFact.Unavailable -> "$countUnit: $countUnavailable"
        is CountFact.Known -> "${count.text} $countUnit"
    }
}

/** Minimum native target: Material's 48dp, never the Web compact 44. */
private val ToggleTarget = maxOf(48.dp, VinkDimens.DensityControlHeight)

/**
 * A confirmed on/off action with an optional count. One clickable node: TalkBack/VoiceOver read
 * the name that follows the confirmed value, then the count; the note is a polite live region and
 * Reconcile a separate Text button shown only when needed. [blocked] non-null blocks presses
 * (`enabled = false`), a blank reason shows nothing. Pending/uncertain/unknown keep the node
 * focusable, ignore clicks and state their note. [onSet] must move the caller's phase to Pending
 * before it returns.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun VinkToggleAction(
    copy: ToggleCopy,
    value: ToggleValue,
    onSet: (Boolean) -> Unit,
    onRecover: (RecoveryIntent) -> Unit,
    modifier: Modifier = Modifier,
    phase: TogglePhase = TogglePhase.Idle,
    count: CountFact = CountFact.Hidden,
    blocked: String? = null,
    icon: IconName? = null,
) {
    val vink = LocalVink.current
    val c = vink.colors
    val view = ToggleView.resolve(value, phase, blocked == null)
    val note = view.note?.let { copy.note(it, blocked) }
    val isBlocked = view.note == ToggleNote.Blocked
    val on = view.pressed == true
    val container = when {
        on && isBlocked -> c[ColorRole.DisabledContainer]
        on -> c[ColorRole.ActionToggleSelectedContainer]
        else -> Color.Transparent
    }
    val content = when {
        isBlocked -> c[ColorRole.DisabledContent]
        on -> c[ColorRole.ActionToggleSelectedContent]
        else -> c[ColorRole.ActionToggleContent]
    }
    val spoken = listOfNotNull(copy.name(view.pressed), copy.countDescription(count)).joinToString(", ")
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space1)) {
        Box(
            modifier
                .heightIn(min = ToggleTarget)
                .widthIn(min = ToggleTarget)
                .clip(RoundedCornerShape(VinkDimens.RadiusFull))
                .background(container)
                .clickable(enabled = !isBlocked, role = Role.Button) { view.dispatch?.let(onSet) }
                .semantics(mergeDescendants = true) {
                    contentDescription = spoken
                    if (note != null && !isBlocked) stateDescription = note
                }
                .padding(PaddingValues(horizontal = VinkDimens.Space3)),
            contentAlignment = Alignment.CenterStart,
        ) {
            CompositionLocalProvider(LocalContentColor provides content) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space2)) {
                    if (icon != null) VinkIcon(icon, Modifier.clearAndSetSemantics {}, size = IconSize.Small)
                    Text(copy.label, Modifier.clearAndSetSemantics {}, color = content, style = vink.text(VinkTextRoles.Label))
                    count.text?.let { shown ->
                        Text(shown, Modifier.clearAndSetSemantics {}, color = content, style = vink.text(VinkTextRoles.Label).copy(fontWeight = FontWeight(VinkTextRoles.Body.weight)))
                    }
                    // One reserved trailing slot, like VinkButton's status slot: MicroArc while
                    // pending, the check once On is confirmed; the label never moves.
                    Box(Modifier.size(VinkDimens.SizeIconSm).clearAndSetSemantics {}, contentAlignment = Alignment.Center) {
                        when {
                            view.busy -> VinkLoadingIndicator()
                            on -> VinkIcon(IconName.Check, size = IconSize.Small)
                        }
                    }
                }
            }
        }
        if (note != null || view.reconcile) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space2), verticalArrangement = Arrangement.Center) {
                if (note != null) {
                    VinkText(note, Modifier.align(Alignment.CenterVertically).semantics { liveRegion = LiveRegionMode.Polite }, TypeRole.Caption, TextTone.Muted)
                }
                if (view.reconcile) {
                    VinkButton(copy.reconcile, onPress = { onRecover(RecoveryIntent.Reconcile) }, variant = ButtonVariant.Text)
                }
            }
        }
    }
}
