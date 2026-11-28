package uk.gcartee.encore.app.ui.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.core.SyncState

class CompanionWordsTest {
    private val day = 24 * 60 * 60_000L

    @Test
    fun the_row_says_what_is_waiting_and_when_it_last_got_through() {
        assertEquals("Up to date · last synced today", companionLine(SyncState.IDLE, 0, "today"))
        assertEquals("Can't reach it · 41 waiting to sync · last synced Tuesday", companionLine(SyncState.OFFLINE, 41, "Tuesday"))
        assertEquals("Signed out · 2 waiting to sync", companionLine(SyncState.SIGNED_OUT, 2, null))
    }

    @Test
    fun home_speaks_up_only_after_a_day_without_sync_or_when_signed_out() {
        val now = 10 * day
        assertTrue(syncStalled(SyncState.SIGNED_OUT, now, now))
        assertTrue(syncStalled(SyncState.OFFLINE, now - day - 1, now))
        assertTrue(syncStalled(SyncState.OFFLINE, null, now))
        assertFalse(syncStalled(SyncState.OFFLINE, now - day + 1, now))
        assertFalse(syncStalled(SyncState.IDLE, now - 3 * day, now))
        assertFalse(syncStalled(SyncState.SYNCING, null, now))
        assertFalse(syncStalled(SyncState.NOT_CONNECTED, null, now))
    }
}
