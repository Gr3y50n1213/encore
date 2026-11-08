package uk.gcartee.encore.app.playback

import android.content.Context
import android.os.Bundle
import androidx.annotation.OptIn
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.LibraryResult
import androidx.media3.session.MediaConstants
import androidx.media3.session.MediaLibraryService.LibraryParams
import androidx.media3.session.MediaLibraryService.MediaLibrarySession
import androidx.media3.session.MediaSession
import androidx.media3.session.SessionError
import com.google.common.collect.ImmutableList
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import com.google.common.util.concurrent.SettableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.DownloadView
import uk.gcartee.encore.core.AlbumSort
import uk.gcartee.encore.core.AlbumSummary
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.core.DownloadKind
import uk.gcartee.encore.core.Song
import uk.gcartee.encore.core.StartedFrom

/**
 * Android Auto's browse tree and search (Phase 8; docs/adr/android-0002). The car
 * draws the screens from templates; Encore gives it the lists: Home (the day's mixes
 * and what was played), Library (playlists, albums, artists, Liked songs) and
 * Downloads. Choosing a song plays it with its album, playlist or mix, through the
 * core like any tap in the app; "Play <name>" by voice goes through the core's
 * search. Offline, the lists show only what can play.
 *
 * Media IDs: `home`, `library`, `downloads`, `library/playlists|albums|artists`,
 * `album:<id>`, `playlist:<id>`, `artist:<id>`, `mix:<id>`, `liked`, and songs as
 * `song:<id>@<context>` (the list they were chosen from).
 */
