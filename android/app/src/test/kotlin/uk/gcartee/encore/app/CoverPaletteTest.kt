package uk.gcartee.encore.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.widgets.CoverPalette
import uk.gcartee.encore.app.widgets.WidgetField

/**
 * The stand-in palette a widget estimates from its cover until the core's arrives (12 November
 * 2026): quadrant colors in the field's blob places, white text that always reads.
 */
class CoverPaletteTest {
    private val size = 16

    /** A [size] x [size] picture whose four quadrants are the given colors (top left, top right, bottom left, bottom right). */
    private fun quadrants(tl: Int, tr: Int, bl: Int, br: Int): IntArray = IntArray(size * size) { i ->
        val x = i % size
        val y = i / size
        when {
            y < size / 2 -> if (x < size / 2) tl else tr
            else -> if (x < size / 2) bl else br
        }
    }

    @Test
    fun theFourQuadrantsBecomeTheFourStopsInTheBlobsPlaces() {
        val p = CoverPalette.estimate(quadrants(0xFF8B0000.toInt(), 0xFF00008B.toInt(), 0xFF006400.toInt(), 0xFF8B8B00.toInt()), size)
        assertNotNull(p)
        p!!
        assertTrue(p.dark)
        assertEquals(0xFFFFFFFF.toInt(), p.text)
        assertEquals(4, p.stops.size)
        fun ch(c: Int, shift: Int) = (c shr shift) and 0xFF
        assertTrue("top left is red", ch(p.stops[0], 16) > ch(p.stops[0], 8) && ch(p.stops[0], 16) > ch(p.stops[0], 0))
        assertTrue("top right is blue", ch(p.stops[1], 0) > ch(p.stops[1], 16))
        assertTrue("bottom left is green", ch(p.stops[2], 8) > ch(p.stops[2], 16))
        assertTrue("bottom right is yellow", ch(p.stops[3], 16) > 0x40 && ch(p.stops[3], 8) > 0x40 && ch(p.stops[3], 0) < 0x20)
    }

    @Test
    fun whiteTextReadsOnEveryStopEvenOfALightCover() {
        val p = CoverPalette.estimate(quadrants(0xFFFFFFFF.toInt(), 0xFFF5E6B8.toInt(), 0xFFB8E0F5.toInt(), 0xFFFFC0CB.toInt()), size)!!
        for (s in p.stops) assertTrue("${Integer.toHexString(s)}: ${WidgetField.contrast(p.text, s)}", WidgetField.contrast(p.text, s) >= 4.5)
        assertTrue(WidgetField.contrast(p.text, p.base) >= 4.5)
        // And on the whole field made from it.
        assertTrue(WidgetField.pixels(p).minOf { WidgetField.contrast(p.text, it) } >= 4.5)
    }

    @Test
    fun aTooSmallOrEmptySampleGivesNothing() {
        assertNull(CoverPalette.estimate(IntArray(0), 16))
        assertNull(CoverPalette.estimate(IntArray(1), 1))
    }
}
