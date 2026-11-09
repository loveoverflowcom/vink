package vink.ui.preview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.input.TextFieldValue
import vink.patterns.CatalogFacts
import vink.patterns.THEME_LIMIT
import vink.patterns.TagFacts
import vink.patterns.TagLink
import vink.patterns.TagOption
import vink.patterns.TagOptions
import vink.patterns.TagSelector
import vink.ui.TextTone
import vink.ui.TypeRole
import vink.ui.VinkDimens
import vink.ui.VinkText

/**
 * TagSelector/TagChip (TAG-PR-05) through the public `:patterns` API, same IDs, scenarios and
 * sample tags as the Leptos gallery. Every tag is [Mẫu]; the interactive owner is in memory and
 * never claims a save.
 */
@Composable
fun TagSpecimen(id: String, scenario: String): Boolean {
    if (id != "composition.tag-selector") return false
    if (scenario == "interactive") TagInteractive() else TagSample(scenario)
    return true
}

private val retired = TagFacts("do-thi", "Đô thị", retired = true, successor = TagLink("mua", "Mùa"))
private val forms = listOf(
    TagFacts("tan-van", "Tản văn", "Văn xuôi ngắn viết từ trải nghiệm và cảm xúc riêng."),
    TagFacts("tho", "Thơ"),
    TagFacts("truyen-ngan", "Truyện ngắn"),
)
private val themes = listOf(
    TagFacts("ky-uc", "Ký ức", "Những điều đã qua còn ở lại trong người viết."),
    TagFacts("gia-dinh", "Gia đình"),
    TagFacts("mua", "Mùa"),
    TagFacts("hoi-hoa", "Hội họa"),
)
private val hoi = TagOptions.Ready(listOf(TagOption(themes[3]), TagOption(themes[0], "Hồi ức")))

private data class Sample(val form: TagFacts?, val themes: List<TagFacts>, val catalog: CatalogFacts, val query: String, val options: TagOptions)

private fun sample(scenario: String): Sample {
    val base = Sample(forms[0], listOf(themes[0]), CatalogFacts.Ready(forms, themes), "hồi", hoi)
    return when (scenario) {
        "empty" -> base.copy(form = null, themes = emptyList(), query = "", options = TagOptions.Idle)
        "full" -> base.copy(themes = themes.take(3))
        "retired" -> base.copy(themes = listOf(themes[0], retired), query = "", options = TagOptions.Idle)
        "refused" -> base.copy(
            themes = listOf(themes[0], retired.copy(problem = "Thẻ này vừa ngừng dùng nên không thể thêm. Chưa lưu thay đổi nào; bỏ hoặc thay thẻ này.")),
            query = "", options = TagOptions.Idle,
        )
        "no-results" -> base.copy(query = "xyz", options = TagOptions.Ready(emptyList()))
        "unavailable" -> base.copy(catalog = CatalogFacts.Unavailable, options = TagOptions.Unavailable)
        "long-label" -> base.copy(
            form = TagFacts("ky", "Ký sự hành trình qua những miền đất phương Nam", "Ghi chép dài về những chuyến đi, người gặp và những đổi thay của làng quê, phố thị và sông nước."),
            themes = listOf(
                TagFacts("a", "Những người đàn bà gánh nước sông"),
                TagFacts("c", "Tuổi thơ dưới mái hiên nhà bà ngoại", retired = true, successor = TagLink("b", "Ký ức tuổi thơ ở quê ngoại")),
            ),
            query = "những",
            options = TagOptions.Ready(listOf(TagOption(TagFacts("d", "Những đổi thay của làng quê sau mùa nước nổi", "Mô tả dài của ban biên tập để kiểm tra chữ xuống dòng ở màn hẹp và chữ 200%."), "Làng quê đổi thay"))),
        )
        else -> base
    }
}

@Composable
private fun TagSample(scenario: String) {
    val log = LocalPreviewLog.current
    val s = sample(scenario)
    var query by remember(scenario) { mutableStateOf(TextFieldValue(s.query)) }
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
        VinkText("Mẫu · không gọi dịch vụ, không lưu", role = TypeRole.Caption, tone = TextTone.Muted)
        TagSelector(
            form = s.form, themes = s.themes, catalog = s.catalog, query = query, options = s.options, exact = null,
            onForm = { log("dạng bài → ${it ?: "bỏ chọn"}") },
            onAddTheme = { log("thêm chủ đề → $it") },
            onRemoveTheme = { log("bỏ chủ đề → $it") },
            onReplaceTheme = { old, new -> log("thay $old → $new") },
            onQuery = { query = it; log("query “${it.text}”${if (it.composition != null) " (đang gõ dấu)" else ""}") },
        )
    }
}

@Composable
private fun TagInteractive() {
    val log = LocalPreviewLog.current
    var form by remember { mutableStateOf<String?>(null) }
    var chosen by remember { mutableStateOf(listOf(retired)) }
    var query by remember { mutableStateOf(TextFieldValue("")) }
    val q = query.text.trim().lowercase()
    val options = when {
        query.composition != null -> TagOptions.Composing
        q.isEmpty() -> TagOptions.Idle
        else -> TagOptions.Ready(themes.filter { it.label.lowercase().startsWith(q) }.map { TagOption(it) })
    }
    val exact = themes.firstOrNull { it.label.lowercase() == q }?.id
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
        VinkText("Mẫu · owner trong bộ nhớ, không lưu", role = TypeRole.Caption, tone = TextTone.Muted)
        TagSelector(
            form = forms.firstOrNull { it.id == form }, themes = chosen, catalog = CatalogFacts.Ready(forms, themes),
            query = query, options = options, exact = exact,
            onForm = { form = it; log("dạng bài → $it") },
            onAddTheme = { id ->
                val tag = themes.firstOrNull { it.id == id }
                if (tag != null && chosen.size < THEME_LIMIT && chosen.none { it.id == id }) chosen = chosen + tag
                query = TextFieldValue("")
                log("thêm chủ đề → $id")
            },
            onRemoveTheme = { id -> chosen = chosen.filter { it.id != id }; log("bỏ chủ đề → $id") },
            onReplaceTheme = { old, new ->
                val tag = themes.firstOrNull { it.id == new }
                if (tag != null) chosen = chosen.map { if (it.id == old) tag else it }
                log("thay $old → $new")
            },
            onQuery = { query = it },
        )
    }
}
