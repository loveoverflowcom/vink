package vink.ui.preview

import androidx.compose.ui.window.ComposeUIViewController
import androidx.compose.runtime.SideEffect
import platform.UIKit.UIViewController
import platform.UIKit.UIUserInterfaceStyle
import vink.ui.ThemePreference

/** The Swift diagnostic host supplies lifecycle/motion; no platform API enters :ui commonMain. */
class NativeSession(args: List<String>) {
    private val model = PreviewModel(replay(args))
    fun setActivity(active: Boolean, reducedMotion: Boolean) {
        model.update("host activity") { it.copy(hostActive = active, systemReducedMotion = reducedMotion) }
    }
    fun controller(): UIViewController {
        lateinit var host: UIViewController
        host = ComposeUIViewController {
            val theme = model.config.theme
            SideEffect {
                host.overrideUserInterfaceStyle = when (theme) {
                    ThemePreference.Dark -> UIUserInterfaceStyle.UIUserInterfaceStyleDark
                    ThemePreference.Light -> UIUserInterfaceStyle.UIUserInterfaceStyleLight
                    else -> UIUserInterfaceStyle.UIUserInterfaceStyleUnspecified
                }
                host.setNeedsStatusBarAppearanceUpdate()
            }
            DiagnosticHost(model)
        }
        return host
    }
}
