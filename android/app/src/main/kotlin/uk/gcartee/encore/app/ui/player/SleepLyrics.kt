package uk.gcartee.encore.app.ui.player

import android.graphics.Bitmap
import android.graphics.Paint
import android.graphics.PorterDuff
import android.graphics.PorterDuffXfermode
import android.graphics.RectF
import android.graphics.Typeface
import android.text.Layout
import android.text.StaticLayout
import android.text.TextPaint
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFontFamilyResolver
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntSize
import java.nio.ByteBuffer
import kotlin.math.abs
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.roundToInt
import kotlin.math.sqrt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.ui.onboarding.reduceMotion

/*
 * Sleep mode's lyrics (owner, 24 and 25 November 2026: blur that grows away from the sung
 * line, every line whole, and no dropped frames on the Tab A7 Lite or the S20 FE).
 *
 * The first version blurred each line live on the graphics chip, and while a line changed
 * every frame blurred all of them again: about 45 ms a frame on the tablet, three frames'
 * time. Now each line is drawn once, off the main thread, as three still pictures: sharp, a
 * little soft and softer. While the strip glides each frame only places those pictures,
 * shrinks them and mixes the two nearest levels of softness, so a frame is a handful of
 * picture draws in one node. Blur this way also works before Android 12.
 */

/** A blurred picture, [block] times smaller than the sharp one each way. */
private class Soft(val bitmap: Bitmap, val block: Int)

/** One line drawn ahead of time: sharp, soft and softer pictures of the same box. */
private class LineArt(val sharp: Bitmap, val soft: Soft, val softer: Soft, val textHeight: Float) {
    fun level(i: Int): Soft = when (i) {
        0 -> Soft(sharp, 1)
        1 -> soft
        else -> softer
    }
}

/**
 * The lines as one strip that glides: the sung line sharp in the middle, and the lines each
 * side smaller, fainter and softer the further they are from it. When the line changes the
 * strip moves to its new place on one soft spring with no bounce. Every line is whole,
 * however long (balanced over as many rows as it needs); a long line on a short screen
 * shrinks until it fits.
 */
@Composable
internal fun SleepLyrics(lines: List<String>, current: Int, title: String, style: TextStyle, gap: Dp, colour: Color) {
    val density = LocalDensity.current
    val gapPx = with(density) { gap.toPx() }
    val fontPx = with(density) { style.fontSize.toPx() }
    val resolver = LocalFontFamilyResolver.current
    val typeface = remember(resolver, style.fontFamily, style.fontWeight) {
        (resolver.resolve(style.fontFamily, style.fontWeight ?: FontWeight.Normal).value as? Typeface) ?: Typeface.DEFAULT_BOLD
    }
    val still = reduceMotion()
    var box by remember { mutableStateOf(IntSize.Zero) }
    val width = box.width

    // The pictures, by line (-1 is the song's title), drawn nearest first and dropped once
    // far behind. A line not ready yet is left out for a frame or two; the lines that come
    // in are the faint outer ones, so that never shows.
    val art = remember(lines, title, width, fontPx, typeface) { mutableStateMapOf<Int, LineArt>() }
    LaunchedEffect(art, current) {
        if (width <= 0) return@LaunchedEffect
        val keep = (current - REACH - 2)..(current + REACH + 3)
        art.keys.filter { it !in keep }.forEach { art.remove(it) }
        val wanted = ((current - REACH - 1).coerceAtLeast(-1)..(current + REACH + 2).coerceAtMost(lines.lastIndex))
            .sortedBy { abs(it - current) }
        for (i in wanted) {
            if (i in art) continue
            val text = if (i < 0) title else lines[i]
            val made = withContext(Dispatchers.Default) { renderLine(text, typeface, fontPx, width) }
            made.sharp.prepareToDraw()
            made.soft.bitmap.prepareToDraw()
            made.softer.bitmap.prepareToDraw()
            art[i] = made
        }
    }

    // Where the strip is, in lines: whole numbers at rest, in between while it glides.
    val pos = remember(lines, title) { Animatable(current.toFloat()) }
    LaunchedEffect(pos, current, still) {
        val to = current.toFloat()
        if (still || abs(to - pos.value) > JUMP_LINES) {
            pos.snapTo(to)
        } else {
            pos.animateTo(to, spring(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = LINE_STIFFNESS))
        }
    }

    val paint = remember {
        // Added, not laid over: on the black screen two levels of softness mixed this way
        // keep the line's full brightness halfway between them.
        Paint(Paint.FILTER_BITMAP_FLAG).apply { xfermode = PorterDuffXfermode(PorterDuff.Mode.ADD) }
    }
    val dst = remember { RectF() }
    val rgb = colour.toArgb() and 0x00FFFFFF
    Spacer(
        Modifier
            .fillMaxSize()
            .onSizeChanged { box = it }
            .semantics { contentDescription = lines.getOrNull(current) ?: title }
            .drawBehind {
                val half = size.height / 2f
                if (half <= 0f) return@drawBehind
                fun h(i: Int): Float = art[i]?.textHeight ?: (fontPx * 1.2f)
                // A long sung line on a short screen shrinks, with the rest, until it fits.
                fun fit(c: Int): Float = minOf(1f, half * 2f * FIT_SHARE / h(c))
                // Where line i sits, and its size, when line c is the sung one.
                fun y(i: Int, c: Int): Float {
                    val role = i - c
                    if (role == 0) return 0f
                    val dir = if (role > 0) 1 else -1
                    val k = fit(c)
                    var at = h(c) * k / 2f
                    for (n in 1 until abs(role)) at += (gapPx + h(c + dir * n) * scaleOf(dir * n)) * k
                    at += (gapPx + h(i) * scaleOf(role) / 2f) * k
                    return dir * at
                }
                val p = pos.value
                val c0 = floor(p).toInt()
                val c1 = c0 + 1
                val f = p - c0
                clipRect(left = -size.width, right = size.width * 2f) {
                    drawIntoCanvas { canvas ->
                        val nc = canvas.nativeCanvas
                        for (i in (c0 - REACH).coerceAtLeast(-1)..(c1 + REACH).coerceAtMost(lines.lastIndex)) {
                            val a = art[i] ?: continue
                            val at = lerp(y(i, c0), y(i, c1), f)
                            // Lines fade out one by one towards the strip's edges.
                            val edge = ((half - abs(at)) / (half * EDGE_FADE_SHARE)).coerceIn(0f, 1f)
                            val fade = lerp(fadeOf(i - c0), fadeOf(i - c1), f) * edge
                            if (fade < 0.004f) continue
                            val scale = lerp(scaleOf(i - c0) * fit(c0), scaleOf(i - c1) * fit(c1), f)
                            val level = lerp(levelOf(i - c0), levelOf(i - c1), f)
                            val low = floor(level).toInt().coerceAtMost(1)
                            val mix = level - low
                            nc.save()
                            nc.translate(size.width / 2f, half + at)
                            nc.scale(scale, scale)
                            draw(nc, a, a.level(low), fade * (1f - mix), rgb, paint, dst)
                            draw(nc, a, a.level(low + 1), fade * mix, rgb, paint, dst)
                            nc.restore()
                        }
                    }
                }
            },
    )
}

