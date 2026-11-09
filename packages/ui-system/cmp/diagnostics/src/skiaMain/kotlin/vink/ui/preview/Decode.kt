package vink.ui.preview

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asComposeImageBitmap
import androidx.compose.ui.unit.IntSize
import org.jetbrains.skia.Bitmap
import org.jetbrains.skia.Codec
import org.jetbrains.skia.Data

internal actual fun decodeWebp(bytes: ByteArray, source: IntSize, output: IntSize): ImageBitmap? {
    val data = Data.makeFromBytes(bytes)
    try {
        val codec = Codec.makeFromData(data)
        try {
            require(codec.width == source.width && codec.height == source.height)
            val bitmap = Bitmap()
            try {
                check(bitmap.allocPixels(codec.imageInfo.withWidthHeight(output.width, output.height)))
                // Eager pixel decode checks the codec status before publishing Ready.
                codec.readPixels(bitmap)
                bitmap.setImmutable()
                return bitmap.asComposeImageBitmap()
            } catch (failure: Exception) {
                bitmap.close()
                throw failure
            }
        } finally {
            codec.close()
        }
    } finally {
        data.close()
    }
}
