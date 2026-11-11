package uk.gcartee.encore.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.notifications.LiveUpdates

/** Phase 9: the pure rules behind Live Updates (the chip text and who may be promoted). */
class LiveUpdatesTest {
    private val minFull = 3_600_001

    @Test
    fun chipTextIsAlwaysUnderSevenCharacters() {
        for (percent in -50..250) {
            val text = LiveUpdates.chipText(percent)
            assertTrue("\"$text\" is too long", text.length < 7)
        }
    }

    @Test
    fun chipTextShowsThePercentAndClamps() {
        assertEquals("0%", LiveUpdates.chipText(0))
        assertEquals("62%", LiveUpdates.chipText(62))
        assertEquals("100%", LiveUpdates.chipText(100))
        assertEquals("0%", LiveUpdates.chipText(-3))
        assertEquals("100%", LiveUpdates.chipText(140))
    }

    @Test
    fun onlyAndroid16Qpr1OrLaterAllowsPromotion() {
        assertFalse("Android 15", LiveUpdates.apiAllowsPromotion(35, 3_500_000, minFull))
        assertFalse("Android 16", LiveUpdates.apiAllowsPromotion(36, 3_600_000, minFull))
        assertTrue("Android 16 QPR1", LiveUpdates.apiAllowsPromotion(36, 3_600_001, minFull))
        assertTrue("Android 17", LiveUpdates.apiAllowsPromotion(37, 3_700_000, minFull))
    }

    @Test
    fun aPersonsOwnTaskIsPromotedWhenTheApiAllowsAndTheToggleIsOn() {
        assertTrue(LiveUpdates.shouldPromote(apiAllows = true, userInitiated = true, toggleOn = true))
    }

    @Test
    fun automaticWorkIsNeverPromoted() {
        assertFalse(LiveUpdates.shouldPromote(apiAllows = true, userInitiated = false, toggleOn = true))
    }

    @Test
    fun theToggleOffMeansNoPromotion() {
        assertFalse(LiveUpdates.shouldPromote(apiAllows = true, userInitiated = true, toggleOn = false))
    }

    @Test
    fun anOldApiMeansNoPromotion() {
        assertFalse(LiveUpdates.shouldPromote(apiAllows = false, userInitiated = true, toggleOn = true))
    }
}