/** One picture of a line, centred on the origin, at [alpha] of the line's colour. */
private fun draw(nc: android.graphics.Canvas, a: LineArt, b: Soft, alpha: Float, rgb: Int, paint: Paint, dst: RectF) {
    val al = (alpha * 255f).roundToInt().coerceIn(0, 255)
    if (al == 0) return
    // A soft picture is smaller; it is stretched back by its block size from the same corner.
    val left = -a.sharp.width / 2f
    val top = -a.sharp.height / 2f
    dst.set(left, top, left + b.bitmap.width * b.block, top + b.bitmap.height * b.block)
    paint.color = (al shl 24) or rgb
    nc.drawBitmap(b.bitmap, null, dst, paint)
}

/** The block size a soft picture was shrunk by (its box is a whole number of blocks). */
private fun ceilDiv(full: Int, small: Int): Int = (full + small - 1) / small

private fun lerp(a: Float, b: Float, t: Float): Float = a + (b - a) * t

/**
 * Draws [text] once, centred and balanced over as many rows as it needs, as a sharp alpha
 * picture with room round it for the softest blur, then the two soft pictures from it.
 */
// Layout.BREAK_STRATEGY_BALANCED is the same value lint wants from LineBreaker (API 29).
@android.annotation.SuppressLint("WrongConstant")
private fun renderLine(text: String, typeface: Typeface, fontPx: Float, width: Int): LineArt {
    val paint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        this.typeface = typeface
        textSize = fontPx
        color = android.graphics.Color.WHITE
    }
    val layout = StaticLayout.Builder.obtain(text, 0, text.length, paint, width)
        .setAlignment(Layout.Alignment.ALIGN_CENTER)
        .setIncludePad(false)
        .setBreakStrategy(Layout.BREAK_STRATEGY_BALANCED)
        .build()
    val margin = ceil(fontPx * SOFTER_SIGMA * 3f).toInt()
    val full = Bitmap.createBitmap(width + 2 * margin, layout.height + 2 * margin, Bitmap.Config.ARGB_8888)
    android.graphics.Canvas(full).apply {
        translate(margin.toFloat(), margin.toFloat())
        layout.draw(this)
    }
    val sharp = full.extractAlpha()
    full.recycle()
    return LineArt(sharp, blurred(sharp, fontPx * SOFT_SIGMA), blurred(sharp, fontPx * SOFTER_SIGMA), layout.height.toFloat())
}

