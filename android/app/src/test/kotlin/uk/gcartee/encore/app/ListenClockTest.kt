package uk.gcartee.encore.app

import org.junit.Assert.assertEquals
import org.junit.Test
import uk.gcartee.encore.app.playback.ListenClock

class ListenClockTest {
    private var t = 0L
    private val clock = ListenClock { t }

    @Test
    fun countsOnlyWhilePlaying() {
        clock.playing(true)
        t = 10_000
        clock.playing(false)
        t = 50_000 // paused: not counted
        assertEquals(10_000, clock.listenedMs)
        clock.playing(true)
        t = 55_000
        assertEquals(15_000, clock.listenedMs)
    }

    @Test
    fun repeatedStateReportsChangeNothing() {
        clock.playing(true)
        t = 1_000
        clock.playing(true)
        t = 2_000
        assertEquals(2_000, clock.listenedMs)
        clock.playing(false)
        clock.playing(false)
        assertEquals(2_000, clock.listenedMs)
    }

    @Test
    fun resetStartsTheNextPlayWithoutStoppingTheClock() {
        clock.playing(true)
        t = 30_000
        clock.reset()
        assertEquals(0, clock.listenedMs)
        t = 31_000
        assertEquals(1_000, clock.listenedMs)
        clock.reset(carriedMs = 30_000)
        t = 32_000
        assertEquals(31_000, clock.listenedMs)
    }
}
