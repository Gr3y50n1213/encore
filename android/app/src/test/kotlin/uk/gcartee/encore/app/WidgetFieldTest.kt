package uk.gcartee.encore.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.widgets.SnapshotPalette
import uk.gcartee.encore.app.widgets.WidgetField

/**
 * The album field a widget wears (widgets redesign, 11 November 2026): the app's Now Playing
 * gradient as pixels, from the palette in the snapshot. Text and buttons sit on it in the
 * palette's own text color, so the text has to read on every pixel of it, in the dark
 * palette the app's artwork fixture makes and in a light one.
 */
class WidgetFieldTest {
    // Golden Hour, the app's artwork fixture (CORE-API, "Widgets", the example).
    private val golden = SnapshotPalette(
        dark = true,
        base = 0xFF3D0000.toInt(),
        stops = listOf(0xFFCF4630.toInt(), 0xFF1B1030.toInt(), 0xFFC02D56.toInt(), 0xFF986E00.toInt()),
        text = 0xFFFFFFFF.toInt(),
    )

    // Made up: light art, a light field with dark text.
    private val daylight = SnapshotPalette(
        dark = false,
        base = 0xFFEADFC6.toInt(),
        stops = listOf(0xFFF4B860.toInt(), 0xFF9CC5B4.toInt(), 0xFFE49BB1.toInt(), 0xFFF1DE8A.toInt()),
        text = 0xFF231B10.toInt(),
    )

    private fun lowestContrast(p: SnapshotPalette, fade: Float): Double =
        WidgetField.pixels(p, fade).minOf { WidgetField.contrast(p.text, it) }

    @Test
    fun theTextReadsOnEveryPixelOfADarkField() {
        assertTrue("dark field: ${lowestContrast(golden, 0f)}", lowestContrast(golden, 0f) >= 4.5)
    }

    @Test
    fun theTextReadsOnEveryPixelOfALightField() {
        assertTrue("light field: ${lowestContrast(daylight, 0f)}", lowestContrast(daylight, 0f) >= 4.5)
    }

    @Test
    fun aStaleOrSoftenedFieldStillReads() {
        for (p in listOf(golden, daylight)) for (fade in listOf(WidgetField.SOFT_FADE, WidgetField.STALE_FADE)) {
            assertTrue("fade $fade: ${lowestContrast(p, fade)}", lowestContrast(p, fade) >= 4.5)
        }
    }

    @Test
    fun theFieldIsOpaqueWithTheRightNumberOfPixels() {
        val pixels = WidgetField.pixels(golden)
        assertEquals(WidgetField.SIZE_PX * WidgetField.SIZE_PX, pixels.size)
        assertTrue(pixels.all { (it ushr 24) == 0xFF })
    }

    @Test
    fun theFieldHasTheAlbumsColorsInTheAppsPlaces() {
        val n = WidgetField.SIZE_PX
        val pixels = WidgetField.pixels(golden)
        fun at(rx: Float, ry: Float) = pixels[(ry * (n - 1)).toInt() * n + (rx * (n - 1)).toInt()]
        // Near the first blob's anchor (0.2, 0.18) the leading red wins; near the last (0.8, 0.92) the amber.
        val red = at(0.28f, 0.18f)
        assertTrue("red leads at the top left: ${Integer.toHexString(red)}", ((red shr 16) and 0xFF) > 0x90)
        val amber = at(0.8f, 0.92f)
        assertTrue("amber at the bottom right: ${Integer.toHexString(amber)}", ((amber shr 8) and 0xFF) > 0x40 && (amber and 0xFF) < 0x30)
        // The field is not flat.
        assertTrue(pixels.distinct().size > 1000)
    }

    // Widget shapes (dp): One UI's 4x1, a tall 2x2, a phone's 4x2, a 6x1 strip.
    private val shapes = listOf(326f to 108f, 158f to 226f, 276f to 220f, 412f to 51f)

    @Test
    fun theFieldTakesTheWidgetsOwnShapeSoNothingIsStretched() {
        val (w, h) = WidgetField.shape(326f, 108f)
        assertEquals(WidgetField.LONG_PX, w)
        // Same shape as the widget, within a pixel of rounding.
        assertEquals(108f / 326f, h.toFloat() / w, 0.01f)
        val (tw, th) = WidgetField.shape(158f, 226f)
        assertEquals(WidgetField.LONG_PX, th)
        assertEquals(158f / 226f, tw.toFloat() / th, 0.02f)
        // A very thin strip keeps a few rows.
        assertTrue(WidgetField.shape(412f, 20f).second >= 8)
    }

