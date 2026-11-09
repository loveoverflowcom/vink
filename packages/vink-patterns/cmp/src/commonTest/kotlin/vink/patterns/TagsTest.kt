package vink.patterns

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** The same words as the Leptos `status_words_never_claim_a_proposal_or_a_result_for_other_input`. */
class TagsTest {
    @Test
    fun statusWordsNeverClaimAProposalOrAnAnswerWhileComposing() {
        assertEquals("", optionsStatus(TagOptions.Composing, "hô", 0))
        assertEquals("", optionsStatus(TagOptions.Idle, "", 0))
        assertTrue(optionsStatus(TagOptions.Ready(emptyList()), " xyz ", 0).contains("chưa thể đề xuất thẻ mới"))
        assertTrue(optionsStatus(TagOptions.Ready(emptyList()), " xyz ", 0).contains("“xyz”"))
        val one = TagOptions.Ready(listOf(TagOption(TagFacts("a", "A"))))
        assertEquals("1 gợi ý cho “a”.", optionsStatus(one, "a", 0))
        assertTrue(optionsStatus(one, "a", THEME_LIMIT).contains("Đã đủ 3 chủ đề"))
        assertTrue(optionsStatus(TagOptions.Unavailable, "a", 0).startsWith("Tạm thời"))
    }

    @Test
    fun aRetiredOrRefusedTagIsWrittenNotColourOnly() {
        assertNull(tagNote(TagFacts("ky-uc", "Ký ức")))
        val retired = TagFacts("do-thi", "Đô thị", retired = true, successor = TagLink("mua", "Mùa"))
        assertTrue(tagNote(retired)!!.contains("“Mùa”"))
        assertEquals("Từ chối.", tagNote(retired.copy(problem = "Từ chối.")))
    }
}
