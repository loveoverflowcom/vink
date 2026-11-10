package vink.ui.preview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import vink.patterns.EndorsementAction
import vink.patterns.EndorsementBlock
import vink.patterns.EndorsementFacts
import vink.patterns.EndorsementKind
import vink.ui.ActionState
import vink.ui.ButtonVariant
import vink.ui.CountFact
import vink.ui.Direction
import vink.ui.Radius
import vink.ui.Space
import vink.ui.SurfaceTone
import vink.ui.TextTone
import vink.ui.ToggleCopy
import vink.ui.TogglePhase
import vink.ui.ToggleValue
import vink.ui.TypeRole
import vink.ui.VinkButton
import vink.ui.VinkDimens
import vink.ui.VinkStack
import vink.ui.VinkSurface
import vink.ui.VinkText
import vink.ui.VinkToggleAction

/**
 * ToggleAction and the LVE EndorsementAction through public APIs, same IDs/scenarios as the Leptos
 * gallery. The lifecycle "owner" is an in-memory sample; nothing is fetched, written or confirmed.
 */
@Composable
fun EndorsementSpecimen(id: String, scenario: String): Boolean {
    when (id) {
        "action.toggle-action" -> if (scenario == "lifecycle") ToggleLifecycle() else ToggleSample(scenario)
        "composition.endorsement" -> EndorsementSample(scenario)
        else -> return false
    }
    return true
}

private val sampleCopy = ToggleCopy("Ghi nhận", "Bỏ ghi nhận", countUnit = "lượt ghi nhận")

@Composable
private fun ToggleSample(scenario: String) {
    val log = LocalPreviewLog.current
    data class Facts(val value: ToggleValue, val phase: TogglePhase, val count: CountFact, val blocked: String? = null, val copy: ToggleCopy = sampleCopy)
    val f = when (scenario) {
        "selected" -> Facts(ToggleValue.On, TogglePhase.Idle, CountFact.Known(13))
        "pending" -> Facts(ToggleValue.Off, TogglePhase.Pending(true), CountFact.Known(12))
        "uncertain" -> Facts(ToggleValue.Off, TogglePhase.Uncertain(true), CountFact.Known(12))
        "failed" -> Facts(ToggleValue.On, TogglePhase.Failed, CountFact.Known(13))
        "unknown" -> Facts(ToggleValue.Unknown, TogglePhase.Idle, CountFact.Known(12))
        "disabled" -> Facts(ToggleValue.Off, TogglePhase.Idle, CountFact.Known(12), "Mẫu · Lý do do caller cấp, hiển thị thành chữ.")
        "count-unavailable" -> Facts(ToggleValue.Off, TogglePhase.Idle, CountFact.Unavailable)
        "long-label" -> Facts(
            ToggleValue.On, TogglePhase.Idle, CountFact.Known(1_234_567),
            copy = ToggleCopy(
                "Ghi nhận giá trị văn chương của truyện ngắn nhiều chương này",
                "Bỏ ghi nhận giá trị văn chương của truyện ngắn nhiều chương này",
                countUnit = "lượt ghi nhận",
            ),
        )
        else -> Facts(ToggleValue.Off, TogglePhase.Idle, CountFact.Known(12))
    }
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
        VinkToggleAction(
            f.copy, f.value,
            onSet = { log("toggle-action → muốn ${if (it) "bật" else "tắt"} (caller giữ phase)") },
            onRecover = { log("toggle-action → ${it.id}") },
            modifier = Modifier.testTag("toggle-sample"),
            phase = f.phase, count = f.count, blocked = f.blocked,
        )
        VinkText("Mẫu · Chỉ owner xác nhận mới đổi dấu chọn; specimen không gọi dịch vụ.", role = TypeRole.Caption, tone = TextTone.Muted)
    }
}

