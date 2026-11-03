package vink.ui.preview

import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.platform.Font
import org.jetbrains.skia.Data
import org.jetbrains.skia.FontMgr
import vink.ui.VinkFontFaces
import vink.ui.VinkFonts

/**
 * Desktop-only: the web app's self-hosted WOFF2 faces, copied onto the classpath by Gradle
 * (`vink-fonts/`). Skia decodes each file or the preview falls back to native families and says
 * so. Nothing here is part of the common library, and Android/iOS font loading is not covered.
 */

object PreviewFonts {
    fun load(): LoadedFonts {
        val decoded = VinkFontFaces.associateWith { face ->
            val bytes = PreviewFonts::class.java.getResourceAsStream("/vink-fonts/${face.file}")?.use { it.readBytes() }
            bytes?.takeIf { FontMgr.default.makeFromData(Data.makeFromBytes(it)) != null }
        }
        val missing = decoded.filterValues { it == null }.keys.map { it.file }
        if (missing.isNotEmpty()) {
            return LoadedFonts(VinkFonts(), "Font hệ thống (không đọc được ${missing.joinToString()})", emptyList())
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
            "Be Vietnam Pro + Literata từ WOFF2 tự host (Skia, desktop)",
            VinkFontFaces.map { it.file },
        )
    }
}
