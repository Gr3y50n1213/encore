package uk.gcartee.encore.app.ui.recap

import android.content.Context
import android.os.PowerManager
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import kotlin.math.PI
import kotlin.math.sin
import kotlinx.coroutines.delay
import uk.gcartee.encore.app.R
import uk.gcartee.encore.app.recap.Beat
import uk.gcartee.encore.app.recap.GroundColours
import uk.gcartee.encore.app.recap.GroundKind
import uk.gcartee.encore.app.recap.PackColours
import uk.gcartee.encore.app.recap.PackId
import uk.gcartee.encore.app.recap.Recap
import uk.gcartee.encore.tokens.EncoreTokens.Recap.Motion as RecapMotion

/** The width every card is designed at (boards 39 to 41 draw them 270 by 480). */
const val DESIGN_WIDTH = 270f
const val DESIGN_HEIGHT = 480f

/**
 * Lays [content] out at the boards' own sizes: inside, a dp is a dp of the 270-wide design,
 * scaled to [widthDp] of real screen. Text keeps the design's size too (the card is a picture
 * of a fixed shape; its words are also given whole to screen readers).
 */
@Composable
fun DesignScale(widthDp: Float, content: @Composable () -> Unit) {
    val d = LocalDensity.current
    val scale = (widthDp / DESIGN_WIDTH).coerceAtLeast(0.1f)
    CompositionLocalProvider(LocalDensity provides Density(d.density * scale, 1f)) { content() }
}

/** Inter (bundled, one variable file) in the heavy weights Tide and Bloom draw with. */
@OptIn(androidx.compose.ui.text.ExperimentalTextApi::class)
private val RecapInter = FontFamily(
    listOf(300, 400, 500, 600, 700, 800).map { w ->
        Font(R.font.inter_variable, weight = FontWeight(w), variationSettings = FontVariation.Settings(FontVariation.weight(w)))
    },
)

/** A pack's type pairing (board 40, "What each pack is"). Sizes are set where it is used. */
@Immutable
class PackType(
    val display: TextStyle,
    val italic: TextStyle,
    val label: TextStyle,
    val labelUpper: Boolean,
    val body: TextStyle,
    val row: TextStyle,
)

fun packType(pack: PackId): PackType = when (pack) {
    // A heavy serif for numbers and names with a small-caps sans for labels. The shipped
    // pack is to bundle an open-licence face; until then the device's serif stands in.
    PackId.EDITORIAL -> PackType(
        display = TextStyle(fontFamily = FontFamily.Serif, fontWeight = FontWeight.Bold, letterSpacing = (-0.04).em, lineHeight = 0.95.em),
        italic = TextStyle(fontFamily = FontFamily.Serif, fontStyle = FontStyle.Italic, fontWeight = FontWeight.Normal, letterSpacing = (-0.01).em, lineHeight = 1.08.em),
        label = TextStyle(fontFamily = RecapInter, fontWeight = FontWeight.SemiBold, fontSize = 8.5.sp, letterSpacing = 0.14.em),
        labelUpper = true,
        body = TextStyle(fontFamily = RecapInter, fontSize = 12.sp, lineHeight = 1.4.em),
        row = TextStyle(fontFamily = RecapInter, fontSize = 11.sp, fontWeight = FontWeight.SemiBold),
    )
    // Inter, heavy for names and numerals.
    PackId.TIDE, PackId.BLOOM -> PackType(
        display = TextStyle(fontFamily = RecapInter, fontWeight = FontWeight.ExtraBold, letterSpacing = (-0.045).em, lineHeight = 0.96.em),
        italic = TextStyle(fontFamily = RecapInter, fontWeight = FontWeight.ExtraBold, letterSpacing = (-0.03).em, lineHeight = 1.05.em),
        label = TextStyle(fontFamily = RecapInter, fontWeight = FontWeight.SemiBold, fontSize = 11.sp, letterSpacing = 0.02.em),
        labelUpper = false,
        body = TextStyle(fontFamily = RecapInter, fontWeight = FontWeight.Medium, fontSize = 13.sp, lineHeight = 1.4.em),
        row = TextStyle(fontFamily = RecapInter, fontWeight = FontWeight.SemiBold, fontSize = 12.5.sp),
    )
    // The system font on Apple devices and Inter elsewhere, in bold and regular.
    PackId.GLASS -> PackType(
        display = TextStyle(fontFamily = RecapInter, fontWeight = FontWeight.Bold, letterSpacing = (-0.02).em, lineHeight = 1.0.em),
        italic = TextStyle(fontFamily = RecapInter, fontWeight = FontWeight.Bold, letterSpacing = (-0.02).em, lineHeight = 1.1.em),
        label = TextStyle(fontFamily = RecapInter, fontWeight = FontWeight.SemiBold, fontSize = 8.5.sp, letterSpacing = 0.14.em),
        labelUpper = true,
        body = TextStyle(fontFamily = RecapInter, fontWeight = FontWeight.Normal, fontSize = 11.5.sp, lineHeight = 1.4.em),
        row = TextStyle(fontFamily = RecapInter, fontWeight = FontWeight.Medium, fontSize = 12.sp),
    )
}

