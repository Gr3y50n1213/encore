package uk.gcartee.encore.app.playback

import android.net.Uri
import android.os.SystemClock
import androidx.annotation.OptIn
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import java.util.TimeZone
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.core.CoreEvent
import uk.gcartee.encore.core.EndReason
import uk.gcartee.encore.core.EventKind
import uk.gcartee.encore.core.InterruptionKind
import uk.gcartee.encore.core.PreviousResult
import uk.gcartee.encore.core.QueueEntry
import uk.gcartee.encore.core.QueueState
import uk.gcartee.encore.core.SkipOrigin

/** What the mini player and Now Playing draw. */
data class PlayerState(
    val entry: QueueEntry? = null,
    val isPlaying: Boolean = false,
    val buffering: Boolean = false,
    val positionMs: Long = 0,
    val durationMs: Long = 0,
)

/**
 * Plays the core's queue with ExoPlayer (CORE-API, "Playback reporting"). The core owns
 * the queue and the play log; this only loads what the core says is current, preloads
 * what `Preload` names for gapless playback, and reports what happened:
 * `playback_started` when sound starts, `playback_progress` each second, and
 * `playback_ended` when a song finishes or fails. Skips need no report: the next
 * `playback_started` closes the play as skipped. Runs on the main thread.
 */
