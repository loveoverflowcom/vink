package vink.patterns

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** The same copy oracle as the Leptos `endorsement_copy_names_its_target_and_keeps_counts_apart`. */
class EndorsementTest {
    @Test
    fun copyNamesItsTargetAndTheSelectedNameContainsTheLabel() {
        assertEquals("Thấm", EndorsementKind.Work.copy.label)
        assertEquals("Bỏ Thấm", EndorsementKind.Work.copy.selectedName)
        assertEquals("lượt Thấm", EndorsementKind.Work.copy.countUnit)
        assertEquals("Đang gửi Thấm…", EndorsementKind.Work.copy.pendingOn)
        assertEquals("Đang bỏ Thấm…", EndorsementKind.Work.copy.pendingOff)
        assertEquals("Không thể Thấm lúc này.", EndorsementKind.Work.copy.failed)
        assertEquals("Chưa rõ bạn đã Thấm chưa.", EndorsementKind.Work.copy.unknown)
        assertEquals("Bạn không thể tự Thấm tác phẩm của mình.", EndorsementKind.Work.reason(EndorsementBlock.OwnContent))
        assertEquals("Hữu ích", EndorsementKind.Comment.copy.label)
        assertEquals("Bỏ đánh dấu hữu ích", EndorsementKind.Comment.copy.selectedName)
        for (kind in EndorsementKind.entries) {
            assertTrue(kind.copy.selectedName.lowercase().contains(kind.copy.label.lowercase()), kind.id)
            for (block in EndorsementBlock.entries) assertTrue(kind.reason(block).isNotBlank(), "$kind $block")
        }
        assertEquals("Cần đăng nhập để Thấm.", EndorsementKind.Work.reason(EndorsementBlock.SignedOut))
    }
}
