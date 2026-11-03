package vink.ui

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.SemanticsPropertyKey
import androidx.compose.ui.semantics.SemanticsPropertyReceiver
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.delay

/**
 * State, feedback and loading components (shared/states.md). They render facts the caller owns
 * and report intents; none fetches, retries, logs out, navigates, clears text or decides
 * permission. Loops (pulse, arc, edge sweep) run only while motion is not reduced AND the host
 * says the subtree is active; otherwise they draw a static frame and keep their label.
 */

/** Whether a loop is running; a test-only semantics key (not exposed to accessibility services). */
val LoopRunning = SemanticsPropertyKey<Boolean>("VinkLoopRunning")
var SemanticsPropertyReceiver.loopRunning by LoopRunning

/** The [ActionPhase] id of a status badge; a test-only semantics key. */
val ActionPhaseId = SemanticsPropertyKey<String>("VinkActionPhase")
var SemanticsPropertyReceiver.actionPhaseId by ActionPhaseId

/** Host-injected activity (visible, foreground, on screen). Additive: an inactive ancestor wins. */
val LocalMotionActive = compositionLocalOf { true }

/**
 * Marks a subtree active or inactive. Hosts map their lifecycle here (desktop: window minimized or
 * hidden; Android: Lifecycle STARTED; iOS: scene foreground — no native host exists yet).
 */
@Composable
fun VinkMotionActivity(active: Boolean, content: @Composable () -> Unit) {
    val parent = LocalMotionActive.current
    CompositionLocalProvider(LocalMotionActive provides (parent && active), content = content)
}

@Composable
private fun loopsRunning(): Boolean = LocalVink.current.motion.loopsRun && LocalMotionActive.current

private fun stateText(running: Boolean) = if (running) "đang chạy" else "dừng"

/**
 * MicroArc for a pending action. Without [label] it is decorative (inside a button, whose own
 * state already says "pending"); with one it is an indeterminate progress node.
 */
@Composable
fun VinkLoadingIndicator(modifier: Modifier = Modifier, label: String? = null, size: IndicatorSize = IndicatorSize.Small) {
    val running = loopsRunning()
    val rotation = if (running) {
        rememberInfiniteTransition(label = "micro-arc")
            .animateFloat(0f, 360f, infiniteRepeatable(tween(VinkMotionTokens.DurationLoopMs, easing = LinearEasing)), label = "angle")
    } else {
        null
    }
    val color = androidx.compose.material3.LocalContentColor.current
    val box = if (size == IndicatorSize.Small) VinkDimens.SizeIndicator else VinkDimens.SizeIcon
    val described = if (label == null) {
        modifier.clearAndSetSemantics { loopRunning = running }
    } else {
        modifier.clearAndSetSemantics {
            contentDescription = label
            progressBarRangeInfo = ProgressBarRangeInfo.Indeterminate
            stateDescription = stateText(running)
            loopRunning = running
        }
    }
    Canvas(described.size(box).graphicsLayer { rotationZ = rotation?.value ?: 0f }) {
        val stroke = Stroke(width = (this.size.minDimension / 8f).coerceAtLeast(2f))
        val inset = stroke.width / 2
        val arc = Size(this.size.width - stroke.width, this.size.height - stroke.width)
        drawArc(color.copy(alpha = 0.24f), 0f, 360f, useCenter = false, topLeft = Offset(inset, inset), size = arc, style = stroke)
        drawArc(color, -90f, if (running) 100f else 270f, useCenter = false, topLeft = Offset(inset, inset), size = arc, style = stroke)
    }
}

/**
 * A 4dp bar at the top edge of a refreshing region. [Progress.Determinate] only with real progress
 * from the caller; indeterminate sweeps while loops run and otherwise shows a static dashed bar.
 */
