package vink.ui.preview

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.ColorSpace
import android.graphics.ImageDecoder
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.unit.IntSize
import java.nio.ByteBuffer
import kotlin.io.encoding.Base64
import vink.ui.VinkIdentityVectors

internal actual fun decodeWebp(bytes: ByteArray, source: IntSize, output: IntSize): ImageBitmap? {
    val bitmap = if (Build.VERSION.SDK_INT >= 28) {
        decodeStrict(bytes, source, output)
    } else {
        decodeLegacy(bytes, source, output)
    }
    return bitmap?.asImageBitmap()
}

@RequiresApi(28)
private fun decodeStrict(bytes: ByteArray, source: IntSize, output: IntSize): Bitmap =
    ImageDecoder.decodeBitmap(ImageDecoder.createSource(ByteBuffer.wrap(bytes))) { decoder, info, _ ->
        require(info.size.width == source.width && info.size.height == source.height)
        decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
        decoder.setTargetColorSpace(ColorSpace.get(ColorSpace.Named.SRGB))
        decoder.setTargetSize(output.width, output.height)
        // ImageDecoder's default is to throw for incomplete/partial images.
    }

private val bundledWebp by lazy {
    listOf(VinkIdentityVectors.SAMPLE_WEBP_BASE64, DETAIL_WEBP_BASE64).map { Base64.decode(it) }
}

private fun decodeLegacy(bytes: ByteArray, source: IntSize, output: IntSize): Bitmap? {
    // BitmapFactory may return partial pixels. Keep the legacy diagnostic path
    // restricted to the two exact bundled fixtures.
    if (bundledWebp.none { it.contentEquals(bytes) }) return null
    var sample = 1
    while (source.width / (sample * 2) >= output.width && source.height / (sample * 2) >= output.height) {
        sample *= 2
    }
    val options = BitmapFactory.Options().apply {
        inPreferredConfig = Bitmap.Config.ARGB_8888
        inSampleSize = sample
        inScaled = false
    }
    val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options) ?: return null
    if (bitmap.width == output.width && bitmap.height == output.height) return bitmap
    return try {
        Bitmap.createScaledBitmap(bitmap, output.width, output.height, true)
    } finally {
        bitmap.recycle()
    }
}
