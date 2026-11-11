package uk.gcartee.encore.app.playback

import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import androidx.annotation.OptIn
import androidx.annotation.VisibleForTesting
import androidx.media3.common.C
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
import uk.gcartee.encore.core.RepeatMode
import uk.gcartee.encore.core.SkipOrigin
import uk.gcartee.encore.core.SleepTimer
import uk.gcartee.encore.core.SleepTimerMode

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
 * the queue and the play log; this only loads what the core says is current, keeps the
 * song that plays next loaded right behind it (gapless, and Next is instant), and reports
 * what happened: `playback_started` when sound starts, `playback_progress` each second,
 * and `playback_ended` when a song finishes or fails. Skips need no report: the next
 * `playback_started` closes the play as skipped. Runs on the main thread.
 *
 * The player's list is the current song, then the next one. It is never edited inside
 * ExoPlayer's own callbacks: [tidy] runs just after, from the main looper. A song counts
 * as finished only when the player really reached its end ([reallyAtEnd]); otherwise
 * the end is not believed and the song plays again (seen on 8 November 2026: the player
 * ended the song it had just moved into, and the queue jumped one song too far).
 */
@OptIn(UnstableApi::class)
class CorePlayback(
    private val host: CoreHost,
    private val exo: ExoPlayer,
    private val headers: ConcurrentHashMap<String, Map<String, String>>,
    private val scope: CoroutineScope,
    /** The rolling cache holds the song under this cache key whole ([RollingCache.isWhole]). */
    private val wholeInCache: (String) -> Boolean = { false },
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
    /** The song each loaded item plays, by queue uid (see [adopt]). */
    private val songOfUid = java.util.concurrent.ConcurrentHashMap<Long, Long>()
    private val main = Handler(Looper.getMainLooper())
    private var tidyPending = false
    /** The song whose false end was already answered by playing it again ([notReallyEnded]). */
    private var replayedUid: Long? = null
    /** Cache keys of streams the server may transcode (no length, so no seeking at first). */
    private val transcodeKeys: MutableSet<String> = ConcurrentHashMap.newKeySet()
    /** A seek asked for while a transcoded song is still arriving: done once it's whole. */
    private var pendingSeekMs: Long? = null
    private var pendingSeekUid: Long? = null

    /** How many times the player said a song ended when it hadn't (tests read it). */
    @VisibleForTesting
    var falseEnds = 0
        private set

    // --- Phase 9: the sleep timer (CORE-API "Sleep timer"). ---
    /** The device's timer as the core last said (`SleepTimerChanged`); null: none. */
    private var sleepTimer: SleepTimer? = null
    private val sleepFade = SleepFade { SystemClock.elapsedRealtime() }
    /** The player's volume before the fade, put back after it; null while not fading. */
    private var fadeBase: Float? = null
    private var fadeJob: Job? = null

    /** The player's own volume (tests read it to see the fade). */
    @VisibleForTesting
    val playerVolume: Float
        get() = exo.volume

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
        // Phase 9: a timer set before the service started (or before a restart).
        sleepTimer = runCatching { core.sleepTimer() }.getOrNull()
        // open_profile loads the last queue, paused.
        runCatching { core.queue() }.getOrNull()?.current?.let { load(it, play = false) }
    }

    /** The profile is closing or the service stops: close the open play honestly. */
    fun stop() {
        main.removeCallbacksAndMessages(null)
        tidyPending = false
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
            exo.seekTo(currentIndex(), 0)
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
        if (exo.playbackState == Player.STATE_ENDED) exo.seekTo(currentIndex(), 0)
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
        val volume = level.coerceIn(0f, 1f)
        // Phase 9: during the sleep timer's fade the new volume is what the fade starts
        // from and what comes back after it; the fade applies it.
        if (fadeBase != null) fadeBase = volume else exo.volume = volume
        runCatching { core.setVolumeLevel(volume.toDouble()) }
    }

    /**
     * Seeks the current song. A transcoded stream (a capped quality) has no length, so the
     * player can't seek in it: once the rolling cache holds it whole it's opened again from
     * there, where it can; until then the seek waits for the rest of it (a few seconds).
     */
    fun seekTo(positionMs: Long) {
        val item = exo.currentMediaItem
        val key = item?.localConfiguration?.customCacheKey
        if (item == null || exo.isCurrentMediaItemSeekable || key == null || key !in transcodeKeys) {
            exo.seekTo(positionMs) // reported in onPositionDiscontinuity
            return
        }
        if (!reopenWhole(currentIndex(), item, positionMs)) {
            Log.i(TAG, "seek to $positionMs waits for the rest of the song")
            pendingSeekMs = positionMs
            pendingSeekUid = currentUid()
        }
    }

    /**
     * Opens [item] (at [index]) again from the rolling cache, where its length is known and
     * it can seek, at [positionMs] when given. False when it isn't a transcoded stream held
     * whole yet.
     */
    private fun reopenWhole(index: Int, item: MediaItem, positionMs: Long? = null): Boolean {
        val config = item.localConfiguration ?: return false
        val key = config.customCacheKey ?: return false
        val uri = config.uri.toString()
        if (key !in transcodeKeys || uri.endsWith(WHOLE) || !wholeInCache(key)) return false
        headers[uri]?.let { headers[uri + WHOLE] = it }
        val whole = item.buildUpon().setUri(uri + WHOLE).build()
        if (positionMs == null) {
            // Not playing yet (the next song): it plays from the start, seekable.
            exo.replaceMediaItem(index, whole)
            return true
        }
        val from = exo.currentPosition
        val play = exo.playWhenReady
        exo.replaceMediaItem(index, whole)
        ownSeek = true
        exo.seekTo(index, positionMs)
        exo.playWhenReady = play
        if (startedUid != null) core.playbackSeeked(from, positionMs)
        pendingSeekMs = null
        Log.i(TAG, "reopened from the cache to seek to $positionMs")
        return true
    }

    /** A load finished: a transcoded song now held whole becomes seekable. */
    override fun onIsLoadingChanged(isLoading: Boolean) {
        if (!isLoading) main.post { useWholeSongs() }
    }

    /**
     * After a load and on every tick: a seek waiting for its song does it now if the song
     * is whole, and the next song, once whole, is opened again from the cache before it
     * plays, so it plays seekable with its length.
     */
    private fun useWholeSongs() {
        val at = pendingSeekMs
        val item = exo.currentMediaItem
        if (at != null && item != null && currentUid() == pendingSeekUid) {
            if (!reopenWhole(currentIndex(), item, at) && currentArrived()) {
                // Arrived but not kept whole (the rolling cache is off): it can't seek.
                Log.i(TAG, "seek dropped: the song isn't in the rolling cache")
                pendingSeekMs = null
            }
        }
        // The current song may have just settled: add the next one now.
        keepNextLoaded()
        // Not in the current song's last seconds: the player may already be reading into
        // the next one for a gapless change, and swapping it then would break the gap.
        val left = if (exo.duration == C.TIME_UNSET) Long.MAX_VALUE else exo.duration - exo.currentPosition
        val next = nextIndex()
        if (left > LATE_SWAP_MS && next < exo.mediaItemCount && reopenWhole(next, exo.getMediaItemAt(next))) {
            Log.i(TAG, "next song opened from the cache, whole: seekable, its length known")
        }
    }

    // --- Core events. ---

    private fun onEvent(event: CoreEvent) {
        when (event.kind) {
            // A swipe that is still settling loads its song itself.
            EventKind.NOW_PLAYING -> if (pendingSettle == null) show(event.nowPlaying, play = exo.playWhenReady)
            // The next song is kept loaded from the start of each song; the core's own
            // reminder near the end only re-checks it.
            EventKind.PRELOAD -> tidy()
            EventKind.QUEUE_CHANGED -> event.queue?.let(::queueChanged)
            EventKind.HANDOFF_INCOMING -> event.handoff?.let(::handoffIncoming)
            EventKind.HANDOFF_CONFIRMED -> {
                // Phase 9: a handoff ends the sleep timer; its fade stops first.
                abortFade()
                fadeOutAndPause()
            }
            EventKind.REMOTE_COMMAND -> event.remoteCommand?.let(::remote)
            EventKind.SLEEP_TIMER_CHANGED -> sleepTimerChanged(event.sleepTimer)
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
        val next = nextIndex()
        if (next < exo.mediaItemCount && (uidAt(next) == entry.uid || adopt(next, entry))) {
            Log.i(TAG, "skip onto the loaded next song (uid ${entry.uid})")
            exo.seekToDefaultPosition(next)
            if (play) exo.play()
            tidy()
            publish()
            return
        }
        load(entry, play)
    }

    private fun load(entry: QueueEntry, play: Boolean) {
        val item = mediaItem(entry) ?: return
        Log.i(TAG, "load uid ${entry.uid} (play=$play)")
        exo.setMediaItem(item)
        exo.prepare()
        exo.playWhenReady = play
        clock.reset()
        publish(entry)
    }

    private fun currentIndex(): Int = exo.currentMediaItemIndex

    private fun nextIndex(): Int = exo.currentMediaItemIndex + 1

    /**
     * Soon, from the main looper (never inside ExoPlayer's own callbacks): drops the songs
     * already played from the player's list and makes sure the song after the current one
     * is the one the core will move to, loading it now so it's ready long before the end
     * (and Next starts at once).
     */
    private fun tidy() {
        if (tidyPending) return
        tidyPending = true
        main.post {
            tidyPending = false
            dropPlayed()
            keepNextLoaded()
        }
    }

    private fun dropPlayed() {
        val current = currentIndex()
        if (current > 0 && current < exo.mediaItemCount) exo.removeMediaItems(0, current)
    }

    private fun keepNextLoaded() {
        if (exo.mediaItemCount == 0 || currentUid() == null) return
        val next = nextIndex()
        if (!currentSettled()) {
            // A transcoded current song still arriving, or waiting to seek: the next one is
            // added after it. Reopening the current song (or any seek in it) makes ExoPlayer
            // drop the next song's download and start it again, and on a capped quality each
            // new start is another transcode on the server (seen on the owner's Jellyfin:
            // one song asked for three times). The player reads songs one after another
            // anyway, so this costs no time.
            if (exo.mediaItemCount > next) exo.removeMediaItems(next, exo.mediaItemCount)
            return
        }
        val target = songAfterThis()
        if (exo.mediaItemCount > next + 1) exo.removeMediaItems(next + 1, exo.mediaItemCount)
        if (target == null) {
            if (exo.mediaItemCount > next) exo.removeMediaItem(next)
            return
        }
        if (uidAt(next) == target.uid || adopt(next, target)) return
        val item = mediaItem(target)
        when {
            item == null -> if (exo.mediaItemCount > next) exo.removeMediaItem(next)
            exo.mediaItemCount > next -> exo.replaceMediaItem(next, item)
            else -> exo.addMediaItem(item)
        }
        Log.i(TAG, "next loaded: uid ${target.uid}")
    }

    /**
     * The current song won't be reopened soon: it isn't a transcoded stream, or it's
     * already open from the cache, or it has fully arrived with no seek waiting.
     */
    private fun currentSettled(): Boolean {
        val config = exo.currentMediaItem?.localConfiguration ?: return true
        val key = config.customCacheKey ?: return true
        if (key !in transcodeKeys || config.uri.toString().endsWith(WHOLE)) return true
        if (pendingSeekMs != null) return false
        return wholeInCache(key) || currentArrived()
    }

    /** All of the current song is buffered (a transcode's length is known once it has). */
    private fun currentArrived(): Boolean {
        val duration = exo.duration
        return duration != C.TIME_UNSET && exo.bufferedPosition >= duration - ARRIVED_SLACK_MS
    }

    /** What plays when the current song ends by itself: the next one, or itself on Repeat One. */
    private fun songAfterThis(): QueueEntry? {
        // Phase 9: the sleep timer stops after this song: nothing is loaded behind it, so
        // it ends by itself (STATE_ENDED) and never runs on into the next one gaplessly.
        if (sleepStopsAfter(currentUid())) return null
        val queue = runCatching { core.queue() }.getOrNull() ?: return null
        if (queue.repeat == RepeatMode.ONE) return queue.current
        return runCatching { core.nextTarget().song }.getOrNull()
    }

    /**
     * The item at [index] already plays [entry]'s song under an older queue uid (Autoplay
     * refills give its songs new uids): take the new uid and details in place. The address
     * is the same, so the player keeps what it has loaded. Without this the same song was
     * fetched again, and on a capped quality that was another transcode on the server
     * (seen on the owner's Jellyfin, 8 November 2026).
     */
    private fun adopt(index: Int, entry: QueueEntry): Boolean {
        if (index >= exo.mediaItemCount) return false
        val uid = uidAt(index) ?: return false
        if (uid == entry.uid) return true
        if (songOfUid[uid] != entry.song.id) return false
        val item = mediaItem(entry) ?: return false
        if (item.localConfiguration?.uri != exo.getMediaItemAt(index).localConfiguration?.uri) return false
        exo.replaceMediaItem(index, item)
        return true
    }

    /** The queue changed (a reorder, Play Next, Autoplay refilled): the next song may differ. */
    private fun queueChanged(state: QueueState) {
        if (exo.mediaItemCount == 0) return
        tidy()
        publish(state.current)
    }

    // --- Player events. ---

    override fun onIsPlayingChanged(isPlaying: Boolean) {
        clock.playing(isPlaying)
        val uid = currentUid()
        // Phase 9: a sleep timer that ran out while paused closes that play first, so
        // Play now starts a new one (`playback_started`, not `playback_resumed`).
        if (isPlaying && uid != null && startedUid == uid) endSleepTimerIfRunOut()
        // Phase 9: a pause during the fade stops it and puts the volume back.
        if (!isPlaying && !exo.playWhenReady && fadeBase != null) abortFade()
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
            if (!reallyAtEnd()) {
                notReallyEnded()
                publish()
                return
            }
            // The last loaded song finished with nothing after it in the player.
            Log.i(TAG, "uid $startedUid finished (end of the player's list)")
            // Phase 9: decided on the stop point held before `playback_ended` moves it.
            val sleepStop = sleepStopsAfter(startedUid)
            val next = core.playbackEnded(EndReason.FINISHED, clock.listenedMs)
            startedUid = null
            if (sleepStop) {
                stoppedAtSongEnd(next)
            } else {
                next?.let { load(it, play = true) }
            }
        }
        publish()
    }

    /** The player is at the end of the current song (not just told it ended). */
    private fun reallyAtEnd(): Boolean {
        val duration = exo.duration
        if (duration == C.TIME_UNSET || duration <= 0) return true
        return exo.currentPosition >= duration - END_SLACK_MS
    }

    /**
     * The player said the song ended at the start of it. Play it again rather than move
     * the queue on (a second false end of the same song is treated as an error: skipped).
     */
    private fun notReallyEnded() {
        val uid = startedUid
        falseEnds++
        Log.w(TAG, "the player ended uid $uid at ${exo.currentPosition} of ${exo.duration} ms; not moving on")
        if (uid != null && replayedUid == uid) {
            replayedUid = null
            val next = runCatching { core.playbackEnded(EndReason.ERROR, clock.listenedMs) }.getOrNull()
            startedUid = null
            if (next != null) load(next, play = true) else exo.stop()
            return
        }
        replayedUid = uid
        ownSeek = true
        exo.seekTo(currentIndex(), 0)
        exo.prepare()
        exo.play()
    }

    override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
        // A seek waiting for its song to arrive whole doesn't carry over to another song.
        if (currentUid() != pendingSeekUid) pendingSeekMs = null
        if (reason != Player.MEDIA_ITEM_TRANSITION_REASON_AUTO) {
            startedIfPlaying()
            publish()
            return
        }
        // Gapless into the next song: the previous one finished. The finished item stays
        // in the player's list until tidy() runs, after this callback.
        // Phase 9: the sleep timer stops after the song that just finished. Nothing should
        // have been loaded behind it (songAfterThis), but if it was, stop here at once.
        val sleepStop = sleepStopsAfter(startedUid)
        if (sleepStop) exo.pause()
        val heard = clock.listenedMs
        val next = core.playbackEnded(EndReason.FINISHED, heard)
        startedUid = null
        val arrived = mediaItem?.mediaId?.toUidOrNull()
        if (sleepStop) {
            if (next != null && next.uid == arrived) {
                ownSeek = true
                exo.seekTo(currentIndex(), 0)
                stoppedAtSongEnd(null)
            } else {
                if (next == null) exo.stop()
                stoppedAtSongEnd(next)
            }
            publish()
            return
        }
        Log.i(TAG, "moved on by itself to uid $arrived; the core says uid ${next?.uid}")
        when {
            next == null -> exo.stop()
            next.uid == arrived -> started(next.uid)
            // The same song under a new uid (Autoplay refilled): keep playing what arrived.
            adopt(currentIndex(), next) -> started(next.uid)
            else -> load(next, play = true)
        }
        tidy()
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
        if (replayedUid != uid) replayedUid = null
        // Load the song after this one now, so it's ready long before the end.
        tidy()
    }

    private fun tick() {
        if (exo.isPlaying && startedUid != null) {
            core.playbackProgress(exo.currentPosition, clock.listenedMs)
        }
        // Phase 9: the sleep timer's fade while playing; its end while paused.
        if (exo.isPlaying) sleepTick() else if (!exo.playWhenReady) endSleepTimerIfRunOut()
        if (transcodeKeys.isNotEmpty() && exo.mediaItemCount > 0) useWholeSongs()
        publish()
    }

    // --- Phase 9: the sleep timer (CORE-API "Sleep timer"; board 31). The core owns the
    // timer, its stop point and the fade rule; this runs the clock and the fade on the
    // player's own volume (never setVolume, which tells other devices), and reports the
    // end so the play log reads "ended by the system", never a skip. ---

    /** Set, extended, cancelled, put back or ended, or its stop point moved with the queue. */
    private fun sleepTimerChanged(timer: SleepTimer?) {
        val before = sleepTimer
        sleepTimer = timer
        if (timer == null) {
            // It ran out while paused and the core found it (`sleep_timer()` closes that
            // paused play as ended by the system): the next Play is a new play.
            if (before != null && ranOut(before) && !exo.playWhenReady) startedUid = null
            // Cancelled, or ended by a handoff: no fade.
            abortFade()
        } else if (fadeBase != null && exo.isPlaying) {
            // Add 15 min during the fade: the core now says KeepPlaying and the fade stops.
            sleepTick()
        }
        // Where preloading stops may have moved: drop, or load again, the song behind.
        tidy()
    }

    /** A Minutes timer whose end has passed. */
    private fun ranOut(timer: SleepTimer): Boolean =
        timer.mode == SleepTimerMode.MINUTES && timer.endsAtUtcMs <= System.currentTimeMillis()

    /** From the 1 s tick while playing: ask the core's fade rule and follow it. */
    private fun sleepTick() {
        if (sleepTimer == null && fadeBase == null) return
        val position = exo.currentPosition.coerceAtLeast(0L)
        val plan = runCatching { core.sleepFadePlan(position, songDurationMs()) }.getOrNull() ?: return
        sleepFade.update(plan, position)
        when {
            sleepFade.timeIsUp(position) -> endBySleepTimer()
            sleepFade.fading -> beginFade()
            fadeBase != null -> abortFade()
        }
    }

    /** The current song's length (0: unknown), the library's when the stream has none. */
    private fun songDurationMs(): Long {
        val duration = exo.duration
        if (duration != C.TIME_UNSET && duration > 0) return duration
        return _state.value.entry?.takeIf { it.uid == currentUid() }?.song?.durationMs ?: 0L
    }

    /** Ramps the player's own volume about 16 times a second until the fade ends or stops. */
    private fun beginFade() {
        if (fadeBase == null) {
            fadeBase = exo.volume
            Log.i(TAG, "sleep timer: fading out")
            // A song that ends during the fade is where the music stops: nothing behind it.
            tidy()
        }
        if (fadeJob?.isActive == true) return
        fadeJob = scope.launch {
            while (isActive) {
                val base = fadeBase ?: break
                if (!sleepFade.fading) break
                val position = exo.currentPosition.coerceAtLeast(0L)
                if (sleepFade.timeIsUp(position)) {
                    endBySleepTimer()
                    break
                }
                exo.volume = base * sleepFade.level(position)
                delay(FADE_STEP_MS)
            }
        }
    }

    /**
     * Silence reached (FadeNow): pause, then close the play as ended by the system with
     * what was heard (`sleep_timer_ended`), and put the volume back. The play is closed,
     * so the next Play reports `playback_started` (startedUid is cleared), not a resume.
     */
    private fun endBySleepTimer() {
        val base = fadeBase
        fadeJob?.cancel()
        fadeJob = null
        fadeBase = null
        sleepFade.clear()
        // Reported as an ordinary pause (`playback_paused`), which never logs a skip.
        exo.pause()
        val open = startedUid != null
        val listened = if (open) clock.listenedMs else 0L
        startedUid = null
        sleepTimer = null
        runCatching { core.sleepTimerEnded(listened) }
        if (base != null) exo.volume = base
        Log.i(TAG, "sleep timer: ended, paused")
        tidy()
        publish()
    }

    /**
     * The song the timer stops after has ended and been reported `Finished`: the next one
     * is loaded and paused, the timer ends with nothing more heard (`sleep_timer_ended(0)`),
     * and the volume comes back if it was fading.
     */
    private fun stoppedAtSongEnd(next: QueueEntry?) {
        val base = fadeBase
        fadeJob?.cancel()
        fadeJob = null
        fadeBase = null
        sleepFade.clear()
        sleepTimer = null
        next?.let { load(it, play = false) }
        runCatching { core.sleepTimerEnded(0L) }
        if (base != null) exo.volume = base
        Log.i(TAG, "sleep timer: stopped at the end of the song, next one loaded and paused")
    }

    /** Pause, handoff, cancel or Add 15 min during the fade: it stops, the volume comes back. */
    private fun abortFade() {
        fadeJob?.cancel()
        fadeJob = null
        sleepFade.clear()
        val base = fadeBase ?: return
        fadeBase = null
        exo.volume = base
        Log.i(TAG, "sleep timer: fade stopped, volume back")
        // The song behind this one, dropped for the stop, loads again.
        tidy()
    }

    /**
     * A Minutes timer that ran out while paused (the tick doesn't fade then): close that
     * play as ended by the system with what was heard, so it never reads as a skip.
     */
    private fun endSleepTimerIfRunOut() {
        val timer = sleepTimer ?: return
        if (!ranOut(timer)) return
        val listened = if (startedUid != null) clock.listenedMs else 0L
        startedUid = null
        sleepTimer = null
        abortFade()
        runCatching { core.sleepTimerEnded(listened) }
        Log.i(TAG, "sleep timer: ran out while paused")
    }

    /**
     * The sleep timer stops after [uid]: a song or album timer's stop point (the uid held
     * now, before `playback_ended` moves it), or any song that ends while fading.
     */
    private fun sleepStopsAfter(uid: Long?): Boolean {
        if (uid == null) return false
        if (sleepFade.stopsAtSongEnd) return true
        val timer = sleepTimer ?: return false
        return timer.mode != SleepTimerMode.MINUTES && timer.stopAfterUid == uid
    }

    // --- Helpers. ---

    private fun mediaItem(entry: QueueEntry): MediaItem? {
        val info = runCatching { core.streamFor(entry.song.id, host.network()) }.getOrElse {
            // SessionExpired and the like: the core says why on the next call; skip it.
            return null
        }
        headers[info.url] = info.headers.associate { it.name to it.value }
        if (songOfUid.size > SONGS_REMEMBERED) songOfUid.clear()
        songOfUid[entry.uid] = entry.song.id
        // A transcoded song the rolling cache already holds whole (played before) opens
        // from there at once: its length is known, so it can seek from the start.
        val transcoded = !info.local && info.mayTranscode
        if (transcoded) transcodeKeys.add(info.cacheKey)
        val uri = if (transcoded && wholeInCache(info.cacheKey)) {
            headers[info.url + WHOLE] = headers.getValue(info.url)
            info.url + WHOLE
        } else {
            info.url
        }
        val art = runCatching { core.artwork(ArtworkOf.SONG, entry.song.id, ARTWORK_PX.toUInt()) }.getOrNull()
        art?.let { headers[it.url] = it.headers.associate { h -> h.name to h.value } }
        val song = entry.song
        return MediaItem.Builder()
            .setMediaId("$UID_PREFIX${entry.uid}")
            .setUri(uri)
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
        private const val TAG = "CorePlayback"
        /**
         * Marks an address to be read from the rolling cache, where the song is whole: a
         * fragment, so it's never sent to the server, and the player sees a different item.
         */
        private const val WHOLE = "#whole"
        /** How close to the end a song must be for the player's "ended" to count. */
        private const val END_SLACK_MS = 3_000L
        /** No swapping the next song for its cached copy this close to the current one's end. */
        private const val LATE_SWAP_MS = 8_000L
        private const val ARRIVED_SLACK_MS = 1_000L
        private const val TICK_MS = 1_000L
        private const val ARTWORK_PX = 512
        private const val MAX_ERRORS_IN_A_ROW = 3
        private const val SONGS_REMEMBERED = 512
        private const val SCAN_STEP_MS = 250L
        private const val SCAN_SPEED = 4
        private const val FADE_MS = 800L
        private const val FADE_STEPS = 16
        /** Phase 9: the sleep timer's fade sets the volume about 16 times a second. */
        private const val FADE_STEP_MS = 62L

        fun String.toUidOrNull(): Long? = removePrefix(UID_PREFIX).toLongOrNull()
    }
}
