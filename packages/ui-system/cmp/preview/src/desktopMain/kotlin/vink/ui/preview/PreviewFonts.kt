package vink.ui.preview

import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.platform.Font
import org.jetbrains.skia.Data
import org.jetbrains.skia.FontMgr
import java.io.IOException
import vink.ui.VinkFontFaces
import vink.ui.VinkFonts

/**
 * Desktop-only TTF adapter for the approved web faces. These lossless derivatives keep the
 * original subset and variation tables; provenance and OFL travel with them in `vink-fonts/`.
 * Android/iOS keep their separate system-font policy. No font is fetched at runtime.
 */

object PreviewFonts {
    internal fun desktopFile(webFile: String): String = webFile.removeSuffix(".woff2") + ".ttf"

    internal fun resource(file: String): ByteArray? =
        PreviewFonts::class.java.getResourceAsStream("/vink-fonts/$file")?.use { it.readBytes() }

    fun load(): LoadedFonts = load(::resource)

    internal fun load(read: (String) -> ByteArray?): LoadedFonts {
        val failures = mutableListOf<String>()
        val decoded = VinkFontFaces.associateWith { face ->
            val file = desktopFile(face.file)
            val bytes = try {
                read(file)
            } catch (_: IOException) {
                failures += "$file: lỗi đọc resource"
                return@associateWith null
            }
            if (bytes == null) {
                failures += "$file: thiếu resource"
                null
            } else {
                val decodes = try {
                    Data.makeFromBytes(bytes).use { data ->
                        FontMgr.default.makeFromData(data)?.use { true } ?: false
                    }
                } catch (_: IllegalArgumentException) {
                    false
                }
                if (!decodes) failures += "$file: Skia không giải mã được"
                bytes.takeIf { decodes }
            }
        }
        if (failures.isNotEmpty()) {
            return LoadedFonts(VinkFonts(), "Font hệ thống (${failures.joinToString()})", emptyList())
        }
        fun face(file: String) = decoded.entries.first { it.key.file == file }.value!!
        val ui = FontFamily(
            Font("be-vietnam-pro-400", face("be-vietnam-pro-400.woff2"), FontWeight.Normal),
            Font("be-vietnam-pro-600", face("be-vietnam-pro-600.woff2"), FontWeight.SemiBold),
        )
        val literata = face("literata-variable.woff2")
        val reading = FontFamily(
            Font("literata-400", literata, FontWeight.Normal, variationSettings = FontVariation.Settings(FontVariation.weight(400))),
            Font("literata-600", literata, FontWeight.SemiBold, variationSettings = FontVariation.Settings(FontVariation.weight(600))),
        )
        return LoadedFonts(
            VinkFonts(ui = ui, reading = reading),
            "Be Vietnam Pro + Literata từ TTF tự host (Skia, desktop)",
            VinkFontFaces.map { desktopFile(it.file) },
        )
    }
}