/** A sample owner: the press only asks; a step button decides what the "owner" did. */
@Composable
private fun ToggleLifecycle() {
    val log = LocalPreviewLog.current
    var value by remember { mutableStateOf(ToggleValue.Off) }
    var phase by remember { mutableStateOf<TogglePhase>(TogglePhase.Idle) }
    var count by remember { mutableLongStateOf(12) }
    var ownerOn by remember { mutableStateOf(false) }
    var ownerCount by remember { mutableLongStateOf(12) }
    var presses by remember { mutableIntStateOf(0) }
    val desired = (phase as? TogglePhase.Pending)?.desired
    fun apply(selected: Boolean) {
        if (selected != ownerOn) ownerCount += if (selected) 1 else -1
        ownerOn = selected
    }
    fun read() {
        value = ToggleValue.fromSelected(ownerOn)
        count = ownerCount
        phase = TogglePhase.Idle
    }
    val step = if (desired != null) ActionState.Enabled else ActionState.Disabled("Bấm Ghi nhận trước.")
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space4)) {
        VinkToggleAction(
            sampleCopy, value,
            onSet = { next ->
                // The caller moves to Pending before returning: a second click is ignored.
                phase = TogglePhase.Pending(next)
                presses += 1
                log("toggle-action → muốn ${if (next) "bật" else "tắt"} (lần $presses)")
            },
            onRecover = { intent -> read(); log("toggle-action → ${intent.id} → owner đang ${if (ownerOn) "bật" else "tắt"}") },
            modifier = Modifier.testTag("toggle-lifecycle"),
            phase = phase, count = CountFact.Known(count),
        )
        VinkSurface(Modifier.fillMaxWidth(), SurfaceTone.Container, Radius.Md, Space.S3) {
            Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2)) {
                VinkText("Mẫu · Bước của owner giả lập", role = TypeRole.Caption, tone = TextTone.Muted)
                VinkStack(direction = Direction.Row, gap = Space.S2, wrap = true) {
                    VinkButton("Chấp nhận", onPress = { desired?.let { apply(it); read(); log("owner → chấp nhận, phản hồi tới") } }, variant = ButtonVariant.Tonal, state = step)
                    VinkButton("Mất phản hồi", onPress = { desired?.let { apply(it); phase = TogglePhase.Uncertain(it); log("owner → chấp nhận, phản hồi bị mất") } }, variant = ButtonVariant.Tonal, state = step)
                    VinkButton("Từ chối", onPress = { if (desired != null) { phase = TogglePhase.Failed; log("owner → từ chối, không đổi gì") } }, variant = ButtonVariant.Tonal, state = step)
                }
                VinkText("Owner (ẩn với người dùng): ${if (ownerOn) "bật" else "tắt"} · $ownerCount", role = TypeRole.Caption, tone = TextTone.Muted)
            }
        }
    }
}

private fun facts(value: ToggleValue, phase: TogglePhase, count: CountFact, block: EndorsementBlock? = null) =
    EndorsementFacts(value, phase, count, block)

