package vink.patterns

import androidx.compose.foundation.layout.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.TextFieldValue
import vink.ui.*

/** Vink view facts only. Adapters own authority, transport, profiles and request identity. */
data class Author(val name: String, val key: String?, val publishedAt: String)
data class DraftSummary(val title: String, val version: Long, val updatedAt: String, val href: String)
data class Feedback(val id: String, val person: Person, val text: String, val context: String, val href: String)
enum class FeedbackIntent(val label: String) {
    Sharing("Chỉ chia sẻ"), Impressions("Muốn nghe cảm nhận"), Technique("Xin góp ý kỹ thuật")
}

fun participants(items: List<Feedback>): List<Person> = items.map { it.person }.filter { !it.key.isNullOrEmpty() }.distinctBy { it.key }

private fun date(timestamp: String): String {
    val parts = timestamp.take(10).split('-')
    return if (parts.size == 3) "${parts[2]}/${parts[1]}/${parts[0]}" else timestamp
}

@Composable
fun WorkByline(author: Author, narrow: Boolean, size: AvatarSize = AvatarSize.Feed) {
    VinkAuthorByline(author.name, author.key, size = size, narrow = narrow, meta = listOf("${date(author.publishedAt)} (UTC)"))
}

@Composable
fun WorkCard(title: String, author: Author, href: String, onNavigate: (String) -> Unit, narrow: Boolean) {
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
        WorkByline(author, narrow)
        VinkHeading(title, HeadingLevel.H3, role = TypeRole.Title)
        VinkLink("Đọc tác phẩm →", href, onNavigate, variant = LinkVariant.Standalone)
    }
}

@Composable
fun ReaderHeader(title: String, author: Author, narrow: Boolean) {
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
        VinkText("Bản công khai", role = TypeRole.Caption, tone = TextTone.Muted)
        VinkHeading(title, HeadingLevel.H1)
        WorkByline(author, narrow, AvatarSize.Reader)
    }
}

@Composable
fun DraftResume(draft: DraftSummary, onNavigate: (String) -> Unit) {
    VinkSurface(tone = SurfaceTone.Primary, radius = Radius.Lg, padding = Space.S3) {
        Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
            VinkText("Trở lại trang đang viết", role = TypeRole.Label)
            VinkHeading(draft.title, HeadingLevel.H2)
            VinkText("Phiên bản ${draft.version} · Cập nhật ${date(draft.updatedAt)} UTC", role = TypeRole.Caption)
            VinkLink("Viết tiếp", draft.href, onNavigate, variant = LinkVariant.Standalone)
        }
    }
}

@Composable
fun EditorToolbar(status: String, actions: @Composable () -> Unit) {
    VinkSurface(tone = SurfaceTone.High, radius = Radius.Lg) {
        Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2)) {
            VinkText(status, role = TypeRole.Label)
            actions()
        }
    }
}

@Composable
fun ParticipantSummary(items: List<Feedback>) {
    val people = participants(items)
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2)) {
        if (people.isNotEmpty()) VinkAvatarGroup(people, "Người góp ý")
        VinkText(if (items.isEmpty()) "Mời một lời góp ý đầu tiên" else "${items.size} nhận xét trong danh sách", role = TypeRole.Caption)
    }
}

@Composable
fun FeedbackItem(item: Feedback, onNavigate: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
        VinkAuthorByline(item.person.name.orEmpty(), item.person.key)
        VinkText(item.text)
        VinkLink(item.context, item.href, onNavigate)
        VinkDivider()
    }
}

@Composable
fun FeedbackThread(items: List<Feedback>, onNavigate: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space4)) {
        ParticipantSummary(items)
        items.forEach { item -> key(item.id) { FeedbackItem(item, onNavigate) } }
    }
}

/** Controlled TextFieldValue retains selection/composition through host updates and errors. */
@Composable
fun FeedbackComposer(intent: FeedbackIntent, value: TextFieldValue, onInput: (TextFieldValue) -> Unit, state: ActionState, onSubmit: () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
        VinkText(intent.label, role = TypeRole.Label)
        if (intent == FeedbackIntent.Sharing) {
            VinkText("Tác giả chỉ chia sẻ bài viết này.", tone = TextTone.Muted)
        } else {
            VinkTextArea(if (intent == FeedbackIntent.Technique) "Góp ý kỹ thuật có ngữ cảnh" else "Cảm nhận của bạn", value, onInput)
            VinkButton("Gửi nhận xét", onSubmit, state = state)
        }
    }
}

@Composable
fun RecentFeedback(items: List<Feedback>, onNavigate: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space4)) {
        VinkHeading("Từ những cuộc trò chuyện", HeadingLevel.H2, role = TypeRole.Title)
        VinkText("Mẫu · Không có dịch vụ nhận xét", role = TypeRole.Caption, tone = TextTone.Muted)
        FeedbackThread(items, onNavigate)
    }
}
