package uk.gcartee.encore.wearlink

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Stage W1: the phone sends the watch at most two updates a second, and always the last one. */
class ThrottleTest {
    @Test
    fun theFirstGoesAtOnceThenTheRestOfTheIntervalIsWaited() {
        val t = Throttle(500)
        assertEquals(0, t.delayFor(10_000))
        t.sent(10_000)
        assertEquals(500, t.delayFor(10_000))
        assertEquals(300, t.delayFor(10_200))
        assertEquals(0, t.delayFor(10_500))
        assertEquals(0, t.delayFor(20_000))
    }

    @Test
    fun aClockThatGoesBackNeverWaitsLongerThanTheInterval() {
        val t = Throttle(500)
        t.sent(10_000)
        assertEquals(500, t.delayFor(5_000))
    }

    /**
     * WearLink's send loop, with the time made up: a change every 10 ms for 3 s. Each send
     * waits as [Throttle.delayFor] says, then everything that came in while it waited goes
     * as one update.
     */
    @Test
    fun aBurstOfChangesIsAtMostTwoASecondAndTheLastIsSent() {
        val t = Throttle(500)
        val changes = (0 until 300).map { it * 10L }
        val sends = mutableListOf<Long>()
        var clock = 0L
        var i = 0
        var lastSeen = -1L
        while (i < changes.size) {
            clock = maxOf(clock, changes[i])
            clock += t.delayFor(clock)
            // Everything that changed up to now is in this one update.
            while (i < changes.size && changes[i] <= clock) lastSeen = changes[i++]
            t.sent(clock)
            sends += clock
        }
        assertEquals(changes.last(), lastSeen)
        for (second in 0 until 3) {
            val inSecond = sends.count { it >= second * 1_000L && it < (second + 1) * 1_000L }
            assertTrue("$inSecond sends in second $second", inSecond <= 2)
        }
        assertTrue(sends.zipWithNext().all { (a, b) -> b - a >= 500 })
    }
}
