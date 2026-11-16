package vink.ui.preview

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.painter.BitmapPainter
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import kotlin.io.encoding.Base64
import kotlin.math.roundToInt
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import vink.ui.*

/** Diagnostic-only, closed set of #258 assets. No arbitrary bytes, URI, service or cover policy. */
enum class EmptyArtwork(val id: String) {
    NoResults("no-results"), NoPublicWorks("no-public-works"), FirstDraft("first-draft");

    internal fun bytes(): ByteArray = Base64.decode(emptyArtworkBase64(id))
}

internal val EmptyArtworkSourceSize = IntSize(512, 427)
private val artworkWorker = Dispatchers.Default.limitedParallelism(1)

internal fun emptyArtworkDecodeSize(viewport: IntSize): IntSize {
    val scale = minOf(1.0, viewport.width.toDouble() / 512, viewport.height.toDouble() / 427)
    return IntSize((512 * scale).roundToInt().coerceAtLeast(1), (427 * scale).roundToInt().coerceAtLeast(1))
}

suspend fun decodeEmptyArtwork(artwork: EmptyArtwork, viewport: IntSize): ImageBitmap? =
    decodeEmptyArtwork(artwork, viewport, artworkWorker)

internal suspend fun decodeEmptyArtwork(artwork: EmptyArtwork, viewport: IntSize, worker: CoroutineDispatcher): ImageBitmap? {
    if (viewport.width <= 0 || viewport.height <= 0) return null
    return withContext(worker) {
        try {
            decodeBundledArtwork(artwork, emptyArtworkDecodeSize(viewport))
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            null
        }
    }
}

internal expect fun decodeBundledArtwork(artwork: EmptyArtwork, output: IntSize): ImageBitmap?

@Composable
internal fun ArtworkEmptyState(artwork: EmptyArtwork, text: String, actionLabel: String, onAction: () -> Unit) {
    var viewport by remember { mutableStateOf(IntSize.Zero) }
    var image by remember(artwork, viewport) { mutableStateOf<ImageBitmap?>(null) }
    val active = LocalMotionActive.current
    LaunchedEffect(artwork, viewport, active) {
        if (active && image == null) image = decodeEmptyArtwork(artwork, viewport)
    }
    EmptyArtworkPanel(image, text, actionLabel, onAction, Modifier.onSizeChanged { viewport = it })
}

/** Text/action render independently of loading, decoder failure or host inactivity. */
@Composable
fun EmptyArtworkPanel(image: ImageBitmap?, text: String, actionLabel: String, onAction: () -> Unit, imageModifier: Modifier = Modifier) {
    Column(Modifier.fillMaxWidth().testTag("empty-artwork-panel"), verticalArrangement = Arrangement.spacedBy(VinkDimens.Space3)) {
        BoxWithConstraints(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            val width = if (maxWidth < 400.dp) 176.dp else 240.dp
            Box(imageModifier.widthIn(max = width).fillMaxWidth().aspectRatio(512f / 427f).testTag("empty-artwork-slot")) {
                if (image != null) Image(BitmapPainter(image), contentDescription = null,
                    modifier = Modifier.fillMaxSize().testTag("empty-artwork-ready"), contentScale = ContentScale.Fit)
            }
        }
        VinkText(text)
        VinkButton(actionLabel, onAction)
    }
}
