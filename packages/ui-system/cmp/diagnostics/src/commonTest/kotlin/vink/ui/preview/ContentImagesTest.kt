package vink.ui.preview

import androidx.compose.ui.graphics.ImageBitmapConfig
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.unit.IntSize
import kotlin.io.encoding.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import vink.ui.VinkIdentityVectors

class ContentImagesTest {
    @Test
    fun rejectsUnsupportedContainersAndOversizeBeforeDecode() {
        val sample = Base64.decode(VinkIdentityVectors.SAMPLE_WEBP_BASE64)
        assertEquals(IntSize(256, 171), lossyWebpSize(sample))
        assertNull(lossyWebpSize(sample.copyOf(sample.size - 1)))
        assertNull(lossyWebpSize(sample.copyOf().apply { this[15] = 'L'.code.toByte() }))
        assertNull(lossyWebpSize(sample.copyOf().apply { this[15] = 'X'.code.toByte() }))
        assertNull(lossyWebpSize(sample.copyOf().apply { this[27] = 9 }))
        assertNull(lossyWebpSize(sample.copyOf().apply {
            this[26] = 0 // 2048 × 2048: edges fit, area does not.
            this[27] = 8
            this[29] = 8
            this[28] = 0
        }))
        assertNull(lossyWebpSize(sample.copyOf(512 * 1024 + 1)))
    }

    @Test
    fun sizingCoversCropWithoutUpscalingOrExceedingTheOutputCap() {
        assertEquals(IntSize(192, 128), cropDecodeSize(IntSize(768, 512), IntSize(192, 108)))
        assertEquals(IntSize(150, 100), cropDecodeSize(IntSize(768, 512), IntSize(100, 100)))
        assertEquals(IntSize(256, 171), cropDecodeSize(IntSize(256, 171), IntSize(500, 400)))
        assertEquals(IntSize(1024, 512), cropDecodeSize(IntSize(2048, 1024), IntSize(4096, 4096)))
    }

    @Test
    fun actualDecoderProducesOpaquePixelsAndRejectsDamagedPayload() = runBlocking {
        val sample = VinkIdentityVectors.SAMPLE_WEBP_BASE64
        val landscape = assertNotNull(decodeContentImage(sample, IntSize(256, 171)))
        assertEquals(IntSize(256, 171), IntSize(landscape.width, landscape.height))
        assertEquals(ImageBitmapConfig.Argb8888, landscape.config)
        val sky = landscape.toPixelMap(12, 12, 1, 1)[0, 0]
        assertTrue(sky.alpha == 1f && sky.blue > sky.red && sky.red > sky.green)

        assertNull(decodeContentImage(VinkIdentityVectors.BROKEN_WEBP_BASE64, IntSize(256, 171)))
        val damaged = Base64.decode(sample).copyOf(348).apply {
            // Keep the VP8 keyframe header and first partition, but truncate the pixel partition.
            this[4] = 0x54 // RIFF size = 340 (0x154)
            this[5] = 0x01
            this[16] = 0x48 // VP8 chunk size = 328 (0x148)
            this[17] = 0x01
        }
        assertNotNull(lossyWebpSize(damaged), "container admission must leave entropy decoding to the codec")
        assertNull(decodeContentImage(Base64.encode(damaged), IntSize(256, 171)))
        assertNull(decodeContentImage("not base64", IntSize(256, 171)))
        assertNull(decodeContentImage(sample, IntSize.Zero))
    }

    @Test
    fun cancelledConsumerCannotPublishACompletedDecode() = runBlocking {
        var published = false
        val request = launch(start = CoroutineStart.UNDISPATCHED) {
            decodeContentImage(DETAIL_WEBP_BASE64, IntSize(192, 108))
            published = true
        }
        request.cancelAndJoin()
        assertFalse(published)
    }
}
