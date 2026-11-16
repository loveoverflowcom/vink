package vink.ui.preview

import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.unit.IntSize
import kotlin.coroutines.CoroutineContext
import kotlin.io.encoding.Base64
import kotlin.test.*
import kotlinx.coroutines.*

class EmptyArtworkTest {
    @Test fun exactAlphaAssetsDecodeRealPixelsWithinBounds() = runBlocking {
        for (artwork in EmptyArtwork.entries) {
            val bytes = artwork.bytes()
            assertTrue(bytes.size < 30 * 1024)
            // Real alpha bytes must stay rejected by the opaque content-image port.
            assertNull(lossyWebpSize(bytes))
            assertNull(decodeContentImage(Base64.encode(bytes), IntSize(192, 161)))
            for (viewport in listOf(IntSize(192, 161), IntSize(2000, 2000))) {
                val image = assertNotNull(decodeEmptyArtwork(artwork, viewport), artwork.id)
                assertEquals(if (viewport.width == 192) IntSize(192, 160) else IntSize(512, 427), IntSize(image.width, image.height))
                val pixels = image.toPixelMap()
                var transparent = 0
                var translucent = 0
                var opaque = 0
                var maxAlpha = 0f
                for (y in 0 until image.height) for (x in 0 until image.width) {
                    val alpha = pixels[x, y].alpha
                    maxAlpha = maxOf(maxAlpha, alpha)
                    when {
                        alpha == 0f -> transparent++
                        alpha == 1f -> opaque++
                        else -> translucent++
                    }
                }
                assertTrue(transparent > 0 && translucent > 0 && maxAlpha > 0.8f, "${artwork.id}: $transparent/$translucent/$opaque")
                println("alpha-artwork=${artwork.id} encoded_bytes=${bytes.size} output=${image.width}x${image.height} transparent=$transparent translucent=$translucent opaque=$opaque max_alpha=$maxAlpha")
            }
        }
        assertNull(decodeEmptyArtwork(EmptyArtwork.NoResults, IntSize.Zero))
        assertNull(decodeEmptyArtwork(EmptyArtwork.NoResults, IntSize(-1, 200)))
    }

    @Test fun cancelledArtworkCannotPublishAfterDecode() = runBlocking {
        for (cancel in listOf(false, true)) {
            val worker = QueueDispatcher()
            val consumer = QueueDispatcher()
            var published = false
            val request = launch(consumer, start = CoroutineStart.UNDISPATCHED) {
                assertNotNull(decodeEmptyArtwork(EmptyArtwork.NoResults, IntSize(192, 161), worker))
                published = true
            }
            worker.runOne()
            assertFalse(published)
            if (cancel) request.cancel()
            consumer.runOne()
            assertEquals(!cancel, published)
            assertEquals(cancel, request.isCancelled)
        }
    }

    private class QueueDispatcher : CoroutineDispatcher() {
        private val tasks = ArrayDeque<Runnable>()
        override fun dispatch(context: CoroutineContext, block: Runnable) { tasks.addLast(block) }
        fun runOne() { assertEquals(1, tasks.size); tasks.removeFirst().run(); assertTrue(tasks.isEmpty()) }
    }
}
