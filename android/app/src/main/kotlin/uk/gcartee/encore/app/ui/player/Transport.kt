package uk.gcartee.encore.app.ui.player

import android.os.SystemClock
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CornerSize
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledIconToggleButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.lerp
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
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
    /** The tablet and desktop player bar (board 29, item 5). */
    Bar(
        AndroidToggleShape.Sizes.Bar.width, AndroidToggleShape.Sizes.Bar.height,
        AndroidToggleShape.Sizes.Bar.checkedRadius,
        AndroidToggleShape.Sizes.Bar.height * AndroidToggleShape.pressedRadiusPercentOfHeight / 100,
        AndroidToggleShape.TransportRow.PrevNext.Sizes.Bar.width, AndroidToggleShape.TransportRow.PrevNext.Sizes.Bar.outerRadius,
    ),
    /** The mini player: 44 dp, a 48 dp touch target kept by the button's minimum size. */
    Compact(
        AndroidToggleShape.Sizes.Compact.width, AndroidToggleShape.Sizes.Compact.height,
        AndroidToggleShape.Sizes.Compact.checkedRadius,
        AndroidToggleShape.Sizes.Compact.height * AndroidToggleShape.pressedRadiusPercentOfHeight / 100,
        AndroidToggleShape.Sizes.Compact.width, AndroidToggleShape.Sizes.Compact.height / 2,
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
 * between the three shapes, so no radius is animated here. It always shows the glyph:
 * the owner asked for the play and pause icons people expect, never a spinner or a bare
 * shape (7 November 2026, in place of board 09's "spinner replaces play"). While a song
 * loads it shows Pause, and a tap cancels the load; buffering shows on the progress bar.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PlayPauseButton(
    playing: Boolean,
    loading: Boolean,
    onToggle: () -> Unit,
    size: TransportSize = TransportSize.Large,
    enabled: Boolean = true,
    container: Color? = null,
    onContainer: Color? = null,
) {
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
    // Over an album field the button is the field's text colour with the field's base as
    // its glyph (board 09: a white button on a dark field).
    val container = container ?: MaterialTheme.colorScheme.primary
    val content = onContainer ?: MaterialTheme.colorScheme.onPrimary
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
                // Says what the glyph shows: Pause while a song loads too (a tap cancels it).
                contentDescription = if (playing || loading) "Pause" else "Play"
                stateDescription = when {
                    loading -> "Loading"
                    playing -> "Playing"
                    else -> "Paused"
                }
            },
    ) {
        Icon(if (playing || loading) Icons.pause() else Icons.play(), contentDescription = null, modifier = Modifier.size((size.playH / 2.4f).dp))
    }
}