/** One card's colours as Compose colours. */
@Immutable
class Ground(val kind: GroundKind, val top: Color, val bottom: Color, val fg: Color, val accentText: Color, val accentShape: Color, val shapes: List<Color>) {
    companion object {
        fun of(g: GroundColours) = Ground(g.kind, Color(g.top), Color(g.bottom), Color(g.fg), Color(g.accentText), Color(g.accentShape), g.shapes.map { Color(it) })
    }
}

/**
 * The pack a recap is drawn in, with the year's colours fitted to it. A pack this app does
 * not know draws Editorial and says so (board 41, "Older app").
 */
@Immutable
class RecapTheme(val pack: PackId, val colours: PackColours, val fallback: Boolean) {
    val type: PackType = packType(pack)

    /** The year's palette, for covers drawn as shapes. */
    val coverColours: List<Color> = (colours.year + DEFAULT_COVER).map { Color(it) }

    fun ground(beat: Beat): Ground = Ground.of(colours.ground(PackColours.groundFor(beat.id, beat.layout)))

    companion object {
        fun of(recap: Recap): RecapTheme {
            val known = PackId.of(recap.pack?.id)
            val pack = known ?: PackId.EDITORIAL
            return RecapTheme(pack, PackColours(pack, recap.palette), fallback = recap.pack != null && known == null)
        }

        private val DEFAULT_COVER = listOf(0xFFE8452C, 0xFFF7A23B, 0xFF2FA7A0, 0xFF0B2E4E, 0xFF1F3D2B, 0xFFF3EEE4).map { it.toInt() }
    }
}

/** Board 39, item 12: ambient motion capped at 30 fps; stopped when [enabled] is false. */
@Composable
fun rememberAmbientClock(enabled: Boolean): State<Float> {
    val t = remember { mutableFloatStateOf(0f) }
    LaunchedEffect(enabled) {
        if (!enabled) return@LaunchedEffect
        val start = withFrameMillis { it } - (t.floatValue * 1000f).toLong()
        while (true) {
            val now = withFrameMillis { it }
            t.floatValue = (now - start) / 1000f
            delay(1000L / RecapMotion.fpsCap)
        }
    }
    return t
}

/** Low Power Mode stops the ambient motion too (board 39, item 12). */
fun powerSaving(context: Context): Boolean =
    runCatching { context.getSystemService(PowerManager::class.java)?.isPowerSaveMode == true }.getOrDefault(false)

private fun cycle(k: Int): Float {
    val c = RecapMotion.ambientCycleSeconds
    val lo = c.firstOrNull() ?: 8
    val hi = c.lastOrNull() ?: 14
    return (lo + (hi - lo) * k / 3f).coerceAtLeast(1f)
}

