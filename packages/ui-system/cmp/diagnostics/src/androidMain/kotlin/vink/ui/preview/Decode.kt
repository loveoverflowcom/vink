package vink.ui.preview

import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.painter.BitmapPainter
import vink.ui.ImageState
actual fun decodeImage(base64: String): ImageState = try {
    val bytes = Base64.decode(base64, Base64.DEFAULT)
    val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
    if (bitmap == null) ImageState.Failed else ImageState.Ready(BitmapPainter(bitmap.asImageBitmap()))
} catch (_: Exception) { ImageState.Failed }
