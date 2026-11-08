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
import kotlinx.coroutines.Job
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
import uk.gcartee.encore.core.HandoffInfo
import uk.gcartee.encore.core.RemoteCommand
import uk.gcartee.encore.core.RemoteCommandKind
import uk.gcartee.encore.tokens.EncoreTokens.Gestures
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
    private var pendingSettle: Job? = null
    private var pendingPlay = false
    private var scanJob: Job? = null
    private var scanFrom = 0L

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
    /**
     * The queue was replaced by one that starts on the song already playing (a radio from
     * it): the new entry takes over at the current position instead of starting again.
     */
    fun continueCurrent() {
        val current = runCatching { core.queue().current }.getOrNull() ?: return
        if (currentUid() == current.uid) {
            if (!exo.playWhenReady) exo.play()
            return
        }
        val at = exo.currentPosition.coerceAtLeast(0)
        val item = mediaItem(current) ?: return
        exo.setMediaItem(item, at)
        exo.prepare()
        exo.playWhenReady = true
        clock.reset()
        publish(current)
    }
    /**
     * A car or another controller handed the session a list (Phase 8): the core has
     * the queue already. Load its current song if it isn't loaded, otherwise carry on
     * where it is (Bluetooth's or the car's Play after a reconnect never restarts it).
     */
    fun loadCurrent() {
        val current = runCatching { core.queue().current }.getOrNull() ?: return
        if (currentUid() == current.uid) {
            if (!exo.playWhenReady) exo.play()
        } else {
            load(current, play = true)
        }
    }
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

    /** Board 42, rule 4: a skip keeps the play state; skipping while paused stays paused. */
    fun next(origin: SkipOrigin) {
        val state = core.skipNext(origin)
        if (origin == SkipOrigin.SWIPE) settle(state.current) else show(state.current, play = playing())
    }

    fun previous(origin: SkipOrigin) {
        when (core.skipPrevious(origin, exo.currentPosition)) {
            PreviousResult.RESTARTED -> {
                ownSeek = true
                exo.seekTo(0)
            }
            PreviousResult.MOVED -> {
                val current = core.queue().current
                if (origin == SkipOrigin.SWIPE) settle(current) else show(current, play = playing())
            }
        }
    }

    private fun playing() = exo.playWhenReady || pendingPlay

    /**
     * Board 42, rule 5: the finger decides, not the audio. After a swipe the new song shows
     * at 0:00 at once, but loads only once no other swipe follows within 600 ms
     * (`gestures.playerSwipe.coalesceMs`), so songs passed over never load or sound.
     */
    private fun settle(entry: QueueEntry?) {
        pendingPlay = playing()
        if (exo.playWhenReady) exo.pause()
        pendingSettle?.cancel()
        publish(entry, positionMs = 0)
        pendingSettle = scope.launch {
            delay(Gestures.PlayerSwipe.coalesceMs.toLong())
            pendingSettle = null
            val play = pendingPlay
            pendingPlay = false
            show(entry, play)
        }
    }

    /** Board 09, item 6: holding Previous or Next scans at 4x, one seek report at the end. */
    fun startScan(forward: Boolean) {
        if (exo.mediaItemCount == 0 || scanJob != null) return
        scanFrom = exo.currentPosition
        scanJob = scope.launch {
            while (isActive) {
                delay(SCAN_STEP_MS)
                val duration = exo.duration.takeIf { it > 0 } ?: break
                val step = if (forward) SCAN_STEP_MS * (SCAN_SPEED - 1) else -SCAN_STEP_MS * (SCAN_SPEED + 1)
                exo.seekTo((exo.currentPosition + step).coerceIn(0, duration - SCAN_STEP_MS))
            }
        }
    }

    fun stopScan() {
        val job = scanJob ?: return
        job.cancel()
        scanJob = null
        if (startedUid != null) core.playbackSeeked(scanFrom, exo.currentPosition)
    }

    fun setVolume(level: Float) {
        exo.volume = level.coerceIn(0f, 1f)
        runCatching { core.setVolumeLevel(exo.volume.toDouble()) }
    }

    fun seekTo(positionMs: Long) {
        exo.seekTo(positionMs) // reported in onPositionDiscontinuity
    }

    // --- Core events. ---

    private fun onEvent(event: CoreEvent) {
        when (event.kind) {
            // A swipe that is still settling loads its song itself.
            EventKind.NOW_PLAYING -> if (pendingSettle == null) show(event.nowPlaying, play = exo.playWhenReady)
            EventKind.PRELOAD -> event.preload?.let(::preload)
            EventKind.QUEUE_CHANGED -> event.queue?.let(::queueChanged)
            EventKind.HANDOFF_INCOMING -> event.handoff?.let(::handoffIncoming)
            EventKind.HANDOFF_CONFIRMED -> fadeOutAndPause()
            EventKind.REMOTE_COMMAND -> event.remoteCommand?.let(::remote)
            else -> Unit
        }
    }

    /**
     * CORE-API handoff, receiving side (board 12): the core has put the queue in place;
     * load the song, say it is ready, and start at the position the core returns (the
     * sender's position plus the time since).
     */
    private fun handoffIncoming(h: HandoffInfo) {
        val entry = h.entry ?: run {
            runCatching { core.handoffFailed(h.transferId, "Nothing to play") }
            return
        }
        if (currentUid() != entry.uid) {
            val item = mediaItem(entry)
            if (item == null) {
                runCatching { core.handoffFailed(h.transferId, "This song can't be played here") }
                return
            }
            exo.setMediaItem(item)
            exo.prepare()
            clock.reset()
        }
        val start = runCatching { core.handoffReady(h.transferId) }.getOrElse { return }
        ownSeek = true
        exo.seekTo(start)
        exo.volume = h.volume.toFloat().coerceIn(0f, 1f)
        exo.playWhenReady = h.playing
        publish(entry)
    }

    /** The other device started: fade out and pause (`HandoffConfirmed`). */
    private fun fadeOutAndPause() {
        scope.launch {
            val from = exo.volume
            for (i in FADE_STEPS downTo 0) {
                exo.volume = from * i / FADE_STEPS
                delay(FADE_MS / FADE_STEPS)
            }
            exo.pause()
            exo.volume = from
        }
    }

    /** Another device controls this one: do it as if this device's own button was pressed. */
    private fun remote(c: RemoteCommand) {
        when (c.kind) {
            RemoteCommandKind.PLAY -> play()
            RemoteCommandKind.PAUSE -> pause()
            RemoteCommandKind.NEXT -> next(SkipOrigin.BUTTON)
            RemoteCommandKind.PREVIOUS -> previous(SkipOrigin.BUTTON)
            RemoteCommandKind.SEEK_TO -> c.positionMs?.let(::seekTo)
            RemoteCommandKind.SET_VOLUME -> c.level?.let { setVolume(it.toFloat()) }
            // Queue edits are done by the core; QueueChanged follows.
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
        if (currentUid() == entry.uid) {
            // Already loaded (the core's NowPlaying can arrive while a swipe settles, and
            // the settle paused it): still honour "keep playing" (board 42, rule 4).
            if (play && !exo.playWhenReady) exo.play()
            return
        }
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
        if (scanJob != null) {
            // Scanning reports one seek when it ends.
            publish()
            return
        }
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
            // The rolling cache's name for it (no token); a downloaded file isn't cached.
            .apply { if (!info.local) setCustomCacheKey(info.cacheKey) }
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

    private fun publish(entry: QueueEntry? = null, positionMs: Long? = null) {
        val current = entry ?: _state.value.entry.takeIf { it?.uid == currentUid() }
            ?: runCatching { core.queue().current }.getOrNull()
        _state.value = PlayerState(
            entry = current,
            isPlaying = exo.isPlaying,
            buffering = exo.playbackState == Player.STATE_BUFFERING,
            positionMs = positionMs ?: if (pendingSettle != null) 0 else exo.currentPosition.coerceAtLeast(0),
            durationMs = current?.song?.durationMs ?: 0,
        )
    }

    companion object {
        const val UID_PREFIX = "uid:"
        private const val TICK_MS = 1_000L
        private const val ARTWORK_PX = 512
        private const val MAX_ERRORS_IN_A_ROW = 3
        private const val SCAN_STEP_MS = 250L
        private const val SCAN_SPEED = 4
        private const val FADE_MS = 800L
        private const val FADE_STEPS = 16

        fun String.toUidOrNull(): Long? = removePrefix(UID_PREFIX).toLongOrNull()
    }
}
