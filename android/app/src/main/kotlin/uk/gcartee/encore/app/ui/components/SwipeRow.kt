package uk.gcartee.encore.app.ui.components

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import kotlin.math.abs
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.tokens.EncoreTokens.Gestures.PlayerSwipe
import uk.gcartee.encore.tokens.EncoreTokens.Gestures.RowSwipe
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/** One side of a row swipe (board 42, "Row swipes"). */
class SwipeAction(
    val label: String,
    val destructive: Boolean,
    val icon: @Composable () -> Painter,
    val perform: () -> Unit,
)

/**
 * Board 42, rule 14: a row swipe arms at 25% of the row (60% when destructive) with one
 * haptic, performs on release, and springs back under that with no action and no haptic.
 * The direction locks in the first 12 dp (sideways more than 1.2 times vertical), so a
 * scroll never swipes; a drag that starts in the 24 dp edge zone is left to Back.
 * [toEnd] is the swipe right in left-to-right languages (Play Next), [toStart] the swipe
 * left (Add to Queue, Remove, Not this); both mirror in right-to-left. TalkBack gets both
 * as custom actions.
 */
@Composable
fun SwipeRow(
    toEnd: SwipeAction?,
    toStart: SwipeAction?,
    rowColor: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.surface,
    content: @Composable () -> Unit,
) {
    if (toEnd == null && toStart == null) {
        content()
        return
    }
    val haptics = rememberHaptics()
    val scope = rememberCoroutineScope()
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    val offset = remember { Animatable(0f) }
    var width by remember { mutableIntStateOf(0) }
    var armed by remember { mutableStateOf(false) }
    // Positive offset is a drag to the right on screen.
    val rightAction = if (rtl) toStart else toEnd
    val leftAction = if (rtl) toEnd else toStart
    val shown = when {
        offset.value > 0f -> rightAction
        offset.value < 0f -> leftAction
        else -> null
    }

    Box(
        Modifier
            .onSizeChanged { width = it.width }
            .semantics {
                customActions = listOfNotNull(toEnd, toStart).map { a -> CustomAccessibilityAction(a.label) { a.perform(); true } }
            }
            .pointerInput(toEnd, toStart, rtl) {
                val edge = PlayerSwipe.edgeZonePt.dp.toPx()
                val lock = PlayerSwipe.directionLockPt.dp.toPx()
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    if (down.position.x < edge || down.position.x > width - edge) return@awaitEachGesture
                    var travel = Offset.Zero
                    var locked = false
                    while (true) {
                        val event = awaitPointerEvent()
                        val change = event.changes.firstOrNull { it.id == down.id } ?: break
                        if (!change.pressed) break
                        val d = change.positionChange()
                        if (!locked) {
                            if (change.isConsumed) return@awaitEachGesture
                            travel += d
                            if (travel.getDistance() < lock) continue
                            if (abs(travel.x) <= abs(travel.y) * PlayerSwipe.directionLockRatio) return@awaitEachGesture
                            locked = true
                        }
                        change.consume()
                        var next = offset.value + d.x
                        if (next > 0f && rightAction == null) next = 0f
                        if (next < 0f && leftAction == null) next = 0f
                        val action = if (next > 0f) rightAction else leftAction
                        val armAt = width * (if (action?.destructive == true) RowSwipe.armPercentDestructive else RowSwipe.armPercentNonDestructive) / 100f
                        val nowArmed = abs(next) >= armAt && action != null
                        if (nowArmed && !armed) haptics.perform(Haptic.Armed)
                        armed = nowArmed
                        scope.launch { offset.snapTo(next) }
                    }
                    val action = if (offset.value > 0f) rightAction else leftAction
                    val perform = armed
                    armed = false
                    scope.launch { offset.animateTo(0f) }
                    if (perform) action?.perform?.invoke()
                }
            },
    ) {
        shown?.let { a ->
            val bg = if (a.destructive) MaterialTheme.colorScheme.errorContainer else MaterialTheme.colorScheme.secondaryContainer
            val fg = if (a.destructive) MaterialTheme.colorScheme.onErrorContainer else MaterialTheme.colorScheme.onSecondaryContainer
            // Only the strip the row has slid away from shows the action, as a rounded
            // container, so rows over an album's gradient need no solid backing.
            val radius = SWIPE_RADIUS_DP.dp
            Row(
                Modifier
                    .matchParentSize()
                    .drawWithContent {
                        val row = this
                        val o = offset.value
                        val left = if (o > 0f) 0f else size.width + o
                        val right = if (o > 0f) o else size.width
                        val r = radius.toPx()
                        clipPath(androidx.compose.ui.graphics.Path().apply {
                            addRoundRect(androidx.compose.ui.geometry.RoundRect(left, 0f, right, size.height, androidx.compose.ui.geometry.CornerRadius(r)))
                        }) { row.drawContent() }
                    }
                    .background(if (armed) bg else bg.copy(alpha = 0.6f))
                    .padding(horizontal = (Grid.base * 3).dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = if (offset.value > 0f) androidx.compose.foundation.layout.Arrangement.Start else androidx.compose.foundation.layout.Arrangement.End,
            ) {
                Icon(a.icon(), contentDescription = null, tint = fg)
                Spacer(Modifier.width(Grid.base.dp))
                Text(a.label, style = LocalEncoreType.current.songRowTitle, color = fg)
            }
        }
        Box(Modifier.graphicsLayer { translationX = offset.value }.background(rowColor.copy(alpha = if (offset.value == 0f) 0f else 1f))) {
            content()
        }
    }
}

private const val SWIPE_RADIUS_DP = 16
