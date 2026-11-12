package uk.gcartee.encore.app.widgets

import android.graphics.Bitmap
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

    /** The palette's stops, moved [fade] (0 to 1) of the way to the base. */
    fun stops(p: SnapshotPalette, fade: Float): List<Int> =
        if (fade <= 0f) p.stops else p.stops.map { blend(it, p.base, fade) }

    /** The field's pixels (pure, JVM-testable). */
    fun pixels(p: SnapshotPalette, fade: Float = 0f, size: Int = SIZE_PX): IntArray =
        FieldMath.pixels(p.base, stops(p, fade), size, size)

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
        return Triple(FieldMath.pixels(spec.palette.base, stops(spec.palette, spec.fade), w, h, aspect), w, h)
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
