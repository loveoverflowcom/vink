package vink.ui.preview

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import java.io.File
import java.security.MessageDigest
import kotlin.test.Test
import kotlin.test.assertEquals
import org.jetbrains.skia.EncodedImageFormat
import vink.ui.ThemePreference
import vink.ui.VinkFontFaces
import vink.ui.VinkPreferences
import vink.ui.VinkTheme

/** Same real specimens and original bytes on both sides; before replays the old WOFF2 input.
 * This is off-screen JVM evidence, separate from native product/runtime acceptance.
 */
class FontCaptureTest {
    @Test
    fun capturesTypographyBylineAndReaderWithBothFormats() {
        val after = PreviewFonts.load()
        assertEquals(3, after.loaded.size, after.status)
        // Exercise the old input through the same decode/construction seam. On Linux this is
        // the honest all-system fallback, on macOS it may be the actual original WOFF2 faces.
        val before = PreviewFonts.load { ttf ->
            javaClass.getResourceAsStream("/vink-web-fonts/${ttf.removeSuffix(".ttf")}.woff2")?.use { it.readBytes() }
        }
        val out = File(System.getProperty("vink.captures") ?: "build/captures")
        out.mkdirs()
        val contexts = mutableListOf<String>()
        val recipes = listOf("foundation.typography" to "combining-marks", "identity.author-byline" to "long-label", "screen.reader" to "long")
        for ((phase, fonts) in listOf("before-woff2" to before, "after-ttf" to after)) {
            for ((entry, scenario) in recipes) {
                for (theme in listOf(ThemePreference.Light, ThemePreference.Dark)) {
                    for (scale in listOf(1f, 2f)) {
                        val width = 390
                        val height = if (entry == "identity.author-byline") {
                            if (scale == 2f) 800 else 400
                        } else if (scale == 2f) 6400 else 3200
                        val name = "fonts--$phase--$entry--$scenario--${theme.value}--$width--text${(scale * 100).toInt()}"
                        val scene = ImageComposeScene(width, height, Density(1f, 1f)) {
                            CompositionLocalProvider(LocalDensity provides Density(1f, scale)) {
                                VinkTheme(VinkPreferences(theme = theme), systemDark = false, fonts = fonts.fonts) {
                                    Box(Modifier.fillMaxSize().padding(16.dp)) { Specimen(entry, scenario) }
                                }
                            }
                        }
                        try {
                            scene.render().encodeToData(EncodedImageFormat.PNG)!!.use { File(out, "$name.png").writeBytes(it.bytes) }
                        } finally { scene.close() }
                        val inputFiles = VinkFontFaces.map { if (phase == "before-woff2") it.file else PreviewFonts.desktopFile(it.file) }
                        contexts += """"$name":{"phase":"$phase","entry":"$entry","scenario":"$scenario","theme":"${theme.value}","widthDp":$width,"heightDp":$height,"density":1,"osFontScale":1,"previewTextScale":$scale,"osInsetsDp":0,"inputFiles":"${inputFiles.joinToString()}","loadedFaceCount":${fonts.loaded.size},"effectiveFonts":"${if (fonts.loaded.isEmpty()) "system fallback" else "Be Vietnam Pro + Literata"}"}"""
                    }
                }
            }
        }
        val sha = System.getenv("GITHUB_SHA") ?: ProcessBuilder("git", "rev-parse", "HEAD").start().inputStream.bufferedReader().use { it.readText().trim() }
        val fingerprint = MessageDigest.getInstance("SHA-256")
        // Bytecode and font inputs identify local dirty builds as well as a CI checkout SHA.
        for (type in listOf(PreviewFonts::class.java, FontCaptureTest::class.java)) {
            fingerprint.update(type.getResourceAsStream("/${type.name.replace('.', '/')}.class")!!.use { it.readBytes() })
        }
        for (face in VinkFontFaces) fingerprint.update(PreviewFonts.resource(PreviewFonts.desktopFile(face.file))!!)
        val hash = fingerprint.digest().joinToString("") { "%02x".format(it) }
        File(out, "font-contexts.json").writeText("""{"captureMode":"offscreen","renderer":"CMP 1.9.0 / Skiko 0.9.22.2 ImageComposeScene","sourceSha":"$sha","buildFingerprint":"$hash","os":"${System.getProperty("os.name")} ${System.getProperty("os.version")} ${System.getProperty("os.arch")}","jvm":"${System.getProperty("java.vendor")} ${System.getProperty("java.version")}","before":"original WOFF2 inputs replayed through the loader seam; not an old screenshot relabelled","captures":{${contexts.joinToString(",")}}}""")
        assertEquals(24, contexts.size)
    }
}
