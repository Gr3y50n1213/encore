package uk.gcartee.encore.app.ui.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import uk.gcartee.encore.app.ui.onboarding.reduceMotion

/*
 * Small answers to a toggle, on springs (board 02: springs only; board 35: "Heart pops
 * 1 -> 1.25 -> 1"). Nothing moves on first draw, only when the state changes, and Reduce
 * Motion keeps every glyph still (the shape morph and colour still say what changed).
 */

/** A glyph's scale: up to [peak] quickly, then a soft spring back to 1. */
@Composable
fun rememberPop(key: Any?, peak: Float = 1.25f): Float {
    val still = reduceMotion()
    val scale = remember { Animatable(1f) }
    var first by remember { mutableStateOf(true) }
    LaunchedEffect(key) {
        if (first) {
            first = false
            return@LaunchedEffect
        }
        if (still) return@LaunchedEffect
        scale.animateTo(peak, tween(POP_UP_MS, easing = FastOutSlowInEasing))
        scale.animateTo(1f, spring(dampingRatio = POP_DAMPING, stiffness = Spring.StiffnessMediumLow))
    }
    return scale.value
}

/** A glyph's turn: one full revolution on a spring, for Repeat, Autoplay and the Shuffle flip. */
@Composable
fun rememberTurn(key: Any?, degrees: Float = 360f): Float {
    val still = reduceMotion()
    val angle = remember { Animatable(0f) }
    var first by remember { mutableStateOf(true) }
    LaunchedEffect(key) {
        if (first) {
            first = false
            return@LaunchedEffect
        }
        if (still) return@LaunchedEffect
        angle.snapTo(0f)
        angle.animateTo(degrees, spring(dampingRatio = TURN_DAMPING, stiffness = Spring.StiffnessLow))
    }
    return angle.value % 360f
}

private const val POP_UP_MS = 110
private const val POP_DAMPING = 0.45f
private const val TURN_DAMPING = 0.8f
