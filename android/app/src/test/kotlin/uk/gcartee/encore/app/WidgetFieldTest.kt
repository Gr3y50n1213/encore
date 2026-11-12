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

    @Test
    fun aFadeMovesTheStopsTowardTheBase() {
        val faded = WidgetField.stops(golden, WidgetField.STALE_FADE)
        // Distance in color, not contrast: a stop already as bright as the base keeps its contrast.
        fun distance(a: Int, b: Int): Int =
            listOf(16, 8, 0).sumOf { s -> Math.abs(((a shr s) and 0xFF) - ((b shr s) and 0xFF)) }
        for ((stop, f) in golden.stops.zip(faded)) {
            assertTrue(
                "${Integer.toHexString(stop)} fades toward ${Integer.toHexString(golden.base)}",
                distance(f, golden.base) < distance(stop, golden.base),
            )
        }
        assertEquals("no fade changes nothing", golden.stops, WidgetField.stops(golden, 0f))
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