/**
 * A blurred copy of an alpha picture, at a fraction of its size: averaged down in blocks,
 * then three box blurs each way (close to a Gaussian of [sigma] pixels at full size). Drawn
 * stretched back up with filtering, it is as smooth as a full-size blur at a fraction of
 * the memory and work.
 */
private fun blurred(src: Bitmap, sigma: Float): Soft {
    val k = (sigma / 1.8f).toInt().coerceIn(1, 4)
    val w = ceilDiv(src.width, k)
    val h = ceilDiv(src.height, k)
    val stride = src.rowBytes
    val raw = ByteBuffer.allocate(stride * src.height)
    src.copyPixelsToBuffer(raw)
    val bytes = raw.array()
    val a = FloatArray(w * h)
    for (y in 0 until src.height) {
        val row = y * stride
        val out = (y / k) * w
        for (x in 0 until src.width) a[out + x / k] += (bytes[row + x].toInt() and 0xFF).toFloat()
    }
    val norm = 1f / (k * k)
    for (i in a.indices) a[i] *= norm
    val s = sigma / k
    // Three passes of a box of radius r spread by about sqrt(r (r + 1)).
    val r = ((sqrt(1f + 4f * s * s) - 1f) / 2f).roundToInt().coerceAtLeast(1)
    val b = FloatArray(w * h)
    repeat(3) {
        boxRows(a, b, w, h, r)
        boxColumns(b, a, w, h, r)
    }
    val out = Bitmap.createBitmap(w, h, Bitmap.Config.ALPHA_8)
    val outStride = out.rowBytes
    val ob = ByteArray(outStride * h)
    for (y in 0 until h) for (x in 0 until w) ob[y * outStride + x] = a[y * w + x].roundToInt().coerceIn(0, 255).toByte()
    out.copyPixelsFromBuffer(ByteBuffer.wrap(ob))
    return Soft(out, k)
}

private fun boxRows(src: FloatArray, dst: FloatArray, w: Int, h: Int, r: Int) {
    val norm = 1f / (2 * r + 1)
    for (y in 0 until h) {
        val o = y * w
        var sum = 0f
        for (x in 0..minOf(r, w - 1)) sum += src[o + x]
        for (x in 0 until w) {
            dst[o + x] = sum * norm
            val add = x + r + 1
            if (add < w) sum += src[o + add]
            val sub = x - r
            if (sub >= 0) sum -= src[o + sub]
        }
    }
}

private fun boxColumns(src: FloatArray, dst: FloatArray, w: Int, h: Int, r: Int) {
    val norm = 1f / (2 * r + 1)
    for (x in 0 until w) {
        var sum = 0f
        for (y in 0..minOf(r, h - 1)) sum += src[y * w + x]
        for (y in 0 until h) {
            dst[y * w + x] = sum * norm
            val add = y + r + 1
            if (add < h) sum += src[add * w + x]
            val sub = y - r
            if (sub >= 0) sum -= src[sub * w + x]
        }
    }
}

/** How far each side of the sung line the strip reaches; the last one is fully faded. */
private const val REACH = 3
/** A jump of more lines than this (a seek) cuts instead of racing through the song. */
private const val JUMP_LINES = 2
private const val NEXT_SCALE = 0.62f
private const val PREV_SCALE = 0.55f
private const val EDGE_SCALE = 0.46f
private const val OUT_SCALE = 0.4f

private fun scaleOf(role: Int): Float = when {
    role == 0 -> 1f
    role == 1 -> NEXT_SCALE
    role == -1 -> PREV_SCALE
    abs(role) == 2 -> EDGE_SCALE
    else -> OUT_SCALE
}

/** The sung line full, the next one clearer than the one just sung, the outer ones faint. */
private fun fadeOf(role: Int): Float = when {
    role == 0 -> 1f
    role == 1 -> 0.6f
    role == -1 -> 0.45f
    abs(role) == 2 -> 0.28f
    else -> 0f
}

/** Which picture: sharp for the sung line, soft next to it, softer beyond. */
private fun levelOf(role: Int): Float = when (abs(role)) {
    0 -> 0f
    1 -> 1f
    else -> 2f
}

/** The blurs, as a share of the font size (a Gaussian's sigma). */
private const val SOFT_SIGMA = 0.035f
private const val SOFTER_SIGMA = 0.075f
/** Between Compose's low and very low stiffness: about half a second to settle, no overshoot. */
private const val LINE_STIFFNESS = 120f
/** The stretch by each edge, as a share of half the strip's height, where lines fade out. */
private const val EDGE_FADE_SHARE = 0.3f
/** The most of the strip's height the sung line may take. */
private const val FIT_SHARE = 0.7f
