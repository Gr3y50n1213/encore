package uk.gcartee.encore.app.playback.tv

import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.DeviceInfo
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * ADR android-0007: a TV Encore drives itself (DLNA, later Roku) as a Media3 player, so the
 * media session, the lock screen, the notification, Bluetooth buttons and the volume keys
 * all drive the TV, and CorePlayback reports plays from it exactly as from ExoPlayer. It is
 * a remote device ([DeviceInfo.PLAYBACK_TYPE_REMOTE]) with the TV's own volume.
 *
 * The playlist is what CorePlayback gives it: the current song and the one after. The TV is
 * told one song at a time ([Renderer.setUri]); when it stops by itself at the end of a song,
 * this moves on to the next item as an automatic transition (CorePlayback then reports the
 * song finished), or ends. The TV's state is read every second on this player's own thread;
 * every command goes there too, in order, never on the main thread.
 */
@OptIn(UnstableApi::class)
class TvPlayer(
    private val renderer: Renderer,
    /** The TV hasn't answered for [LOST_AFTER] polls in a row. Called on the main thread. */
    private val onLost: () -> Unit,
    /** The TV refused the session's first song, so nothing will play. Called on the main thread. */
    private val onRefused: () -> Unit = {},
    looper: Looper = Looper.getMainLooper(),
) : SimpleBasePlayer(looper) {
    private class Item(val uid: Long, val media: MediaItem)

    private val handler = Handler(looper)
    private val io: ExecutorService = Executors.newSingleThreadExecutor { r -> Thread(r, "TvPlayer").apply { isDaemon = true } }
    private val items = ArrayList<Item>()
    private var nextUid = 1L
    private var index = 0
    private var playWhenReady = false
    private var playWhenReadyReason = Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST
    private var playbackState = Player.STATE_IDLE
    private var positionMs = 0L
    private var positionAt = SystemClock.elapsedRealtime()
    /** The TV is playing: the position moves on between polls. */
    private var moving = false
    private var volume = renderer.maxVolume / 2
    private var error: PlaybackException? = null
    /** Report the next state change as an automatic move to the next song. */
    private var autoTransition = false
    /** The item the TV has been told to play. */
    private var loadedUid: Long? = null
    /** The TV has said PLAYING for the song it has: a STOPPED after that is its end. */
    private var heardPlaying = false
    /** Bumped by every command: a poll asked before one is out of date. */
    private var generation = 0
    /** A TV can say STOPPED for a moment while it starts or seeks: not an end before this. */
    private var quietUntil = 0L
    /** Where the TV last said it was while playing. */
    private var lastHeardMs = 0L
    private var failures = 0
    private var polls = 0
    private var released = false
    /** On [io] only: the TV has taken a song, and the owner of this player was told it never did. */
    private var taken = false
    private var toldRefused = false

    private val commands = Player.Commands.Builder()
        .addAll(
            Player.COMMAND_PLAY_PAUSE,
            Player.COMMAND_PREPARE,
            Player.COMMAND_STOP,
            Player.COMMAND_SEEK_TO_DEFAULT_POSITION,
            Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_PREVIOUS,
            Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_NEXT,
            Player.COMMAND_SEEK_TO_MEDIA_ITEM,
            Player.COMMAND_SET_MEDIA_ITEM,
            Player.COMMAND_CHANGE_MEDIA_ITEMS,
            Player.COMMAND_GET_CURRENT_MEDIA_ITEM,
            Player.COMMAND_GET_TIMELINE,
            Player.COMMAND_GET_METADATA,
            Player.COMMAND_GET_DEVICE_VOLUME,
            Player.COMMAND_SET_DEVICE_VOLUME_WITH_FLAGS,
            Player.COMMAND_ADJUST_DEVICE_VOLUME_WITH_FLAGS,
            Player.COMMAND_RELEASE,
        )
        .build()

    private val deviceInfo = DeviceInfo.Builder(DeviceInfo.PLAYBACK_TYPE_REMOTE)
        .setMinVolume(0)
        .setMaxVolume(renderer.maxVolume)
        .build()

    /** The TV's name, for "Playing on <TV>". */
    val name: String get() = renderer.name

    override fun getState(): State {
        val playlist = items.map { item ->
            val duration = item.media.mediaMetadata.durationMs ?: 0L
            MediaItemData.Builder(item.uid)
                .setMediaItem(item.media)
                .setIsSeekable(true)
                .setDurationUs(if (duration > 0) duration * 1_000 else C.TIME_UNSET)
                .build()
        }
        val state = when {
            playlist.isEmpty() && playbackState != Player.STATE_IDLE -> Player.STATE_ENDED
            else -> playbackState
        }
        val now = currentPositionMs()
        val builder = State.Builder()
            .setAvailableCommands(commands)
            .setPlayWhenReady(playWhenReady, playWhenReadyReason)
            .setPlaybackState(state)
            .setPlayerError(error)
            .setDeviceInfo(deviceInfo)
            .setDeviceVolume(volume.coerceIn(0, renderer.maxVolume))
            .setPlaylist(playlist)
            .setCurrentMediaItemIndex(if (playlist.isEmpty()) C.INDEX_UNSET else index.coerceIn(0, playlist.lastIndex))
            .setContentPositionMs(
                if (moving) PositionSupplier.getExtrapolating(now, 1f) else PositionSupplier.getConstant(now),
            )
        if (autoTransition) {
            autoTransition = false
            builder.setPositionDiscontinuity(Player.DISCONTINUITY_REASON_AUTO_TRANSITION, 0)
        }
        return builder.build()
    }

    private fun currentPositionMs(): Long {
        if (!moving) return positionMs
        val at = positionMs + (SystemClock.elapsedRealtime() - positionAt)
        val duration = current()?.media?.mediaMetadata?.durationMs ?: 0L
        return if (duration > 0) at.coerceAtMost(duration) else at
    }

    private fun setPosition(ms: Long) {
        positionMs = ms.coerceAtLeast(0)
        positionAt = SystemClock.elapsedRealtime()
    }

    private fun current(): Item? = items.getOrNull(index)

    // --- Commands (main thread): the state changes at once, the TV is told on [io]. ---

    override fun handleSetMediaItems(mediaItems: MutableList<MediaItem>, startIndex: Int, startPositionMs: Long): ListenableFuture<*> {
        val keepAt = startIndex == C.INDEX_UNSET
        val at = if (keepAt) currentPositionMs() else if (startPositionMs == C.TIME_UNSET) 0L else startPositionMs
        items.clear()
        mediaItems.forEach { items.add(Item(nextUid++, it)) }
        index = if (keepAt) index.coerceIn(0, (items.size - 1).coerceAtLeast(0)) else startIndex.coerceIn(0, (items.size - 1).coerceAtLeast(0))
        setPosition(at)
        loadedUid = null
        if (items.isEmpty()) {
            send { renderer.stop() }
            moving = false
        } else if (playbackState != Player.STATE_IDLE) {
            loadCurrent()
        }
        return done()
    }

    override fun handleAddMediaItems(index: Int, mediaItems: MutableList<MediaItem>): ListenableFuture<*> {
        val at = index.coerceIn(0, items.size)
        val wasEmpty = items.isEmpty()
        items.addAll(at, mediaItems.map { Item(nextUid++, it) })
        if (!wasEmpty && at <= this.index) this.index += mediaItems.size
        return done()
    }

    override fun handleRemoveMediaItems(fromIndex: Int, toIndex: Int): ListenableFuture<*> {
        val from = fromIndex.coerceIn(0, items.size)
        val to = toIndex.coerceIn(from, items.size)
        val removesCurrent = index in from until to
        repeat(to - from) { items.removeAt(from) }
        when {
            removesCurrent -> {
                index = from.coerceAtMost((items.size - 1).coerceAtLeast(0))
                setPosition(0)
                loadedUid = null
                if (items.isEmpty()) {
                    send { renderer.stop() }
                    moving = false
                } else if (playbackState != Player.STATE_IDLE) {
                    loadCurrent()
                }
            }
            index >= to -> index -= (to - from)
        }
        return done()
    }

    override fun handleMoveMediaItems(fromIndex: Int, toIndex: Int, newIndex: Int): ListenableFuture<*> {
        val currentUid = current()?.uid
        val moved = ArrayList(items.subList(fromIndex, toIndex))
        repeat(toIndex - fromIndex) { items.removeAt(fromIndex) }
        items.addAll(newIndex.coerceIn(0, items.size), moved)
        index = items.indexOfFirst { it.uid == currentUid }.coerceAtLeast(0)
        return done()
    }

    override fun handleReplaceMediaItems(fromIndex: Int, toIndex: Int, mediaItems: MutableList<MediaItem>): ListenableFuture<*> {
        val currentUid = current()?.uid
        val oldCurrentUri = current()?.media?.localConfiguration?.uri
        val replacesCurrent = index in fromIndex until toIndex
        repeat(toIndex - fromIndex) { items.removeAt(fromIndex) }
        val added = mediaItems.map { Item(nextUid++, it) }
        items.addAll(fromIndex, added)
        if (replacesCurrent) {
            // Same place in the list; the TV only needs telling if the song's address changed.
            index = index.coerceIn(0, (items.size - 1).coerceAtLeast(0))
            val now = current()
            if (now != null && now.media.localConfiguration?.uri != oldCurrentUri && playbackState != Player.STATE_IDLE) {
                loadCurrent(currentPositionMs())
            } else if (now != null && loadedUid == currentUid) {
                loadedUid = now.uid
            }
        } else if (index >= toIndex) {
            index += mediaItems.size - (toIndex - fromIndex)
        }
        return done()
    }

    override fun handlePrepare(): ListenableFuture<*> {
        error = null
        if (items.isEmpty()) {
            playbackState = Player.STATE_ENDED
            return done()
        }
        if (playbackState == Player.STATE_IDLE || loadedUid != current()?.uid) {
            playbackState = Player.STATE_BUFFERING
            loadCurrent(currentPositionMs())
        }
        startPolling()
        return done()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        this.playWhenReady = playWhenReady
        playWhenReadyReason = Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST
        if (playbackState == Player.STATE_IDLE || items.isEmpty()) return done()
        if (playWhenReady && playbackState == Player.STATE_ENDED) return done()
        if (loadedUid != current()?.uid) {
            loadCurrent(currentPositionMs())
            return done()
        }
        setPosition(currentPositionMs())
        moving = false
        if (playWhenReady) send { renderer.play() } else send { renderer.pause() }
        return done()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        val target = if (mediaItemIndex == C.INDEX_UNSET) index else mediaItemIndex
        val at = if (positionMs == C.TIME_UNSET) 0L else positionMs
        if (target !in items.indices) return done()
        if (target != index || loadedUid != items[target].uid) {
            index = target
            setPosition(at)
            if (playbackState != Player.STATE_IDLE) {
                if (playbackState == Player.STATE_ENDED) playbackState = Player.STATE_BUFFERING
                loadCurrent(at)
            }
            return done()
        }
        if (playbackState == Player.STATE_ENDED) playbackState = Player.STATE_BUFFERING
        setPosition(at)
        send { renderer.seek(at) }
        return done()
    }

    override fun handleStop(): ListenableFuture<*> {
        playbackState = Player.STATE_IDLE
        moving = false
        setPosition(currentPositionMs())
        loadedUid = null
        send { renderer.stop() }
        return done()
    }

    override fun handleRelease(): ListenableFuture<*> {
        released = true
        handler.removeCallbacksAndMessages(null)
        io.execute { runCatching { renderer.stop() } }
        io.shutdown()
        return done()
    }

    override fun handleSetDeviceVolume(deviceVolume: Int, flags: Int): ListenableFuture<*> {
        volume = deviceVolume.coerceIn(0, renderer.maxVolume)
        val level = volume
        send(bump = false) { renderer.setVolume(level) }
        return done()
    }

    override fun handleIncreaseDeviceVolume(flags: Int): ListenableFuture<*> = handleSetDeviceVolume(volume + volumeStep(), flags)

    override fun handleDecreaseDeviceVolume(flags: Int): ListenableFuture<*> = handleSetDeviceVolume(volume - volumeStep(), flags)

    private fun volumeStep() = (renderer.maxVolume / VOLUME_STEPS).coerceAtLeast(1)

    /** Tells the TV to play the current item from [fromMs] (then pause, if paused). */
    private fun loadCurrent(fromMs: Long = currentPositionMs()) {
        val item = current() ?: return
        val config = item.media.localConfiguration ?: return
        val url = config.uri.toString()
        val metadata = item.media.mediaMetadata
        val meta = TvMeta(
            title = metadata.title?.toString().orEmpty(),
            artist = metadata.artist?.toString(),
            album = metadata.albumTitle?.toString(),
            coverUrl = metadata.extras?.getString(EXTRA_COVER),
            mimeType = config.mimeType ?: MimeTypes.AUDIO_MPEG,
            durationMs = metadata.durationMs ?: 0L,
        )
        loadedUid = item.uid
        heardPlaying = false
        lastHeardMs = fromMs
        moving = false
        setPosition(fromMs)
        if (playbackState != Player.STATE_IDLE) playbackState = Player.STATE_BUFFERING
        val play = playWhenReady
        send {
            try {
                renderer.setUri(url, meta)
            } catch (e: Exception) {
                if (!taken && !toldRefused) {
                    toldRefused = true
                    handler.post { if (!released) onRefused() }
                }
                throw e
            }
            taken = true
            // Most TVs can't seek before they play: play, seek, then pause if paused.
            if (play || fromMs > SEEK_MIN_MS) renderer.play()
            if (fromMs > SEEK_MIN_MS) renderer.seek(fromMs)
            if (!play && fromMs > SEEK_MIN_MS) renderer.pause()
        }
    }

    /** Runs [block] on the TV's thread; a failure is a player error, not a crash. */
    private fun send(bump: Boolean = true, block: () -> Unit) {
        if (released) return
        if (bump) {
            generation++
            quietUntil = SystemClock.elapsedRealtime() + QUIET_MS
        }
        runCatching {
            io.execute {
                try {
                    block()
                } catch (e: Exception) {
                    // Never with the address: it carries the session's pass.
                    Log.w(TAG, "the TV refused a command: ${e.javaClass.simpleName}")
                }
            }
        }
    }

    /** Done at once: SimpleBasePlayer reads [getState] again after each command. */
    private fun done(): ListenableFuture<*> = Futures.immediateVoidFuture()

    // --- Polling: the TV's state each second, on [io]; applied on the main thread. ---

    private var pollingStarted = false

    private fun startPolling() {
        if (pollingStarted || released) return
        pollingStarted = true
        handler.postDelayed(::poll, POLL_MS)
    }

    private fun poll() {
        if (released) return
        val asked = generation
        val withVolume = polls++ % VOLUME_EVERY == 0
        runCatching {
            io.execute {
                val result = runCatching { renderer.poll(withVolume) }
                handler.post { apply(result, asked) }
            }
        }
        handler.postDelayed(::poll, POLL_MS)
    }

    private fun apply(result: Result<RendererStatus>, asked: Int) {
        if (released) return
        val status = result.getOrElse {
            failures++
            if (failures == LOST_AFTER) {
                Log.w(TAG, "the TV stopped answering")
                onLost()
            }
            return
        }
        failures = 0
        status.volume?.let { volume = it.coerceIn(0, renderer.maxVolume) }
        if (asked != generation || playbackState == Player.STATE_IDLE || loadedUid == null) {
            // A command went out since: this answer is about before it.
            invalidateState()
            return
        }
        when (status.state) {
            RendererState.PLAYING -> {
                heardPlaying = true
                lastHeardMs = status.positionMs
                setPosition(status.positionMs)
                moving = true
                playbackState = Player.STATE_READY
                if (!playWhenReady) {
                    // Started on the TV's own remote.
                    playWhenReady = true
                    playWhenReadyReason = Player.PLAY_WHEN_READY_CHANGE_REASON_REMOTE
                }
            }
            RendererState.PAUSED -> {
                setPosition(status.positionMs)
                moving = false
                playbackState = Player.STATE_READY
                if (playWhenReady && heardPlaying) {
                    // Paused on the TV's own remote.
                    playWhenReady = false
                    playWhenReadyReason = Player.PLAY_WHEN_READY_CHANGE_REASON_REMOTE
                }
            }
            RendererState.TRANSITIONING -> {
                moving = false
                playbackState = Player.STATE_BUFFERING
            }
            RendererState.STOPPED, RendererState.NO_MEDIA -> {
                moving = false
                val duration = current()?.media?.mediaMetadata?.durationMs ?: 0L
                val nearEnd = duration <= 0 || lastHeardMs >= duration - END_SLACK_MS
                if (heardPlaying && playWhenReady && SystemClock.elapsedRealtime() >= quietUntil) {
                    if (nearEnd) {
                        ended()
                    } else {
                        // Stopped on the TV's own remote mid-song: paused here, told again on Play.
                        setPosition(lastHeardMs)
                        playWhenReady = false
                        playWhenReadyReason = Player.PLAY_WHEN_READY_CHANGE_REASON_REMOTE
                        playbackState = Player.STATE_READY
                        loadedUid = null
                    }
                } else if (playbackState == Player.STATE_BUFFERING && !playWhenReady) {
                    // Told the song and not to play it: ready, paused.
                    playbackState = Player.STATE_READY
                }
            }
        }
        invalidateState()
    }

    /** The TV stopped at the end of the song: the next item, or the end. */
    private fun ended() {
        val duration = current()?.media?.mediaMetadata?.durationMs ?: 0L
        if (index + 1 < items.size) {
            index++
            autoTransition = true
            setPosition(0)
            loadCurrent(0)
        } else {
            setPosition(if (duration > 0) duration else currentPositionMs())
            playbackState = Player.STATE_ENDED
            heardPlaying = false
        }
    }

    companion object {
        private const val TAG = "TvPlayer"
        /** MediaMetadata extras key: the cover through the relay, for the TV only. */
        const val EXTRA_COVER = "uk.gcartee.encore.tv.cover"
        private const val POLL_MS = 1_000L
        private const val VOLUME_EVERY = 5
        private const val LOST_AFTER = 6
        private const val VOLUME_STEPS = 20
        private const val SEEK_MIN_MS = 1_000L
        private const val QUIET_MS = 3_000L
        private const val END_SLACK_MS = 10_000L
    }
}
