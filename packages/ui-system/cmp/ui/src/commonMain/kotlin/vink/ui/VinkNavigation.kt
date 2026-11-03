package vink.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.IconButton
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.Text
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.material3.LocalContentColor
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Shell and navigation (UIS-04, shared/navigation.md): the same pure layout/overlay policy as the
 * Leptos renderer, and controlled primitives. Destinations, their order and visibility, the back
 * stack, deep links, stored preferences and focus after navigation belong to the host's adapter;
 * these composables never import a navigation controller and only call `onNavigate(key)`.
 */
enum class WindowClass(val id: String) {
    Compact("compact"),
    Medium("medium"),
    Expanded("expanded");

    companion object {
        /** Lower bounds (medium, expanded) of the Web shell in CSS px. */
        val CSS_PX: Pair<Double, Double> = 760.0 to 1100.0

        /** Lower bounds (medium, expanded) of Material window size classes in dp. */
        val DP: Pair<Double, Double> = 600.0 to 840.0

        private fun classify(width: Double, bounds: Pair<Double, Double>): WindowClass = when {
            width < bounds.first -> Compact
            width < bounds.second -> Medium
            else -> Expanded
        }

        fun fromCssPx(width: Double): WindowClass = classify(width, CSS_PX)

        /** The host passes its available window width in dp (not the screen's). */
        fun fromDp(width: Double): WindowClass = classify(width, DP)
    }
}

/** A person's explicit rail choice; it wins over the contextual default. */
enum class RailPreference(val value: String) {
    Expanded("expanded"),
    Compact("compact");

    companion object {
        fun parse(value: String): RailPreference? = entries.firstOrNull { it.value == value }
    }
}

/** What the current destination asks of the shell (Vink: the editor sets both). */
data class NavContext(val compactRail: Boolean = false, val hideBar: Boolean = false)

enum class NavLayout(val id: String) {
    RailExpanded("rail:expanded"),
    RailCompact("rail:compact"),
    Bar("bar"),
    BarHidden("bar:hidden");

    val railExpanded: Boolean get() = this == RailExpanded
    val isRail: Boolean get() = this == RailExpanded || this == RailCompact

    /** The preference the rail toggle stores: the opposite of the rail as shown; bars have none. */
    val nextPreference: RailPreference?
        get() = when (this) {
            RailExpanded -> RailPreference.Compact
            RailCompact -> RailPreference.Expanded
            Bar, BarHidden -> null
        }

    companion object {
        /** User preference > contextual default; the bottom bar ignores rail preferences. */
        fun resolve(window: WindowClass, context: NavContext, preference: RailPreference?): NavLayout = when {
            window == WindowClass.Compact -> if (context.hideBar) BarHidden else Bar
            preference == RailPreference.Expanded -> RailExpanded
            preference == RailPreference.Compact -> RailCompact
            window == WindowClass.Expanded && !context.compactRail -> RailExpanded
            else -> RailCompact
        }
    }
}

enum class CloseReason(val id: String) {
    Dismiss("dismiss"),
    Escape("escape"),
    Back("back"),
    Outside("outside"),
    Resize("resize"),
    Navigate("navigate");

    /** Where focus goes after the overlay closes. A navigation leaves focus to the new destination. */
    fun focusReturn(triggerVisible: Boolean): FocusReturn = when {
        this == Navigate -> FocusReturn.Leave
        triggerVisible -> FocusReturn.Trigger
        else -> FocusReturn.Main
    }

    companion object {
        fun parse(id: String): CloseReason? = entries.firstOrNull { it.id == id }
    }
}

enum class FocusReturn(val id: String) { Trigger("trigger"), Main("main"), Leave("leave") }

data class Closed(val id: String, val reason: CloseReason)

sealed interface OpenResult {
    data object Ok : OpenResult

    data class Busy(val active: String) : OpenResult
}

/**
 * One active modal at a time. A small owner the host keeps next to its overlays; snapshot state,
 * so composables that read [active] recompose. It renders, focuses and listens to nothing.
 */
class OverlayLayer {
    var active: String? by mutableStateOf(null)
        private set

    fun isOpen(id: String): Boolean = active == id

    /** Opening the active overlay again is a no-op; another one is refused. */
    fun open(id: String): OpenResult {
        val current = active
        if (current != null && current != id) return OpenResult.Busy(current)
        active = id
        return OpenResult.Ok
    }

    /** Closes [id] if it is the active overlay; a late close of anything else is stale (null). */
    fun close(id: String, reason: CloseReason): Closed? = if (isOpen(id)) take(reason) else null

    /** Back closes the active overlay before anything leaves the screen; null lets the back stack continue. */
    fun back(): Closed? = take(CloseReason.Back)

    fun escape(): Closed? = take(CloseReason.Escape)

    private fun take(reason: CloseReason): Closed? {
        val id = active ?: return null
        active = null
        return Closed(id, reason)
    }
}

/** A destination the host offers: which exist and in which order is the host's decision. */
data class NavDestination(val key: String, val label: String, val icon: IconName)

private val NativeTarget = maxOf(48.dp, VinkDimens.SizeNavItem)

/**
 * One destination. Selected state is a semantics fact, not a tab: route navigation does not use
 * tab roles. [compact] stacks the icon above the label (compact rail, bottom bar).
 */
@Composable
fun VinkNavItem(
    destination: NavDestination,
    selected: Boolean,
    onNavigate: (String) -> Unit,
    modifier: Modifier = Modifier,
    compact: Boolean = false,
) {
    NavTarget(destination.label, destination.icon, selected, compact, modifier, Role.Button) { onNavigate(destination.key) }
}

