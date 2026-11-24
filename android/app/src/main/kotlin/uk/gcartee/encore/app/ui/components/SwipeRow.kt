package uk.gcartee.encore.app.ui.components

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.drawscope.translate
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
            // Mock-up 46, the owner's Gmail swipe (24 November 2026): a bubble opens in the
            // strip the row has left. It is a circle while it grows to the row's height, then
            // stretches into a pill that follows the finger; it fills with colour, and its
            // icon pops, once letting go would act. The theme colour for Play next and Add to
            // queue, red to remove. The words went: the icon says it, TalkBack reads the name.
            val scheme = MaterialTheme.colorScheme
            val still = uk.gcartee.encore.app.ui.onboarding.reduceMotion()
            val soft = if (a.destructive) scheme.errorContainer else scheme.primaryContainer
            val strong = if (a.destructive) scheme.error else scheme.primary
            val onSoft = if (a.destructive) scheme.onErrorContainer else scheme.onPrimaryContainer
            val onStrong = if (a.destructive) scheme.onError else scheme.onPrimary
            val fill by animateColorAsState(if (armed) strong else soft, spring(stiffness = Spring.StiffnessMediumLow), label = "fill")
            val ink by animateColorAsState(if (armed) onStrong else onSoft, spring(stiffness = Spring.StiffnessMediumLow), label = "ink")
            val pop by animateFloatAsState(
                if (armed && !still) ICON_POP else 1f,
                spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessMedium),
                label = "pop",
            )
            val icon = a.icon()
            Box(
                Modifier
                    .matchParentSize()
                    .drawBehind {
                        val o = offset.value
                        val gap = BLOB_GAP_DP.dp.toPx()
                        val tallest = size.height - 2 * BLOB_INSET_DP.dp.toPx()
                        val wide = abs(o) - 2 * gap
                        if (wide <= 0f) return@drawBehind
                        // Under the row's height it is a circle; reduce motion skips to the pill.
                        val h = if (still) tallest else minOf(wide, tallest)
                        val w = maxOf(wide, h)
                        val left = if (o > 0f) gap else size.width - gap - w
                        val top = (size.height - h) / 2f
                        drawRoundRect(fill, topLeft = Offset(left, top), size = Size(w, h), cornerRadius = CornerRadius(h / 2f))
                        val side = minOf(ICON_DP.dp.toPx(), h * ICON_SHARE) * pop
                        translate(left + (w - side) / 2f, top + (h - side) / 2f) {
                            with(icon) { draw(Size(side, side), colorFilter = ColorFilter.tint(ink)) }
                        }
                    },
            )
        }
        Box(Modifier.graphicsLayer { translationX = offset.value }.background(rowColor.copy(alpha = if (offset.value == 0f) 0f else 1f))) {
            content()
        }
    }
}

private const val BLOB_GAP_DP = 8
private const val BLOB_INSET_DP = 4
private const val ICON_DP = 24
private const val ICON_SHARE = 0.55f
private const val ICON_POP = 1.2f
