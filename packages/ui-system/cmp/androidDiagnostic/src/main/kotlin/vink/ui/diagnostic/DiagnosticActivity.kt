package vink.ui.diagnostic

import android.os.Bundle
import android.os.Handler
import android.database.ContentObserver
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.*
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.core.view.WindowCompat
import vink.ui.preview.*

class DiagnosticActivity : ComponentActivity() {
    lateinit var model: PreviewModel
        private set
    private val motionObserver = object : ContentObserver(Handler(android.os.Looper.getMainLooper())) {
        override fun onChange(selfChange: Boolean) = refreshMotion()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val args = listOf("entry", "scenario", "theme", "motion", "scale").mapNotNull { key ->
            (savedInstanceState?.getString(key) ?: intent.getStringExtra(key))?.let { "--$key=$it" }
        }
        model = PreviewModel(replay(args).copy(hostActive = false))
        setContent {
            val dark = when (model.config.theme) {
                vink.ui.ThemePreference.Dark -> true
                vink.ui.ThemePreference.Light -> false
                else -> isSystemInDarkTheme()
            }
            SideEffect {
                WindowCompat.getInsetsController(window, window.decorView).apply {
                    isAppearanceLightStatusBars = !dark
                    isAppearanceLightNavigationBars = !dark
                }
            }
            DiagnosticHost(model)
        }
    }

    override fun onResume() {
        super.onResume()
        contentResolver.registerContentObserver(Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, motionObserver)
        refreshMotion()
        model.update("host resumed") { it.copy(hostActive = true) }
    }

    override fun onPause() {
        contentResolver.unregisterContentObserver(motionObserver)
        model.update("host paused") { it.copy(hostActive = false) }
        super.onPause()
    }

    private fun refreshMotion() {
        val reduced = Settings.Global.getFloat(contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
        model.update("OS motion preference") { it.copy(systemReducedMotion = reduced) }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        val c = model.config
        outState.putString("entry", c.entry)
        outState.putString("scenario", c.scenario)
        outState.putString("theme", c.theme.value)
        outState.putString("motion", if (c.motion == vink.ui.MotionPreference.Reduce) "reduce" else "system")
        outState.putString("scale", if (c.textScale == 2f) "200" else "100")
        super.onSaveInstanceState(outState)
    }
}