/** Where a card's water line sits in Tide, as a share of the height. */
fun tideLevel(beat: Beat?): Float = 0.6f + 0.05f * ((beat?.layout ?: 0) % 3) - if (beat?.id == "intro") 0.08f else 0f

/**
 * A card's background: flat paper, ink or spot (Editorial), layered waves under a sky
 * (Tide), soft colour fields on cream (Bloom) or glows on near-black (Glass). Vector
 * shapes and gradients only. [time] drives the ambient drift.
 */
fun DrawScope.drawGround(theme: RecapTheme, g: Ground, seed: Long, level: Float, time: Float, alpha: Float = 1f) {
    val w = size.width
    val h = size.height
    if (g.top == g.bottom) drawRect(g.top, alpha = alpha) else drawRect(Brush.verticalGradient(listOf(g.top, g.bottom)), alpha = alpha)
    val rnd = java.util.Random(seed)
    when (theme.pack) {
        PackId.EDITORIAL -> Unit
        PackId.TIDE -> {
            // The sun's glow over the sky.
            val glow = Color(theme.colours.gold)
            drawCircle(
                Brush.radialGradient(listOf(glow.copy(alpha = 0.4f), glow.copy(alpha = 0f)), center = Offset(w * 0.5f, h * 0.2f), radius = w * 0.7f),
                radius = w * 0.7f,
                center = Offset(w * 0.5f, h * 0.2f),
                alpha = alpha,
            )
            val waves = g.shapes.ifEmpty { listOf(g.bottom) }
            for (k in 0 until 4) {
                val colour = waves[k % waves.size]
                val y0 = h * level + k * h * 0.055f
                val amp = h * (0.014f + 0.006f * k)
                val len = w * (0.8f + 0.3f * rnd.nextFloat())
                val phase = rnd.nextFloat() * 2f * PI.toFloat() + time / cycle(k) * 2f * PI.toFloat() * (if (k % 2 == 0) 1f else -1f)
                val crest = Path()
                val fill = Path()
                val steps = 48
                for (i in 0..steps) {
                    val x = w * i / steps
                    val y = y0 + amp * sin(2f * PI.toFloat() * x / len + phase)
                    if (i == 0) {
                        crest.moveTo(x, y)
                        fill.moveTo(x, y)
                    } else {
                        crest.lineTo(x, y)
                        fill.lineTo(x, y)
                    }
                }
                fill.lineTo(w, h)
                fill.lineTo(0f, h)
                fill.close()
                drawPath(fill, colour, alpha = alpha)
                drawPath(crest, Color.White.copy(alpha = 0.3f), style = Stroke(width = 1.2.dp.toPx()), alpha = alpha)
            }
            drawRect(Color(theme.colours.deep), topLeft = Offset(0f, h * level + 4 * h * 0.055f + h * 0.03f), size = Size(w, h), alpha = alpha)
        }
        PackId.BLOOM -> {
            val fields = g.shapes.ifEmpty { listOf(g.accentShape) }
            for (k in 0 until 4) {
                val c = fields[k % fields.size]
                val cx = w * (0.1f + 0.8f * rnd.nextFloat())
                val cy = h * (0.05f + 0.9f * rnd.nextFloat())
                val breathe = 1f + 0.04f * sin(time / cycle(k) * 2f * PI.toFloat())
                val r = w * (0.38f + 0.22f * rnd.nextFloat()) * breathe
                drawCircle(
                    Brush.radialGradient(listOf(c, c.copy(alpha = 0.85f), c.copy(alpha = 0f)), center = Offset(cx, cy), radius = r),
                    radius = r,
                    center = Offset(cx, cy),
                    alpha = alpha,
                )
            }
        }
        PackId.GLASS -> {
            val glows = g.shapes.ifEmpty { listOf(g.accentShape) }
            val spots = listOf(Offset(0.12f, 0.08f), Offset(0.92f, 0.3f), Offset(0.2f, 0.95f), Offset(0.95f, 0.92f))
            for (k in 0 until 4) {
                val c = glows[k % glows.size]
                val drift = sin(time / cycle(k) * 2f * PI.toFloat()) * w * 0.04f
                val center = Offset(w * spots[k].x + drift, h * spots[k].y - drift * 0.5f)
                val r = w * (0.7f + 0.2f * rnd.nextFloat())
                drawCircle(Brush.radialGradient(listOf(c, c.copy(alpha = 0f)), center = center, radius = r), radius = r, center = center, alpha = alpha)
            }
        }
    }
}

