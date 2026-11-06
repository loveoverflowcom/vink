package vink.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

/** The same assertions as the Leptos `toggle` tests, against the generated shared vectors. */
class VinkToggleTest {
    private fun value(id: String) = ToggleValue.entries.first { it.id == id }

    private fun phase(id: String): TogglePhase = when (id) {
        "idle" -> TogglePhase.Idle
        "pending:on" -> TogglePhase.Pending(true)
        "pending:off" -> TogglePhase.Pending(false)
        "uncertain:on" -> TogglePhase.Uncertain(true)
        "uncertain:off" -> TogglePhase.Uncertain(false)
        "failed" -> TogglePhase.Failed
        else -> error(id)
    }

    private fun known(id: String): Boolean? = when (id) {
        "on" -> true
        "off" -> false
        "-" -> null
        else -> error(id)
    }

    @Test
    fun everySharedVectorResolvesTheSame() {
        assertEquals("toggle-contract-v1", VinkToggleVectors.VERSION)
        assertEquals(36, VinkToggleVectors.views.size)
        for (v in VinkToggleVectors.views) {
            val case = "${v.value} × ${v.phase} × ${v.availability}"
            assertEquals(v.phase, phase(v.phase).id, case)
            val got = ToggleView.resolve(value(v.value), phase(v.phase), v.availability == "enabled")
            assertEquals(known(v.pressed), got.pressed, "pressed $case")
            assertEquals(v.busy, got.busy, "busy $case")
            assertEquals(known(v.dispatch), got.dispatch, "dispatch $case")
            assertEquals(v.note, got.note?.id ?: "-", "note $case")
            assertEquals(v.recover == "reconcile", got.reconcile, "recover $case")
        }
    }

    @Test
    fun countsReadAsFactsAndUnavailableNeverBecomesZero() {
        for ((fact, shown) in VinkToggleVectors.counts) {
            val parsed = when (fact) {
                "hidden" -> CountFact.Hidden
                "unavailable" -> CountFact.Unavailable
                else -> CountFact.Known(fact.removePrefix("known:").toLong())
            }
            assertEquals(shown, parsed.text, fact)
        }
        assertEquals(CountFact.Unavailable, CountFact.fromLong(-1))
        assertFailsWith<IllegalArgumentException> { CountFact.Known(-1) }
    }

    @Test
    fun copyNamesFollowTheConfirmedValueAndBlankReasonsShowNothing() {
        val copy = ToggleCopy("Đánh giá cao", "Bỏ đánh giá cao", countUnit = "lượt đánh giá cao")
        assertEquals("Đánh giá cao", copy.name(false))
        assertEquals("Đánh giá cao", copy.name(null))
        assertEquals("Bỏ đánh giá cao", copy.name(true))
        assertEquals("1.234 lượt đánh giá cao", copy.countDescription(CountFact.Known(1234)))
        assertEquals("lượt đánh giá cao: chưa có số liệu", copy.countDescription(CountFact.Unavailable))
        assertNull(copy.countDescription(CountFact.Hidden))
        assertNull(copy.note(ToggleNote.Blocked, "  "))
        assertEquals("Lý do", copy.note(ToggleNote.Blocked, "Lý do"))
    }
}
