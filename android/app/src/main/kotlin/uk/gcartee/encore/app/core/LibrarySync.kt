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

/** Runs `sync_library` off the main thread, one at a time, and reports its progress. */
class LibrarySync(private val host: CoreHost) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val _state = MutableStateFlow(SyncUi())
    val state: StateFlow<SyncUi> = _state.asStateFlow()
    private var job: Job? = null

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
    }
}
