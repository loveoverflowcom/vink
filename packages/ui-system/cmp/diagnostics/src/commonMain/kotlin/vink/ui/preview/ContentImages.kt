package vink.ui.preview

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.painter.BitmapPainter
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.unit.IntSize
import kotlin.io.encoding.Base64
import kotlin.math.roundToInt
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import vink.ui.ImageState
import vink.ui.LocalMotionActive

private const val MaxEncodedBytes = 512 * 1024
private const val MaxSourceEdge = 2048
private const val MaxSourcePixels = 2 * 1024 * 1024
private const val MaxOutputEdge = 1024

// The diagnostic host shares one worker slot across specimens. Painter creation and
// state publication happen back in the owning composition.
private val imageWorker = Dispatchers.Default.limitedParallelism(1)

suspend fun decodeContentImage(base64: String, viewport: IntSize): ImageBitmap? {
    if (base64.length > (MaxEncodedBytes + 2) / 3 * 4 || viewport.width <= 0 || viewport.height <= 0) return null
    return withContext(imageWorker) {
        try {
            val bytes = Base64.decode(base64)
            val source = lossyWebpSize(bytes) ?: return@withContext null
            decodeWebp(bytes, source, cropDecodeSize(source, viewport))
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            null
        }
    }
}

internal expect fun decodeWebp(bytes: ByteArray, source: IntSize, output: IntSize): ImageBitmap?

// Match the simple opaque container in shared/content-images.md and the fixture generator.
internal fun lossyWebpSize(bytes: ByteArray): IntSize? {
    if (bytes.size !in 30..MaxEncodedBytes) return null

    fun unsigned(offset: Int) = bytes[offset].toInt() and 0xff
    fun littleEndian(offset: Int, count: Int): Int =
        (0 until count).fold(0) { value, i -> value or (unsigned(offset + i) shl (8 * i)) }
    fun matches(offset: Int, signature: String) =
        signature.indices.all { i -> unsigned(offset + i) == signature[i].code }

    val payload = littleEndian(16, 4)
    val frame = littleEndian(20, 3)
    if (!matches(0, "RIFF") || !matches(8, "WEBPVP8 ") ||
        littleEndian(4, 4) != bytes.size - 8 ||
        payload !in 10..bytes.size - 20 || payload + payload % 2 != bytes.size - 20 ||
        (payload % 2 != 0 && bytes.last() != 0.toByte()) ||
        frame and 1 != 0 || frame ushr 5 !in 1..payload - 10 ||
        !matches(23, "\u009d\u0001\u002a")
    ) return null

    val width = littleEndian(26, 2) and 0x3fff
    val height = littleEndian(28, 2) and 0x3fff
    if (width !in 1..MaxSourceEdge || height !in 1..MaxSourceEdge || width * height > MaxSourcePixels) return null
    return IntSize(width, height)
}

internal fun cropDecodeSize(source: IntSize, viewport: IntSize): IntSize {
    val cover = maxOf(viewport.width.toDouble() / source.width, viewport.height.toDouble() / source.height)
    val cap = MaxOutputEdge.toDouble() / maxOf(source.width, source.height)
    val scale = minOf(1.0, cover, cap)
    return IntSize(
        (source.width * scale).roundToInt().coerceAtLeast(1),
        (source.height * scale).roundToInt().coerceAtLeast(1),
    )
}

@Composable
internal fun FixtureImage(base64: String, content: @Composable (ImageState, Modifier) -> Unit) {
    var viewport by remember { mutableStateOf(IntSize.Zero) }
    var image by remember(base64, viewport) { mutableStateOf<ImageState>(ImageState.Loading) }
    val active = LocalMotionActive.current
    LaunchedEffect(base64, viewport, active) {
        if (active && viewport.width > 0 && viewport.height > 0 && image == ImageState.Loading) {
            val bitmap = decodeContentImage(base64, viewport)
            image = if (bitmap == null) ImageState.Failed else ImageState.Ready(BitmapPainter(bitmap))
        }
    }
    content(image, Modifier.onSizeChanged { viewport = it })
}