@OptIn(UnstableApi::class)
class AutoLibrary(
    private val context: Context,
    private val host: CoreHost,
    private val scope: CoroutineScope,
    private val playback: () -> CorePlayback?,
) : MediaLibrarySession.Callback {
    private val core get() = host.core
    private val view: DownloadView get() = host.downloads.view.value

    override fun onGetLibraryRoot(
        session: MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        params: LibraryParams?,
    ): ListenableFuture<LibraryResult<MediaItem>> {
        val extras = Bundle().apply {
            putInt(MediaConstants.EXTRAS_KEY_CONTENT_STYLE_BROWSABLE, MediaConstants.EXTRAS_VALUE_CONTENT_STYLE_GRID_ITEM)
            putInt(MediaConstants.EXTRAS_KEY_CONTENT_STYLE_PLAYABLE, MediaConstants.EXTRAS_VALUE_CONTENT_STYLE_LIST_ITEM)
            putBoolean(SEARCH_SUPPORTED, true)
        }
        val root = folder(ROOT, "Encore")
        return Futures.immediateFuture(LibraryResult.ofItem(root, LibraryParams.Builder().setExtras(extras).build()))
    }

    override fun onGetChildren(
        session: MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        parentId: String,
        page: Int,
        pageSize: Int,
        params: LibraryParams?,
    ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> {
        if (!host.ensureProfile()) {
            return Futures.immediateFuture(LibraryResult.ofError(SessionError(SessionError.ERROR_SESSION_AUTHENTICATION_EXPIRED, "Open Encore on your phone to sign in")))
        }
        val result = SettableFuture.create<LibraryResult<ImmutableList<MediaItem>>>()
        scope.launch {
            val items = runCatching { children(parentId) }.getOrElse { emptyList() }
            items.forEach { grantArt(browser, it) }
            val from = (page.coerceAtLeast(0) * pageSize.coerceAtLeast(1)).coerceAtMost(items.size)
            val to = (from + pageSize.coerceAtLeast(1)).coerceAtMost(items.size)
            result.set(LibraryResult.ofItemList(ImmutableList.copyOf(items.subList(from, to)), params))
        }
        return result
    }

    override fun onGetItem(
        session: MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        mediaId: String,
    ): ListenableFuture<LibraryResult<MediaItem>> {
        val item = runCatching { itemFor(mediaId) }.getOrNull()
            ?: return Futures.immediateFuture(LibraryResult.ofError(SessionError.ERROR_BAD_VALUE))
        grantArt(browser, item)
        return Futures.immediateFuture(LibraryResult.ofItem(item, null))
    }

    override fun onSearch(
        session: MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        query: String,
        params: LibraryParams?,
    ): ListenableFuture<LibraryResult<Void>> {
        val n = searchItems(query).size
        session.notifySearchResultChanged(browser, query, n, params)
        return Futures.immediateFuture(LibraryResult.ofVoid())
    }

    override fun onGetSearchResult(
        session: MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        query: String,
        page: Int,
        pageSize: Int,
        params: LibraryParams?,
    ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> {
        val items = searchItems(query)
        items.forEach { grantArt(browser, it) }
        val from = (page.coerceAtLeast(0) * pageSize.coerceAtLeast(1)).coerceAtMost(items.size)
        val to = (from + pageSize.coerceAtLeast(1)).coerceAtMost(items.size)
        return Futures.immediateFuture(LibraryResult.ofItemList(ImmutableList.copyOf(items.subList(from, to)), params))
    }

    /**
     * A tap in the car, or "Play <name>" by voice: the core builds the queue (with the
     * same offline rules as a tap in the app), then the player loads its current song.
     */
    override fun onSetMediaItems(
        mediaSession: MediaSession,
        controller: MediaSession.ControllerInfo,
        mediaItems: MutableList<MediaItem>,
        startIndex: Int,
        startPositionMs: Long,
    ): ListenableFuture<MediaSession.MediaItemsWithStartPosition> {
        val result = SettableFuture.create<MediaSession.MediaItemsWithStartPosition>()
        scope.launch {
            val chosen = mediaItems.getOrNull(startIndex.coerceAtLeast(0)) ?: mediaItems.firstOrNull()
            val played = runCatching { chosen?.let { play(it) } ?: false }.getOrDefault(false)
            val current = runCatching { core.queue().current }.getOrNull()
            if (!played || current == null) {
                result.setException(IllegalStateException("Nothing to play"))
                return@launch
            }
            result.set(MediaSession.MediaItemsWithStartPosition(listOf(placeholder(current.uid)), 0, 0))
        }
        return result
    }

    override fun onAddMediaItems(
        mediaSession: MediaSession,
        controller: MediaSession.ControllerInfo,
        mediaItems: MutableList<MediaItem>,
    ): ListenableFuture<MutableList<MediaItem>> {
        // "Add to queue" from a car: the songs join Up Next through the core.
        val ids = mediaItems.mapNotNull { songOf(it.mediaId)?.first }
        if (ids.isNotEmpty()) runCatching { core.addToUpNext(ids) }
        return Futures.immediateFuture(mutableListOf())
    }

    /** Bluetooth's or the car's Play after the app was closed: the queue as it was. */
    override fun onPlaybackResumption(
        mediaSession: MediaSession,
        controller: MediaSession.ControllerInfo,
        isForPlayback: Boolean,
    ): ListenableFuture<MediaSession.MediaItemsWithStartPosition> {
        if (!host.ensureProfile()) return Futures.immediateFailedFuture(IllegalStateException("No profile"))
        val current = runCatching { core.queue().current }.getOrNull()
            ?: return Futures.immediateFailedFuture(IllegalStateException("Nothing queued"))
        return Futures.immediateFuture(MediaSession.MediaItemsWithStartPosition(listOf(placeholder(current.uid)), 0, 0))
    }

    // --- The tree. ---

    private suspend fun children(parentId: String): List<MediaItem> {
        val v = view
        return when {
            parentId == ROOT -> listOf(folder(HOME, "Home"), folder(LIBRARY, "Library"), folder(DOWNLOADS, "Downloads"))
            parentId == HOME -> {
                val home = core.home()
                val mixes = home.madeForYou.mixes.map { m ->
                    folder("mix:${m.mixId}", m.title, m.subtitle, art = m.coverSongIds.firstOrNull()?.let { ArtworkProvider.uri(context, ArtworkOf.SONG, it) }, playable = true)
                }
                val played = home.recentlyPlayed.filter { v.canPlay(it.id) }.map { song(it, "home") }
                mixes + played
            }
            parentId == LIBRARY -> listOf(
                folder(PLAYLISTS, "Playlists"),
                folder(ALBUMS, "Albums"),
                folder(ARTISTS, "Artists"),
                folder(LIKED, "Liked songs", playable = true),
            )
            parentId == PLAYLISTS -> core.playlists(0u, LIMIT).filter { v.online || v.playlist(it.id) != null }.map { p ->
                folder("playlist:${p.id}", p.name, "${p.songCount} songs", art = ArtworkProvider.uri(context, ArtworkOf.PLAYLIST, p.id), playable = true)
            }
            parentId == ALBUMS -> core.albums(AlbumSort.RECENTLY_ADDED, 0u, LIMIT).filter { v.online || v.album(it.id) != null }.map(::album)
            parentId == ARTISTS -> core.artists(0u, LIMIT).map { a ->
                folder("artist:${a.id}", a.name, art = ArtworkProvider.uri(context, ArtworkOf.ARTIST, a.id))
            }
            parentId == DOWNLOADS -> v.items.mapNotNull { i ->
                when (i.target.kind) {
                    DownloadKind.ALBUM -> folder("album:${i.target.id}", i.title, i.subtitle, art = ArtworkProvider.uri(context, ArtworkOf.ALBUM, i.target.id), playable = true)
                    DownloadKind.PLAYLIST -> folder("playlist:${i.target.id}", i.title, "${i.saved} songs", art = ArtworkProvider.uri(context, ArtworkOf.PLAYLIST, i.target.id), playable = true)
                    DownloadKind.MIX -> folder("mix:${i.target.mixId}", i.title, playable = true)
                    DownloadKind.LIKED_SONGS -> folder(LIKED, i.title, playable = true)
                    DownloadKind.SONG -> runCatching { songById(i.target.id) }.getOrNull()?.let { song(it, "downloads") }
                    DownloadKind.SMART -> null
                }
            }
            parentId == LIKED -> core.likedSongs(0u, LIMIT).filter { v.canPlay(it.id) }.map { song(it, LIKED) }
            parentId.startsWith("album:") -> core.album(parentId.id()).songs.filter { v.canPlay(it.id) }.map { song(it, parentId) }
            parentId.startsWith("playlist:") -> core.playlist(parentId.id()).entries.mapNotNull { it.song }.filter { v.canPlay(it.id) }.map { song(it, parentId) }
            parentId.startsWith("artist:") -> core.artist(parentId.id()).albums.filter { v.online || v.album(it.id) != null }.map(::album)
            parentId.startsWith("mix:") -> core.mix(parentId.removePrefix("mix:")).songs.map { it.song }.filter { v.canPlay(it.id) }.map { song(it, parentId) }
            else -> emptyList()
        }
    }

    private fun itemFor(mediaId: String): MediaItem? = when {
        mediaId == ROOT -> folder(ROOT, "Encore")
        mediaId.startsWith("song:") -> songOf(mediaId)?.let { (id, ctx) -> song(songById(id), ctx) }
        mediaId.startsWith("album:") -> album(core.album(mediaId.id()).album)
        else -> null
    }

    private fun searchItems(query: String): List<MediaItem> {
        if (!host.ensureProfile()) return emptyList()
        val r = runCatching { core.search(query) }.getOrNull() ?: return emptyList()
        val v = view
        return r.songs.filter { v.canPlay(it.id) }.map { song(it, "search") } +
            r.albums.filter { v.online || v.album(it.id) != null }.map(::album) +
            r.playlists.filter { v.online || v.playlist(it.id) != null }.map { p -> folder("playlist:${p.id}", p.name, playable = true) }
    }

    // --- Playing. ---

    /** Play what the car chose; false when nothing could start (offline, gone). */
    private suspend fun play(item: MediaItem): Boolean {
        val id = item.mediaId
        val query = item.requestMetadata.searchQuery
        if (id.isEmpty() && query != null) return playSearch(query)
        val (songs, start, from, ctx, label) = when {
            id.startsWith("song:") -> {
                val (songId, list) = songOf(id) ?: return false
                val ids = listIds(list).ifEmpty { listOf(songId) }
                Quint(ids, ids.indexOf(songId).coerceAtLeast(0), startedFrom(list), list.substringAfter(':', ""), null)
            }
            id.startsWith("album:") -> Quint(core.album(id.id()).songs.map { it.id }, 0, StartedFrom.ALBUM, id.id().toString(), core.album(id.id()).album.title)
            id.startsWith("playlist:") -> Quint(listIds(id), 0, StartedFrom.PLAYLIST, id.id().toString(), core.playlist(id.id()).playlist.name)
            id.startsWith("mix:") -> Quint(listIds(id), 0, StartedFrom.MIX, id.removePrefix("mix:"), null)
            id == LIKED -> Quint(listIds(LIKED), 0, StartedFrom.PLAYLIST, null, "Liked songs")
            else -> return false
        }
        val playable = songs.filter { view.canPlay(it) }
        if (playable.isEmpty()) return false
        val startId = songs.getOrNull(start)?.takeIf { view.canPlay(it) } ?: playable.first()
        core.play(playable, playable.indexOf(startId).coerceAtLeast(0).toUInt(), from, ctx)
        host.playingFrom.value = label
        return true
    }

    /** "Play <name>" by voice in the car (VoicePlay, shared with the phone's assistant). */
    private fun playSearch(query: String): Boolean = VoicePlay.queue(host, query)

    private fun listIds(list: String): List<Long> = runCatching {
        when {
            list.startsWith("album:") -> core.album(list.id()).songs.map { it.id }
            list.startsWith("playlist:") -> core.playlist(list.id()).entries.mapNotNull { it.song?.id }
            list.startsWith("mix:") -> core.mix(list.removePrefix("mix:")).songs.map { it.song.id }
            list == LIKED -> core.likedSongs(0u, LIMIT).map { it.id }
            else -> emptyList()
        }
    }.getOrDefault(emptyList())

    private fun startedFrom(list: String) = when {
        list.startsWith("album:") -> StartedFrom.ALBUM
        list.startsWith("playlist:") || list == LIKED -> StartedFrom.PLAYLIST
        list.startsWith("mix:") -> StartedFrom.MIX
        list == "search" -> StartedFrom.SEARCH
        else -> StartedFrom.OTHER
    }

    // --- Items. ---

    private fun folder(id: String, title: String, subtitle: String? = null, art: android.net.Uri? = null, playable: Boolean = false): MediaItem =
        MediaItem.Builder()
            .setMediaId(id)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(title)
                    .setSubtitle(subtitle)
                    .setArtworkUri(art)
                    .setIsBrowsable(true)
                    .setIsPlayable(playable)
                    .setMediaType(MediaMetadata.MEDIA_TYPE_FOLDER_MIXED)
                    .build(),
            )
            .build()

    private fun album(a: AlbumSummary): MediaItem =
        MediaItem.Builder()
            .setMediaId("album:${a.id}")
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(a.title)
                    .setArtist(a.artist)
                    .setSubtitle(a.artist)
                    .setArtworkUri(ArtworkProvider.uri(context, ArtworkOf.ALBUM, a.id))
                    .setIsBrowsable(true)
                    .setIsPlayable(true)
                    .setMediaType(MediaMetadata.MEDIA_TYPE_ALBUM)
                    .build(),
            )
            .build()

    private fun song(s: Song, list: String): MediaItem =
        MediaItem.Builder()
            .setMediaId("song:${s.id}@$list")
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(s.title)
                    .setArtist(s.artist)
                    .setSubtitle(s.artist)
                    .setAlbumTitle(s.albumTitle)
                    .setDurationMs(s.durationMs)
                    .setArtworkUri(ArtworkProvider.uri(context, ArtworkOf.SONG, s.id))
                    .setIsBrowsable(false)
                    .setIsPlayable(true)
                    .setMediaType(MediaMetadata.MEDIA_TYPE_MUSIC)
                    .build(),
            )
            .build()

    /** What the player loads: CorePlayback turns the core's current song into it. */
    private fun placeholder(uid: Long): MediaItem = MediaItem.Builder().setMediaId("${CorePlayback.UID_PREFIX}$uid").build()

    private fun songById(id: Long): Song = core.song(id)

    private fun grantArt(browser: MediaSession.ControllerInfo, item: MediaItem) {
        item.mediaMetadata.artworkUri?.let { ArtworkProvider.grant(context, browser.packageName, it) }
    }

    private fun songOf(mediaId: String): Pair<Long, String>? {
        val body = mediaId.removePrefix("song:")
        val id = body.substringBefore('@').toLongOrNull() ?: return null
        return id to body.substringAfter('@', "")
    }

    private fun String.id(): Long = substringAfter(':').toLong()

    private data class Quint(val songs: List<Long>, val start: Int, val from: StartedFrom, val context: String?, val label: String?)

    private companion object {
        const val ROOT = "root"
        const val HOME = "home"
        const val LIBRARY = "library"
        const val DOWNLOADS = "downloads"
        const val PLAYLISTS = "library/playlists"
        const val ALBUMS = "library/albums"
        const val ARTISTS = "library/artists"
        const val LIKED = "liked"
        const val LIMIT = 500u
        const val SEARCH_SUPPORTED = "android.media.browse.SEARCH_SUPPORTED"
    }
}