/**
 * Between cards: Editorial wipes the new ground in from the left (its "slow wipes"); the
 * other packs cross-fade, and Tide's water level eases from one card's to the next.
 */
fun DrawScope.drawGroundChange(theme: RecapTheme, from: Ground?, fromBeat: Beat?, to: Ground, toBeat: Beat?, t: Float, time: Float) {
    val toLevel = tideLevel(toBeat)
    if (from == null || t >= 1f) {
        drawGround(theme, to, toBeat?.shapeSeed ?: 0L, toLevel, time)
        return
    }
    val fromLevel = tideLevel(fromBeat)
    if (theme.pack == PackId.EDITORIAL) {
        drawGround(theme, from, fromBeat?.shapeSeed ?: 0L, fromLevel, time)
        clipRect(right = size.width * t) { drawGround(theme, to, toBeat?.shapeSeed ?: 0L, toLevel, time) }
    } else {
        val level = fromLevel + (toLevel - fromLevel) * t
        drawGround(theme, from, fromBeat?.shapeSeed ?: 0L, level, time)
        drawGround(theme, to, toBeat?.shapeSeed ?: 0L, level, time, alpha = t)
    }
}

/**
 * Offline or not, the cards draw covers as shapes (board 41: "shapes in place of covers"):
 * a sunset, waves under a moon, an arch or a ball, in the year's colours.
 */
@Composable
fun CoverArt(motif: Int, colours: List<Color>, modifier: Modifier = Modifier) {
    Canvas(modifier) { drawCover(motif, colours) }
}

private fun DrawScope.drawCover(motif: Int, colours: List<Color>) {
    val w = size.width
    val h = size.height
    fun c(i: Int) = colours[Math.floorMod(i, colours.size)]
    when (Math.floorMod(motif, 4)) {
        0 -> {
            drawRect(Brush.verticalGradient(listOf(c(0), c(1))))
            drawCircle(Color(0xFFF7C96B), radius = w * 0.24f, center = Offset(w * 0.5f, h * 0.55f))
            for (k in 0 until 4) drawRect(c(3), topLeft = Offset(0f, h * (0.66f + k * 0.085f)), size = Size(w, h * 0.04f))
        }
        1 -> {
            drawRect(c(3))
            drawCircle(Color(0xFFF3EEE4), radius = w * 0.11f, center = Offset(w * 0.7f, h * 0.3f))
            for (k in 0 until 3) {
                val p = Path()
                val y0 = h * (0.52f + k * 0.13f)
                p.moveTo(0f, y0)
                val steps = 24
                for (i in 0..steps) {
                    val x = w * i / steps
                    p.lineTo(x, y0 + h * 0.04f * sin(2f * PI.toFloat() * x / w * 1.5f + k))
                }
                p.lineTo(w, h)
                p.lineTo(0f, h)
                p.close()
                drawPath(p, listOf(c(2), Color(0xFF7FD3C8), Color(0xFFD8F1EC))[k])
            }
        }
        2 -> {
            drawRect(c(4))
            val stroke = w * 0.075f
            val rings = listOf(c(0), Color(0xFFF7C96B), Color(0xFFB9E07A), c(2))
            for ((k, ring) in rings.withIndex()) {
                val r = w * (0.34f - k * 0.08f)
                if (r <= stroke) break
                val cx = w * 0.5f
                val top = h * 0.58f
                drawArc(ring, 180f, 180f, false, topLeft = Offset(cx - r, top - r), size = Size(2 * r, 2 * r), style = Stroke(stroke))
                drawLine(ring, Offset(cx - r, top), Offset(cx - r, h), stroke)
                drawLine(ring, Offset(cx + r, top), Offset(cx + r, h), stroke)
            }
        }
        else -> {
            drawRect(c(5))
            drawLine(c(3).copy(alpha = 0.5f), Offset(w * 0.2f, h * 0.8f), Offset(w * 0.8f, h * 0.8f), w * 0.02f, cap = StrokeCap.Round)
            drawCircle(c(0), radius = w * 0.26f, center = Offset(w * 0.5f, h * 0.48f))
            drawCircle(Color.White.copy(alpha = 0.55f), radius = w * 0.06f, center = Offset(w * 0.42f, h * 0.4f))
        }
    }
}

