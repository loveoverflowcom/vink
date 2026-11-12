package vink.ui.preview

import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import vink.ui.*

/** Native interaction recipe; state is synthetic, never a product form or persistence adapter. */
@Composable
fun InteractionSpecimen(scenario: String) {
    val log = LocalPreviewLog.current
    val layer = remember { OverlayLayer() }
    var reason by remember { mutableStateOf(CloseReason.Dismiss) }
    var text by rememberSaveable(stateSaver = TextFieldValue.Saver) { mutableStateOf(TextFieldValue("Tiếng Việt [Mẫu]")) }
    var query by rememberSaveable(stateSaver = TextFieldValue.Saver) { mutableStateOf(TextFieldValue("Thơ")) }
    var selected by remember { mutableStateOf(false) }
    var submissions by remember { mutableIntStateOf(0) }
    val trigger = remember { FocusRequester() }
    val input = remember { FocusRequester() }
    val next = remember { FocusRequester() }
    val safe = remember { FocusRequester() }
    val close: (CloseReason) -> Unit = {
        reason = it
        layer.close("interaction", it)
        log("interaction → ${it.id}")
    }
    VinkStack {
        VinkText("Mẫu tương tác · không lưu hoặc gửi dữ liệu", tone = TextTone.Muted)
        VinkTextField("Tên bản thảo tiếng Việt có dấu", text, { text = it },
            inputModifier = Modifier.focusRequester(input).testTag("interaction-title"),
            imeAction = ImeAction.Next, onImeAction = { next.requestFocus() })
        VinkSearchField("Tìm tác phẩm", query, { query = it },
            inputModifier = Modifier.focusRequester(next).testTag("interaction-search"),
            onSubmit = { submissions++; log("interaction → tìm ($submissions)") })
        VinkText("Đã tìm $submissions lần", modifier = Modifier.testTag("interaction-count"))
        VinkFilterChip("Thơ và những câu chuyện tiếng Việt", selected, { selected = it }, Modifier.testTag("interaction-chip"))
        VinkButton("Mở bảng nhập [Mẫu]", {
            reason = CloseReason.Dismiss
            layer.open("interaction")
        }, Modifier.focusRequester(trigger).testTag("interaction-trigger"))
        VinkDialog(
            layer.isOpen("interaction"), "Bản thảo đang viết [Mẫu]", {
                close(if (it == DismissReason.Outside) CloseReason.Outside else CloseReason.Escape)
            }, Modifier.testTag("interaction-panel"),
            kind = if (scenario == "sheet") DialogKind.Sheet else DialogKind.Dialog,
            dismiss = if (scenario == "explicit") DismissPolicy.Explicit else DismissPolicy.EscapeAndOutside,
            closeReason = reason, initialFocus = safe, returnFocus = trigger, mainFocus = input,
            content = {
                VinkTextArea("Nội dung chưa lưu [Mẫu]", text, { text = it }, rows = 3,
                    imeAction = ImeAction.Done, onImeAction = { close(CloseReason.Dismiss) })
                VinkButton("Chuyển vùng [Mẫu]", { close(CloseReason.Navigate) }, variant = ButtonVariant.Text)
                if (scenario == "sheet") repeat(4) {
                    VinkText("[Mẫu] Nội dung dài để kiểm cuộn: tiếng Việt, đoạn văn và khổ thơ vẫn giữ nguyên.")
                }
            },
        ) {
            VinkButton("Giữ chữ và đóng", { close(CloseReason.Dismiss) },
                Modifier.focusRequester(safe).testTag("interaction-close"))
        }
    }
}