/**
 * Previous or Next: tonal, shaped by its arrow. The end the arrow points to is fully round
 * and the end beside Play is 10 dp. A tap squeezes it (the round end to the play button's
 * pressed corner, the other to 4 dp) and throws the arrow forward, springing back, so a
 * skip is seen as well as felt. Start and end follow the layout direction, so both flip
 * with the arrows in right-to-left languages.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SkipButton(
    next: Boolean,
    onClick: () -> Unit,
    size: TransportSize = TransportSize.Large,
    enabled: Boolean = true,
    onScan: ((active: Boolean) -> Unit)? = null,
    tint: Color? = null,
    colors: androidx.compose.material3.IconButtonColors? = null,
) {
    val haptics = rememberHaptics()
    val interaction = remember { MutableInteractionSource() }
    // Board 09, item 6: held, it scans at 4x with a tick each second; the tap that ends a
    // scan is not also a skip.
    var scanned by remember { mutableStateOf(false) }
    if (onScan != null) {
        LaunchedEffect(interaction) {
            var job: Job? = null
            interaction.interactions.collect { i ->
                when (i) {
                    is PressInteraction.Press -> job = launch {
                        delay(Timing.longPressAndroidMs.toLong())
                        scanned = true
                        onScan(true)
                        while (isActive) {
                            haptics.perform(Haptic.Tick)
                            delay(1_000)
                        }
                    }
                    is PressInteraction.Release, is PressInteraction.Cancel -> {
                        job?.cancel()
                        if (scanned) onScan(false)
                    }
                }
            }
        }
    }
    // A skip is a quick tap, far shorter than a press morph, so the library's pressed shape
    // never got to show. The squeeze is held here for long enough to be seen, and the tap
    // also throws the arrow the way it points.
    val still = reduceMotion()
    val scope = rememberCoroutineScope()
    val kick = remember { Animatable(0f) }
    var squeezed by remember { mutableStateOf(false) }
    LaunchedEffect(interaction) {
        var release: Job? = null
        var downAt = 0L
        interaction.interactions.collect { i ->
            when (i) {
                is PressInteraction.Press -> {
                    release?.cancel()
                    downAt = SystemClock.uptimeMillis()
                    squeezed = true
                }
                is PressInteraction.Release, is PressInteraction.Cancel -> release = launch {
                    delay((SQUEEZE_SHOWN_MS - (SystemClock.uptimeMillis() - downAt)).coerceAtLeast(0L))
                    squeezed = false
                }
            }
        }
    }
    val squeeze by animateFloatAsState(if (squeezed) 1f else 0f, MaterialTheme.motionScheme.fastSpatialSpec(), label = "squeeze")
    val tap = rememberDebounced {
        if (scanned) {
            scanned = false
        } else {
            haptics.perform(Haptic.Transport)
            onClick()
            if (!still) {
                scope.launch {
                    kick.snapTo(0f)
                    kick.animateTo(1f, tween(KICK_OUT_MS, easing = LinearOutSlowInEasing))
                    kick.animateTo(0f, spring(dampingRatio = KICK_DAMPING, stiffness = Spring.StiffnessMedium))
                }
            }
        }
    }
    // Pressed, the round end tightens to the play button's pressed corner and the end
    // beside Play to its own pressed corner; the spring may run a little past either end.
    val outer = CornerSize(lerp(size.skipOuter.dp, size.pressedRadius.dp, squeeze).coerceAtLeast(0.dp))
    val inner = CornerSize(
        lerp(AndroidToggleShape.TransportRow.PrevNext.innerRadius.dp, AndroidToggleShape.TransportRow.PrevNext.pressedInnerRadius.dp, squeeze).coerceAtLeast(0.dp),
    )
    // Towards the round end, which is where the arrow points in either layout direction.
    val way = if (next != (LocalLayoutDirection.current == LayoutDirection.Rtl)) 1f else -1f
    val reach = (size.playH / KICK_REACH_DIVISOR).dp
    FilledTonalIconButton(
        onClick = tap,
        enabled = enabled,
        interactionSource = interaction,
        colors = colors ?: if (tint != null) {
            IconButtonDefaults.filledTonalIconButtonColors(containerColor = tint.copy(alpha = 0.16f), contentColor = tint)
        } else {
            IconButtonDefaults.filledTonalIconButtonColors()
        },
        shape = if (next) RoundedCornerShape(inner, outer, outer, inner) else RoundedCornerShape(outer, inner, inner, outer),
        modifier = Modifier.size(width = size.skipW.dp, height = size.playH.dp),
    ) {
        Icon(
            if (next) Icons.next() else Icons.previous(),
            contentDescription = if (next) "Next" else "Previous",
            modifier = Modifier
                .size((size.playH / 2.6f).dp)
                .graphicsLayer {
                    translationX = if (still) 0f else way * reach.toPx() * (SQUEEZE_SHARE * squeeze.coerceIn(0f, 1f) + kick.value)
                },
        )
    }
}

// How long the pressed shape stays after a quick tap, and the arrow's throw on a skip.
private const val SQUEEZE_SHOWN_MS = 140L
private const val KICK_OUT_MS = 90
private const val KICK_DAMPING = 0.42f
private const val KICK_REACH_DIVISOR = 12f
private const val SQUEEZE_SHARE = 0.4f

/** Three separate buttons 8 dp apart, not one joined pill. */
@Composable
fun TransportRow(
    playing: Boolean,
    loading: Boolean,
    onPrevious: () -> Unit,
    onToggle: () -> Unit,
    onNext: () -> Unit,
    size: TransportSize = TransportSize.Large,
    onScan: ((forward: Boolean, active: Boolean) -> Unit)? = null,
    container: Color? = null,
    onContainer: Color? = null,
) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(AndroidToggleShape.TransportRow.gap.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        SkipButton(next = false, onClick = onPrevious, size = size, onScan = onScan?.let { s -> { a -> s(false, a) } }, tint = container)
        PlayPauseButton(playing, loading, onToggle, size, container = container, onContainer = onContainer)
        SkipButton(next = true, onClick = onNext, size = size, onScan = onScan?.let { s -> { a -> s(true, a) } }, tint = container)
    }
}
