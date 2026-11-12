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

    fun bitmap(p: SnapshotPalette, fade: Float = 0f): Bitmap {
        val pixels = pixels(p, fade)
        return Bitmap.createBitmap(pixels, SIZE_PX, SIZE_PX, Bitmap.Config.ARGB_8888)
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
