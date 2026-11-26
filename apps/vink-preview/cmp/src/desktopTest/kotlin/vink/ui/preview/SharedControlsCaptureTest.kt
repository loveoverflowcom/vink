package vink.ui.preview

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import java.io.File
import kotlin.test.Test
import org.jetbrains.skia.EncodedImageFormat
import vink.ui.FieldMode
import vink.ui.FieldVariant
import vink.ui.ThemePreference
import vink.ui.VinkPreferences
import vink.ui.VinkTextArea
import vink.ui.VinkTextField
import vink.ui.VinkTheme

/** Isolated real controls: no tall workbench chrome hiding the 200% text specimen. */
class SharedControlsCaptureTest {
    @Test
    fun captureTonalStatesAndLongLabels() {
        val out = File(System.getProperty("vink.captures") ?: "build/captures")
        val fonts = PreviewFonts.load()
        for (theme in listOf(ThemePreference.Light, ThemePreference.Dark)) {
            for (scale in listOf(1f, 2f)) {
                val scene = ImageComposeScene(390, 2600, Density(1f, 1f)) {
                    CompositionLocalProvider(LocalDensity provides Density(1f, scale)) {
                        VinkTheme(VinkPreferences(theme = theme), systemDark = false, fonts = fonts.fonts) {
                            Box(Modifier.fillMaxSize().padding(16.dp)) {
                                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                                    VinkTextField("Tên bản thảo", TextFieldValue(""), {}, variant = FieldVariant.FloatingTonal)
                                    VinkTextField("Tên bản thảo", TextFieldValue("Mùa mưa ở lại"), {}, variant = FieldVariant.FloatingTonal)
                                    VinkTextField("Tiêu đề", TextFieldValue(""), {}, error = "Hãy đặt tên cho bản thảo.", variant = FieldVariant.FloatingTonal)
                                    VinkTextField("Tác giả", TextFieldValue("Nguyễn Hoài Thương"), {}, mode = FieldMode.ReadOnly, variant = FieldVariant.FloatingTonal)
                                    VinkTextField("Tài khoản", TextFieldValue("Chưa có thông tin"), {}, mode = FieldMode.Disabled, variant = FieldVariant.FloatingTonal)
                                    VinkTextField("Số chương dự kiến cho tập truyện ngắn đầu tay của bạn (có thể để 0)", TextFieldValue("0"), {}, variant = FieldVariant.FloatingTonal)
                                    VinkTextArea("Nội dung", TextFieldValue("Dòng đầu\n\n  Khổ thơ sau"), {})
                                }
                            }
                        }
                    }
                }
                try {
                    out.mkdirs()
                    scene.render().encodeToData(EncodedImageFormat.PNG)!!.use {
                        File(out, "shared-controls--${theme.value}--390--text${(scale * 100).toInt()}.png").writeBytes(it.bytes)
                    }
                } finally { scene.close() }
            }
        }
    }
}