@Composable
fun VinkEdgeProgress(progress: Progress, label: String, modifier: Modifier = Modifier) {
    val c = LocalVink.current.colors
    val value = progress.value
    val running = loopsRunning() && value == null
    val sweep = if (running) {
        rememberInfiniteTransition(label = "edge-progress")
            .animateFloat(0f, 1f, infiniteRepeatable(tween(VinkMotionTokens.DurationLoopMs, easing = LinearEasing)), label = "sweep")
    } else {
        null
    }
    val track = c[ColorRole.PrimaryContainer]
    val bar = c[ColorRole.Primary]
    Canvas(
        modifier.fillMaxWidth().height(VinkDimens.SizeEdgeProgress).clearAndSetSemantics {
            contentDescription = label
            progressBarRangeInfo = if (value != null) ProgressBarRangeInfo(value, 0f..1f) else ProgressBarRangeInfo.Indeterminate
            if (value == null) stateDescription = stateText(running)
            loopRunning = running
        },
    ) {
        drawRect(track)
        when {
            value != null -> drawRect(bar, size = Size(size.width * value, size.height))
            sweep != null -> {
                val w = size.width * 0.3f
                val x = (sweep.value * 1.3f - 0.3f) * size.width
                val start = x.coerceAtLeast(0f)
                val end = (x + w).coerceAtMost(size.width)
                if (end > start) drawRect(bar, topLeft = Offset(start, 0f), size = Size(end - start, size.height))
            }
            else -> {
                val dash = 12.dp.toPx()
                var x = 0f
                while (x < size.width) {
                    drawRect(bar, topLeft = Offset(x, 0f), size = Size(minOf(dash, size.width - x), size.height))
                    x += dash * 2
                }
            }
        }
    }
}

/** SurfacePulse: the geometry of real content, hidden from accessibility (the region says "loading"). */
@Composable
fun VinkSkeleton(shape: SkeletonShape, modifier: Modifier = Modifier) {
    val running = loopsRunning()
    val pulse = if (running) {
        rememberInfiniteTransition(label = "pulse").animateFloat(
            1f, 0.45f,
            infiniteRepeatable(tween(VinkMotionTokens.DurationPulseMs / 2), RepeatMode.Reverse),
            label = "alpha",
        )
    } else {
        null
    }
    val fill = LocalVink.current.colors[ColorRole.SurfaceContainerHighest]
    val line = with(LocalDensity.current) { VinkTextRoles.Body.size.toDp() }
    @Composable
    fun Bar(fraction: Float, height: Dp = line) =
        Box(Modifier.fillMaxWidth(fraction).height(height).background(fill, RoundedCornerShape(VinkDimens.RadiusSm)))

    @Composable
    fun Dot(size: Dp) = Box(Modifier.size(size).background(fill, CircleShape))

    Column(
        modifier.fillMaxWidth().clearAndSetSemantics { loopRunning = running }.graphicsLayer { alpha = pulse?.value ?: 1f },
        verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2),
    ) {
        when (shape) {
            is SkeletonShape.Text -> repeat(shape.lines.coerceAtLeast(1)) { i -> Bar(if (i == shape.lines - 1) 0.6f else 1f) }
            SkeletonShape.Card -> {
                Bar(0.7f, line * 1.5f)
                Row(horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space2), verticalAlignment = Alignment.CenterVertically) {
                    Dot(VinkDimens.SizeAvatarSm)
                    Bar(0.4f)
                }
                Bar(1f)
                Bar(0.85f)
            }
            SkeletonShape.Byline -> Row(horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space3), verticalAlignment = Alignment.CenterVertically) {
                Dot(VinkDimens.SizeAvatarMd)
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2)) {
                    Bar(0.5f)
                    Bar(0.3f, line * 0.8f)
                }
            }
        }
    }
}

/** Container and content roles of a tone: icon and text always accompany the colour. */
private fun toneRoles(tone: Tone): Pair<ColorRole, ColorRole> = when (tone) {
    Tone.Error -> ColorRole.ErrorContainer to ColorRole.OnErrorContainer
    Tone.Warning -> ColorRole.WarningContainer to ColorRole.OnWarningContainer
    Tone.Success -> ColorRole.SuccessContainer to ColorRole.OnSuccessContainer
    Tone.Info -> ColorRole.InfoContainer to ColorRole.OnInfoContainer
    Tone.Neutral -> ColorRole.SurfaceContainerHigh to ColorRole.OnSurface
}

/** The symbol colour of a panel on the surface (4.5:1 roles). */
private fun toneAccent(tone: Tone): ColorRole = when (tone) {
    Tone.Error -> ColorRole.Error
    Tone.Warning -> ColorRole.Warning
    Tone.Success -> ColorRole.Success
    Tone.Info -> ColorRole.Info
    Tone.Neutral -> ColorRole.OnSurfaceVariant
}

private fun Modifier.announce(announce: Announce): Modifier = when (announce) {
    Announce.Off -> this
    Announce.Polite -> semantics { liveRegion = LiveRegionMode.Polite }
    Announce.Assertive -> semantics { liveRegion = LiveRegionMode.Assertive }
}

