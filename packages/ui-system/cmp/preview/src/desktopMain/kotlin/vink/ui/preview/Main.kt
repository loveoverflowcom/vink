package vink.ui.preview

import androidx.compose.runtime.remember
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState

/** `cargo xtask ui-preview cmp run`: the desktop window. In-memory state; no service is called. */
fun main() = application {
    val fonts = remember { PreviewFonts.load() }
    val model = remember { PreviewModel() }
    Window(
        onCloseRequest = ::exitApplication,
        title = "Vink UI · preview CMP",
        state = rememberWindowState(width = 1280.dp, height = 900.dp),
    ) {
        Workbench(model, fonts)
    }
}
