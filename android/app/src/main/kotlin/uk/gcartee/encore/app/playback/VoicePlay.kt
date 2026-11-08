package uk.gcartee.encore.app.playback

import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.StartedFrom

/**
 * "Play <name> on Encore" (Phase 8): from Android Auto's voice and from the phone's
 * assistant (`MEDIA_PLAY_FROM_SEARCH`). The core's search picks the best match: an
 * album or playlist plays whole, else the matching songs. Offline, only what can play
 * is queued. A bare "play music" keeps the queue as it was. Builds the queue only;
 * the caller loads the player.
 */
object VoicePlay {
    fun queue(host: CoreHost, query: String?): Boolean {
        val core = host.core
        if (query.isNullOrBlank()) return runCatching { core.queue().current }.getOrNull() != null
        val r = runCatching { core.search(query) }.getOrNull() ?: return false
        val view = host.downloads.view.value
        val top = r.top
        val songs: List<Long> = when {
            top?.album != null -> runCatching { core.album(top.album!!.id).songs.map { it.id } }.getOrDefault(emptyList())
            top?.playlist != null -> runCatching { core.playlist(top.playlist!!.id).entries.mapNotNull { it.song?.id } }.getOrDefault(emptyList())
            else -> r.songs.map { it.id }
        }.filter(view::canPlay)
        if (songs.isEmpty()) return false
        val played = runCatching { core.play(songs, 0u, StartedFrom.SEARCH, null) }.isSuccess
        if (played) host.playingFrom.value = "“$query”"
        return played
    }
}
