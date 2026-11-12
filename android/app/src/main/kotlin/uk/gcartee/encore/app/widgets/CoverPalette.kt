package uk.gcartee.encore.app.widgets

import android.graphics.Bitmap
import java.io.File

/**
 * A stand-in palette for a cover the core hasn't made one for yet (widgets redesign, 12 November
 * 2026: after an update the snapshot has no palettes until the app has run, so the widgets sat in
 * the theme's colors for a while). It is estimated from the cover's own pixels, in the widget's
 * process, and is replaced by the core's palette (the app's own colors, from the 64 px image) as
 * soon as the snapshot carries one, so the colors may shift a little when it arrives. Always a
 * dark scheme with white text, every stop dark enough for it to read. Nothing here starts the core.
 */
object CoverPalette {
    /** The cover is looked at in this many pixels a side. */
    const val SAMPLE_PX = 16

    /** Stops are darkened until white text reads on them by this much (the text rule is 4.5). */
    private const val MIN_CONTRAST = 4.8

    private const val WHITE = 0xFFFFFFFF.toInt()
    private const val BLACK = 0xFF000000.toInt()

    /**
     * The palette for a [size] x [size] sample (row by row, ARGB): the four quadrants' average colors
     * become the four stops, top left, top right, bottom left, bottom right, which are where the
     * field's blobs sit; the base is the whole picture's average, deepened. Null for an empty sample.
     */
    fun estimate(pixels: IntArray, size: Int): SnapshotPalette? {
        if (size < 2 || pixels.size < size * size) return null
        val half = size / 2
        fun average(x0: Int, x1: Int, y0: Int, y1: Int): Int {
            var r = 0L
            var g = 0L
            var b = 0L
            var n = 0
            for (y in y0 until y1) for (x in x0 until x1) {
                val p = pixels[y * size + x]
                r += (p shr 16) and 0xFF
                g += (p shr 8) and 0xFF
                b += p and 0xFF
                n++
            }
            if (n == 0) return BLACK
            return (0xFF shl 24) or ((r / n).toInt() shl 16) or ((g / n).toInt() shl 8) or (b / n).toInt()
        }
        val quadrants = listOf(
            average(0, half, 0, half),
            average(half, size, 0, half),
            average(0, half, half, size),
            average(half, size, half, size),
        )
        val whole = average(0, size, 0, size)
        val stops = quadrants.map { readable(it) }
        return SnapshotPalette(dark = true, base = WidgetField.blend(whole, BLACK, 0.65f), stops = stops, text = WHITE)
    }

    /** [c], moved toward black until white text reads on it. */
    private fun readable(c: Int): Int {
        var out = c
        var guard = 0
        while (WidgetField.contrast(WHITE, out) < MIN_CONTRAST && guard++ < 30) out = WidgetField.blend(out, BLACK, 0.12f)
        return out
    }

    /** [cover] as a palette, or null. */
    fun estimate(cover: Bitmap): SnapshotPalette? {
        val small = Bitmap.createScaledBitmap(cover, SAMPLE_PX, SAMPLE_PX, true)
        val pixels = IntArray(SAMPLE_PX * SAMPLE_PX)
        small.getPixels(pixels, 0, SAMPLE_PX, 0, 0, SAMPLE_PX, SAMPLE_PX)
        if (small !== cover) small.recycle()
        return estimate(pixels, SAMPLE_PX)
    }

    /** A normal profile's read, with estimated palettes filled in; anything else as it is. */
    fun fill(read: SnapshotRead, widgetsDir: File): SnapshotRead =
        if (read is SnapshotRead.Ok && read.snapshot.privacy == SnapshotPrivacy.NORMAL) SnapshotRead.Ok(fill(read.snapshot, widgetsDir)) else read

    /** [snapshot] with an estimated palette wherever the core hasn't made one and the cover file is there. */
    fun fill(snapshot: Snapshot, widgetsDir: File): Snapshot {
        val cache = HashMap<String, SnapshotPalette?>()
        fun of(cover: String?): SnapshotPalette? {
            if (cover == null) return null
            return cache.getOrPut(cover) {
                SnapshotReader.coverFile(widgetsDir, snapshot.profileId, cover)
                    ?.let { SnapshotReader.loadCover(it, SAMPLE_PX * 4) }
                    ?.let { estimate(it) }
            }
        }
        return snapshot.copy(
            song = snapshot.song?.let { if (it.palette == null) it.copy(palette = of(it.cover)) else it },
            pinnedMixes = snapshot.pinnedMixes.map { if (it.palette == null) it.copy(palette = of(it.cover)) else it },
            nextUp = snapshot.nextUp?.let { if (it.palette == null) it.copy(palette = of(it.cover)) else it },
            recentAlbums = snapshot.recentAlbums.map { if (it.palette == null) it.copy(palette = of(it.cover)) else it },
        )
    }
}
