package uk.gcartee.encore.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test
import uk.gcartee.encore.app.widgets.SnapshotRead
import uk.gcartee.encore.app.widgets.SnapshotReader

/**
 * The album colors in a widget's snapshot (CORE-API, "Widgets", Colors): read strictly, and
 * anything malformed is no palette, never an unreadable file. Instrumented, because the
 * real org.json is on the device.
 */
class SnapshotPaletteTest {
    private val profile = "jf-0123456789abcdef"
    private val golden = """{"scheme":"dark","base":"3D0000","stops":["CF4630","1B1030","C02D56","986E00"],"text":"FFFFFF"}"""

    private fun file(song: String, nextUp: String = "null", albums: String = "[]") =
        """{"schema":1,"profileId":"$profile","privacy":"normal","song":$song,"isPlaying":false,""" +
            """"positionMs":0,"positionAtUtcMs":0,"pinnedMixes":[],"nextUp":$nextUp,"recentAlbums":$albums,""" +
            """"reachable":true,"writtenAtUtcMs":1,"recap":null}"""

    private fun song(palette: String?) =
        """{"songId":1,"title":"Slow Tide","artist":"Ember Cove","albumId":2,"albumTitle":"Golden Hour Static",""" +
            """"cover":null,${if (palette == null) "" else "\"palette\":$palette,"}"durationMs":1000,"nowPlaying":false}"""

    private fun read(text: String) = (SnapshotReader.parse(text, profile) as SnapshotRead.Ok).snapshot

    @Test
    fun aValidPaletteIsRead() {
        val p = read(file(song(golden))).song!!.palette!!
        assertEquals(true, p.dark)
        assertEquals(0xFF3D0000.toInt(), p.base)
        assertEquals(listOf(0xFFCF4630.toInt(), 0xFF1B1030.toInt(), 0xFFC02D56.toInt(), 0xFF986E00.toInt()), p.stops)
        assertEquals(0xFFFFFFFF.toInt(), p.text)
    }

    @Test
    fun aLightSchemeIsNotDark() {
        val light = """{"scheme":"light","base":"EADFC6","stops":["F4B860","9CC5B4","E49BB1","F1DE8A"],"text":"231B10"}"""
        val p = read(file(song(light))).song!!.palette!!
        assertEquals(false, p.dark)
        assertEquals(0xFF231B10.toInt(), p.text)
    }

    @Test
    fun lowerCaseHexIsFine() {
        val lower = """{"scheme":"dark","base":"3d0000","stops":["cf4630","1b1030","c02d56","986e00"],"text":"ffffff"}"""
        assertNotNull(read(file(song(lower))).song!!.palette)
    }

    @Test
    fun noPaletteKeyOrNullIsNoPalette() {
        assertNull(read(file(song(null))).song!!.palette)
        assertNull(read(file(song("null"))).song!!.palette)
    }

    @Test
    fun anythingMalformedIsNoPaletteAndTheFileStillReads() {
        val bad = listOf(
            // not an object
            "\"x\"", "5", "[]",
            // a bad hex, a short hex, a missing hash-free but wrong-length one
            """{"scheme":"dark","base":"GG0000","stops":["CF4630","1B1030","C02D56","986E00"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"3D000","stops":["CF4630","1B1030","C02D56","986E00"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"#3D0000","stops":["CF4630","1B1030","C02D56","986E00"],"text":"FFFFFF"}""",
            // three stops, five stops, stops of the wrong type
            """{"scheme":"dark","base":"3D0000","stops":["CF4630","1B1030","C02D56"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"3D0000","stops":["CF4630","1B1030","C02D56","986E00","111111"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"3D0000","stops":"CF4630","text":"FFFFFF"}""",
            """{"scheme":"dark","base":"3D0000","stops":["CF4630",null,"C02D56","986E00"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"3D0000","stops":[1,2,3,4],"text":"FFFFFF"}""",
            // missing parts
            """{"scheme":"dark","stops":["CF4630","1B1030","C02D56","986E00"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"3D0000","stops":["CF4630","1B1030","C02D56","986E00"]}""",
        )
        for (palette in bad) {
            val snapshot = read(file(song(palette)))
            assertEquals("the song still reads: $palette", "Slow Tide", snapshot.song!!.title)
            assertNull("no palette: $palette", snapshot.song!!.palette)
        }
    }

    @Test
    fun mixesAndAlbumsCarryTheirOwn() {
        val mix = """{"mixId":"mix-1","title":"Evening Drive","cover":null,"palette":$golden,"pinned":false}"""
        val album = """{"albumId":2,"title":"Golden Hour Static","artist":"Ember Cove","cover":null,"palette":$golden}"""
        val noColours = """{"albumId":3,"title":"Tidewater","artist":"Moth Choir","cover":null,"palette":null}"""
        val s = read(file(song(null), nextUp = mix, albums = "[$album,$noColours]"))
        assertEquals(0xFF3D0000.toInt(), s.nextUp!!.palette!!.base)
        assertEquals(0xFF3D0000.toInt(), s.recentAlbums[0].palette!!.base)
        assertNull(s.recentAlbums[1].palette)
        assertEquals("Tidewater", s.recentAlbums[1].title)
    }
}
