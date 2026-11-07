package uk.gcartee.encore.app.ui.components

import android.view.View
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalView
import androidx.core.view.HapticFeedbackConstantsCompat
import androidx.core.view.ViewCompat

/**
 * One haptic per action (plan sections 16 and 22, Android column). The constants go
 * through HapticFeedbackConstantsCompat so newer ones fall back on older Android.
 */
enum class Haptic(val constant: Int) {
    /** Board 22's "soft": a light confirmation of a plain tap. */
    Soft(HapticFeedbackConstantsCompat.CONTEXT_CLICK),
    /** Play, pause, next, previous. */
    Transport(HapticFeedbackConstantsCompat.VIRTUAL_KEY),
    ToggleOn(HapticFeedbackConstantsCompat.TOGGLE_ON),
    ToggleOff(HapticFeedbackConstantsCompat.TOGGLE_OFF),
    LongPress(HapticFeedbackConstantsCompat.LONG_PRESS),
    ScrubStart(HapticFeedbackConstantsCompat.GESTURE_START),
    ScrubEnd(HapticFeedbackConstantsCompat.GESTURE_END),
    /** Slider detents, chips, segmented choices. */
    Tick(HapticFeedbackConstantsCompat.SEGMENT_FREQUENT_TICK),
    DragStart(HapticFeedbackConstantsCompat.DRAG_START),
    Armed(HapticFeedbackConstantsCompat.GESTURE_THRESHOLD_ACTIVATE),
    Disarmed(HapticFeedbackConstantsCompat.GESTURE_THRESHOLD_DEACTIVATE),
    /** Signed in, profile switched, handoff done, a destructive confirm. */
    Confirm(HapticFeedbackConstantsCompat.CONFIRM),
    /** Wrong password, a blocked action. */
    Reject(HapticFeedbackConstantsCompat.REJECT),
}

class Haptics(private val view: View) {
    fun perform(haptic: Haptic) {
        ViewCompat.performHapticFeedback(view, haptic.constant)
    }
}

@Composable
fun rememberHaptics(): Haptics {
    val view = LocalView.current
    return remember(view) { Haptics(view) }
}
