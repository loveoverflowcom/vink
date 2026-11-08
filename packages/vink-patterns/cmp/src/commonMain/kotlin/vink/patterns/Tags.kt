package vink.patterns

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.LocalContentColor
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.TextFieldValue
import vink.ui.*

/*
 * Work tags (TAG-PR-05): the CMP counterpart of the Leptos TagSelector/TagChip, same facts,
 * words and intents. The host owns catalog reads, suggestion requests, versions and saving;
 * nothing here fetches, stores or saves. No native product screen consumes it yet.
 */

/** The most themes a work carries in the current contract; the host still enforces it. */
const val THEME_LIMIT = 3

/** A catalog tag reduced to what a person reads; `problem` is the server's refusal in words. */
data class TagFacts(
    val id: String,
    val label: String,
    val description: String? = null,
    val retired: Boolean = false,
    val successor: TagLink? = null,
    val problem: String? = null,
)

data class TagLink(val id: String, val label: String)

data class TagOption(val tag: TagFacts, val viaAlias: String? = null)

/** Candidates for the current query, as the host's request owner knows them. */
sealed interface TagOptions {
    data object Idle : TagOptions
    /** An IME composition is open; nothing is looked up or announced yet. */
    data object Composing : TagOptions
    data object Loading : TagOptions
    /** Answer to exactly the current query; empty means no match. */
    data class Ready(val found: List<TagOption>) : TagOptions
    data object Invalid : TagOptions
    data object Unavailable : TagOptions
}

sealed interface CatalogFacts {
    data object Loading : CatalogFacts
    data class Ready(val forms: List<TagFacts>, val themes: List<TagFacts>, val truncated: Boolean = false) : CatalogFacts
    data object Unavailable : CatalogFacts
}

/** The polite status line under the query; empty while idle or composing. Same words as Web. */
fun optionsStatus(options: TagOptions, query: String, themes: Int): String {
    val q = query.trim()
    return when (options) {
        TagOptions.Idle, TagOptions.Composing -> ""
        TagOptions.Loading -> "Đang tìm chủ đề…"
        is TagOptions.Ready -> when {
            options.found.isEmpty() -> "Không có chủ đề nào khớp “$q”. Danh mục do ban biên tập quản lý; hiện chưa thể đề xuất thẻ mới."
            themes >= THEME_LIMIT -> "${options.found.size} gợi ý cho “$q”. Đã đủ $THEME_LIMIT chủ đề: bỏ một chủ đề để thêm chủ đề khác."
            else -> "${options.found.size} gợi ý cho “$q”."
        }
        TagOptions.Invalid -> "Gõ chữ hoặc số để tìm, tối đa 40 ký tự."
        TagOptions.Unavailable -> "Tạm thời không tìm được chủ đề. Gõ lại để thử lại."
    }
}

/** The written note under a chip, or null; never colour alone. */
fun tagNote(tag: TagFacts): String? = when {
    tag.problem != null -> tag.problem
    tag.retired && tag.successor != null -> "Thẻ này đã ngừng dùng; vẫn được giữ trên bản nháp. Ban biên tập gợi ý “${tag.successor.label}”."
    tag.retired -> "Thẻ này đã ngừng dùng; vẫn được giữ trên bản nháp."
    else -> null
}

/** A selected tag with a remove action of its own name and, when hinted, its successor. */
@Composable
fun TagChip(
    tag: TagFacts,
    removeLabel: String,
    onRemove: (() -> Unit)?,
    onReplace: ((String) -> Unit)?,
    modifier: Modifier = Modifier,
) {
    val c = LocalVink.current.colors
    val (bg, fg) = when {
        tag.problem != null -> ColorRole.ErrorContainer to ColorRole.OnErrorContainer
        tag.retired -> ColorRole.WarningContainer to ColorRole.OnWarningContainer
        else -> ColorRole.SecondaryContainer to ColorRole.OnSecondaryContainer
    }
    val note = tagNote(tag)
    Column(modifier, verticalArrangement = Arrangement.spacedBy(VinkDimens.Space1)) {
        Row(
            Modifier.background(c[bg], RoundedCornerShape(VinkDimens.RadiusSm)).heightIn(min = VinkDimens.SizeHitTarget).padding(start = VinkDimens.Space3),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space2),
        ) {
            CompositionLocalProvider(LocalContentColor provides c[fg]) {
                VinkText(tag.label, role = TypeRole.Label)
                if (tag.retired) VinkText("Đã ngừng dùng", role = TypeRole.Caption)
                if (onRemove != null) VinkIconButton(IconName.Close, removeLabel, onRemove)
            }
        }
        if (note != null) {
            Row(horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space1)) {
                VinkIcon(if (tag.problem != null) IconName.Error else IconName.Info, size = IconSize.Small, tint = c[if (tag.problem != null) ColorRole.Error else ColorRole.OnSurfaceVariant])
                VinkText(note, role = TypeRole.Caption, tone = TextTone.Muted)
            }
        }
        val successor = tag.successor
        if (successor != null && onReplace != null && (tag.retired || tag.problem != null)) {
            VinkButton("Thay bằng “${successor.label}”", { onReplace(successor.id) }, variant = ButtonVariant.Text)
        }
    }
}

/**
 * The author's form and themes for one work: single-choice form chips, removable theme chips,
 * a search field kept as a [TextFieldValue] (selection and composition survive), a polite status
 * and the candidates. The IME action while composing never chooses; otherwise it adds only the
 * exact match the host names.
 */
