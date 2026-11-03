package vink.ui.preview

import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState

/**
 * `cargo xtask ui-preview cmp run`: the desktop window. In-memory state; no service is called.
 * Lifecycle adapter: a minimized window is inactive, so no loading loop runs while it is hidden.
 */
fun main(args: Array<String>) = application {
    val fonts = remember { PreviewFonts.load() }
    val model = remember { PreviewModel(if (args.isEmpty()) PreviewConfig() else replay(args.toList())) }
    val windowState = rememberWindowState(width = 1280.dp, height = 900.dp)
    LaunchedEffect(windowState) {
        snapshotFlow { windowState.isMinimized }.collect { minimized ->
            if (model.config.hostActive == minimized) {
                model.update(if (minimized) "cửa sổ thu nhỏ → không hoạt động" else "cửa sổ hiện lại → hoạt động") { it.copy(hostActive = !minimized) }
            }
        }
    }
    Window(onCloseRequest = ::exitApplication, title = "Vink UI · preview CMP", state = windowState) {
        Workbench(model, fonts)
    }
}
