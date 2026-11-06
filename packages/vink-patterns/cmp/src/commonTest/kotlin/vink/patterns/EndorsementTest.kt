package vink.patterns

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** The same copy oracle as the Leptos `endorsement_copy_names_its_target_and_keeps_counts_apart`. */
class EndorsementTest {
    @Test
    fun copyNamesItsTargetAndTheSelectedNameContainsTheLabel() {
        assertEquals("Đánh giá cao", EndorsementKind.Work.copy.label)
        assertEquals("Bỏ đánh giá cao", EndorsementKind.Work.copy.selectedName)
        assertEquals("Hữu ích", EndorsementKind.Comment.copy.label)
        assertEquals("Bỏ đánh dấu hữu ích", EndorsementKind.Comment.copy.selectedName)
        for (kind in EndorsementKind.entries) {
            assertTrue(kind.copy.selectedName.lowercase().contains(kind.copy.label.lowercase()), kind.id)
            for (block in EndorsementBlock.entries) assertTrue(kind.reason(block).isNotBlank(), "$kind $block")
        }
        assertEquals("Cần đăng nhập để đánh giá.", EndorsementKind.Work.reason(EndorsementBlock.SignedOut))
    }
}
