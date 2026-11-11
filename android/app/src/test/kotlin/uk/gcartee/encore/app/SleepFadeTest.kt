package uk.gcartee.encore.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.playback.SleepFade
import uk.gcartee.encore.app.playback.SleepTimerNotifier
import uk.gcartee.encore.core.SleepFadeAction
import uk.gcartee.encore.core.SleepFadePlan

/** Phase 9: the sleep timer's fade (CORE-API "Sleep timer"; board 31, "Fading"). */
class SleepFadeTest {
    private var now = 1_000_000L
    private val fade = SleepFade { now }

    private fun keep() = SleepFadePlan(SleepFadeAction.KEEP_PLAYING, 0L, 0L)
    private fun fadeNow(position: Long, ms: Long) = SleepFadePlan(SleepFadeAction.FADE_NOW, position, ms)
    private fun songEnd(from: Long, ms: Long) = SleepFadePlan(SleepFadeAction.STOP_AT_SONG_END, from, ms)

    @Test
    fun keepPlayingIsFullVolumeAndNeverStops() {
        // No timer, a song or album timer (they never fade), or the end not near yet.
        fade.update(keep(), 30_000L)
        assertFalse(fade.fading)
        assertEquals(1f, fade.level(30_000L), 0f)
        assertFalse(fade.timeIsUp(30_000L))
        assertFalse(fade.stopsAtSongEnd)
    }

    @Test
    fun fadeNowRampsToSilenceOverTenSecondsThenStops() {
        fade.update(fadeNow(60_000L, 10_000L), 60_000L)
        assertTrue(fade.fading)
        assertEquals(1f, fade.level(60_000L), 0.001f)
        var last = 1f
        for (step in 1..16) {
            now += 625L
            val level = fade.level(60_000L + step * 625L)
            assertTrue("the volume only goes down", level <= last)
            last = level
            if (step < 16) assertFalse(fade.timeIsUp(0L))
        }
        assertEquals(0f, last, 0.001f)
        assertTrue("silence reached: pause and end the timer", fade.timeIsUp(0L))
    }

    @Test
    fun halfwayThroughTheFadeIsHalfTheVolume() {
        fade.update(fadeNow(0L, 10_000L), 0L)
        now += 5_000L
        assertEquals(0.5f, fade.level(5_000L), 0.01f)
    }

    @Test
    fun theCoresSecondlyAnswerDoesNotRestartTheFade() {
        fade.update(fadeNow(0L, 10_000L), 0L)
        now += 4_000L
        val before = fade.level(4_000L)
        // A second later the core says what is left: the same end.
        fade.update(fadeNow(4_000L, 6_000L), 4_000L)
        assertEquals(before, fade.level(4_000L), 0.01f)
        now += 6_000L
        assertTrue(fade.timeIsUp(10_000L))
    }

    @Test
    fun aFadeOfNothingLeftStopsAtOnce() {
        fade.update(fadeNow(0L, 0L), 0L)
        assertTrue(fade.timeIsUp(0L))
    }

    @Test
    fun aPauseStopsTheFadeAndTheVolumeIsFullAgain() {
        fade.update(fadeNow(0L, 10_000L), 0L)
        now += 6_000L
        assertTrue(fade.level(6_000L) < 0.5f)
        fade.clear() // the player paused
        assertFalse(fade.fading)
        assertEquals(1f, fade.level(6_000L), 0f)
        assertFalse(fade.timeIsUp(6_000L))
    }

    @Test
    fun addFifteenMinutesDuringTheFadeStopsItAndTheVolumeIsFullAgain() {
        fade.update(fadeNow(0L, 10_000L), 0L)
        now += 7_000L
        assertTrue(fade.level(7_000L) < 0.5f)
        // The end moved 15 minutes on: the core says KeepPlaying.
        fade.update(keep(), 8_000L)
        assertFalse(fade.fading)
        assertEquals(1f, fade.level(8_000L), 0f)
        assertFalse(fade.stopsAtSongEnd)
    }

    @Test
    fun aSkipDuringTheFadeCarriesItOnFromTheVolumeReached() {
        fade.update(fadeNow(200_000L, 10_000L), 200_000L)
        now += 4_000L
        val reached = fade.level(204_000L)
        // Next: a new song at 0:00; the timer's end is the same moment.
        fade.update(fadeNow(0L, 6_000L), 0L)
        val after = fade.level(0L)
        assertTrue("not back to full after the skip ($after)", after <= reached + 0.001f)
        now += 3_000L
        assertTrue(fade.level(3_000L) < after)
        now += 3_000L
        assertTrue(fade.timeIsUp(6_000L))
    }

    @Test
    fun aSongThatEndsInsideTheWindowFadesToItsEndAndStopsThere() {
        // A 3-minute song whose end is 6 s before the timer's: full until 174 s, silent at 180 s.
        fade.update(songEnd(174_000L, 6_000L), 170_000L)
        assertTrue(fade.fading)
        assertTrue("don't start the next song", fade.stopsAtSongEnd)
        assertEquals(1f, fade.level(170_000L), 0f)
        assertEquals(1f, fade.level(174_000L), 0.001f)
        assertEquals(0.5f, fade.level(177_000L), 0.01f)
        assertEquals(0f, fade.level(180_000L), 0.001f)
        // The player stops at the song's end, not by the clock.
        now += 60_000L
        assertFalse(fade.timeIsUp(180_000L))
    }

    @Test
    fun aSongEndingDuringATimedFadeIsWhereTheMusicStops() {
        fade.update(fadeNow(0L, 10_000L), 0L)
        assertTrue(fade.stopsAtSongEnd)
    }

    @Test
    fun aSkipFromASongEndFadeIntoALongerSongFadesByTheClockFromWhereItWas() {
        fade.update(songEnd(174_000L, 6_000L), 174_000L)
        val reached = fade.level(177_000L)
        fade.update(fadeNow(0L, 3_000L), 0L)
        assertTrue(fade.level(0L) <= reached + 0.001f)
        now += 3_000L
        assertTrue(fade.timeIsUp(3_000L))
    }

    @Test
    fun theChipTextIsShort() {
        assertEquals("24m", SleepTimerNotifier.chipText(24 * 60_000L + 10_000L))
        assertEquals("<1m", SleepTimerNotifier.chipText(30_000L))
        assertEquals("1h", SleepTimerNotifier.chipText(60 * 60_000L))
        assertEquals("1h 5m", SleepTimerNotifier.chipText(65 * 60_000L))
        assertEquals("23h59m", SleepTimerNotifier.chipText(1_439 * 60_000L))
        for (minutes in 0L..1_440L) {
            assertTrue(SleepTimerNotifier.chipText(minutes * 60_000L).length <= 6)
        }
    }
}