/**
 * Symbol → heading → explanation → primary recovery → secondary → code. Each activation calls
 * [onRecover] once with the action's intent; the caller decides what the intent does.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun VinkStatePanel(
    content: PanelContent,
    onRecover: (RecoveryIntent) -> Unit,
    modifier: Modifier = Modifier,
    level: HeadingLevel = HeadingLevel.H2,
    actions: (@Composable () -> Unit)? = null,
) {
    val c = LocalVink.current.colors
    Column(modifier.fillMaxWidth().widthIn(max = VinkDimens.Measure), verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
        VinkIcon(content.icon, tint = c[toneAccent(content.tone)])
        VinkHeading(content.title, level, role = TypeRole.Title)
        if (content.message.isNotBlank()) VinkText(content.message, tone = TextTone.Muted)
        if (content.actions.isNotEmpty() || actions != null) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space2), verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2)) {
                content.actions.forEachIndexed { i, action ->
                    VinkButton(
                        action.label,
                        { onRecover(action.intent) },
                        variant = if (i == 0) ButtonVariant.Filled else ButtonVariant.Text,
                    )
                }
                actions?.invoke()
            }
        }
        content.code?.let { VinkText("Mã: $it", role = TypeRole.Caption, tone = TextTone.Muted) }
    }
}

@Composable
private fun NoticeBox(
    banner: Boolean,
    tone: Tone,
    message: String,
    title: String?,
    action: RecoveryAction?,
    announce: Announce,
    onRecover: (RecoveryIntent) -> Unit,
    onDismiss: (() -> Unit)?,
    modifier: Modifier,
) {
    val vink = LocalVink.current
    val (bg, fg) = toneRoles(tone)
    val ink = vink.colors[fg]
    Row(
        modifier.fillMaxWidth()
            .background(vink.colors[bg], RoundedCornerShape(if (banner) VinkDimens.RadiusSm else VinkDimens.RadiusMd))
            .announce(announce)
            .padding(start = VinkDimens.Space4, end = if (onDismiss != null) VinkDimens.Space1 else VinkDimens.Space4, top = VinkDimens.Space3, bottom = VinkDimens.Space3),
        horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space3),
    ) {
        VinkIcon(tone.icon, Modifier.padding(top = 2.dp), size = IconSize.Small, tint = ink)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(VinkDimens.Space1)) {
            title?.let { Text(it, style = vink.text(VinkTextRoles.Label), color = ink) }
            Text(message, style = vink.text(VinkTextRoles.Body), color = ink)
            action?.let {
                TextButton(
                    onClick = { onRecover(it.intent) },
                    modifier = Modifier.heightIn(min = 48.dp),
                    colors = ButtonDefaults.textButtonColors(contentColor = ink),
                    // Aligned with the message; the 48dp target comes from the height and min width.
                    contentPadding = PaddingValues(horizontal = 0.dp),
                ) {
                    Text(it.label, style = vink.text(VinkTextRoles.Label).copy(fontWeight = FontWeight(VinkTextRoles.Label.weight), textDecoration = androidx.compose.ui.text.style.TextDecoration.Underline))
                }
            }
        }
        if (onDismiss != null) {
            CompositionLocalProvider(androidx.compose.material3.LocalContentColor provides ink) {
                androidx.compose.material3.IconButton(onClick = onDismiss) { VinkIcon(IconName.Close, label = "Đóng thông báo", size = IconSize.Small, tint = ink) }
            }
        }
    }
}

/** An inline notice beside the region or action it is about; never truncated. */
@Composable
fun VinkInlineNotice(
    tone: Tone,
    message: String,
    modifier: Modifier = Modifier,
    title: String? = null,
    action: RecoveryAction? = null,
    announce: Announce = Announce.Polite,
    onRecover: (RecoveryIntent) -> Unit = {},
    onDismiss: (() -> Unit)? = null,
) = NoticeBox(false, tone, message, title, action, announce, onRecover, onDismiss, modifier)

/** A page- or region-wide notice at the top; it does not cover controls. */
@Composable
fun VinkBanner(
    tone: Tone,
    message: String,
    modifier: Modifier = Modifier,
    title: String? = null,
    action: RecoveryAction? = null,
    announce: Announce = Announce.Polite,
    onRecover: (RecoveryIntent) -> Unit = {},
    onDismiss: (() -> Unit)? = null,
) = NoticeBox(true, tone, message, title, action, announce, onRecover, onDismiss, modifier)