    @Test
    fun theTextReadsOnEveryShapeOfEveryField() {
        for (p in listOf(golden, daylight)) for ((w, h) in shapes) for (fade in listOf(0f, WidgetField.SOFT_FADE, WidgetField.STALE_FADE)) {
            val (pixels, _, _) = WidgetField.pixelsFor(WidgetField.Spec(p, fade), w, h)
            val lowest = pixels.minOf { WidgetField.contrast(p.text, it) }
            assertTrue("${w}x$h fade $fade: $lowest", lowest >= 4.5)
        }
    }

    @Test
    fun aWideFieldStillHasTheAlbumsColorsLeftAndRight() {
        // Round blobs on a 4x1 strip: the red leads on the left, the dark blue-violet on the right; not one smear.
        val (pixels, w, h) = WidgetField.pixelsFor(WidgetField.Spec(golden, 0f), 326f, 108f)
        fun at(rx: Float, ry: Float) = pixels[(ry * (h - 1)).toInt() * w + (rx * (w - 1)).toInt()]
        val left = at(0.2f, 0.3f)
        val right = at(0.88f, 0.3f)
        assertTrue("left ${Integer.toHexString(left)} is redder than right ${Integer.toHexString(right)}", ((left shr 16) and 0xFF) > ((right shr 16) and 0xFF) + 0x20)
        assertTrue(pixels.distinct().size > 500)
    }

    @Test
    fun aFadeMovesTheStopsTowardTheBase() {
        val unfaded = WidgetField.stops(golden, 0f)
        val faded = WidgetField.stops(golden, WidgetField.STALE_FADE)
        val base = WidgetField.baseOf(golden)
        // Distance in color, not contrast: a stop already as bright as the base keeps its contrast.
        fun distance(a: Int, b: Int): Int =
            listOf(16, 8, 0).sumOf { s -> Math.abs(((a shr s) and 0xFF) - ((b shr s) and 0xFF)) }
        for ((stop, f) in unfaded.zip(faded)) {
            assertTrue(
                "${Integer.toHexString(stop)} fades toward ${Integer.toHexString(base)}",
                distance(f, base) < distance(stop, base),
            )
        }
    }

    @Test
    fun aDarkAlbumsFieldIsLiftedOutOfFlatGreyButABrightOneIsLeftAlone() {
        // Bright stops are untouched.
        assertEquals(0xFFCF4630.toInt(), WidgetField.lifted(0xFFCF4630.toInt(), 0.30f))
        // A near-black brown comes up to value 0.30 and keeps its hue (red still leads).
        val up = WidgetField.lifted(0xFF1A0E08.toInt(), 0.30f)
        val r = (up shr 16) and 0xFF
        val g = (up shr 8) and 0xFF
        val b = up and 0xFF
        assertTrue("value lifted to 0.30: ${Integer.toHexString(up)}", Math.abs(r - 0x4C) <= 2)
        assertTrue("hue kept: ${Integer.toHexString(up)}", r > g && g > b)
        // A true grey stays grey.
        val grey = WidgetField.lifted(0xFF101010.toInt(), 0.30f)
        assertEquals((grey shr 16) and 0xFF, grey and 0xFF)
        // A near-black album's field has real range, not one flat tone, and its text still reads.
        val dull = SnapshotPalette(
            dark = true,
            base = 0xFF0C0A0A.toInt(),
            stops = listOf(0xFF1A0E08.toInt(), 0xFF0B0B10.toInt(), 0xFF140C0C.toInt(), 0xFF1E1408.toInt()),
            text = 0xFFFFFFFF.toInt(),
        )
        val pixels = WidgetField.pixels(dull)
        assertTrue("brightest pixel is lifted: ${Integer.toHexString(pixels.maxBy { it and 0xFF00 })}", pixels.any { ((it shr 16) and 0xFF) >= 0x30 })
        assertTrue(pixels.minOf { WidgetField.contrast(dull.text, it) } >= 4.5)
    }

    @Test
    fun theBlendAndTheContrastMathAreRight() {
        assertEquals(0xFF000000.toInt(), WidgetField.blend(0xFF000000.toInt(), 0xFFFFFFFF.toInt(), 0f))
        assertEquals(0xFFFFFFFF.toInt(), WidgetField.blend(0xFF000000.toInt(), 0xFFFFFFFF.toInt(), 1f))
        assertEquals(0xFF808080.toInt(), WidgetField.blend(0xFF000000.toInt(), 0xFFFFFFFF.toInt(), 0.5f))
        assertEquals(21.0, WidgetField.contrast(0xFFFFFFFF.toInt(), 0xFF000000.toInt()), 0.01)
        assertEquals(1.0, WidgetField.contrast(0xFF123456.toInt(), 0xFF123456.toInt()), 0.0001)
    }
}
