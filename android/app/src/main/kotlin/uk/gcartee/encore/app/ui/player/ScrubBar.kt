package uk.gcartee.encore.app.ui.player

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import kotlin.math.PI
import kotlin.math.sin
import kotlinx.coroutines.delay
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.formatDuration
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.tokens.EncoreTokens.Motion

/**
 * The playing position between the player's reports, on frame time, so the bar and the
 * lyrics glide instead of stepping once a second. Each report re-bases it; a report a
 * hair behind what is already shown is ignored rather than pulling the bar backwards.
 */
@Composable
fun rememberSmoothPosition(positionMs: Long, playing: Boolean, durationMs: Long): State<Long> {
    val out = remember { mutableLongStateOf(positionMs) }
    LaunchedEffect(positionMs, playing, durationMs) {
        val shown = out.longValue
        val base = if (playing && positionMs < shown && shown - positionMs < JITTER_MS) shown else positionMs
        out.longValue = base
        if (!playing) return@LaunchedEffect
        val start = withFrameMillis { it }
        val end = if (durationMs > 0) durationMs else Long.MAX_VALUE
        while (true) {
            withFrameMillis { now -> out.longValue = (base + (now - start)).coerceAtMost(end) }
        }
    }
    return out
}

/**
 * Board 09, item 5: Android's bar is wavy while playing and flat when paused, with a tall
 * pill handle and a gap around it, as in the Material 3 Expressive slider. The wave grows
 * out of the handle: its end is pinned to the centre line at the playing point and the
 * waves travel away from it, slowly, so the head never bobs. Tap or drag scrubs; sliding
 * the finger down halves, then quarters the speed (`motion.scrubSlowdown`). Haptics at
 * the start and end only. While it is held, [onHeld] tells Now Playing to ignore its own
 * swipe-down (item 1, GUARD). [smooth] is the frame-time position from
 * [rememberSmoothPosition]; [positionMs] is the reported one, for TalkBack.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ScrubBar(
    positionMs: Long,
    durationMs: Long,
    playing: Boolean,
    colour: Color,
    onSeek: (Long) -> Unit,
    onScrub: (Long?) -> Unit,
    onHeld: (Boolean) -> Unit = {},
    modifier: Modifier = Modifier,
    buffering: Boolean = false,
    smooth: (() -> Long)? = null,
) {
    val haptics = rememberHaptics()
    val still = reduceMotion()
    val duration = durationMs.coerceAtLeast(1)
    var scrub by remember { mutableStateOf<Float?>(null) }
    // Board 09, States: "Buffering: bar shows a moving stripe", after 500 ms of stalling so a
    // quick load never flashes (board 20). The play button keeps its glyph (owner's choice).
    var stalled by remember { mutableStateOf(false) }
    LaunchedEffect(buffering) {
        if (buffering) {
            delay(STALL_MS)
            stalled = true
        } else {
            stalled = false
        }
    }
    val reported = (scrub ?: (positionMs.toFloat() / duration)).coerceIn(0f, 1f)
    // The wave eases in when playing starts and flattens when it stops or is held.
    val amplitude by animateFloatAsState(if (playing && scrub == null && !still) 1f else 0f, tween(AMPLITUDE_MS), label = "amplitude")
    val phase by rememberInfiniteTransition(label = "wave").animateFloat(
        0f, (2 * PI).toFloat(),
        infiniteRepeatable(tween(WAVE_PERIOD_MS, easing = LinearEasing)),
        label = "phase",
    )
    BoxWithConstraints(
        modifier
            .fillMaxWidth()
            .height(BAR_TOUCH_DP.dp)
            .semantics {
                contentDescription = "Song position"
                stateDescription = "${formatDuration((reported * duration).toLong())} of ${formatDuration(duration)}"
                progressBarRangeInfo = ProgressBarRangeInfo(reported, 0f..1f)
                setProgress { v -> onSeek((v.coerceIn(0f, 1f) * duration).toLong()); true }
            }
            .pointerInput(duration) {
                val bands = Motion.scrubSlowdown
                val band = BAND_DP.dp.toPx()
                awaitEachGesture {
                    val down = awaitFirstDown()
                    down.consume()
                    onHeld(true)
                    haptics.perform(Haptic.ScrubStart)
                    var f = (down.position.x / size.width).coerceIn(0f, 1f)
                    scrub = f
                    onScrub((f * duration).toLong())
                    var lastX = down.position.x
                    while (true) {
                        val event = awaitPointerEvent()
                        val change = event.changes.firstOrNull { it.id == down.id } ?: break
                        if (!change.pressed) break
                        change.consume()
                        val below = (change.position.y - size.height).coerceAtLeast(0f)
                        val speed = bands[(below / band).toInt().coerceAtMost(bands.lastIndex)].toFloat()
                        f = (f + (change.position.x - lastX) / size.width * speed).coerceIn(0f, 1f)
                        lastX = change.position.x
                        scrub = f
                        onScrub((f * duration).toLong())
                    }
                    onSeek((f * duration).toLong())
                    scrub = null
                    onScrub(null)
                    onHeld(false)
                    haptics.perform(Haptic.ScrubEnd)
                }
            },
        contentAlignment = Alignment.CenterStart,
    ) {
        if (stalled && scrub == null) {
            LinearWavyProgressIndicator(
                color = colour,
                trackColor = colour.copy(alpha = 0.24f),
                modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
            )
        } else {
            Canvas(Modifier.fillMaxSize()) {
                // Read here, in drawing, so the glide redraws without recomposing anything.
                val fraction = (scrub ?: ((smooth?.invoke() ?: positionMs).toFloat() / duration)).coerceIn(0f, 1f)
                val stroke = STROKE_DP.dp.toPx()
                val gap = GAP_DP.dp.toPx()
                val handleW = HANDLE_W_DP.dp.toPx()
                val handleH = HANDLE_H_DP.dp.toPx()
                val wave = WAVELENGTH_DP.dp.toPx()
                val height = WAVE_HEIGHT_DP.dp.toPx() * amplitude
                val cy = size.height / 2
                val head = (fraction * size.width).coerceIn(handleW / 2, size.width - handleW / 2)
                // Unplayed track, then the stop dot at the end.
                val trackStart = head + handleW / 2 + gap
                if (trackStart < size.width) {
                    drawLine(colour.copy(alpha = 0.24f), Offset(trackStart, cy), Offset(size.width - stroke / 2, cy), stroke, StrokeCap.Round)
                }
                drawCircle(colour, stroke / 2, Offset(size.width - stroke / 2, cy))
                // Played part: a wave pinned at the head, travelling away from it.
                val playedEnd = head - handleW / 2 - gap
                if (playedEnd > stroke / 2) {
                    val path = Path()
                    var x = stroke / 2
                    var first = true
                    val step = STEP_PX
                    while (true) {
                        val d = playedEnd - x
                        val taper = smoothStep((d / (wave * TAPER_WAVES)).coerceIn(0f, 1f))
                        val y = cy + height * taper * sin((2 * PI * d / wave - phase).toFloat())
                        if (first) {
                            path.moveTo(x, y)
                            first = false
                        } else {
                            path.lineTo(x, y)
                        }
                        if (x >= playedEnd) break
                        x = (x + step).coerceAtMost(playedEnd)
                    }
                    drawPath(path, colour, style = Stroke(width = stroke, cap = StrokeCap.Round))
                }
                // The tall pill handle; the gap around it is the field showing through.
                drawRoundRect(
                    colour,
                    topLeft = Offset(head - handleW / 2, cy - handleH / 2),
                    size = Size(handleW, handleH),
                    cornerRadius = CornerRadius(handleW / 2),
                )
            }
        }
    }
}

private fun smoothStep(t: Float): Float = t * t * (3 - 2 * t)

// Board 09's handle, measured from the Android render.
private const val BAR_TOUCH_DP = 48
private const val BAND_DP = 56
private const val HANDLE_W_DP = 4
private const val HANDLE_H_DP = 24
private const val GAP_DP = 4
private const val STALL_MS = 500L
private const val STROKE_DP = 4
private const val WAVELENGTH_DP = 36
private const val WAVE_HEIGHT_DP = 3
private const val TAPER_WAVES = 0.6f
private const val WAVE_PERIOD_MS = 2_600
private const val AMPLITUDE_MS = 400
private const val STEP_PX = 3f
private const val JITTER_MS = 400L
