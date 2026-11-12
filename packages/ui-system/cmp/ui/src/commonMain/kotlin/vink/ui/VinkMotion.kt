package vink.ui

import androidx.compose.animation.Animatable
import androidx.compose.animation.core.Animatable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.remember
import androidx.compose.ui.graphics.Color

/** Restart only the visual effect, including when motion is disabled mid-flight. */
@Composable
internal fun rememberEffectColor(target: Color): State<Color> {
    val value = remember { Animatable(target) }
    val motion = LocalVink.current.motion
    val active = LocalMotionActive.current
    LaunchedEffect(target, motion.reduced, active) {
        if (motion.reduced || !active) value.snapTo(target)
        else value.animateTo(target, motion.effects())
    }
    return value.asState()
}

/** Opening only: closing removes the controlled overlay immediately, without delayed callbacks. */
@Composable
internal fun rememberOverlayEntrance(spatial: Boolean): State<Float> {
    val motion = LocalVink.current.motion
    val active = LocalMotionActive.current
    val value = remember { Animatable(if (motion.reduced || !active) 1f else 0f) }
    LaunchedEffect(motion.reduced, active) {
        if (motion.reduced || !active) value.snapTo(1f)
        else value.animateTo(1f, if (spatial) motion.spatial() else motion.effects())
    }
    return value.asState()
}
