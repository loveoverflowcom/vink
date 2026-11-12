package vink.ui

import androidx.activity.compose.BackHandler
import androidx.compose.runtime.Composable

@Composable
internal actual fun DialogBackHandler(onBack: () -> Unit) {
    // Installed inside the Dialog's composition/window. Explicit policies consume Back without
    // dismissing; Android dispatch handles the IME first. No host router listener is installed.
    BackHandler(onBack = onBack)
}
