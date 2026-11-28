package uk.gcartee.encore.app.ui.settings

import uk.gcartee.encore.core.SyncState

/** How long sync may fail before Home says so. */
private const val STALLED_AFTER_MS = 24 * 60 * 60_000L

/**
 * The companion server's row in words (Settings > Servers and sources): what is happening,
 * what is waiting, and when everything last got through. [lastSynced] is that time as text.
 */
fun companionLine(state: SyncState, waiting: Long, lastSynced: String?): String {
    val head = when (state) {
        SyncState.IDLE -> "Up to date"
        SyncState.SYNCING -> "Syncing…"
        SyncState.OFFLINE -> "Can't reach it"
        SyncState.SIGNED_OUT -> "Signed out"
        SyncState.NOT_CONNECTED -> "Not connected"
    }
    return listOfNotNull(
        head,
        if (waiting > 0) "$waiting waiting to sync" else null,
        lastSynced?.let { "last synced $it" },
    ).joinToString(" · ")
}

/**
 * Whether Home should say sync has stopped: the server no longer accepts this device, or
 * nothing has got through for a day.
 */
fun syncStalled(state: SyncState, lastSyncedUtcMs: Long?, nowUtcMs: Long): Boolean = when (state) {
    SyncState.SIGNED_OUT -> true
    SyncState.OFFLINE -> lastSyncedUtcMs == null || nowUtcMs - lastSyncedUtcMs > STALLED_AFTER_MS
    else -> false
}
