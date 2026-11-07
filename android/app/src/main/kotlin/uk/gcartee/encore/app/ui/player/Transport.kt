package uk.gcartee.encore.app.ui.player

import android.os.SystemClock
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CornerSize
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledIconToggleButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.tokens.EncoreTokens.AndroidToggleShape
import uk.gcartee.encore.tokens.EncoreTokens.Motion.Timing

/** The sizes of section 22 / board 35, from the tokens. */
enum class TransportSize(val playW: Int, val playH: Int, val checkedRadius: Int, val pressedRadius: Int, val skipW: Int, val skipOuter: Int) {
    Large(
        AndroidToggleShape.Sizes.Large.width, AndroidToggleShape.Sizes.Large.height,
        AndroidToggleShape.Sizes.Large.checkedRadius, AndroidToggleShape.Sizes.Large.pressedRadius,
        AndroidToggleShape.TransportRow.PrevNext.width, AndroidToggleShape.TransportRow.PrevNext.outerRadius,
    ),
    Medium(
        AndroidToggleShape.Sizes.Medium.width, AndroidToggleShape.Sizes.Medium.height,
        AndroidToggleShape.Sizes.Medium.checkedRadius,
        AndroidToggleShape.Sizes.Medium.height * AndroidToggleShape.pressedRadiusPercentOfHeight / 100,
        AndroidToggleShape.TransportRow.PrevNext.Sizes.Medium.width, AndroidToggleShape.TransportRow.PrevNext.Sizes.Medium.outerRadius,
    ),
}

/** The 200 ms transport debounce (board 05, item 5; tokens motion.timing.transportDebounceMs). */
@Composable
fun rememberDebounced(action: () -> Unit): () -> Unit {
    val last = remember { longArrayOf(0L) }
    return {
        val now = SystemClock.uptimeMillis()
        if (now - last[0] >= Timing.transportDebounceMs) {
            last[0] = now
            action()
        }
    }
}

/**
 * Play and pause: a toggle whose checked state is "is playing" (section 22). Paused it
 * is round (50%), playing a rounded square, pressed tighter again; the library morphs
 * between the three shapes, so no radius is animated here. Loading shows a spinner
 * and stays tappable: a tap pauses, which cancels the load.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PlayPauseButton(playing: Boolean, loading: Boolean, onToggle: () -> Unit, size: TransportSize = TransportSize.Large, enabled: Boolean = true) {
    val haptics = rememberHaptics()
    val tap = rememberDebounced {
        haptics.perform(Haptic.Transport)
        onToggle()
    }
    val shapes = IconButtonDefaults.toggleableShapes(
        shape = RoundedCornerShape(percent = AndroidToggleShape.restRadiusPercent),
        pressedShape = RoundedCornerShape(size.pressedRadius.dp),
        checkedShape = RoundedCornerShape(size.checkedRadius.dp),
    )
    val container = MaterialTheme.colorScheme.primary
    val content = MaterialTheme.colorScheme.onPrimary
    FilledIconToggleButton(
        checked = playing,
        onCheckedChange = { tap() },
        enabled = enabled,
        shapes = shapes,
        // The same size and colour playing and paused; only the shape tells the state.
        colors = IconButtonDefaults.filledIconToggleButtonColors(
            containerColor = container,
            contentColor = content,
            checkedContainerColor = container,
            checkedContentColor = content,
        ),
        modifier = Modifier
            .size(width = size.playW.dp, height = size.playH.dp)
            .semantics {
                contentDescription = if (playing) "Pause" else "Play"
                stateDescription = when {
                    loading -> "Loading"
                    playing -> "Playing"
                    else -> "Paused"
                }
            },
    ) {
        if (loading) {
            CircularProgressIndicator(Modifier.size((size.playH / 3).dp), color = content, strokeWidth = 3.dp)
        } else {
            Icon(if (playing) Icons.pause() else Icons.play(), contentDescription = null, modifier = Modifier.size((size.playH / 2.4f).dp))
        }
    }
}

/**
 * Previous or Next: tonal, shaped by its arrow. The end the arrow points to is fully round
 * and the end beside Play is 10 dp (4 dp pressed). Start and end follow the layout
 * direction, so both flip with the arrows in right-to-left languages.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SkipButton(next: Boolean, onClick: () -> Unit, size: TransportSize = TransportSize.Large, enabled: Boolean = true) {
    val haptics = rememberHaptics()
    val tap = rememberDebounced {
        haptics.perform(Haptic.Transport)
        onClick()
    }
    fun shape(inner: Dp): RoundedCornerShape {
        val round = CornerSize(size.skipOuter.dp)
        val flat = CornerSize(inner)
        return if (next) RoundedCornerShape(flat, round, round, flat) else RoundedCornerShape(round, flat, flat, round)
    }
    FilledTonalIconButton(
        onClick = tap,
        enabled = enabled,
        shapes = IconButtonDefaults.shapes(
            shape = shape(AndroidToggleShape.TransportRow.PrevNext.innerRadius.dp),
            pressedShape = shape(AndroidToggleShape.TransportRow.PrevNext.pressedInnerRadius.dp),
        ),
        modifier = Modifier.size(width = size.skipW.dp, height = size.playH.dp),
    ) {
        Icon(
            if (next) Icons.next() else Icons.previous(),
            contentDescription = if (next) "Next" else "Previous",
            modifier = Modifier.size((size.playH / 2.6f).dp),
        )
    }
}

/** Three separate buttons 8 dp apart, not one joined pill. */
@Composable
fun TransportRow(playing: Boolean, loading: Boolean, onPrevious: () -> Unit, onToggle: () -> Unit, onNext: () -> Unit, size: TransportSize = TransportSize.Large) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(AndroidToggleShape.TransportRow.gap.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        SkipButton(next = false, onClick = onPrevious, size = size)
        PlayPauseButton(playing, loading, onToggle, size)
        SkipButton(next = true, onClick = onNext, size = size)
    }
}
