package uk.gcartee.encore.app.core

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uk.gcartee.encore.core.EncoreException
import uk.gcartee.encore.core.EventKind
import uk.gcartee.encore.core.Network

/** Board 06 "Syncing" row: "Indexing 3,200 of 12,480", content stays usable. */
data class SyncUi(
    val running: Boolean = false,
    val done: Long = 0,
    val total: Long = 0,
    /** Set when the last sync failed; the cached library stays usable (board 05 states). */
    val problem: String? = null,
    val signInAgain: Boolean = false,
    /** Bumped after each finished sync so lists reload from the mirror. */
    val generation: Int = 0,
)

/**
 * Runs `sync_library` off the main thread, one at a time, and reports its progress.
 *
 * It also tries again by itself (found 8 November 2026: after the owner's server came
 * back, every open app kept "Couldn't reach the server" until it was restarted). While
 * the app is in front, a failed sync is retried after 15 s, 30 s, 1, 2, then every
 * 5 minutes; at once when the network comes back or changes; and coming back to the
 * app syncs when the last sync failed or is over 15 minutes old. "Sign in again" waits
 * for the person.
 */
class LibrarySync(private val host: CoreHost) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val _state = MutableStateFlow(SyncUi())
    val state: StateFlow<SyncUi> = _state.asStateFlow()
    private var job: Job? = null
    private var retry: Job? = null
    @Volatile private var foreground = false
    @Volatile private var failures = 0
    @Volatile private var lastOkAt = 0L

    /** The app came to the front. */
    fun onForeground() {
        foreground = true
        val stale = _state.value.problem != null || android.os.SystemClock.elapsedRealtime() - lastOkAt > FRESH_MS
        if (stale && canRetry()) syncNow()
    }

    /** The app went to the background: no retries until it's back. */
    fun onBackground() {
        foreground = false
        retry?.cancel()
    }

    /** The network came back or changed (Connectivity): a failed sync tries again now. */
    fun onNetworkChanged() {
        if (_state.value.problem != null && canRetry()) syncNow()
    }

    private fun canRetry() = host.profileOpen.value && !_state.value.signInAgain

    private fun scheduleRetry() {
        retry?.cancel()
        if (!foreground || !canRetry()) return
        val wait = RETRY_MS.getOrElse(failures - 1) { RETRY_MS.last() }
        retry = scope.launch {
            kotlinx.coroutines.delay(wait)
            if (foreground && canRetry()) syncNow()
        }
    }

    init {
        scope.launch {
            host.events.collect { e ->
                if (e.kind == EventKind.SYNC_PROGRESS) {
                    _state.value = _state.value.copy(running = true, done = e.syncDone, total = e.syncTotal)
                }
            }
        }
    }

    fun syncNow() {
        if (job?.isActive == true) return
        retry?.cancel()
        job = scope.launch {
            _state.value = _state.value.copy(running = true, done = 0, total = 0, problem = null)
            val result = runCatching { host.core.syncLibrary() }
            val error = result.exceptionOrNull()
            _state.value = _state.value.copy(
                running = false,
                problem = error?.userMessage(),
                signInAgain = error is EncoreException.SessionExpired,
                generation = _state.value.generation + 1,
            )
            if (error == null) {
                failures = 0
                lastOkAt = android.os.SystemClock.elapsedRealtime()
            } else {
                failures++
                scheduleRetry()
            }
            if (error == null && host.network() == Network.WIFI) {
                // So search finds songs by their lyrics before they are opened; on Wi-Fi only (CORE-API).
                runCatching {
                    while (host.core.fetchLyricsAhead(LYRICS_BATCH.toUInt()) > 0u) { /* next batch */ }
                }
            }
        }
    }

    private companion object {
        const val LYRICS_BATCH = 25
        const val FRESH_MS = 15 * 60_000L
        val RETRY_MS = longArrayOf(15_000, 30_000, 60_000, 120_000, 300_000)
    }
}
