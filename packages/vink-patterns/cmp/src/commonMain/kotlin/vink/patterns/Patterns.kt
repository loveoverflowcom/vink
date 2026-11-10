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
/** `Unspecified` has no label: the host heading names the form and the field label repeats it. */
enum class FeedbackIntent(val label: String?) {
    Unspecified(null), Sharing("Chỉ chia sẻ"), Impressions("Muốn nghe cảm nhận"), Technique("Xin góp ý kỹ thuật")
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
fun FeedbackComposer(intent: FeedbackIntent, value: TextFieldValue, onInput: (TextFieldValue) -> Unit, state: ActionState, onSubmit: () -> Unit, commentsClosed: Boolean = false) {
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
        intent.label?.let { VinkText(it, role = TypeRole.Label) }
        VinkTextArea(if (intent == FeedbackIntent.Technique) "Góp ý kỹ thuật có ngữ cảnh" else "Nhận xét của bạn", value, onInput)
        VinkButton("Gửi nhận xét", onSubmit, state = if (state is ActionState.Pending) state else if (commentsClosed) ActionState.Disabled("Tác giả đã tắt bình luận. Bản đang soạn được giữ lại.") else state)
    }
}

/** Authorized display facts; no HTTP, actor lookup or write authority in this library. */
data class DiscussionComment(val id: String, val author: Author, val body: String, val status: String? = null, val context: DiscussionContext? = null, val canReply: Boolean = false, val canRemove: Boolean = false)
sealed interface DiscussionContext {
    data object Root : DiscussionContext
    data class Reply(val name: String, val href: String?) : DiscussionContext
    data object Unavailable : DiscussionContext
}
data class DiscussionThreadFacts(val rootId: String, val root: DiscussionComment?, val replies: List<DiscussionComment>)

/** Context does not add an indentation level. Callers retain draft/request scope. */
@Composable
fun DiscussionThread(facts: DiscussionThreadFacts, onReply: (String) -> Unit, onRemove: (String) -> Unit, onNavigate: (String) -> Unit, actionState: ActionState = ActionState.Enabled) {
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
        if (facts.root == null) VinkText("Bình luận gốc không còn hiển thị.", tone = TextTone.Muted)
        else key(facts.root.id) { DiscussionCommentContent(facts.root, onReply, onRemove, onNavigate, actionState) }
        Column(Modifier.padding(start = VinkDimens.Space3), verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
            facts.replies.forEach { comment -> key(comment.id) { DiscussionCommentContent(comment, onReply, onRemove, onNavigate, actionState) } }
        }
    }
}

@Composable
@OptIn(ExperimentalLayoutApi::class)
private fun DiscussionCommentContent(comment: DiscussionComment, onReply: (String) -> Unit, onRemove: (String) -> Unit, onNavigate: (String) -> Unit, state: ActionState) {
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2)) {
        WorkByline(comment.author, narrow = true, size = AvatarSize.Medium)
        when (val context = comment.context) {
            DiscussionContext.Root -> VinkText("Trả lời bình luận gốc", role = TypeRole.Caption)
            DiscussionContext.Unavailable -> VinkText("Bình luận được trả lời không còn hiển thị", role = TypeRole.Caption)
            is DiscussionContext.Reply -> if (context.href != null) VinkLink("Đang trả lời @${context.name}", context.href, onNavigate)
                else VinkText("Đang trả lời @${context.name} · Ngoài trang đã tải; chưa có đường mở ngữ cảnh này.", role = TypeRole.Caption)
            null -> Unit
        }
        comment.status?.let { VinkText(it, role = TypeRole.Label) }
        VinkText(comment.body)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(VinkDimens.Space2)) {
            if (comment.canReply) VinkButton("Trả lời", { onReply(comment.id) }, variant = ButtonVariant.Text)
            if (comment.canRemove) VinkButton("Gỡ nhận xét của tôi", { onRemove(comment.id) }, variant = ButtonVariant.Text, state = state)
        }
        VinkDivider()
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

/** What a literary endorsement (LVE #73) is about; copy only, the App's core decides. */
enum class EndorsementKind(val id: String) {
    /** One published revision of a work: “Đánh giá cao”. */
    Work("work"),

    /** A root comment or a flat reply, each its own target: “Hữu ích”. */
    Comment("comment"),
    ;

    val copy: ToggleCopy
        get() = when (this) {
            Work -> ToggleCopy(
                "Đánh giá cao", "Bỏ đánh giá cao", countUnit = "lượt đánh giá cao",
                pendingOn = "Đang gửi đánh giá cao…", pendingOff = "Đang bỏ đánh giá cao…",
                failed = "Không thể đánh giá lúc này.", unknown = "Chưa rõ bạn đã đánh giá cao chưa.",
            )
            Comment -> ToggleCopy(
                "Hữu ích", "Bỏ đánh dấu hữu ích", countUnit = "lượt hữu ích",
                pendingOn = "Đang gửi đánh dấu hữu ích…", pendingOff = "Đang bỏ đánh dấu hữu ích…",
                failed = "Không thể đánh dấu lúc này.", unknown = "Chưa rõ bạn đã đánh dấu hữu ích chưa.",
            )
        }

    /** Visible text, never hover-only; no login control is implied (no auth authority yet). */
    fun reason(block: EndorsementBlock): String = when (block) {
        EndorsementBlock.OwnContent -> if (this == Work) "Bạn không thể đánh giá tác phẩm của chính mình." else "Bạn không thể đánh dấu nhận xét của chính mình."
        EndorsementBlock.NotPermitted -> if (this == Work) "Tài khoản này chưa thể đánh giá." else "Tài khoản này chưa thể đánh dấu hữu ích."
        EndorsementBlock.SignedOut -> if (this == Work) "Cần đăng nhập để đánh giá." else "Cần đăng nhập để đánh dấu hữu ích."
        EndorsementBlock.Unavailable -> if (this == Work) "Không thể đánh giá lúc này." else "Không thể đánh dấu lúc này."
    }
}

/** Why the viewer cannot endorse here, as the adapter learned it; advisory, writes decide again. */
enum class EndorsementBlock { OwnContent, NotPermitted, SignedOut, Unavailable }

/** View facts an adapter projects from summary/own-state reads and its own command reducer. */
data class EndorsementFacts(
    val value: ToggleValue = ToggleValue.Unknown,
    val phase: TogglePhase = TogglePhase.Idle,
    /** Public count of this target only; never summed into a root or work. */
    val count: CountFact = CountFact.Hidden,
    val block: EndorsementBlock? = null,
)

/**
 * The endorsement control for one target. A press emits the desired selection; the adapter owns
 * `command_key`, `expected_order`, session fencing and late answers. CHOICE-07 A: Pen Flight R2.
 */
@Composable
fun EndorsementAction(
    kind: EndorsementKind,
    facts: EndorsementFacts,
    onSet: (Boolean) -> Unit,
    onRecover: (RecoveryIntent) -> Unit,
    modifier: Modifier = Modifier,
) {
    VinkToggleAction(
        copy = kind.copy,
        icon = IconName.PenFlight,
        value = facts.value,
        onSet = onSet,
        onRecover = onRecover,
        modifier = modifier,
        phase = facts.phase,
        count = facts.count,
        blocked = facts.block?.let(kind::reason),
    )
}
