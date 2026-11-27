package uk.gcartee.encore.app

import java.io.File
import java.time.LocalDate
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.recap.GroundKind
import uk.gcartee.encore.app.recap.MathCount
import uk.gcartee.encore.app.recap.PackColours
import uk.gcartee.encore.app.recap.PackId
import uk.gcartee.encore.app.recap.RecapColour
import uk.gcartee.encore.app.recap.RecapFormat
import uk.gcartee.encore.app.recap.RecapPalette
import uk.gcartee.encore.app.recap.RecapRules
import uk.gcartee.encore.app.recap.SongKey
import uk.gcartee.encore.app.recap.parseRecap
import uk.gcartee.encore.tokens.EncoreTokens.Recap.Taps

/** Phase 11: the story's taps and timing, the season window, the song key and the colours. */
class RecapRulesTest {
    @Test
    fun tapsGoBackOnTheLeftThirdAndForwardElsewhere() {
        val w = 1000f
        assertEquals(35, Taps.backZonePercent)
        assertEquals(RecapRules.Tap.BACK, RecapRules.tapZone(0f, w))
        assertEquals(RecapRules.Tap.BACK, RecapRules.tapZone(349f, w))
        assertEquals(RecapRules.Tap.FORWARD, RecapRules.tapZone(350f, w))
        assertEquals(RecapRules.Tap.FORWARD, RecapRules.tapZone(999f, w))
        assertEquals(RecapRules.Tap.FORWARD, RecapRules.tapZone(10f, 0f))
    }

    @Test
    fun theFirstTapAfterACardIsIgnored() {
        assertFalse(RecapRules.tapCounts(0))
        assertFalse(RecapRules.tapCounts(Taps.firstTapIgnoredMs - 1L))
        assertTrue(RecapRules.tapCounts(Taps.firstTapIgnoredMs.toLong()))
    }

    @Test
    fun cardsStayForTheirSecondsWithinTheRange() {
        assertEquals(7_000L, RecapRules.cardMillis(7))
        assertEquals(9_000L, RecapRules.cardMillis(9))
        assertEquals(6_000L, RecapRules.cardMillis(0))
        assertEquals(9_000L, RecapRules.cardMillis(60))
        assertEquals(2_000L, RecapRules.REVEAL_REST_MS)
    }

    @Test
    fun itResumesWithinThirtyDays() {
        val day = 24L * 60 * 60 * 1000
        val now = 100L * day
        assertEquals(4, RecapRules.resumeIndex(4, now - day, now, finished = false, count = 12))
        assertEquals(0, RecapRules.resumeIndex(4, now - 31 * day, now, finished = false, count = 12))
        assertEquals(0, RecapRules.resumeIndex(4, now - day, now, finished = true, count = 12))
        assertEquals(2, RecapRules.resumeIndex(9, now, now, finished = false, count = 3))
        assertEquals(0, RecapRules.resumeIndex(3, now + day, now, finished = false, count = 12))
        assertEquals(0, RecapRules.resumeIndex(3, now, now, finished = false, count = 0))
    }

    @Test
    fun homeOffersTheYearFromDecemberToTheEighthOfJanuary() {
        assertNull(RecapRules.seasonYear(LocalDate.of(2026, 11, 30)))
        assertEquals(2026, RecapRules.seasonYear(LocalDate.of(2026, 12, 1)))
        assertEquals(2026, RecapRules.seasonYear(LocalDate.of(2026, 12, 31)))
        assertEquals(2026, RecapRules.seasonYear(LocalDate.of(2027, 1, 1)))
        assertEquals(2026, RecapRules.seasonYear(LocalDate.of(2027, 1, 8)))
        assertNull(RecapRules.seasonYear(LocalDate.of(2027, 1, 9)))
        assertNull(RecapRules.seasonYear(LocalDate.of(2027, 6, 1)))
        assertTrue(RecapRules.inFinalRun(LocalDate.of(2027, 1, 3)))
        assertFalse(RecapRules.inFinalRun(LocalDate.of(2026, 12, 3)))
    }

    @Test
    fun theSeasonsTurnRoundInTheSouth() {
        assertTrue(RecapRules.southern("AU", "Europe/London"))
        assertTrue(RecapRules.southern("gb", "Australia/Sydney"))
        assertTrue(RecapRules.southern("NZ", "UTC"))
        assertFalse(RecapRules.southern("GB", "Europe/London"))
        assertFalse(RecapRules.southern("", "America/New_York"))
    }

    @Test
    fun numbersAndDatesAsTheCardsWriteThem() {
        assertEquals("12 am", RecapFormat.hour(0))
        assertEquals("11 am", RecapFormat.hour(11))
        assertEquals("12 pm", RecapFormat.hour(12))
        assertEquals("11 pm", RecapFormat.hour(23))
        assertEquals("12a", RecapFormat.hourShort(24))
        assertEquals("6p", RecapFormat.hourShort(18))
        assertEquals(25L, RecapFormat.percent(251))
        assertEquals(26L, RecapFormat.percent(255))
        assertEquals("Mar 2", RecapFormat.day("2026-03-02"))
        assertEquals("not a day", RecapFormat.day("not a day"))
        assertEquals("6 h 41 m", RecapFormat.hoursMinutes((6 * 60 + 41) * 60_000L))
        assertEquals("41 m", RecapFormat.hoursMinutes(41 * 60_000L))
        assertEquals("Tuesday", RecapFormat.weekday(1))
        assertEquals("25%", RecapFormat.mathValue(MathCount("Share", 251, "permille")))
        assertEquals("1 play", RecapFormat.mathValue(MathCount("Plays", 1, "plays")))
    }

