package uk.gcartee.encore.app.ui.player

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.formatDuration
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.tokens.EncoreTokens.Motion

/**
 * Board 09, item 5: Android's bar is wavy while playing and flat when paused, with a tall
 * pill handle and a gap around it, as in the Material 3 Expressive slider. Tap or drag
 * scrubs; sliding the finger down halves, then quarters the speed (`motion.scrubSlowdown`).
 * Haptics at the start and end only. While it is held, [onHeld] tells Now Playing to
 * ignore its own swipe-down (item 1, GUARD).
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
) {
    val haptics = rememberHaptics()
    val duration = durationMs.coerceAtLeast(1)
    var scrub by remember { mutableStateOf<Float?>(null) }
    val fraction = (scrub ?: (positionMs.toFloat() / duration)).coerceIn(0f, 1f)
    BoxWithConstraints(
        modifier
            .fillMaxWidth()
            .height(BAR_TOUCH_DP.dp)
            .semantics {
                contentDescription = "Song position"
                stateDescription = "${formatDuration((fraction * duration).toLong())} of ${formatDuration(duration)}"
                progressBarRangeInfo = ProgressBarRangeInfo(fraction, 0f..1f)
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
        val handleX = maxWidth * fraction
        LinearWavyProgressIndicator(
            progress = { fraction },
            color = colour,
            trackColor = colour.copy(alpha = 0.24f),
            // Flat when paused or while scrubbing, wavy while playing.
            amplitude = { if (playing && scrub == null) 1f else 0f },
            modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
        )
        // The tall pill handle; the gap around it is the field showing through.
        Box(
            Modifier
                .offset(x = (handleX - HANDLE_W_DP.dp / 2).coerceAtLeast(0.dp))
                .size(width = (HANDLE_W_DP + 2 * GAP_DP).dp, height = (HANDLE_H_DP + 2 * GAP_DP).dp)
                .padding(GAP_DP.dp)
                .background(colour, RoundedCornerShape(50)),
        )
    }
}

// Board 09's handle, measured from the Android render.
private const val BAR_TOUCH_DP = 48
private const val BAND_DP = 56
private const val HANDLE_W_DP = 4
private const val HANDLE_H_DP = 24
private const val GAP_DP = 2