@Composable
fun TagSelector(
    form: TagFacts?,
    themes: List<TagFacts>,
    catalog: CatalogFacts,
    query: TextFieldValue,
    options: TagOptions,
    exact: String?,
    onForm: (String?) -> Unit,
    onAddTheme: (String) -> Unit,
    onRemoveTheme: (String) -> Unit,
    onReplaceTheme: (String, String) -> Unit,
    onQuery: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
) {
    val full = themes.size >= THEME_LIMIT
    fun chosen(id: String) = themes.any { it.id == id }
    var browsing by remember { mutableStateOf(false) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(VinkDimens.Space6)) {
        Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
            VinkText("Dạng bài · chọn 1", Modifier.semantics { heading() }, role = TypeRole.Label)
            val listed = (catalog as? CatalogFacts.Ready)?.forms?.any { it.id == form?.id } == true
            if (form != null && (form.retired || form.problem != null || !listed)) {
                TagChip(form, "Bỏ dạng bài ${form.label}", { onForm(null) }, { onForm(it) })
            }
            when (catalog) {
                CatalogFacts.Loading -> VinkText("Đang tải danh mục…", role = TypeRole.Caption, tone = TextTone.Muted)
                CatalogFacts.Unavailable -> VinkText("Chưa tải được danh mục dạng bài. Thẻ đã chọn vẫn được giữ.", role = TypeRole.Caption, tone = TextTone.Muted)
                is CatalogFacts.Ready -> if (catalog.forms.isEmpty()) {
                    VinkText("Danh mục chưa có dạng bài nào.", role = TypeRole.Caption, tone = TextTone.Muted)
                } else {
                    VinkStack(direction = Direction.Row, gap = Space.S2, wrap = true) {
                        catalog.forms.forEach { tag ->
                            key(tag.id) {
                                VinkFilterChip(tag.label, form?.id == tag.id, { on -> onForm(if (on) tag.id else null) })
                            }
                        }
                    }
                }
            }
            form?.description?.let { VinkText("${form.label}: $it", role = TypeRole.Caption, tone = TextTone.Muted) }
        }
        Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
            VinkText("Chủ đề · ${themes.size}/$THEME_LIMIT", Modifier.semantics { heading() }, role = TypeRole.Label)
            if (themes.isEmpty()) {
                VinkText("Chưa chọn chủ đề nào.", role = TypeRole.Caption, tone = TextTone.Muted)
            } else {
                VinkStack(direction = Direction.Row, gap = Space.S2, wrap = true) {
                    themes.forEach { tag ->
                        key(tag.id) {
                            TagChip(tag, "Bỏ chủ đề ${tag.label}", { onRemoveTheme(tag.id) }, { onReplaceTheme(tag.id, it) })
                        }
                    }
                }
            }
            VinkSearchField(
                label = "Tìm chủ đề",
                value = query,
                onValueChange = onQuery,
                onSubmit = { if (query.composition == null && exact != null && !full && !chosen(exact)) onAddTheme(exact) },
                help = "Gõ tên chủ đề hoặc tên gọi khác, rồi chọn trong gợi ý. Enter chỉ chọn khi gõ đúng tên.",
            )
            val status = optionsStatus(options, query.text, themes.size)
            if (status.isNotEmpty()) {
                VinkText(status, Modifier.semantics { liveRegion = LiveRegionMode.Polite }, role = TypeRole.Caption, tone = TextTone.Muted)
            }
            if (options is TagOptions.Ready) {
                options.found.forEach { option -> key(option.tag.id) { OptionRow(option, chosen(option.tag.id), full, onAddTheme) } }
            }
            if (catalog is CatalogFacts.Ready && catalog.themes.isNotEmpty()) {
                VinkButton(
                    if (browsing) "Ẩn các chủ đề có sẵn" else "Xem các chủ đề có sẵn (${catalog.themes.size})",
                    { browsing = !browsing },
                    variant = ButtonVariant.Text,
                    leading = IconName.ExpandMore,
                )
                if (browsing) {
                    catalog.themes.forEach { tag -> key(tag.id) { OptionRow(TagOption(tag), chosen(tag.id), full, onAddTheme) } }
                    if (catalog.truncated) VinkText("Danh mục còn nhiều chủ đề hơn; gõ để tìm.", role = TypeRole.Caption, tone = TextTone.Muted)
                }
            }
        }
    }
}

@Composable
private fun OptionRow(option: TagOption, chosen: Boolean, full: Boolean, onAdd: (String) -> Unit) {
    val label = buildString {
        append(option.tag.label)
        option.viaAlias?.let { append(" · qua tên “$it”") }
        if (chosen) append(" · Đã chọn")
    }
    VinkSurface(tone = SurfaceTone.Low, radius = Radius.Md, padding = Space.None) {
        Column {
            VinkButton(
                label,
                { onAdd(option.tag.id) },
                variant = ButtonVariant.Text,
                leading = IconName.Add,
                state = if (chosen || full) ActionState.Disabled(null) else ActionState.Enabled,
            )
            option.tag.description?.let {
                VinkText(it, Modifier.padding(start = VinkDimens.Space4, end = VinkDimens.Space4, bottom = VinkDimens.Space3), role = TypeRole.Caption, tone = TextTone.Muted)
            }
        }
    }
}
