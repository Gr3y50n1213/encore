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
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.background
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.WavyProgressIndicatorDefaults
import androidx.compose.ui.unit.IntOffset
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
import androidx.compose.ui.platform.withInfiniteAnimationFrameMillis
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
 * It runs as an infinite animation, so Compose's InfiniteAnimationPolicy can pause it
 * (UI tests do: a loop on plain frames never lets the screen go idle while playing).
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
            withInfiniteAnimationFrameMillis { now -> out.longValue = (base + (now - start)).coerceAtMost(end) }
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
            // Material 3 Expressive's own wavy indicator, its wave and motion as designed,
            // a touch slower than its default; it reads the frame-time position itself, so
            // it glides rather than stepping. The tall pill handle sits at the head.
            LinearWavyProgressIndicator(
                progress = { (scrub ?: ((smooth?.invoke() ?: positionMs).toFloat() / duration)).coerceIn(0f, 1f) },
                color = colour,
                trackColor = colour.copy(alpha = 0.24f),
                amplitude = { if (playing && scrub == null && !still) 1f else 0f },
                waveSpeed = WavyProgressIndicatorDefaults.LinearDeterminateWavelength * WAVE_SPEED,
                modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
            )
            val widthPx = constraints.maxWidth
            Box(
                Modifier
                    .align(Alignment.CenterStart)
                    .offset {
                        val f = (scrub ?: ((smooth?.invoke() ?: positionMs).toFloat() / duration)).coerceIn(0f, 1f)
                        val handle = HANDLE_W_DP.dp.toPx()
                        IntOffset((f * widthPx - handle / 2 - GAP_DP.dp.toPx()).coerceIn(0f, (widthPx - handle - 2 * GAP_DP.dp.toPx()).coerceAtLeast(0f)).toInt(), 0)
                    }
                    .size(width = (HANDLE_W_DP + 2 * GAP_DP).dp, height = (HANDLE_H_DP + 2 * GAP_DP).dp)
                    .padding(GAP_DP.dp)
                    .background(colour, RoundedCornerShape(50)),
            )
        }
    }
}

// Board 09's handle, measured from the Android render.
private const val BAR_TOUCH_DP = 48
private const val BAND_DP = 56
private const val HANDLE_W_DP = 4
private const val HANDLE_H_DP = 24
private const val GAP_DP = 2
private const val WAVE_SPEED = 0.75f
private const val STALL_MS = 500L
private const val JITTER_MS = 400L
