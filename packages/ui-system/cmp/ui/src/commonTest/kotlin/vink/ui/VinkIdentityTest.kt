package vink.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class VinkIdentityTest {
    @Test
    fun initialsMatchTheSharedGoldenVectors() {
        for ((name, expected) in VinkIdentityVectors.initials) assertEquals(expected, initials(name), "$name")
    }

    @Test
    fun tonesMatchTheSharedGoldenVectorsAndIgnoreComposition() {
        for ((key, expected) in VinkIdentityVectors.tones) assertEquals(expected, tone(key), "$key")
        assertEquals(tone("Nguyễn"), tone("Nguyễn"))
    }

    @Test
    fun sameInitialsCanDifferByKeyAndOrderNeverMatters() {
        val keys = VinkIdentityVectors.people.mapNotNull { it.first }
        assertEquals(keys.map(::tone), keys.reversed().map(::tone).reversed())
        assertEquals(initials("Trần Linh"), initials("Trần Lâm"))
        assertNotEquals(tone("acct-001"), tone("acct-002"))
        assertEquals(ColorRole.AvatarNeutral to ColorRole.OnAvatarNeutral, avatarRoles(tone(null)))
    }

    @Test
    fun groupsNeverInventCounts() {
        val people = listOf("An Nhiên", "Trần Linh", "Đỗ Ưng", "Nguyễn").map { Person(it, it) }
        assertEquals(4, overflow(3, 7))
        assertNull(overflow(3, null))
        assertNull(overflow(3, 2))
        assertNull(overflow(3, 3))
        assertEquals("Người nhận xét: An Nhiên, Trần Linh, Đỗ Ưng và 4 người khác", groupSummary("Người nhận xét", people, 3, 7))
        assertEquals("Người nhận xét: An Nhiên, Trần Linh, Đỗ Ưng", groupSummary("Người nhận xét", people, 3, null))
        assertEquals("Người nhận xét: chưa rõ", groupSummary("Người nhận xét", emptyList(), 3, null))
    }

    @Test
    fun linksAndImagesUseTheLeptosAllowLists() {
        for (ok in listOf("/tac-pham/1", "tac-pham/1", "#mo-dau", "?q=thu", "https://vink.vn/a", "HTTP://x.y", "mailto:a@b.vn")) {
            assertEquals(ok, safeHref(ok), ok)
        }
        for (bad in listOf("javascript:alert(1)", "JaVaScRiPt:alert(1)", "java\tscript:alert(1)", "data:text/html,x", "//evil.example", "vbscript:x", "")) {
            assertNull(safeHref(bad), bad)
        }
        for (ok in listOf("/pkg/a.avif", "https://img.vink.vn/a.webp", "data:image/avif;base64,AAAA", "blob:https://vink.vn/1")) {
            assertEquals(ok, safeImageSrc(ok), ok)
        }
        for (bad in listOf("javascript:alert(1)", "//evil/a.png", "data:image/svg+xml;base64,PHN2Zz4=", "a.png\" onerror=\"x", "")) {
            assertNull(safeImageSrc(bad), bad)
        }
    }

    @Test
    fun everyRegistryIconHasPathData() {
        for (icon in IconName.entries) assertTrue(icon.path.startsWith("M"), icon.id)
        assertEquals("0 0 24 24", VinkIconSource.VIEW_BOX)
    }
}
