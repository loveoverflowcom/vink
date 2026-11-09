package vink.ui.preview

import androidx.compose.ui.graphics.ImageBitmapConfig
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.unit.IntSize
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.time.TimeSource
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.alloc
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.ptr
import kotlinx.coroutines.runBlocking
import platform.posix.RUSAGE_SELF
import platform.posix.getrusage
import platform.posix.rusage

@OptIn(ExperimentalForeignApi::class)
class ContentImageWorkloadTest {
    @Test
    fun recordsSevenDetailDecodesAtThumbnailSize() = runBlocking {
        val before = processPeakBytes()
        val times = List(7) {
            val start = TimeSource.Monotonic.markNow()
            val image = assertNotNull(decodeContentImage(DETAIL_WEBP_BASE64, IntSize(192, 108)))
            val elapsed = start.elapsedNow()
            assertEquals(IntSize(192, 128), IntSize(image.width, image.height))
            assertEquals(ImageBitmapConfig.Argb8888, image.config)
            assertEquals(98_304, image.asSkiaBitmap().rowBytes * image.height)
            elapsed.inWholeMicroseconds
        }
        println("Detail WebP: count=${times.size} elapsed_us=$times output=192x128 ARGB8888 pixel_bytes=98304 " +
            "process_peak_rss_bytes_before=$before process_peak_rss_bytes_after=${processPeakBytes()}")
    }

    private fun processPeakBytes(): Long? = memScoped {
        val usage = alloc<rusage>()
        if (getrusage(RUSAGE_SELF, usage.ptr) == 0) usage.ru_maxrss else null
    }
}