private fun phaseIcon(phase: ActionPhase): IconName = when (phase) {
    ActionPhase.Idle, ActionPhase.Pending -> IconName.Info
    ActionPhase.Dirty -> IconName.Edit
    ActionPhase.Confirmed -> IconName.Check
    is ActionPhase.Failed -> phase.kind.icon
    ActionPhase.Cancelled -> IconName.Close
    ActionPhase.Uncertain -> IconName.Help
}

/** The phase of one action as icon (or arc) + label; colour is never the only signal. */
@Composable
fun VinkStatusBadge(phase: ActionPhase, label: String, modifier: Modifier = Modifier) {
    val vink = LocalVink.current
    val (bg, fg) = toneRoles(phase.tone)
    val ink = vink.colors[fg]
    Row(
        modifier.background(vink.colors[bg], RoundedCornerShape(VinkDimens.RadiusFull))
            .semantics(mergeDescendants = true) { actionPhaseId = phase.id }
            .heightIn(min = 32.dp).padding(horizontal = VinkDimens.Space3, vertical = VinkDimens.Space1),
        horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space2),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        CompositionLocalProvider(androidx.compose.material3.LocalContentColor provides ink) {
            if (phase == ActionPhase.Pending) VinkLoadingIndicator() else VinkIcon(phaseIcon(phase), size = IconSize.Small, tint = ink)
        }
        Text(label, style = vink.text(VinkTextRoles.Label), color = ink)
    }
}

/** The end of a list: a quiet marker, not an action and not announced. */
@Composable
fun VinkListEnd(modifier: Modifier = Modifier, label: String = "Đã hết danh sách") {
    Column(modifier.fillMaxWidth().padding(vertical = VinkDimens.Space4), verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2), horizontalAlignment = Alignment.CenterHorizontally) {
        VinkDivider(decorative = true)
        VinkText(label, role = TypeRole.Caption, tone = TextTone.Muted)
    }
}

/**
 * One short message with at most one action and a close button, announced politely. Controlled:
 * the caller shows/hides it; [rememberSnackbarTimeout] closes an action-less one.
 */
@Composable
fun VinkSnackbar(message: SnackbarMessage?, onAction: () -> Unit, onDismiss: () -> Unit, modifier: Modifier = Modifier) {
    if (message == null) return
    val vink = LocalVink.current
    val ink = vink.colors[ColorRole.InverseOnSurface]
    Row(
        modifier.widthIn(max = 560.dp).fillMaxWidth()
            .background(vink.colors[ColorRole.InverseSurface], RoundedCornerShape(VinkDimens.RadiusSm))
            .semantics { liveRegion = LiveRegionMode.Polite }
            .heightIn(min = 48.dp).padding(start = VinkDimens.Space4, end = VinkDimens.Space1),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(message.text, Modifier.weight(1f).padding(vertical = VinkDimens.Space2), style = vink.text(VinkTextRoles.Body), color = ink)
        message.action?.let {
            TextButton(onClick = onAction, modifier = Modifier.heightIn(min = 48.dp), colors = ButtonDefaults.textButtonColors(contentColor = ink)) {
                Text(it, style = vink.text(VinkTextRoles.Label).copy(fontWeight = FontWeight(VinkTextRoles.Label.weight), textDecoration = androidx.compose.ui.text.style.TextDecoration.Underline))
            }
        }
        androidx.compose.material3.IconButton(onClick = onDismiss) { VinkIcon(IconName.Close, label = "Đóng", size = IconSize.Small, tint = ink) }
    }
}

/**
 * Closes an action-less snackbar after `duration.snackbar`, one delay per message (the Compose
 * clock drives it in tests). A snackbar with an action waits for the person.
 */
@Composable
fun rememberSnackbarTimeout(message: SnackbarMessage?, onDismiss: () -> Unit) {
    val dismiss by rememberUpdatedState(onDismiss)
    LaunchedEffect(message) {
        if (message != null && message.action == null) {
            delay(VinkMotionTokens.DurationSnackbarMs.toLong())
            dismiss()
        }
    }
}

