package vink.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** The same assertions as the Leptos `state` tests, against the generated shared vectors. */
class VinkStateTest {
    @Test
    fun regionLayoutsMatchTheSharedVectors() {
        assertEquals(7 * 16, VinkStateVectors.regions.size)
        for ((content, phase, expected) in VinkStateVectors.regions) {
            val c = Content.parse(content) ?: error(content)
            val p = Phase.parse(phase) ?: error(phase)
            assertEquals(content, c.id)
            assertEquals(phase, p.id)
            val actual = when (val r = RegionState(c, p).layout()) {
                is RegionResult.Lawful -> r.layout.id
                is RegionResult.Rejected -> "!" + r.reason.id
            }
            assertEquals(expected, actual, "$content × $phase")
        }
    }

    @Test
    fun intentsMatchTheSharedVectorsAndUnknownWritesNeverRetry() {
        for ((kind, allowed) in VinkStateVectors.intents) {
            val k = ProblemKind.parse(kind) ?: error(kind)
            for (intent in RecoveryIntent.entries) {
                assertEquals(intent.id in allowed, k.allows(intent), "$kind ${intent.id}")
            }
        }
        val uncertain = Problem.fallback(ProblemKind.Uncertain)
        assertNull(uncertain.withAction(RecoveryIntent.Retry, "Gửi lại"))
        assertEquals(1, uncertain.withAction(RecoveryIntent.Reconcile, "Kiểm tra lại")?.actions?.size)
    }

    @Test
    fun placementsAndNoticeSpecsMatchTheSharedVectors() {
        for ((kind, hasData, writing, expected) in VinkStateVectors.placements) {
            val (p, a) = placementFor(ProblemKind.parse(kind as String)!!, hasData as Boolean, writing as Boolean)
            assertEquals(expected, "${p.id}/${a.id}", "$kind $hasData $writing")
        }
        for ((placement, tone, announce, dismiss, expected) in VinkStateVectors.notices) {
            val result = NoticeSpec.create(Placement.parse(placement)!!, Tone.parse(tone)!!, Announce.parse(announce)!!, Dismiss.parse(dismiss)!!)
            val actual = when (result) {
                is NoticeSpec.Result.Ok -> "ok"
                is NoticeSpec.Result.Rejected -> result.error.id
            }
            assertEquals(expected, actual, "$placement $tone $announce $dismiss")
        }
    }

    @Test
    fun theLoadingGateMatchesTheSharedVectors() {
        val (delay, minVisible, slow) = VinkStateVectors.gatePolicy
        val policy = LoadingPolicy(delay, minVisible, slow)
        assertEquals(LoadingPolicy(), policy, "vectors use the tokens")
        for (case in VinkStateVectors.gate) {
            val gate = LoadingGate(policy)
            val events = case.events.toMutableList()
            for (check in case.checks) {
                while (events.isNotEmpty() && events.first().second <= check.at) {
                    val (what, t) = events.removeAt(0)
                    if (what == "start") gate.start(t) else gate.finish(t)
                }
                assertEquals(check.visible, gate.visible(check.at), "${case.name} visible @${check.at}")
                assertEquals(check.slow, gate.slow(check.at), "${case.name} slow @${check.at}")
                assertEquals(check.next, gate.nextChange(check.at), "${case.name} next @${check.at}")
            }
        }
    }

    @Test
    fun codesAreStableIdentifiersOnly() {
        for (ok in listOf("draft.save.conflict", "e503", "net-timeout_2")) assertTrue(DiagnosticCode.parse(ok) != null, ok)
        for (bad in listOf("", "Draft", "at vink::api::save (api.rs:10)", "Bearer eyJhbGciOi", "-x", "a".repeat(65))) {
            assertNull(DiagnosticCode.parse(bad), bad)
        }
        assertNull(Problem.fallback(ProblemKind.Unknown).withCode("{\"error\":\"raw\"}").code)
    }

    @Test
    fun panelsOnlyOfferLawfulActions() {
        val start = RecoveryAction(RecoveryIntent.Start, "Viết bản nháp đầu tiên")
        val clear = RecoveryAction(RecoveryIntent.ClearFilter, "Xóa bộ lọc")
        assertIs<PanelContent.Result.Ok>(PanelContent.empty(EmptyKind.FirstUse, "t", "m", listOf(start)))
        assertEquals(
            PanelContent.Result.Rejected(PanelError.IntentNotAllowed(RecoveryIntent.ClearFilter)),
            PanelContent.empty(EmptyKind.FirstUse, "t", "m", listOf(clear)),
        )
        assertIs<PanelContent.Result.Ok>(PanelContent.empty(EmptyKind.NoResults, "t", "m", listOf(clear)))
        for (kind in ProblemKind.entries) assertTrue(!kind.allows(RecoveryIntent.Start) && !kind.allows(RecoveryIntent.ClearFilter), kind.id)
        assertEquals(Tone.Neutral, PanelContent.cancelled("Đã dừng", "", "Tải lại").tone, "cancelled is never red")
        assertTrue(!DismissPolicy.Explicit.allows(DismissReason.Escape))
        assertTrue(DismissPolicy.EscapeOnly.allows(DismissReason.Escape))
        assertTrue(!DismissPolicy.EscapeOnly.allows(DismissReason.Outside))
        assertTrue(DismissPolicy.EscapeAndOutside.allows(DismissReason.Outside))
        assertEquals(Dismiss.Auto, SnackbarMessage("Đã lưu").dismiss)
        assertEquals(Dismiss.Manual, SnackbarMessage("Đã xóa", "Hoàn tác").dismiss)
    }

    @Test
    fun progressIsClampedAndNeverInvented() {
        assertNull(Progress.Indeterminate.value)
        assertEquals(1f, Progress.Determinate(1.4f).value)
        assertEquals(0f, Progress.Determinate(Float.NaN).value)
    }
}
