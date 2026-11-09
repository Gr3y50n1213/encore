package uk.gcartee.encore.app.ui.player

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.drag
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.PointerInputScope
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.unit.dp
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min
import uk.gcartee.encore.tokens.EncoreTokens.Gestures.PlayerSwipe

/**
 * The swipe down that closes Now Playing into the mini player (board 09). The cover
 * follows the finger; letting go either closes it or springs it back.
 *
 * The owner (9 November 2026): closing was "a little too sensitive… no matter what mode
 * you're in", worst in landscape, and even a tap could close it. Three causes, each fixed:
 * - **Any movement started it.** The system's touch slop (about 8 dp) in any direction
 *   was enough, even over a button. Now the finger has to move [COLLAPSE_START_DP] and
 *   mostly downward, by the ratio board 42 uses to tell sideways from vertical.
 *   Anything under the finger that takes the movement first keeps it: the artwork's
 *   sideways skip, the scrub bar, a list scrolling.
 * - **A flick needed no distance.** 900 dp/s was enough, so a quick tap that drifted a
 *   few dp read as one. A flick now needs [COLLAPSE_FLICK_DP_PER_S] and
 *   [COLLAPSE_FLICK_MIN_DP] of travel, and a flick back up keeps it open.
 * - **Letting go after 18 % of the way closed it.** That was about 22 dp in landscape,
 *   where the way from the cover to the mini player is short. Now it takes
 *   [COLLAPSE_CLOSE_FRACTION] of the way and at least [COLLAPSE_CLOSE_MIN_DP], and the
 *   way is never under [COLLAPSE_MIN_WAY_DP] (see [collapseCloses]).
 */
internal suspend fun PointerInputScope.detectCollapseDrag(
    onDrag: (dy: Float) -> Unit,
    onRelease: (velocityY: Float) -> Unit,
) {
    val start = COLLAPSE_START_DP.dp.toPx()
    awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false)
        val tracker = VelocityTracker()
        tracker.addPosition(down.uptimeMillis, down.position)
        var moved = Offset.Zero
        // Until it starts, it only watches and takes nothing.
        while (true) {
            val change = awaitPointerEvent().changes.firstOrNull { it.id == down.id } ?: return@awaitEachGesture
            // Lifted first: a tap. Taken by something under the finger: theirs.
            if (!change.pressed || change.isConsumed) return@awaitEachGesture
            tracker.addPosition(change.uptimeMillis, change.position)
            moved += change.positionChange()
            if (moved.getDistance() < start) continue
            if (!isCollapseDirection(moved.x, moved.y)) return@awaitEachGesture
            change.consume()
            break
        }
        val lifted = drag(down.id) { change ->
            tracker.addPosition(change.uptimeMillis, change.position)
            onDrag(change.positionChange().y)
            change.consume()
        }
        // Cancelled (another gesture took the finger): decided by distance alone.
        onRelease(if (lifted) tracker.calculateVelocity().y else 0f)
    }
}

/** Mostly downward: down by more than board 42's ratio times sideways. */
internal fun isCollapseDirection(dx: Float, dy: Float): Boolean =
    dy > 0f && dy >= abs(dx) * PlayerSwipe.directionLockRatio

/**
 * Whether letting go closes Now Playing. [draggedDp] is how far the cover has come down,
 * [wayDp] the whole way to the mini player, [velocityDpPerS] the finger's speed (down is
 * positive).
 */
internal fun collapseCloses(draggedDp: Float, wayDp: Float, velocityDpPerS: Float): Boolean {
    // Changed their mind: a flick back up keeps it open, however far it had come.
    if (velocityDpPerS <= -COLLAPSE_FLICK_DP_PER_S) return false
    if (velocityDpPerS >= COLLAPSE_FLICK_DP_PER_S && draggedDp >= COLLAPSE_FLICK_MIN_DP) return true
    // Never more than most of the way, so a short way can still be closed by dragging.
    val needed = min(max(wayDp * COLLAPSE_CLOSE_FRACTION, COLLAPSE_CLOSE_MIN_DP), wayDp * COLLAPSE_CLOSE_MAX_FRACTION)
    return draggedDp >= needed
}

/** Twice the usual touch slop: a tap that drifts never starts the swipe. */
internal const val COLLAPSE_START_DP = 16f
internal const val COLLAPSE_FLICK_DP_PER_S = 1200f
internal const val COLLAPSE_FLICK_MIN_DP = 40f
internal const val COLLAPSE_CLOSE_FRACTION = 0.3f
internal const val COLLAPSE_CLOSE_MIN_DP = 80f
private const val COLLAPSE_CLOSE_MAX_FRACTION = 0.6f
/** The shortest way from the cover to the mini player, so landscape isn't a hair trigger. */
internal const val COLLAPSE_MIN_WAY_DP = 200f