/**
 * A controlled dialog: Title → Context → Content → Actions. [dismiss] decides which gestures may
 * close it without an action (Escape/Back, click outside); [initialFocus] is requested once it
 * opens (for a destructive confirmation: the safe action) and [returnFocus] after it closes.
 * No focus coordinator or stacking here: one dialog per screen until UIS-04.
 *
 * Platform: `Dialog` is an overlay layer in the same window on desktop (Compose 1.9), a separate
 * dialog window on Android and a presented layer on iOS; Escape (desktop) and Back (Android) are
 * read as [DismissReason.Escape]. AlertDialog has no Compose role: the title is the pane title and
 * a heading.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun VinkDialog(
    open: Boolean,
    title: String,
    onDismiss: (DismissReason) -> Unit,
    modifier: Modifier = Modifier,
    role: DialogRole = DialogRole.Dialog,
    kind: DialogKind = DialogKind.Dialog,
    dismiss: DismissPolicy = DismissPolicy.EscapeOnly,
    context: String? = null,
    initialFocus: FocusRequester? = null,
    returnFocus: FocusRequester? = null,
    content: @Composable () -> Unit = {},
    actions: @Composable RowScope.() -> Unit,
) {
    var wasOpen by remember { mutableStateOf(false) }
    LaunchedEffect(open) {
        if (open) {
            wasOpen = true
        } else if (wasOpen) {
            wasOpen = false
            runCatching { returnFocus?.requestFocus() }
        }
    }
    if (!open) return
    val onDismissNow by rememberUpdatedState(onDismiss)
    Dialog(
        onDismissRequest = { if (dismiss.allows(DismissReason.Outside)) onDismissNow(DismissReason.Outside) },
        properties = DialogProperties(
            dismissOnBackPress = false,
            dismissOnClickOutside = kind == DialogKind.Dialog && dismiss.allows(DismissReason.Outside),
            usePlatformDefaultWidth = kind == DialogKind.Dialog,
        ),
    ) {
        val vink = LocalVink.current
        val keys = Modifier.onPreviewKeyEvent {
            if (it.type == KeyEventType.KeyDown && (it.key == Key.Escape || it.key == Key.Back)) {
                if (dismiss.allows(DismissReason.Escape)) onDismissNow(DismissReason.Escape)
                true
            } else {
                false
            }
        }
        val shape = if (kind == DialogKind.Sheet) {
            RoundedCornerShape(topStart = VinkDimens.RadiusXl, topEnd = VinkDimens.RadiusXl)
        } else {
            RoundedCornerShape(VinkDimens.RadiusXl)
        }
        val panel: @Composable (Modifier) -> Unit = { m ->
            Column(
                m.background(vink.colors[ColorRole.SurfaceContainerHigh], shape)
                    .semantics { paneTitle = title }
                    .padding(VinkDimens.Space6),
                verticalArrangement = Arrangement.spacedBy(VinkDimens.Space4),
            ) {
                CompositionLocalProvider(androidx.compose.material3.LocalContentColor provides vink.colors[ColorRole.OnSurface]) {
                    Text(
                        title,
                        Modifier.semantics { heading() },
                        style = vink.text(VinkTextRoles.Title),
                        color = vink.colors[ColorRole.OnSurface],
                    )
                    context?.let { VinkText(it, tone = TextTone.Muted) }
                    content()
                    FlowRow(
                        Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space2, Alignment.End),
                        verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2),
                    ) {
                        Row(horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space2), verticalAlignment = Alignment.CenterVertically, content = actions)
                    }
                }
            }
        }
        if (kind == DialogKind.Sheet) {
            val outside = remember { MutableInteractionSource() }
            Box(
                keys.fillMaxSize().background(vink.colors[ColorRole.Scrim])
                    .clickable(outside, indication = null, enabled = dismiss.allows(DismissReason.Outside)) { onDismissNow(DismissReason.Outside) },
                contentAlignment = Alignment.BottomCenter,
            ) {
                panel(modifier.widthIn(max = 640.dp).fillMaxWidth().clickable(remember { MutableInteractionSource() }, indication = null) {})
            }
        } else {
            panel(keys.then(modifier).widthIn(max = 560.dp))
        }
        LaunchedEffect(Unit) { runCatching { initialFocus?.requestFocus() } }
    }
}

/** The visible/slow facts of a [LoadingGate] at the composition's clock. */
@Immutable
data class LoadingGateView(val visible: Boolean, val slow: Boolean)

/**
 * Adapter for [LoadingGate]: [now] is read during composition (a snapshot-backed manual clock
 * recomposes on advance), and ONE coroutine delay is scheduled per next change, never a polling
 * loop. The caller injects the clock; the library has no network timer.
 */
