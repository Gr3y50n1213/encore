package uk.gcartee.encore.app.core

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uk.gcartee.encore.core.DownloadKind
import uk.gcartee.encore.core.DownloadMarks
import uk.gcartee.encore.core.DownloadProgress
import uk.gcartee.encore.core.DownloadState
import uk.gcartee.encore.core.DownloadTarget
import uk.gcartee.encore.core.DownloadedItem

/**
 * What every list needs to draw download badges and to dim what can't play offline
 * (boards 06, 08, 11, 20), as sets, so a row asks in constant time. Read from the
 * core's `download_marks` on every `DownloadsChanged`; the transfer's progress comes
 * from `DownloadProgress` events.
 */
data class DownloadView(
    val saved: Set<Long> = emptySet(),
    val pending: Set<Long> = emptySet(),
    val failed: Set<Long> = emptySet(),
    val playableOffline: Set<Long> = emptySet(),
    val items: List<DownloadedItem> = emptyList(),
    val online: Boolean = true,
    val activeSong: Long? = null,
) {
    private val byTarget: Map<Pair<DownloadKind, String>, DownloadedItem> = items.associateBy { key(it.target) }

    fun item(target: DownloadTarget): DownloadedItem? = byTarget[key(target)]

    fun album(id: Long): DownloadedItem? = byTarget[DownloadKind.ALBUM to id.toString()]

    fun playlist(id: Long): DownloadedItem? = byTarget[DownloadKind.PLAYLIST to id.toString()]

    fun mix(id: String): DownloadedItem? = byTarget[DownloadKind.MIX to id]

    /** Offline, only these play; online, everything does. */
    fun canPlay(songId: Long): Boolean = online || songId in playableOffline

    /** Board 06, item 5: an album with no saved song dims offline. */
    fun albumPlayable(albumSongs: Collection<Long>): Boolean = online || albumSongs.any { it in playableOffline }

    companion object {
        fun key(t: DownloadTarget): Pair<DownloadKind, String> = when (t.kind) {
            DownloadKind.MIX -> t.kind to t.mixId.orEmpty()
            DownloadKind.LIKED_SONGS, DownloadKind.SMART -> t.kind to ""
            else -> t.kind to t.id.toString()
        }

        fun of(m: DownloadMarks) = DownloadView(
            saved = m.savedSongIds.toHashSet(),
            pending = m.pendingSongIds.toHashSet(),
            failed = m.failedSongIds.toHashSet(),
            playableOffline = m.playableOfflineSongIds.toHashSet(),
            items = m.items,
            online = m.online,
            activeSong = m.activeSongId,
        )
    }
}

class Downloads(private val host: CoreHost) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val _view = MutableStateFlow(DownloadView())
    val view: StateFlow<DownloadView> = _view.asStateFlow()
    private val _progress = MutableStateFlow<DownloadProgress?>(null)
    /** The transfer under way, if any. */
    val progress: StateFlow<DownloadProgress?> = _progress.asStateFlow()
    private var pending: Job? = null

    /** Read the marks again soon; a burst of changes reads once. */
    fun refresh() {
        if (pending?.isActive == true) return
        pending = scope.launch {
            delay(COALESCE_MS)
            refreshNow()
        }
    }

    fun refreshNow() {
        val marks = runCatching { host.core.downloadMarks() }.getOrNull()
        _view.value = marks?.let(DownloadView::of) ?: DownloadView(online = _view.value.online)
        if (marks?.activeSongId == null) _progress.value = null
    }

    fun progressed(p: DownloadProgress) {
        _progress.value = p
    }

    /** A profile switched or closed: nothing of the last one shows. */
    fun reset() {
        _view.value = DownloadView()
        _progress.value = null
        refresh()
    }

    private companion object {
        const val COALESCE_MS = 150L
    }
}

/** A target for a song, album, playlist or mix. */
fun songTarget(id: Long) = DownloadTarget(DownloadKind.SONG, id, null)
fun albumTarget(id: Long) = DownloadTarget(DownloadKind.ALBUM, id, null)
fun playlistTarget(id: Long) = DownloadTarget(DownloadKind.PLAYLIST, id, null)
fun mixTarget(id: String) = DownloadTarget(DownloadKind.MIX, 0, id)

/** Whether an item is fully on the device. */
val DownloadedItem.saved get() = state == DownloadState.SAVED
