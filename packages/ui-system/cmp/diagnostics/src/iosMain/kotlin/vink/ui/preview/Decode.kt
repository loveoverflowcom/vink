package vink.ui.preview

import kotlin.io.encoding.Base64
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.graphics.painter.BitmapPainter
import org.jetbrains.skia.Image
import vink.ui.ImageState
actual fun decodeImage(base64: String): ImageState = try {
    ImageState.Ready(BitmapPainter(Image.makeFromEncoded(Base64.decode(base64)).toComposeImageBitmap()))
} catch (_: Exception) { ImageState.Failed }
