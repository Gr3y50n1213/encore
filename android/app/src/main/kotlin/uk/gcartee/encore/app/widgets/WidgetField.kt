package uk.gcartee.encore.app.widgets

import android.graphics.Bitmap
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min
import uk.gcartee.encore.app.ui.player.FieldMath

/**
 * An album's field as a small bitmap for a widget's background (board 34, redesign of
 * 11 November 2026): the app's Now Playing gradient, one still frame, from the palette the
 * core put in the snapshot. A widget can't draw a gradient itself (Glance has none), so it
 * is made here, once per load in the widget's process, and shared by every size the widget
 * draws. Nothing here starts the core or reads anything but the palette it is given.
 */
object WidgetField {
    /** About 64 KB: smooth, so stretched over any shape it stays free of blocks. */
    const val SIZE_PX = 128

    /** A stale widget (over 24 h) fades its stops toward the base by this much. */
    const val STALE_FADE = 0.6f

    /** The frame behind Mixes and Recently played is the lead album's field, softened by this much. */
    const val SOFT_FADE = 0.3f

    /** A dark album's stops are never darker than this (HSV value), so the field isn't a flat black-grey. */
    private const val MIN_STOP_VALUE = 0.30f

    /** The same for the base the stops sit on. */
    private const val MIN_BASE_VALUE = 0.12f

    /** A very dark color made lighter and a little richer, keeping its hue; anything brighter is left alone (pure). */
    fun lifted(c: Int, minValue: Float): Int {
        val r = ((c shr 16) and 0xFF) / 255f
        val g = ((c shr 8) and 0xFF) / 255f
        val b = (c and 0xFF) / 255f
        val mx = max(r, max(g, b))
        if (mx >= minValue) return c
        val mn = min(r, min(g, b))
        val d = mx - mn
        val s = if (mx <= 0f) 0f else d / mx
        val h = when {
            d <= 0f -> 0f
            mx == r -> (((g - b) / d) % 6f + 6f) % 6f
            mx == g -> (b - r) / d + 2f
            else -> (r - g) / d + 4f
        }
        // Only a tinted color is made richer; a true grey stays grey.
        val s2 = if (s > 0.08f) min(1f, s * 1.25f) else s
        val v = minValue
        val chroma = v * s2
        val x = chroma * (1f - abs(h % 2f - 1f))
        val m = v - chroma
        val (r1, g1, b1) = when (h.toInt()) {
            0 -> Triple(chroma, x, 0f)
            1 -> Triple(x, chroma, 0f)
            2 -> Triple(0f, chroma, x)
            3 -> Triple(0f, x, chroma)
            4 -> Triple(x, 0f, chroma)
            else -> Triple(chroma, 0f, x)
        }
        fun ch(f: Float) = ((f + m) * 255f + 0.5f).toInt().coerceIn(0, 255)
        return (0xFF shl 24) or (ch(r1) shl 16) or (ch(g1) shl 8) or ch(b1)
    }

    /** The base the field is drawn on: the palette's, lifted when the art is dark. */
    fun baseOf(p: SnapshotPalette): Int = if (p.dark) lifted(p.base, MIN_BASE_VALUE) else p.base

    /** The palette's stops (dark art lifted), moved [fade] (0 to 1) of the way to the base. */
    fun stops(p: SnapshotPalette, fade: Float): List<Int> {
        val lit = if (p.dark) p.stops.map { lifted(it, MIN_STOP_VALUE) } else p.stops
        return if (fade <= 0f) lit else lit.map { blend(it, baseOf(p), fade) }
    }

    /** The field's pixels (pure, JVM-testable). */
    fun pixels(p: SnapshotPalette, fade: Float = 0f, size: Int = SIZE_PX): IntArray =
        FieldMath.pixels(baseOf(p), stops(p, fade), size, size)

    /** What a widget's background is made from: the palette and how far it is faded. Made into pixels per size. */
    data class Spec(val palette: SnapshotPalette, val fade: Float)

    /** The long side of a widget's field, in pixels; the short side follows the widget's shape. */
    const val LONG_PX = 128

    /** A widget's shape limits: beyond these the field keeps the limit's look (a 6x1 strip is not drawn thinner). */
    private const val MIN_ASPECT = 0.2f
    private const val MAX_ASPECT = 2.2f

    /** The (w, h) in pixels of a field for a widget [wDp] x [hDp]: its own shape, [LONG_PX] on the long side. */
    fun shape(wDp: Float, hDp: Float): Pair<Int, Int> {
        val aspect = (hDp / wDp.coerceAtLeast(1f)).coerceIn(MIN_ASPECT, MAX_ASPECT)
        return if (aspect <= 1f) LONG_PX to (LONG_PX * aspect).toInt().coerceAtLeast(8)
        else (LONG_PX / aspect).toInt().coerceAtLeast(8) to LONG_PX
    }

    /** The field's pixels at a widget's own shape: round blobs, nothing stretched (pure, JVM-testable). */
    fun pixelsFor(spec: Spec, wDp: Float, hDp: Float): Triple<IntArray, Int, Int> {
        val (w, h) = shape(wDp, hDp)
        val aspect = (hDp / wDp.coerceAtLeast(1f)).coerceIn(MIN_ASPECT, MAX_ASPECT)
        return Triple(FieldMath.pixels(baseOf(spec.palette), stops(spec.palette, spec.fade), w, h, aspect), w, h)
    }

    /** The field for a widget [wDp] x [hDp], at its shape. */
    fun bitmapFor(spec: Spec, wDp: Float, hDp: Float): Bitmap {
        val (pixels, w, h) = pixelsFor(spec, wDp, hDp)
        return Bitmap.createBitmap(pixels, w, h, Bitmap.Config.ARGB_8888)
    }

    /** [from] moved [share] of the way to [to], both opaque ARGB ints. */
    fun blend(from: Int, to: Int, share: Float): Int {
        fun ch(shift: Int): Int {
            val a = (from shr shift) and 0xFF
            val b = (to shr shift) and 0xFF
            return (a + (b - a) * share + 0.5f).toInt().coerceIn(0, 255)
        }
        return (0xFF shl 24) or (ch(16) shl 16) or (ch(8) shl 8) or ch(0)
    }

    /** WCAG relative luminance of an opaque ARGB int. */
    fun luminance(c: Int): Double {
        fun lin(v: Int): Double {
            val s = v / 255.0
            return if (s <= 0.03928) s / 12.92 else Math.pow((s + 0.055) / 1.055, 2.4)
        }
        return 0.2126 * lin((c shr 16) and 0xFF) + 0.7152 * lin((c shr 8) and 0xFF) + 0.0722 * lin(c and 0xFF)
    }

    /** WCAG contrast ratio of two opaque ARGB ints, 1 to 21. */
    fun contrast(a: Int, b: Int): Double {
        val la = luminance(a) + 0.05
        val lb = luminance(b) + 0.05
        return if (la > lb) la / lb else lb / la
    }
}