@OptIn(UnstableApi::class)
class CorePlayback(
    private val host: CoreHost,
    private val exo: ExoPlayer,
    private val headers: ConcurrentHashMap<String, Map<String, String>>,
    private val scope: CoroutineScope,
) : Player.Listener {
    private val core get() = host.core
    private val clock = ListenClock { SystemClock.elapsedRealtime() }
    /** The play the core was told about (`playback_started`), by queue uid. */
    private var startedUid: Long? = null
    private var ownSeek = false
    private var errorsInARow = 0

    private val _state = MutableStateFlow(PlayerState())
    val state: StateFlow<PlayerState> = _state.asStateFlow()

    fun start() {
        exo.addListener(this)
        scope.launch { host.events.collect(::onEvent) }
        scope.launch {
            while (isActive) {
                delay(TICK_MS)
                tick()
            }
        }
        // open_profile loads the last queue, paused.
        runCatching { core.queue() }.getOrNull()?.current?.let { load(it, play = false) }
    }

    /** The profile is closing or the service stops: close the open play honestly. */
    fun stop() {
        if (startedUid != null) {
            val reason = if (exo.playWhenReady) EndReason.ENDED_BY_SYSTEM else EndReason.PAUSED_ABANDONED
            runCatching { core.playbackEnded(reason, clock.listenedMs) }
            startedUid = null
        }
        exo.removeListener(this)
    }

    // --- Actions from the app's own buttons (origin Button) and the media session. ---

    /** After `play(...)` or `jump_to`: start the queue's current song now. */
    fun playCurrent() {
        val current = core.queue().current ?: return
        if (currentUid() == current.uid) {
            exo.seekTo(0, 0)
            exo.play()
        } else {
            load(current, play = true)
        }
    }

    fun togglePlay() = if (exo.playWhenReady) pause() else play()

    fun play() {
        if (exo.mediaItemCount == 0) {
            core.queue().current?.let { load(it, play = true) }
            return
        }
        if (exo.playbackState == Player.STATE_IDLE) exo.prepare()
        if (exo.playbackState == Player.STATE_ENDED) exo.seekTo(0, 0)
        exo.play()
    }

    fun pause() = exo.pause()

    fun next(origin: SkipOrigin) {
        val state = core.skipNext(origin)
        show(state.current, play = true)
    }

    fun previous(origin: SkipOrigin) {
        when (core.skipPrevious(origin, exo.currentPosition)) {
            PreviousResult.RESTARTED -> {
                ownSeek = true
                exo.seekTo(0)
            }
            PreviousResult.MOVED -> show(core.queue().current, play = true)
        }
    }

    fun seekTo(positionMs: Long) {
        exo.seekTo(positionMs) // reported in onPositionDiscontinuity
    }

    // --- Core events. ---

    private fun onEvent(event: CoreEvent) {
        when (event.kind) {
            EventKind.NOW_PLAYING -> show(event.nowPlaying, play = exo.playWhenReady)
            EventKind.PRELOAD -> event.preload?.let(::preload)
            EventKind.QUEUE_CHANGED -> event.queue?.let(::queueChanged)
            else -> Unit
        }
    }

    /** Makes `entry` the playing item, reusing the preloaded one when it matches. */
    private fun show(entry: QueueEntry?, play: Boolean) {
        if (entry == null) {
            exo.stop()
            exo.clearMediaItems()
            startedUid = null
            publish()
            return
        }
        if (currentUid() == entry.uid) return
        if (exo.mediaItemCount > 1 && uidAt(1) == entry.uid) {
            exo.seekToDefaultPosition(1)
            exo.removeMediaItem(0)
            if (play) exo.play()
            publish()
            return
        }
        load(entry, play)
    }

    private fun load(entry: QueueEntry, play: Boolean) {
        val item = mediaItem(entry) ?: return
        exo.setMediaItem(item)
        exo.prepare()
        exo.playWhenReady = play
        clock.reset()
        publish(entry)
    }

    private fun preload(entry: QueueEntry) {
        while (exo.mediaItemCount > 1) exo.removeMediaItem(exo.mediaItemCount - 1)
        mediaItem(entry)?.let(exo::addMediaItem)
    }

    /** The queue changed after a preload (a reorder, Play Next): replace the stale one. */
    private fun queueChanged(state: QueueState) {
        if (exo.mediaItemCount < 2) return
        val expected = runCatching { core.nextTarget().song }.getOrNull()
        if (uidAt(1) == expected?.uid) return
        while (exo.mediaItemCount > 1) exo.removeMediaItem(exo.mediaItemCount - 1)
        expected?.let { mediaItem(it)?.let(exo::addMediaItem) }
        publish(state.current)
    }

    // --- Player events. ---

    override fun onIsPlayingChanged(isPlaying: Boolean) {
        clock.playing(isPlaying)
        val uid = currentUid()
        if (isPlaying && uid != null && startedUid != uid) {
            started(uid)
        } else if (startedUid == uid && uid != null) {
            if (isPlaying) core.playbackResumed(exo.currentPosition)
            else if (!exo.playWhenReady) core.playbackPaused(exo.currentPosition)
        }
        publish()
    }

    /** A skip onto the preloaded song can keep the player playing: sound started. */
    private fun startedIfPlaying() {
        val uid = currentUid() ?: return
        if (exo.isPlaying && startedUid != uid) started(uid)
    }

    override fun onPlaybackStateChanged(playbackState: Int) {
        if (playbackState == Player.STATE_ENDED && startedUid != null) {
            // The last loaded song finished with nothing preloaded.
            val next = core.playbackEnded(EndReason.FINISHED, clock.listenedMs)
            startedUid = null
            next?.let { load(it, play = true) }
        }
        publish()
    }

    override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
        if (reason != Player.MEDIA_ITEM_TRANSITION_REASON_AUTO) {
            startedIfPlaying()
            publish()
            return
        }
        // Gapless into the preloaded song: the previous one finished.
        val heard = clock.listenedMs
        val next = core.playbackEnded(EndReason.FINISHED, heard)
        startedUid = null
        exo.removeMediaItem(0)
        val arrived = mediaItem?.mediaId?.toUidOrNull()
        when {
            next == null -> exo.stop()
            next.uid == arrived -> started(next.uid)
            else -> load(next, play = true)
        }
        publish()
    }

    override fun onPlayerError(error: PlaybackException) {
        // Never logged with its URL: stream URLs carry the token.
        errorsInARow++
        val next = if (startedUid != null || currentUid() != null) {
            runCatching { core.playbackEnded(EndReason.ERROR, clock.listenedMs) }.getOrNull()
        } else {
            null
        }
        startedUid = null
        if (next != null && errorsInARow < MAX_ERRORS_IN_A_ROW) load(next, play = true) else exo.stop()
        publish()
    }

    override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
        if (playWhenReady || startedUid == null) return
        when (reason) {
            Player.PLAY_WHEN_READY_CHANGE_REASON_AUDIO_BECOMING_NOISY ->
                core.playbackInterrupted(InterruptionKind.HEADPHONES_UNPLUGGED)
            Player.PLAY_WHEN_READY_CHANGE_REASON_AUDIO_FOCUS_LOSS ->
                core.playbackInterrupted(InterruptionKind.CALL)
        }
    }

    override fun onPositionDiscontinuity(old: Player.PositionInfo, new: Player.PositionInfo, reason: Int) {
        if (reason != Player.DISCONTINUITY_REASON_SEEK || old.mediaItemIndex != new.mediaItemIndex) return
        if (ownSeek) {
            ownSeek = false
        } else if (startedUid != null) {
            core.playbackSeeked(old.positionMs, new.positionMs)
        }
        publish()
    }

    private fun started(uid: Long) {
        clock.reset()
        clock.playing(exo.isPlaying)
        val offsetMinutes = TimeZone.getDefault().getOffset(System.currentTimeMillis()) / 60_000
        core.playbackStarted(uid, offsetMinutes)
        startedUid = uid
        errorsInARow = 0
    }

    private fun tick() {
        if (exo.isPlaying && startedUid != null) {
            core.playbackProgress(exo.currentPosition, clock.listenedMs)
        }
        publish()
    }

    // --- Helpers. ---

    private fun mediaItem(entry: QueueEntry): MediaItem? {
        val info = runCatching { core.streamFor(entry.song.id, host.network()) }.getOrElse {
            // SessionExpired and the like: the core says why on the next call; skip it.
            return null
        }
        headers[info.url] = info.headers.associate { it.name to it.value }
        val art = runCatching { core.artwork(ArtworkOf.SONG, entry.song.id, ARTWORK_PX.toUInt()) }.getOrNull()
        art?.let { headers[it.url] = it.headers.associate { h -> h.name to h.value } }
        val song = entry.song
        return MediaItem.Builder()
            .setMediaId("$UID_PREFIX${entry.uid}")
            .setUri(info.url)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(song.title)
                    .setArtist(song.artist)
                    .setAlbumTitle(song.albumTitle)
                    .setAlbumArtist(song.albumArtist)
                    .setDurationMs(song.durationMs)
                    .setArtworkUri(art?.url?.let(Uri::parse))
                    .setIsPlayable(true)
                    .setIsBrowsable(false)
                    .build(),
            )
            .build()
    }

    private fun currentUid(): Long? = exo.currentMediaItem?.mediaId?.toUidOrNull()

    private fun uidAt(index: Int): Long? =
        if (index < exo.mediaItemCount) exo.getMediaItemAt(index).mediaId.toUidOrNull() else null

    private fun publish(entry: QueueEntry? = null) {
        val current = entry ?: _state.value.entry.takeIf { it?.uid == currentUid() }
            ?: runCatching { core.queue().current }.getOrNull()
        _state.value = PlayerState(
            entry = current,
            isPlaying = exo.isPlaying,
            buffering = exo.playbackState == Player.STATE_BUFFERING,
            positionMs = exo.currentPosition.coerceAtLeast(0),
            durationMs = current?.song?.durationMs ?: 0,
        )
    }

    companion object {
        const val UID_PREFIX = "uid:"
        private const val TICK_MS = 1_000L
        private const val ARTWORK_PX = 512
        private const val MAX_ERRORS_IN_A_ROW = 3

        fun String.toUidOrNull(): Long? = removePrefix(UID_PREFIX).toLongOrNull()
    }
}
