package uk.gcartee.encore.app.ui.player

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import kotlin.math.abs
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.core.QueueEntry
import uk.gcartee.encore.tokens.EncoreTokens.Gestures.PlayerSwipe as G

/** What one slot of the swipe draws: the current song, or the neighbour sliding in. */
enum class Slot { Current, Neighbour }

/**
 * Board 42 for the mini player and the Now Playing artwork, with the same numbers on
 * both: the direction locks in the first 12 dp (sideways over 1.2 times vertical); past
 * 60 dp, or on a flick over 500 dp/s, the skip arms with one GESTURE_THRESHOLD_ACTIVATE
 * tick and release performs it. The neighbour is only a drawing while the finger moves;
 * nothing loads. Left goes forward and right goes back, mirrored in right-to-left. At an
 * end of the queue the content follows at a third of the speed, stops at 40 dp, springs
 * back and says "End of queue" for 1.5 s. A drag from the 24 dp screen edge is Back's.
 * [onSwipeUp] (the mini player) expands Now Playing; down does nothing.
 */
@Composable
fun SkipSwipe(
    host: CoreHost,
    current: QueueEntry?,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    modifier: Modifier = Modifier,
    onSwipeUp: (() -> Unit)? = null,
    onEndOfQueue: (Boolean) -> Unit = {},
    content: @Composable BoxScope.(entry: QueueEntry?, slot: Slot) -> Unit,
) {
    val haptics = rememberHaptics()
    val scope = rememberCoroutineScope()
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val still = reduceMotion()
    val offset = remember { Animatable(0f) }
    var width by remember { mutableIntStateOf(0) }
    var neighbour by remember { mutableStateOf<QueueEntry?>(null) }
    var endShown by remember { mutableStateOf(false) }
    LaunchedEffect(endShown) {
        onEndOfQueue(endShown)
        if (endShown) {
            delay(G.endOfQueueLabelMs.toLong())
            endShown = false
        }
    }
    val hero = spring<Float>(dampingRatio = Spring.DampingRatioLowBouncy, stiffness = Spring.StiffnessMediumLow)
    // A committed skip always plays the whole slide, however light the flick, on one curve.
    val commit = androidx.compose.animation.core.tween<Float>(COMMIT_MS, easing = androidx.compose.animation.core.FastOutSlowInEasing)
    // After the slide the neighbour stays in place until the new song is current, so the
    // song just left never flashes back for a frame.
    var committing by remember { mutableStateOf(false) }
    LaunchedEffect(current?.uid) {
        if (committing) {
            offset.snapTo(0f)
            neighbour = null
            committing = false
        }
    }
    LaunchedEffect(committing) {
        if (committing) {
            delay(COMMIT_TIMEOUT_MS)
            if (committing) {
                offset.animateTo(0f, hero)
                neighbour = null
                committing = false
            }
        }
    }

    Box(
        modifier
            .onSizeChanged { width = it.width }
            .pointerInput(current?.uid, rtl, width) {
                val lock = G.directionLockPt.dp.toPx()
                val arm = G.armPt.dp.toPx()
                val flick = G.flickPtPerSecond.dp.toPx()
                val band = G.rubberBandMaxPt.dp.toPx()
                val edge = G.edgeZonePt.dp.toPx()
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    // Rule 11: the screen edge belongs to the system back gesture.
                    val screenX = down.position.x + 0f
                    if (screenX < edge || screenX > size.width - edge) return@awaitEachGesture
                    val tracker = VelocityTracker()
                    tracker.addPosition(down.uptimeMillis, down.position)
                    var travel = Offset.Zero
                    var horizontal: Boolean? = null
                    var raw = 0f
                    var armed = false
                    var forward = false
                    var atEnd = false
                    // Catch it mid-flight (rule 10): the finger takes over from where it is.
                    scope.launch { offset.stop() }
                    raw = offset.value
                    while (true) {
                        val event = awaitPointerEvent()
                        val change = event.changes.firstOrNull { it.id == down.id } ?: break
                        if (!change.pressed) break
                        tracker.addPosition(change.uptimeMillis, change.position)
                        val d = change.positionChange()
                        if (horizontal == null) {
                            travel += d
                            if (travel.getDistance() < lock) continue
                            horizontal = abs(travel.x) > abs(travel.y) * G.directionLockRatio
                            if (horizontal == false) {
                                if (travel.y < 0 && onSwipeUp != null) {
                                    change.consume()
                                    onSwipeUp()
                                }
                                return@awaitEachGesture
                            }
                            // Which neighbour, decided once per drag (rule 2).
                            forward = (travel.x < 0) != rtl
                            val target = runCatching { if (forward) host.core.nextTarget() else host.core.previousTarget() }.getOrNull()
                            neighbour = target?.song
                            atEnd = target == null || target.endOfQueue || target.song == null
                        }
                        change.consume()
                        raw += d.x
                        val shown = if (atEnd) (raw * G.rubberBandSpeedFraction.toFloat()).coerceIn(-band, band) else raw
                        scope.launch { offset.snapTo(shown) }
                        val nowArmed = !atEnd && abs(raw) >= arm
                        if (nowArmed && !armed) haptics.perform(Haptic.Armed)
                        armed = nowArmed
                    }
                    val velocity = tracker.calculateVelocity().x
                    val flicked = !atEnd && abs(velocity) > flick && (velocity < 0) == (raw < 0)
                    if (horizontal == true && (armed || flicked)) {
                        if (flicked && !armed) haptics.perform(Haptic.Armed)
                        scope.launch {
                            if (still) {
                                if (forward) onNext() else onPrevious()
                                offset.snapTo(0f)
                                neighbour = null
                            } else {
                                offset.animateTo(if (raw < 0) -width.toFloat() else width.toFloat(), commit)
                                committing = true
                                if (forward) onNext() else onPrevious()
                            }
                        }
                    } else {
                        if (horizontal == true && atEnd) endShown = true
                        scope.launch {
                            offset.animateTo(0f, hero)
                            neighbour = null
                        }
                    }
                }
            },
    ) {
        val x = if (still) 0f else offset.value
        Box(Modifier.fillMaxWidth().graphicsLayer {
            translationX = x
            // The song you are leaving fades out as it goes and shrinks a little.
            val f = (abs(x) / (width.coerceAtLeast(1))).coerceIn(0f, 1f)
            alpha = 1f - f
            scaleX = 1f - 0.08f * f
            scaleY = 1f - 0.08f * f
        }) { content(current, Slot.Current) }
        if (neighbour != null && x != 0f) {
            // The song you head to arrives from the side you drag toward, fading in.
            val from = if (x < 0) width.toFloat() else -width.toFloat()
            Box(Modifier.fillMaxWidth().graphicsLayer {
                translationX = from + x
                alpha = (abs(x) / (width.coerceAtLeast(1))).coerceIn(0f, 1f)
            }) { content(neighbour, Slot.Neighbour) }
        }
    }
}

private const val COMMIT_MS = 320
private const val COMMIT_TIMEOUT_MS = 1_500L