    @Test
    fun theSongKeyIsTheCoresOwn() {
        // core/encore-sources/src/song_key.rs, fallback_key_is_frozen.
        assertEquals("t1:5a0ab3d41d0b7ae62cddb08fa411c1f1:222", SongKey.t1("Ember Cove", "Golden Hour Static", 222_400))
        assertEquals(SongKey.t1("Ember Cove", "Golden Hour Static", 222_400), SongKey.t1("ember  cove ", "GOLDEN HOUR STATIC", 222_100))
        assertEquals(SongKey.t1("Beyoncé", "Halo", 261_000), SongKey.t1("Beyoncé", "Halo", 261_000))
        assertEquals("abc", SongKey.normalize("ＡＢＣ"))
        assertTrue(SongKey.same("t1:aa:200", "t1:aa:202"))
        assertFalse(SongKey.same("t1:aa:200", "t1:aa:203"))
        assertFalse(SongKey.same("t1:aa:200", "t1:bb:200"))
        assertFalse(SongKey.same("mb:x", "mb:y"))
        assertNull(SongKey.split("mb:5b11f4ce"))
    }

    @Test
    fun contrastIsMeasuredTheWcagWay() {
        assertEquals(21.0, RecapColour.contrast(RecapColour.WHITE, 0xFF000000.toInt()), 0.01)
        assertEquals(1.0, RecapColour.contrast(0xFF777777.toInt(), 0xFF777777.toInt()), 0.0001)
        val c = 0xFF3A7BD5.toInt()
        val back = RecapColour.hsl(c).let { RecapColour.fromHsl(it[0], it[1], it[2]) }
        assertTrue(abs(RecapColour.red(c) - RecapColour.red(back)) <= 1 && abs(RecapColour.blue(c) - RecapColour.blue(back)) <= 1)
    }

    private fun abs(i: Int) = if (i < 0) -i else i

    private fun palettes(): List<RecapPalette?> {
        val fromGoldens = listOf("heavy", "light", "cleared", "so-far").mapNotNull { name ->
            javaClass.getResource("/recap/$name.expected.json")?.readText()?.let { parseRecap(it).palette }
        }
        val dir = File("../../core/encore-recap/tests/golden")
        val more = if (dir.isDirectory) dir.listFiles { f -> f.name.endsWith(".expected.json") }.orEmpty().mapNotNull { parseRecap(it.readText()).palette } else emptyList()
        // Extremes too: very light, very dark and grey art, and none at all.
        val extremes = listOf(
            RecapPalette("albums", listOf("#ffffff", "#fefefe", "#f0f0f0"), "#ffffff", 0, null),
            RecapPalette("albums", listOf("#000000", "#050505", "#101010"), "#000000", 0, null),
            RecapPalette("albums", listOf("#808080", "#7f7f7f"), "#808080", 0, null),
            RecapPalette("albums", listOf("#ffff00", "#00ffff", "#ff00ff", "#00ff00"), "#ffff00", 60, null),
        )
        return fromGoldens + more + extremes + listOf(null)
    }

    @Test
    fun everyPackPassesTheTextContrastCheck() {
        val min = RecapColour.MIN_CONTRAST
        for (p in palettes()) for (pack in PackId.entries) {
            val colours = PackColours(pack, p)
            for (kind in GroundKind.entries) {
                val g = colours.ground(kind)
                val where = "$pack $kind ${p?.colours}"
                for (bg in listOf(g.top, g.bottom)) {
                    assertTrue("text on $where", RecapColour.contrast(g.fg, bg) >= min)
                    assertTrue("accent text on $where", RecapColour.contrast(g.accentText, bg) >= min)
                }
            }
            when (pack) {
                // Ink on Bloom's colour fields, white on Tide's waves and through Glass's lens.
                PackId.BLOOM -> colours.fields.forEach { assertTrue("ink on field $it ${p?.colours}", RecapColour.contrast(it, colours.bloomInk) >= min) }
                PackId.TIDE -> colours.waves.forEach { assertTrue("white on wave ${p?.colours}", RecapColour.contrast(it, RecapColour.WHITE) >= min) }
                PackId.GLASS -> colours.glows.forEach {
                    assertTrue("white on lens ${p?.colours}", RecapColour.contrast(RecapColour.mix(it, RecapColour.WHITE, PackColours.LENS_WHITE), RecapColour.WHITE) >= min)
                }
                PackId.EDITORIAL -> assertTrue("ink on spot ${p?.colours}", RecapColour.contrast(colours.spot, colours.ink) >= min)
            }
        }
    }

    @Test
    fun groundsFollowTheBoardAndTurnWithTheLayout() {
        assertEquals(GroundKind.SPOT, PackColours.groundFor("intro", 0))
        assertEquals(GroundKind.INK, PackColours.groundFor("topSong", 0))
        assertEquals(GroundKind.PAPER, PackColours.groundFor("minutes", 0))
        assertEquals(GroundKind.PAPER2, PackColours.groundFor("yourSound", 0))
        assertEquals(GroundKind.PAPER, PackColours.groundFor("closer", 3))
        assertTrue(PackColours.groundFor("minutes", 1) != GroundKind.PAPER)
    }

    @Test
    fun days_and_hours_leave_out_a_nought() {
        assertEquals("15 hours", uk.gcartee.encore.app.recap.RecapFormat.daysAndHours(0, 15, " · "))
        assertEquals("1 day · 3 hours", uk.gcartee.encore.app.recap.RecapFormat.daysAndHours(1, 3, " · "))
        assertEquals("2 days", uk.gcartee.encore.app.recap.RecapFormat.daysAndHours(2, 0, " and "))
    }
}
