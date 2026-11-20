package vink.patterns

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import vink.ui.IconName
import vink.ui.toggleGlyph

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
        assertEquals("Ưng", EndorsementKind.Comment.copy.label)
        assertEquals("Bỏ Ưng", EndorsementKind.Comment.copy.selectedName)
        assertEquals("lượt Ưng", EndorsementKind.Comment.copy.countUnit)
        assertEquals("Đang gửi Ưng…", EndorsementKind.Comment.copy.pendingOn)
        assertEquals("Chưa rõ bạn đã Ưng chưa.", EndorsementKind.Comment.copy.unknown)
        assertEquals("Bạn không thể tự Ưng nhận xét của mình.", EndorsementKind.Comment.reason(EndorsementBlock.OwnContent))
        assertEquals("Cần đăng nhập để Ưng.", EndorsementKind.Comment.reason(EndorsementBlock.SignedOut))
        assertEquals("Tạm chưa thể Ưng.", EndorsementKind.Comment.reason(EndorsementBlock.Unavailable))
        for (kind in EndorsementKind.entries) {
            assertTrue(kind.copy.selectedName.lowercase().contains(kind.copy.label.lowercase()), kind.id)
            for (block in EndorsementBlock.entries) assertTrue(kind.reason(block).isNotBlank(), "$kind $block")
        }
        assertEquals("Cần đăng nhập để Thấm.", EndorsementKind.Work.reason(EndorsementBlock.SignedOut))
    }

    /** Same oracle as the Leptos `a_selected_glyph_follows_only_the_confirmed_value`. */
    @Test
    fun ungUsesAnOutlineHeartUntilTheConfirmedValueIsOn() {
        assertEquals(IconName.PenFlight, EndorsementKind.Work.icon(selected = true))
        assertEquals(IconName.HeartOutline, EndorsementKind.Comment.icon(selected = false))
        assertEquals(IconName.Heart, EndorsementKind.Comment.icon(selected = true))
        val outline = EndorsementKind.Comment.icon(selected = false)
        val filled = EndorsementKind.Comment.icon(selected = true)
        assertEquals(filled, toggleGlyph(outline, filled, pressed = true))
        assertEquals(outline, toggleGlyph(outline, filled, pressed = false))
        assertEquals(outline, toggleGlyph(outline, filled, pressed = null))
        assertEquals(outline, toggleGlyph(outline, null, pressed = true))
    }
}
