package vink.ui.preview

import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontSynthesis
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.sp
import java.io.File
import java.security.MessageDigest
import java.text.Normalizer
import kotlin.math.abs
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.jetbrains.skia.Data
import org.jetbrains.skia.Font
import org.jetbrains.skia.FontMgr
import org.jetbrains.skia.FontHinting
import org.jetbrains.skia.FontVariation
import org.jetbrains.skia.Typeface
import vink.ui.VinkFontFaces

class FontTest {
    private fun typeface(file: String): Typeface {
        val bytes = assertNotNull(PreviewFonts.resource(file), "missing resource: $file")
        return Data.makeFromBytes(bytes).use { data ->
            assertNotNull(FontMgr.default.makeFromData(data), "Skia cannot decode $file")
        }
    }

    @Test
    fun packagedFacesHaveTheApprovedMetadataAndVietnameseGlyphs() {
        val diagnostics = mutableListOf("${System.getProperty("os.name")} ${System.getProperty("os.arch")} · JDK ${System.getProperty("java.version")} · CMP 1.9.0 / Skiko 0.9.22.2")
        val vietnamese = (0x1ea0..0x1ef9).map { it.toChar() }.joinToString("") +
            "ÀÁÂÃÈÉÊÌÍÒÓÔÕÙÚÝàáâãèéêìíòóôõùúýĂăĐđĨĩŨũƠơƯư"
        val scalars = (vietnamese + Normalizer.normalize(vietnamese, Normalizer.Form.NFD)).codePoints().distinct().toArray()
        for (face in VinkFontFaces) {
            val file = PreviewFonts.desktopFile(face.file)
            val bytes = assertNotNull(PreviewFonts.resource(file))
            assertTrue(bytes.take(4) == listOf<Byte>(0, 1, 0, 0), "$file must be sfnt TrueType")
            typeface(file).use { native ->
                assertEquals(face.family, native.familyName, file)
                val expectedWeight = if (file == "be-vietnam-pro-600.ttf") 600 else 400
                assertEquals(expectedWeight, native.fontStyle.weight, file)
                assertTrue(native.getUTF32Glyphs(scalars).all { it.toInt() != 0 }, "$file has missing Vietnamese NFC/NFD glyphs")
                val axes = native.variationAxes.orEmpty().associate { it.tag to (it.minValue to it.maxValue) }
                if (face.family == "Literata") {
                    assertEquals(mapOf("wght" to (200f to 900f), "opsz" to (7f to 72f)), axes)
                } else {
                    assertTrue(axes.isEmpty(), file)
                }
                diagnostics += "$file bytes=${bytes.size} sha256=${sha256(bytes)} family=${native.familyName} weight=${native.fontStyle.weight} axes=$axes VietnameseGlyphs=${scalars.size}/${scalars.size}"
            }
            // This is a probe, not an assertion that WOFF2 fails on every OS. Check packaging
            // independently of decoder support and keep the runtime's actual result in evidence.
            val web = assertNotNull(javaClass.getResourceAsStream("/vink-web-fonts/${face.file}")?.use { it.readBytes() })
            assertEquals("wOF2", web.take(4).map { it.toInt().toChar() }.joinToString(""))
            val decode = Data.makeFromBytes(web).use { data ->
                FontMgr.default.makeFromData(data)?.use { "${it.familyName}/${it.fontStyle.weight}" } ?: "null"
            }
            diagnostics += "${face.file} bytes=${web.size} sha256=${sha256(web)} SkiaDecode=$decode"
        }
        val output = File(System.getProperty("vink.captures") ?: "build/captures")
        output.mkdirs()
        File(output, "font-diagnostics.txt").writeText(diagnostics.joinToString("\n", postfix = "\n"))
        println(diagnostics.joinToString("\n"))
    }

    @Test
    fun composeMeasuresWithTheLoadedFacesAtBothWeights() {
        val loaded = PreviewFonts.load()
        assertEquals(3, loaded.loaded.size, loaded.status)
        val measurer = TextMeasurer(createFontFamilyResolver(), Density(1f), LayoutDirection.Ltr)
        for ((family, file) in listOf(
            loaded.fonts.ui to "be-vietnam-pro-400.ttf",
            loaded.fonts.ui to "be-vietnam-pro-600.ttf",
            loaded.fonts.reading to "literata-variable.ttf",
        )) {
            val weights = if (file == "literata-variable.ttf") listOf(400, 600) else listOf(if ("600" in file) 600 else 400)
            for (weight in weights) {
                typeface(file).use { original ->
                    original.makeClone(FontVariation("wght", weight.toFloat())).use { native ->
                        Font(native, 32f).use { skia ->
                            // Paragraph layout uses fractional advances; Skia Font defaults
                            // to pixel-hinted advances, which would test a different metric.
                            skia.isSubpixel = true
                            skia.isLinearMetrics = true
                            skia.hinting = FontHinting.NONE
                            val style = TextStyle(fontFamily = family, fontWeight = FontWeight(weight), fontStyle = FontStyle.Normal, fontSynthesis = FontSynthesis.None, fontSize = 32.sp, letterSpacing = 0.sp)
                            val result = measurer.measure("HHHH", style)
                            val expected = skia.measureTextWidth("HHHH")
                            val actual = result.getHorizontalPosition(4, true)
                            assertTrue(abs(expected - actual) < 0.05f, "$file/$weight: Compose=$actual, actual face=$expected")
                            val fallback = measurer.measure("HHHH", style.copy(fontFamily = FontFamily.Default)).getHorizontalPosition(4, true)
                            assertTrue(abs(fallback - actual) > 0.1f, "$file/$weight must not measure as the system fallback")
                        }
                    }
                }
            }
        }
    }

    @Test
    fun anyMissingOrCorruptFaceUsesAnHonestCompleteFallback() {
        for (face in VinkFontFaces) {
            val broken = PreviewFonts.desktopFile(face.file)
            for (corrupt in listOf(false, true)) {
                val loaded = PreviewFonts.load { file ->
                    if (file != broken) PreviewFonts.resource(file)
                    else if (corrupt) byteArrayOf(0, 1, 0, 0, 99) else null
                }
                assertTrue(loaded.loaded.isEmpty(), "$broken / corrupt=$corrupt")
                assertEquals(FontFamily.SansSerif, loaded.fonts.ui)
                assertEquals(FontFamily.Serif, loaded.fonts.reading)
                assertTrue(loaded.status.startsWith("Font hệ thống"), loaded.status)
                assertTrue(broken in loaded.status, loaded.status)
                assertTrue((if (corrupt) "Skia không giải mã được" else "thiếu resource") in loaded.status, loaded.status)
            }
        }
    }

    private fun sha256(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
}
