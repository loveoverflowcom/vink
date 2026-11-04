package vink.patterns

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import vink.ui.Person

class ParticipantsTest {
    @Test
    fun onlyReturnedKnownPeopleInFirstAppearanceOrder() {
        fun reply(id: String, key: String?, name: String) = Feedback(id, Person(key, name), "Lời góp ý", "Tác phẩm", "#reader/feedback")
        val comments = listOf(reply("1", "b", "Bình"), reply("2", null, "Chưa rõ"), reply("3", "a", "An"), reply("4", "b", "Bình"), reply("5", "", "Không có ID"))
        assertEquals(listOf("b", "a"), participants(comments).map { it.key })
        assertEquals(emptyList(), participants(emptyList()))
    }

    @Test
    fun onlyNamedFeedbackIntentsCarryALabel() {
        assertNull(FeedbackIntent.Unspecified.label)
        assertEquals(listOf("Chỉ chia sẻ", "Muốn nghe cảm nhận", "Xin góp ý kỹ thuật"), listOf(FeedbackIntent.Sharing, FeedbackIntent.Impressions, FeedbackIntent.Technique).map { it.label })
    }
}