/** Bloom's living blob: a rounded shape with uneven corners. */
val BlobShape: Shape = RoundedCornerShape(topStartPercent = 58, topEndPercent = 42, bottomEndPercent = 55, bottomStartPercent = 45)

/**
 * A cover in the pack's own frame: Editorial a square with a spot-colour block behind it,
 * Tide a sun with a soft ring, Bloom a tilted blob, Glass a lens.
 */
@Composable
fun CoverFrame(theme: RecapTheme, ground: Ground, motif: Int, size: Dp, modifier: Modifier = Modifier, block: Boolean = true) {
    val colours = theme.coverColours
    when (theme.pack) {
        PackId.EDITORIAL -> Box(modifier.size(size + if (block) size * 0.1f else 0.dp)) {
            if (block) {
                Box(Modifier.offset(size * 0.1f, size * 0.1f).size(size).background(if (ground.kind == GroundKind.SPOT) ground.fg else ground.accentShape, RoundedCornerShape(6.dp)))
            }
            CoverArt(motif, colours, Modifier.size(size).clip(RoundedCornerShape(6.dp)))
        }
        PackId.TIDE -> Box(modifier.size(size).drawBehind { drawCircle(Color.White.copy(alpha = 0.28f), radius = this.size.minDimension / 2f + 6.dp.toPx()) }) {
            CoverArt(motif, colours, Modifier.fillMaxSize().clip(CircleShape))
        }
        PackId.BLOOM -> CoverArt(motif, colours, modifier.size(size).rotate(-6f).clip(BlobShape))
        PackId.GLASS -> Box(modifier.size(size).clip(CircleShape).border(1.dp, Color.White.copy(alpha = 0.45f), CircleShape)) {
            CoverArt(motif, colours, Modifier.fillMaxSize())
            Box(
                Modifier.fillMaxSize().background(
                    Brush.radialGradient(listOf(Color.White.copy(alpha = 0.45f), Color.White.copy(alpha = 0.08f), Color.White.copy(alpha = 0.04f))),
                ),
            )
        }
    }
}

/**
 * The frame the lists sit in: none in Editorial and Tide, a frosted cream card in Bloom and
 * a frosted lens in Glass (board 40, top artist row).
 */
@Composable
fun Panel(theme: RecapTheme, modifier: Modifier = Modifier, content: @Composable BoxScope.() -> Unit) {
    when (theme.pack) {
        PackId.BLOOM -> Box(modifier.background(Color(theme.colours.cream).copy(alpha = 0.82f), RoundedCornerShape(22.dp)).padding(horizontal = 14.dp, vertical = 6.dp), content = content)
        PackId.GLASS -> Box(
            modifier
                .background(Color.White.copy(alpha = PackColours.LENS_WHITE), RoundedCornerShape(24.dp))
                .border(1.dp, Color.White.copy(alpha = 0.38f), RoundedCornerShape(24.dp))
                .padding(horizontal = 16.dp, vertical = 12.dp),
            content = content,
        )
        else -> Box(modifier, content = content)
    }
}
