package vink.ui.preview

import androidx.compose.runtime.saveable.rememberSaveable

import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.TextFieldValue
import vink.ui.*
import vink.patterns.*

/** Sample consumer. Fixtures never enter app adapters or the patterns library. */
@Composable
fun ScreenRecipe(id: String, scenario: String) {
    var body by rememberSaveable(stateSaver = TextFieldValue.Saver) { mutableStateOf(TextFieldValue(VinkFixture.READING)) }
    var title by rememberSaveable(stateSaver = TextFieldValue.Saver) { mutableStateOf(TextFieldValue(if (scenario == "long") VinkFixture.LONG_TITLE else VinkFixture.TITLE)) }
    var query by rememberSaveable(stateSaver = TextFieldValue.Saver) { mutableStateOf(TextFieldValue("")) }
    var theme by remember { mutableStateOf<String?>("system") }
    var message by remember { mutableStateOf("") }
    val log = LocalPreviewLog.current
    val navigate: (String) -> Unit = { message = "Mẫu · Đích $it"; log("screen → $it (sample)") }
    val state = when (scenario) {
        "pending" -> ActionState.Pending("Mẫu · Đang chờ phản hồi")
        "auth-required" -> ActionState.Disabled("Cần đăng nhập; mẫu không có dịch vụ xác thực.")
        "uncertain" -> ActionState.Disabled("Chưa rõ kết quả; cần đối chiếu trước khi gửi thêm.")
        else -> ActionState.Enabled
    }
    val author = Author(if (scenario == "long") VinkFixture.COMBINING else "[Mẫu] Nguyễn Thị Ánh Tuyết", "screen-author", "2026-11-02T02:00:00Z")
    val draft = DraftSummary(title.text, 7, "2026-11-02T02:00:00Z", "#screen.editor/dirty")
    val comments = listOf(Feedback("feedback-01", Person("screen-commenter", "[Mẫu] Lê Mai"), VinkFixture.BODY, VinkFixture.TITLE, "#screen.reader/feedback"))
    val submit = { message = "Mẫu · Intent gửi nhận xét, chưa gửi hoặc lưu."; log("feedback → submit intent (sample)") }
    Column(Modifier.fillMaxWidth().testTag("screen:$id:$scenario"), verticalArrangement = Arrangement.spacedBy(VinkDimens.Space4)) {
        VinkText("Mẫu · Dữ liệu cố định · Không kết nối dịch vụ", role = TypeRole.Caption, tone = TextTone.Muted)
        if (scenario == "loading") {
            VinkText("Đang tải…")
            VinkSkeleton(shape = SkeletonShape.Card)
        } else if (scenario in listOf("empty", "no-results", "not-found", "auth-required", "forbidden") && id !in listOf("composition.feedback", "screen.me")) {
            val panel = when (scenario) {
                "empty" -> (PanelContent.empty(EmptyKind.FirstUse, "Mẫu · empty", "Chưa có dữ liệu.") as PanelContent.Result.Ok).content
                "no-results" -> (PanelContent.empty(EmptyKind.NoResults, "Mẫu · no-results", "Thử tìm trong danh sách đã tải.") as PanelContent.Result.Ok).content
                else -> PanelContent.fromProblem(Problem.fallback(when (scenario) {
                    "auth-required" -> ProblemKind.AuthRequired
                    "forbidden" -> ProblemKind.Forbidden
                    else -> ProblemKind.NotFound
                }))
            }
            VinkStatePanel(panel, {})
        } else {
            when (scenario) {
                "error" -> VinkInlineNotice(Tone.Error, "Chưa tải/lưu được. Chữ đang viết được giữ nguyên; hãy thử lại.")
                "conflict" -> VinkInlineNotice(Tone.Warning, "Phiên bản đã đổi. Giữ chữ trên trang để đối chiếu, chưa xác nhận lưu.")
                "uncertain" -> VinkInlineNotice(Tone.Warning, "Chưa rõ kết quả. Đối chiếu trước khi gửi thêm; không tự gửi lại.")
                "refresh" -> VinkInlineNotice(Tone.Info, "Đang tải lại. Nội dung đã tải vẫn còn trên trang.")
            }
            BoxWithConstraints {
                val narrow = maxWidth.value < 760
                Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space4)) {
                    when (id) {
                        "screen.explore" -> {
                            VinkHeading("Khám phá", HeadingLevel.H1)
                            VinkText("Đọc một câu chuyện, góp một ý hay, rồi viết tiếp.")
                            VinkSearchField("Tìm trong danh sách mẫu", query, { query = it })
                            VinkText("Mẫu · Chỉ tìm title/tác giả trong danh sách đã tải, tối đa 12 ở app.", role = TypeRole.Caption)
                            WorkCard(title.text, author, "#screen.reader/default", navigate, narrow)
                            VinkSurface(tone = SurfaceTone.Primary, radius = Radius.Lg, padding = Space.S6) {
                                Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
                                    VinkHeading("Một nơi bạn nhớ", HeadingLevel.H2, role = TypeRole.Title)
                                    VinkLink("Bắt đầu viết", "#screen.editor/dirty", navigate)
                                }
                            }
                            VinkListEnd()
                        }
                        "screen.reader" -> {
                            VinkLink("← Khám phá", "#screen.explore/default", navigate)
                            ReaderHeader(title.text, author, narrow)
                            VinkText(VinkFixture.READING, role = TypeRole.Reading)
                            if (scenario == "feedback") {
                                FeedbackThread(comments, navigate)
                                FeedbackComposer(FeedbackIntent.Impressions, body, { body = it }, state, submit)
                            }
                        }
                        "screen.desk" -> {
                            VinkHeading("Bàn viết", HeadingLevel.H1)
                            DraftResume(draft, navigate)
                            VinkSearchField("Tìm trong bản thảo đã tải", query, { query = it })
                            VinkText("Mẫu · Tối đa 100 nháp trong app; bản nháp chỉ bạn đọc được.", role = TypeRole.Caption)
                            VinkLink(draft.title, draft.href, navigate)
                        }
                        "screen.editor" -> {
                            VinkHeading("Bản nháp — mẫu chrome", HeadingLevel.H1, role = TypeRole.Title)
                            EditorToolbar("Mẫu · $scenario · Không kết nối lưu") {
                                VinkButton("Xem trước", { log("editor → preview (sample)") }, variant = ButtonVariant.Text)
                                VinkButton("Lưu nháp", { message = "Mẫu · Chỉ ghi intent, chưa lưu lên máy chủ."; log("editor → save intent (sample)") }, state = state)
                            }
                            VinkTextField("Tiêu đề", title, { if (it.text.length <= 200) title = it })
                            VinkTextArea("Nội dung — controlled specimen", body, { body = it })
                            VinkText("Mẫu không có CodeMirror, backend hay native editor parity. Editor app được kiểm riêng.", role = TypeRole.Caption)
                        }
                        "screen.me" -> {
                            VinkHeading("Tôi", HeadingLevel.H1)
                            VinkAvatar(null, null, kind = AvatarKind.Account)
                            VinkText("Chưa có thông tin tài khoản")
                            VinkText("Vink chưa mở đăng nhập.", tone = TextTone.Muted)
                            VinkRadioGroup("Giao diện", listOf(ChoiceOption("light", "Sáng"), ChoiceOption("dark", "Tối"), ChoiceOption("system", "Hệ thống")), theme, { theme = it })
                        }
                        else -> {
                            RecentFeedback(comments, navigate)
                            FeedbackComposer(when (scenario) { "sharing" -> FeedbackIntent.Sharing; "technique" -> FeedbackIntent.Technique; else -> FeedbackIntent.Impressions }, body, { body = it }, state, submit)
                        }
                    }
                }
            }
            if (scenario == "error") VinkButton("Thử lại", { message = "Mẫu · Đã yêu cầu tải lại; không gọi dịch vụ."; log("screen → retry (sample)") })
        }
        VinkText(message, modifier = Modifier.testTag("sample-result"))
    }
}