@Composable
private fun EndorsementSample(scenario: String) {
    val log = LocalPreviewLog.current
    val idle = TogglePhase.Idle
    val (work, root, replies) = when (scenario) {
        "selected" -> Triple(facts(ToggleValue.On, idle, CountFact.Known(13)), facts(ToggleValue.On, idle, CountFact.Known(4)), listOf(facts(ToggleValue.On, idle, CountFact.Known(1)), facts(ToggleValue.Off, idle, CountFact.Known(0))))
        "pending" -> Triple(facts(ToggleValue.Off, TogglePhase.Pending(true), CountFact.Known(12)), facts(ToggleValue.On, TogglePhase.Pending(false), CountFact.Known(4)), listOf(facts(ToggleValue.Off, idle, CountFact.Known(1)), facts(ToggleValue.Off, idle, CountFact.Known(0))))
        "uncertain" -> Triple(facts(ToggleValue.Off, TogglePhase.Uncertain(true), CountFact.Known(12)), facts(ToggleValue.Off, idle, CountFact.Known(3)), listOf(facts(ToggleValue.On, TogglePhase.Uncertain(false), CountFact.Known(1)), facts(ToggleValue.Off, idle, CountFact.Known(0))))
        "own-content" -> Triple(facts(ToggleValue.Off, idle, CountFact.Known(12), EndorsementBlock.OwnContent), facts(ToggleValue.Off, idle, CountFact.Known(3)), listOf(facts(ToggleValue.Off, idle, CountFact.Known(1), EndorsementBlock.OwnContent), facts(ToggleValue.Off, idle, CountFact.Known(0))))
        "signed-out" -> {
            fun guest(n: Long) = facts(ToggleValue.Unknown, idle, CountFact.Known(n), EndorsementBlock.SignedOut)
            Triple(guest(12), guest(3), listOf(guest(1), guest(0)))
        }
        "unavailable" -> {
            val gone = facts(ToggleValue.Unknown, idle, CountFact.Unavailable, EndorsementBlock.Unavailable)
            Triple(gone, facts(ToggleValue.Off, idle, CountFact.Known(3)), listOf(gone, facts(ToggleValue.Off, idle, CountFact.Unavailable)))
        }
        else -> Triple(facts(ToggleValue.Off, idle, CountFact.Known(12)), facts(ToggleValue.Off, idle, CountFact.Known(3)), listOf(facts(ToggleValue.On, idle, CountFact.Known(1)), facts(ToggleValue.Off, idle, CountFact.Known(0))))
    }
    fun set(target: String): (Boolean) -> Unit = { log("$target → muốn ${if (it) "bật" else "tắt"} (caller giữ phase)") }
    fun recover(target: String): (vink.ui.RecoveryIntent) -> Unit = { log("$target → ${it.id}") }
    Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space4)) {
        VinkText("Mẫu · Người, tác phẩm và số đếm là dữ liệu trình bày; không gọi dịch vụ, Pen Flight R2 A · Mềm (CHOICE-07).", role = TypeRole.Caption, tone = TextTone.Muted)
        VinkSurface(Modifier.fillMaxWidth(), SurfaceTone.Container, Radius.Md, Space.S4) {
            Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2)) {
                VinkText("Mùa hạ cuối cùng — Mẫu", role = TypeRole.Title)
                VinkText("An Nhiên · bản công khai", role = TypeRole.Caption, tone = TextTone.Muted)
                EndorsementAction(EndorsementKind.Work, work, set("endorse-work"), recover("endorse-work"), Modifier.testTag("endorse-work"))
            }
        }
        VinkSurface(Modifier.fillMaxWidth(), SurfaceTone.Low, Radius.Md, Space.S4) {
            Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
                VinkText("Trần Linh — nhận xét gốc (Mẫu)", role = TypeRole.Label)
                VinkText("Đoạn mở đầu giữ nhịp rất tốt; câu thứ ba hơi dài.")
                EndorsementAction(EndorsementKind.Comment, root, set("endorse-root"), recover("endorse-root"), Modifier.testTag("endorse-root"))
                Column(Modifier.padding(start = VinkDimens.Space4), verticalArrangement = Arrangement.spacedBy(VinkDimens.Space4)) {
                    listOf(
                        "An Nhiên — phản hồi (Mẫu)" to "Cảm ơn, mình sẽ ngắt câu đó.",
                        "Nguyễn Hoài Thương — phản hồi (Mẫu)" to "Mình thấy câu dài lại hợp với cảnh mưa.",
                    ).forEachIndexed { i, (who, text) ->
                        Column(verticalArrangement = Arrangement.spacedBy(VinkDimens.Space2)) {
                            VinkText(who, role = TypeRole.Label)
                            VinkText(text)
                            EndorsementAction(EndorsementKind.Comment, replies[i], set("endorse-reply-${i + 1}"), recover("endorse-reply-${i + 1}"), Modifier.testTag("endorse-reply-${i + 1}"))
                        }
                    }
                }
            }
        }
    }
}
