package vink.ui.diagnostic

import android.os.Build
import android.os.Debug
import android.os.SystemClock
import android.util.Log
import androidx.compose.ui.graphics.ImageBitmapConfig
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.unit.IntSize
import androidx.lifecycle.Lifecycle
import java.io.File
import kotlinx.coroutines.runBlocking
import vink.ui.VinkIdentityVectors
import vink.ui.preview.DETAIL_WEBP_BASE64
import vink.ui.preview.decodeContentImage
import androidx.core.view.WindowCompat
import org.junit.Rule
import org.junit.Test
import org.junit.Assert.*

class DiagnosticTest {
    @get:Rule val ui = createAndroidComposeRule<DiagnosticActivity>()

    @Test fun controlledVietnameseTextSurvivesThemeAndRecreation() {
        val input = ui.onAllNodes(hasSetTextAction()).onFirst()
        input.performTextReplacement("Đường về – một buổi chiều")
        ui.onNodeWithText("Đổi sáng/tối").performClick()
        input.assertTextContains("Đường về – một buổi chiều", substring = true)
        ui.runOnIdle {
            assertFalse(WindowCompat.getInsetsController(ui.activity.window, ui.activity.window.decorView).isAppearanceLightStatusBars)
        }
        ui.activityRule.scenario.recreate()
        ui.onAllNodes(hasSetTextAction()).onFirst().assertTextContains("Đường về – một buổi chiều", substring = true)
    }

    @Test fun hostStopsActivityInBackgroundAndResumes() {
        ui.activityRule.scenario.moveToState(Lifecycle.State.CREATED)
        assertFalse(ui.activity.model.config.hostActive)
        ui.activityRule.scenario.moveToState(Lifecycle.State.RESUMED)
        ui.runOnIdle { assertTrue(ui.activity.model.config.hostActive) }
    }

    @Test fun pendingAndDisabledDoNotDispatchAgain() {
        ui.runOnIdle {
            ui.activity.model.update("test") { it.copy(entry = "action.button", scenario = "pending") }
        }
        ui.onNodeWithTag("pending-save").performClick()
        ui.onNodeWithTag("pending-save").performClick()
        ui.onNodeWithText("Đã nhận 1 lần kích hoạt").assertExists()
    }

    @Test fun webpSpecimenRendersPixelsAndReplacesThemWithFallback() {
        val alt = "[Mẫu] Ảnh nền lavender"
        val failed = "[Mẫu] Bìa tập thơ — Không tải được ảnh"
        ui.runOnIdle {
            ui.activity.model.update("test") { it.copy(entry = "identity.image-frame", scenario = "default") }
        }
        ui.waitUntil(10_000) { ui.onAllNodesWithContentDescription(alt).fetchSemanticsNodes().isNotEmpty() }
        val ready = ui.onNodeWithTag("frame:ready").assertExists()
        if (Build.VERSION.SDK_INT >= 26) {
            val frame = ready.captureToImage()
            val sky = frame.toPixelMap(frame.width / 4, frame.height / 5, 1, 1)[0, 0]
            assertTrue("expected lavender pixels, got $sky", sky.blue > sky.red && sky.red > sky.green)
        }
        ui.onNodeWithText("Đổi sáng/tối").performClick()
        ui.onNodeWithContentDescription(alt).assertExists()

        ui.runOnIdle { ui.activity.model.update("test") { it.copy(scenario = "image-error") } }
        ui.waitUntil(10_000) { ui.onAllNodesWithContentDescription(failed).fetchSemanticsNodes().size == 2 }
        ui.onNodeWithContentDescription(alt).assertDoesNotExist()
    }

    @Test fun webpAdapterRejectsInvalidBytesAndReportsBoundedOutput() = runBlocking<Unit> {
        val viewport = IntSize(192, 108)
        val sample = android.util.Base64.decode(VinkIdentityVectors.SAMPLE_WEBP_BASE64, android.util.Base64.DEFAULT)
        val damaged = sample.copyOf(348).apply {
            // Keep the VP8 keyframe header and first partition, but truncate the pixel partition.
            this[4] = 0x54 // RIFF size = 340 (0x154)
            this[5] = 0x01
            this[16] = 0x48 // VP8 chunk size = 328 (0x148)
            this[17] = 0x01
        }
        val wrongCodec = sample.copyOf().apply { this[15] = 'L'.code.toByte() }
        val oversize = sample.copyOf().apply { this[27] = 9 }
        for (bytes in listOf(damaged, wrongCodec, oversize, sample.copyOf(sample.size - 1))) {
            val base64 = android.util.Base64.encodeToString(bytes, android.util.Base64.NO_WRAP)
            assertNull(decodeContentImage(base64, viewport))
        }
        assertNull(decodeContentImage(VinkIdentityVectors.BROKEN_WEBP_BASE64, viewport))
        val pssBefore = Debug.getPss()
        val peakBefore = processPeakRssKiB()
        val times = List(7) {
            val start = SystemClock.elapsedRealtimeNanos()
            val result = checkNotNull(decodeContentImage(DETAIL_WEBP_BASE64, viewport))
            val elapsedMicros = (SystemClock.elapsedRealtimeNanos() - start) / 1_000
            assertEquals(192, result.width)
            assertEquals(128, result.height)
            assertEquals(ImageBitmapConfig.Argb8888, result.config)
            assertEquals(98_304, result.asAndroidBitmap().allocationByteCount)
            elapsedMicros
        }
        val pssAfter = Debug.getPss()
        Log.i("VinkWebP", "count=${times.size} elapsed_us=$times output=192x128 ARGB8888 allocation=98304 " +
            "process_pss_kib_before=$pssBefore process_pss_kib_after=$pssAfter " +
            "process_peak_rss_kib_before=$peakBefore process_peak_rss_kib_after=${processPeakRssKiB()}")
    }

    private fun processPeakRssKiB(): Int? = File("/proc/self/status").useLines { lines ->
        lines.firstOrNull { it.startsWith("VmHWM:") }
            ?.removePrefix("VmHWM:")?.trimStart()?.substringBefore(' ')?.toIntOrNull()
    }
}