@Composable
fun rememberLoadingGate(loading: Boolean, now: () -> Long, policy: LoadingPolicy = LoadingPolicy()): LoadingGateView {
    val gate = remember(policy) { LoadingGate(policy) }
    val last = remember(policy) { arrayOfNulls<Boolean>(1) }
    var wake by remember { mutableIntStateOf(0) }
    @Suppress("UNUSED_EXPRESSION")
    wake
    val t = now()
    if (last[0] != loading) {
        if (loading) gate.start(t) else gate.finish(t)
        last[0] = loading
    }
    val next = gate.nextChange(t)
    LaunchedEffect(next) {
        if (next != null) {
            delay((next - t).coerceAtLeast(0))
            wake++
        }
    }
    return LoadingGateView(gate.visible(t), gate.slow(t))
}

private fun defaultEmpty(kind: EmptyKind): PanelContent {
    val (title, message) = when (kind) {
        EmptyKind.FirstUse -> "Chưa có gì ở đây" to "Nội dung sẽ xuất hiện khi có."
        EmptyKind.NoResults -> "Không có kết quả phù hợp" to "Thử từ khóa hoặc bộ lọc khác."
    }
    return (PanelContent.empty(kind, title, message) as PanelContent.Result.Ok).content
}

/**
 * A region that holds content through its states: EdgeProgress? → Notice? → Body (Skeleton |
 * StatePanel | content). Refresh keeps [content] composed (focus, scroll, selection); a failed
 * refresh is a notice, not a blank page. [problem] is the caller's projection for the current
 * failure (its copy and actions); without one the kind's fallback copy is used with no actions.
 * [indicatorVisible] comes from [rememberLoadingGate]: before it, the skeleton's space is reserved
 * but nothing is drawn or animated. An unlawful state shows a visible diagnostic instead.
 */
@Composable
fun VinkContentRegion(
    state: RegionState,
    label: String,
    onRecover: (RecoveryIntent) -> Unit,
    modifier: Modifier = Modifier,
    problem: Problem? = null,
    empty: PanelContent? = null,
    copy: RegionCopy = RegionCopy(),
    skeleton: SkeletonShape = SkeletonShape.Card,
    indicatorVisible: Boolean = true,
    content: @Composable () -> Unit,
) {
    val layout = when (val result = state.layout()) {
        is RegionResult.Rejected -> {
            VinkText("Trạng thái không hợp lệ: ${result.reason.id}", modifier.testTag("unlawful"), tone = TextTone.Muted)
            return
        }
        is RegionResult.Lawful -> result.layout
    }
    val loading = state.phase is Phase.Loading
    fun problemFor(kind: ProblemKind) = problem?.takeIf { it.kind == kind } ?: Problem.fallback(kind)
    val retry = problem?.actions?.firstOrNull { it.intent == RecoveryIntent.Retry }
    Column(
        modifier.fillMaxWidth().semantics { if (loading) stateDescription = "đang tải" },
        verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3),
    ) {
        if (layout.progress) VinkEdgeProgress(Progress.Indeterminate, "${copy.loading} $label")
        when (val notice = layout.notice) {
            null -> {}
            RegionNotice.Slow -> VinkInlineNotice(Tone.Info, copy.slow, onRecover = onRecover)
            RegionNotice.Partial -> VinkInlineNotice(Tone.Warning, copy.partial, action = retry, onRecover = onRecover)
            RegionNotice.LowContent -> VinkInlineNotice(Tone.Info, copy.lowContent, onRecover = onRecover)
            is RegionNotice.Refresh -> {
                val p = problemFor(notice.kind)
                VinkInlineNotice(
                    notice.kind.tone, p.message, title = p.title,
                    action = p.actions.firstOrNull { it.intent == RecoveryIntent.Retry }, onRecover = onRecover,
                )
            }
        }
        when (val body = layout.body) {
            Body.Skeleton -> Box(Modifier.semantics { contentDescription = "${copy.loading} $label" }) {
                if (indicatorVisible) {
                    VinkSkeleton(skeleton)
                } else {
                    VinkMotionActivity(false) { VinkSkeleton(skeleton, Modifier.graphicsLayer { alpha = 0f }) }
                }
            }
            is Body.Panel -> Box(Modifier.semantics { liveRegion = LiveRegionMode.Polite }) {
                val panel = when (val k = body.kind) {
                    is PanelKind.Error -> PanelContent.fromProblem(problemFor(k.kind))
                    is PanelKind.Empty -> empty?.takeIf { it.kind == k } ?: defaultEmpty(k.kind)
                    PanelKind.Cancelled -> PanelContent.cancelled(copy.cancelledTitle, copy.cancelledMessage, copy.reload)
                }
                VinkStatePanel(panel, onRecover)
            }
            Body.Content -> content()
        }
    }
}
