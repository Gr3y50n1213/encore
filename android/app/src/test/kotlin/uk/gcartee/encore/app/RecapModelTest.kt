package uk.gcartee.encore.app

import java.io.File
import java.time.ZoneOffset
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.recap.BeatData
import uk.gcartee.encore.app.recap.Recap
import uk.gcartee.encore.app.recap.RecapFormat
import uk.gcartee.encore.app.recap.RecapKind
import uk.gcartee.encore.app.recap.RecapText
import uk.gcartee.encore.app.recap.parseRecap

/**
 * Phase 11: the core's golden recap documents (core/encore-recap/tests/golden, copied to
 * test resources) read into the app's types, every beat known, and a newer document's
 * unknown fields and beat types ignored.
 */
class RecapModelTest {
    private fun golden(name: String): String =
        requireNotNull(javaClass.getResource("/recap/$name.expected.json")) { "no golden $name" }.readText()

    private fun read(name: String): Recap = parseRecap(golden(name))

    @Test
    fun theFullStoryReadsWhole() {
        val r = read("heavy")
        assertEquals(1, r.schema)
        assertEquals(2026, r.year)
        assertEquals(RecapKind.FULL, r.kind)
        assertEquals("editorial", r.pack?.id)
        assertFalse(r.soFar)
        assertEquals(6, r.palette?.colours?.size)
        assertEquals(listOf("intro", "minutes", "topSong", "topArtist", "topAlbum", "yourSound", "moodArc", "longestStreak", "biggestDiscovery", "mostSungLine", "personality", "closer"), r.beats.map { it.id })
        assertTrue("every beat known", r.beats.none { it.data is BeatData.Unknown })
        assertEquals(r.beats.size, r.story.size)
        val minutes = r.beats[1].data as BeatData.Minutes
        assertEquals(4466L, minutes.minutes)
        assertEquals(12, minutes.months.size)
        assertEquals(54L, minutes.changePercent)
        val top = r.beats[2].data as BeatData.TopSong
        assertEquals(5, top.top.size)
        assertEquals(82L, top.top.first().plays)
        val sound = r.beats[5].data as BeatData.YourSound
        assertEquals(12, sound.months.size)
        assertEquals("electronic", sound.drift?.genre)
        val closer = r.beats.last().data as BeatData.Closer
        assertEquals(100L, closer.count)
        assertEquals(100, closer.songKeys.size)
        assertNotNull(r.detail)
        assertEquals(50, r.detail?.topSongs?.size)
        assertTrue(r.beats.all { it.math.rule.isNotBlank() })
    }

    @Test
    fun theShortAndTheEmptyStories() {
        val light = read("light")
        assertEquals(RecapKind.SHORT, light.kind)
        assertEquals(listOf("minutes", "topSong", "topArtist"), light.story.map { it.id })
        assertNull(light.detail)
        val none = read("very-new")
        assertEquals(RecapKind.NONE, none.kind)
        assertEquals("notEnoughPlays", none.noneReason)
        assertTrue(none.story.isEmpty())
        assertNull(none.pack)
    }

    @Test
    fun sinceAndSoFar() {
        val cleared = read("cleared")
        assertEquals("2026-08-03", cleared.since?.day)
        assertEquals("cleared", cleared.since?.reason)
        assertEquals("Aug 3", RecapFormat.day(cleared.since!!.day))
        val soFar = read("so-far")
        assertTrue(soFar.soFar)
        assertFalse(soFar.locked)
        // Counted through a December day, never past the year's end.
        assertTrue(RecapFormat.countedThrough(soFar, ZoneOffset.UTC).startsWith("Dec "))
        assertEquals("Dec 31", RecapFormat.countedThrough(read("heavy"), ZoneOffset.UTC))
    }

    @Test
    fun everyCardHasATextVersion() {
        for (name in listOf("heavy", "light", "cleared", "so-far")) {
            val r = read(name)
            r.story.forEachIndexed { i, b ->
                val text = RecapText.describe(r, b, i, r.story.size, "Dec 31")
                assertTrue("$name ${b.id}: $text", text.startsWith("Card ${i + 1} of ${r.story.size}."))
                assertTrue("$name ${b.id} says the date", text.contains("Counted through Dec 31."))
            }
        }
    }

    @Test
    fun namesCanBeHiddenFromAShare() {
        val r = read("heavy")
        val names = RecapText.names(r)
        assertTrue("Staefund" in names)
        assertTrue("Klonugu Rumaepir" in names)
        val line = r.beats.first { it.id == "mostSungLine" }.text
        val masked = RecapText.mask(line, names)
        assertFalse(masked.contains("Staefund"))
        assertTrue(masked.contains(RecapText.HIDDEN))
    }

    @Test
    fun aNewerCoreDoesNotBreakTheApp() {
        val json = """
            {"schema":2,"engine":3,"year":2031,"kind":"epic","soFar":true,"locked":false,
             "countedThroughUtcMs":1,"futureField":{"a":[1,2.5e3,null,true]},
             "pack":{"id":"contour","version":1},"seed":"00","palette":null,"yearLine":"A \"quoted\" line é",
             "totals":{"plays":10,"listenedMs":0,"minutes":0,"songs":1,"artists":1,"albums":1,"activeDays":1},
             "beats":[
               {"id":"householdDeepCut","role":"surprise","fixed":false,"seconds":7,"layout":0,"layoutCount":3,"shapeSeed":1,"template":"x#0","text":"New.","data":{"type":"householdDeepCut","x":1},"math":{"counts":[],"rule":""}},
               {"id":"longestStreak","role":"surprise","fixed":false,"seconds":7,"layout":1,"layoutCount":3,"shapeSeed":2,"template":"s#0","text":"8 days.","newBeatField":1,"data":{"type":"longestStreak","days":8,"firstDay":"2031-01-01","lastDay":"2031-01-08","extra":[]},"math":{"counts":[{"label":"Days","value":8,"unit":"days","more":0}],"rule":"r"}},
               {"id":"topSong","role":"bigNumbers","fixed":true,"seconds":9,"layout":0,"layoutCount":4,"shapeSeed":3,"template":"t#0","text":"Broken.","data":{"type":"topSong"},"math":{"counts":[],"rule":""}}
             ],
             "detail":null,"logHash":"ab"}
        """.trimIndent()
        val r = parseRecap(json)
        assertEquals(RecapKind.FULL, r.kind)
        assertEquals("A \"quoted\" line é", r.yearLine)
        assertEquals(3, r.beats.size)
        assertTrue(r.beats[0].data is BeatData.Unknown)
        assertTrue("a known type with its numbers missing is left out", r.beats[2].data is BeatData.Unknown)
        assertEquals(listOf("longestStreak"), r.story.map { it.id })
        assertEquals(8L, (r.story[0].data as BeatData.LongestStreak).days)
    }

    @Test
    fun everyGoldenTheCoreHasReads() {
        // The core's own folder, when the tests run inside the repository (android/app).
        val dir = File("../../core/encore-recap/tests/golden")
        if (!dir.isDirectory) return
        val files = dir.listFiles { f -> f.name.endsWith(".expected.json") }.orEmpty()
        assertTrue(files.isNotEmpty())
        for (f in files) {
            val r = parseRecap(f.readText())
            assertTrue("${f.name}: every beat known", r.beats.none { it.data is BeatData.Unknown })
            assertEquals(f.name, r.kind == RecapKind.NONE, r.story.isEmpty())
        }
    }
}
