package vink.ui.preview

import kotlin.test.*
import vink.ui.*

class ReplayTest {
    @Test fun everyScenarioCanBeReplayedAndUnknownInputIsRejected() {
        VinkCatalog.entries.forEach { entry ->
            entry.scenarios.forEach { scenario ->
                val config = replay(listOf("--entry=${entry.id}", "--scenario=${scenario.id}", "--theme=dark", "--scale=200", "--motion=reduce"))
                assertEquals(entry.id, config.entry)
                assertEquals(scenario.id, config.scenario)
                assertEquals(ThemePreference.Dark, config.theme)
                assertEquals(2f, config.textScale)
            }
        }
        listOf("--entry=missing", "--scenario=missing", "--scale=300", "--theme=sepia", "--motion=fast", "--token=secret").forEach {
            assertFails { replay(listOf(it)) }
        }
    }
    @Test fun vietnameseSearchMatchesCanonicalAndCombiningForms() {
        assertEquals("duong truong", fold("Đường Trường"))
        assertEquals(fold("Ỗ Ữ Màu"), fold("Ỗ Ữ Màu"))
    }
}