/** A shell utility that opens an overlay (Vink: Tôi at the foot of the rail), not a destination. */
@Composable
fun VinkNavUtility(
    label: String,
    icon: IconName,
    open: Boolean,
    onOpen: () -> Unit,
    modifier: Modifier = Modifier,
    current: Boolean = false,
    compact: Boolean = false,
) {
    NavTarget(label, icon, current || open, compact, modifier.semantics { stateDescription = if (open) "Đang mở" else "Đã đóng" }, Role.Button, onOpen)
}

@Composable
private fun NavTarget(
    label: String,
    icon: IconName,
    selected: Boolean,
    compact: Boolean,
    modifier: Modifier,
    role: Role,
    onClick: () -> Unit,
) {
    val colors = LocalVink.current.colors
    val content = if (selected) colors[ColorRole.OnPrimaryContainer] else colors[ColorRole.OnSurfaceVariant]
    val shape = RoundedCornerShape(VinkDimens.RadiusXl)
    val target = modifier
        .fillMaxWidth()
        .heightIn(min = NativeTarget)
        .clip(shape)
        .background(if (selected) colors[ColorRole.PrimaryContainer] else colors[ColorRole.SurfaceContainerLow].copy(alpha = 0f), shape)
        .selectable(selected = selected, role = role, onClick = onClick)
    CompositionLocalProvider(LocalContentColor provides content) {
        if (compact) {
            Column(target.padding(vertical = 12.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                VinkIcon(icon)
                Text(label, color = content, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center)
            }
        } else {
            Row(target.padding(12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                VinkIcon(icon)
                Text(label, color = content, fontWeight = FontWeight.SemiBold)
            }
        }
    }
}

/**
 * Rail: brand → permanent toggle → destinations → account foot. The toggle is always shown (also
 * collapsed), ≥48dp, named by the action it performs and explained by a tooltip on hover/focus.
 * The rail scrolls when short, so the toggle and the account stay reachable.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VinkNavigationRail(
    expanded: Boolean,
    onToggle: () -> Unit,
    destinations: List<NavDestination>,
    selected: String?,
    onNavigate: (String) -> Unit,
    expandLabel: String,
    collapseLabel: String,
    modifier: Modifier = Modifier,
    brand: @Composable () -> Unit = {},
    account: @Composable ColumnScope.(compact: Boolean) -> Unit = {},
) {
    val colors = LocalVink.current.colors
    val name = if (expanded) collapseLabel else expandLabel
    BoxWithConstraints(
        modifier
            .width(if (expanded) VinkDimens.SizeRailExpanded else VinkDimens.SizeRailCompact)
            .fillMaxHeight()
            .background(colors[ColorRole.SurfaceContainerLow])
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Vertical + WindowInsetsSides.Start)),
    ) {
        // At least the rail's height, so the account sits at the foot; taller content scrolls.
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .heightIn(min = maxHeight)
                .padding(vertical = 24.dp, horizontal = 12.dp),
            horizontalAlignment = if (expanded) Alignment.Start else Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(
                horizontalAlignment = if (expanded) Alignment.Start else Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(24.dp),
            ) {
                Box(Modifier.padding(horizontal = if (expanded) 8.dp else 0.dp)) { brand() }
                TooltipBox(
                    positionProvider = TooltipDefaults.rememberPlainTooltipPositionProvider(),
                    tooltip = { PlainTooltip { Text(name) } },
                    state = rememberTooltipState(),
                ) {
                    IconButton(onClick = onToggle, modifier = Modifier.size(48.dp).semantics { contentDescription = name }) {
                        VinkIcon(if (expanded) IconName.MenuOpen else IconName.Menu)
                    }
                }
                Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (destination in destinations) {
                        VinkNavItem(destination, destination.key == selected, onNavigate, compact = !expanded)
                    }
                }
            }
            Column(Modifier.fillMaxWidth().padding(top = 24.dp)) { account(!expanded) }
        }
    }
}

/** Bottom bar for compact windows: equal destinations, labels wrap, insets grow it. */
@Composable
fun VinkBottomNavigation(
    destinations: List<NavDestination>,
    selected: String?,
    onNavigate: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = LocalVink.current.colors
    Row(
        modifier
            .fillMaxWidth()
            .background(colors[ColorRole.SurfaceContainerLow])
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom + WindowInsetsSides.Horizontal))
            .heightIn(min = VinkDimens.SizeBottomBar)
            .padding(8.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        for (destination in destinations) {
            VinkNavItem(destination, destination.key == selected, onNavigate, Modifier.weight(1f), compact = true)
        }
    }
}

/** Header of compact layouts: brand and destination actions, never the primary destinations. */
@Composable
fun VinkTopBar(modifier: Modifier = Modifier, brand: @Composable () -> Unit = {}, actions: @Composable RowScope.() -> Unit = {}) {
    Row(
        modifier
            .fillMaxWidth()
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal))
            .heightIn(min = 64.dp)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        brand()
        Row(verticalAlignment = Alignment.CenterVertically, content = actions)
    }
}

/**
 * The adaptive frame for a resolved [layout]. [content] is called from one fixed call site, so
 * switching rail ↔ bar keeps its remembered state (text, selection, scroll) instead of remounting.
 */
@Composable
fun VinkAdaptiveScaffold(
    layout: NavLayout,
    rail: @Composable () -> Unit,
    bar: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    topBar: @Composable () -> Unit = {},
    content: @Composable () -> Unit,
) {
    val colors = LocalVink.current.colors
    Column(modifier.fillMaxSize().background(colors[ColorRole.Surface])) {
        if (!layout.isRail) topBar()
        Row(Modifier.weight(1f).fillMaxWidth()) {
            if (layout.isRail) rail()
            Box(Modifier.weight(1f).fillMaxHeight()) { content() }
        }
        if (layout == NavLayout.Bar) bar()
    }
}
