package uk.gcartee.encore.app.ui.home

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import androidx.graphics.shapes.Morph
import kotlin.math.PI
import kotlin.math.sin
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.toPath
import uk.gcartee.encore.app.ui.onboarding.reduceMotion

/**
 * "Encore is learning": a Material 3 Expressive shape that turns slowly and morphs from
 * burst to cookie to sunny to flower (the loading indicator's family, at hero size), in
 * the accent's gradient, with the sparkle mark at its heart and four small stars that
 * twinkle around it in turn. It says "working on it" without a progress bar's precision.
 * Reduce Motion keeps it still.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LearningOrb(modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    val still = reduceMotion()
    val shapes = remember { listOf(MaterialShapes.SoftBurst, MaterialShapes.Cookie9Sided, MaterialShapes.Sunny, MaterialShapes.Flower) }
    val morphs = remember { shapes.indices.map { Morph(shapes[it], shapes[(it + 1) % shapes.size]) } }
    val motion = rememberInfiniteTransition(label = "learning")
    val cycle by motion.animateFloat(0f, shapes.size.toFloat(), infiniteRepeatable(tween(shapes.size * STEP_MS, easing = LinearEasing)), label = "cycle")
    val turn by motion.animateFloat(0f, 360f, infiniteRepeatable(tween(TURN_MS, easing = LinearEasing)), label = "turn")
    val twinkle by motion.animateFloat(0f, 1f, infiniteRepeatable(tween(TWINKLE_MS, easing = LinearEasing)), label = "twinkle")
    val pulse by motion.animateFloat(0.92f, 1.08f, infiniteRepeatable(tween(PULSE_MS, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "pulse")
    val path = remember { Path() }
    val star = remember { Path() }
    val from = scheme.primary
    val to = scheme.tertiary
    val sparkle = scheme.tertiary
    Box(modifier.clearAndSetSemantics { }, contentAlignment = Alignment.Center) {
        Canvas(Modifier.fillMaxSize()) {
            val step = cycle.toInt() % morphs.size
            val local = cycle - cycle.toInt()
            // Each shape holds a moment, then morphs into the next.
            val p = if (still) 0f else FastOutSlowInEasing.transform(((local - HOLD) / (1f - HOLD)).coerceIn(0f, 1f))
            val orb = size.minDimension * ORB_FRACTION
            val corner = Offset((size.width - orb) / 2, (size.height - orb) / 2)
            rotate(if (still) 0f else turn) {
                translate(corner.x, corner.y) {
                    drawPath(
                        morphs[step].toPath(p, Size(orb, orb), path),
                        Brush.linearGradient(listOf(from, to), start = Offset.Zero, end = Offset(orb, orb)),
                    )
                }
            }
            SPARKLES.forEachIndexed { k, (x, y, r) ->
                val phase = (twinkle + k / SPARKLES.size.toFloat()) % 1f
                val glow = if (still) 0.6f else sin(phase * PI).toFloat()
                if (glow > 0.02f) drawSparkle(star, Offset(size.width * x, size.height * y), size.minDimension * r * (0.5f + 0.5f * glow), sparkle.copy(alpha = glow))
            }
        }
        Icon(
            Icons.spark(),
            contentDescription = null,
            tint = scheme.onPrimary,
            modifier = Modifier.size(ICON_DP.dp).graphicsLayer {
                val s = if (still) 1f else pulse
                scaleX = s
                scaleY = s
            },
        )
    }
}

/** A four-pointed sparkle: concave sides drawn as curves through the centre. */
private fun DrawScope.drawSparkle(path: Path, c: Offset, r: Float, colour: Color) {
    path.reset()
    path.moveTo(c.x, c.y - r)
    path.quadraticTo(c.x, c.y, c.x + r, c.y)
    path.quadraticTo(c.x, c.y, c.x, c.y + r)
    path.quadraticTo(c.x, c.y, c.x - r, c.y)
    path.quadraticTo(c.x, c.y, c.x, c.y - r)
    path.close()
    drawPath(path, colour)
}

// Where the four stars sit (fractions of the box) and how big they get.
private val SPARKLES = listOf(
    Triple(0.16f, 0.2f, 0.07f),
    Triple(0.86f, 0.28f, 0.05f),
    Triple(0.8f, 0.84f, 0.065f),
    Triple(0.18f, 0.78f, 0.045f),
)
private const val STEP_MS = 1_800
private const val HOLD = 0.45f
private const val TURN_MS = 16_000
private const val TWINKLE_MS = 3_200
private const val PULSE_MS = 1_400
private const val ORB_FRACTION = 0.62f
private const val ICON_DP = 44
