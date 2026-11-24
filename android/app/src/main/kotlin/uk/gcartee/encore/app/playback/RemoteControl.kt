package uk.gcartee.encore.app.playback

import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.ChosenBy
import uk.gcartee.encore.core.DeviceState
import uk.gcartee.encore.core.EventKind
import uk.gcartee.encore.core.OutputDevice
import uk.gcartee.encore.core.QueueEntry
import uk.gcartee.encore.core.RemoteCommand
import uk.gcartee.encore.core.RemoteCommandKind
import uk.gcartee.encore.core.Song

/**
 * Board 12, "Remote control" (owner, 24 November 2026: "if you know how Spotify does it,
 * where you can change your song, change your playlist on your output device from a
 * different device"). While this device controls another one, the mini player and Now
 * Playing show that device's song, position and state, and every control (play, pause,
 * skips, scrubbing, volume, Play next, Add to queue, and starting new music) acts there.
 * Moving music to a device starts controlling it; taking it back here, or the device going
 * away, stops.
 */
object RemoteControl {
    private const val TAG = "RemoteControl"
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var host: CoreHost? = null
    private val _device = MutableStateFlow<OutputDevice?>(null)
    private val song = MutableStateFlow<Song?>(null)

    /** The device being controlled, kept current by the core's `RemoteState` events. */
    val device = _device.asStateFlow()
    val active: Boolean get() = _device.value != null

    /** What the player screens show while controlling: the other device's song and state. */
    val state: Flow<PlayerState?> = combine(_device, song) { d, s ->
        if (d == null) {
            null
        } else {
            PlayerState(
                entry = s?.let { QueueEntry(-it.id, it, ChosenBy.USER) },
                isPlaying = d.state == DeviceState.PLAYING,
                buffering = false,
                positionMs = d.positionMs,
                durationMs = d.song?.durationMs ?: s?.durationMs ?: 0,
            )
        }
    }

    /** Once, from the app root: follow the controlled device's state. */
    fun bind(host: CoreHost) {
        if (this.host != null) return
        this.host = host
        scope.launch {
            host.events.collect { e ->
                when (e.kind) {
                    EventKind.REMOTE_STATE -> e.device?.let(::update)
                    EventKind.DEVICES_CHANGED -> {
                        val id = _device.value?.deviceId ?: return@collect
                        e.outputDevices?.devices?.firstOrNull { it.deviceId == id }?.let(::update)
                    }
                    else -> Unit
                }
            }
        }
    }

    /** Control [deviceId] from here (after moving music there, or "Control only"). */
    fun start(deviceId: String) {
        val h = host ?: return
        scope.launch {
            val d = withContext(Dispatchers.IO) { runCatching { h.core.controlOnly(deviceId) }.getOrNull() }
            if (d != null) update(d)
        }
    }

    /** Stop controlling (the music came back here, or the device went away). */
    fun stop() {
        if (_device.value == null) return
        _device.value = null
        song.value = null
        val h = host ?: return
        scope.launch(Dispatchers.IO) { runCatching { h.core.stopRemoteControl() } }
    }

    private fun update(d: OutputDevice) {
        if (d.state == DeviceState.OFFLINE) {
            Log.i(TAG, "the controlled device went offline")
            stop()
            return
        }
        _device.value = d
        val id = d.song?.songId
        if (id == null) {
            song.value = null
        } else if (song.value?.id != id) {
            val h = host ?: return
            scope.launch {
                song.value = withContext(Dispatchers.IO) { runCatching { h.core.song(id) }.getOrNull() }
            }
        }
    }

    fun send(
        kind: RemoteCommandKind,
        positionMs: Long? = null,
        level: Double? = null,
        songIds: List<Long> = emptyList(),
        uid: Long? = null,
        toIndex: UInt? = null,
    ) {
        val d = _device.value ?: return
        val h = host ?: return
        // Shown at once; the device's own report follows and wins.
        when (kind) {
            RemoteCommandKind.PLAY -> _device.value = d.copy(state = DeviceState.PLAYING)
            RemoteCommandKind.PAUSE -> _device.value = d.copy(state = DeviceState.PAUSED)
            RemoteCommandKind.SEEK_TO -> positionMs?.let { _device.value = d.copy(positionMs = it) }
            RemoteCommandKind.SET_VOLUME -> level?.let { _device.value = d.copy(volume = it) }
            else -> Unit
        }
        scope.launch(Dispatchers.IO) {
            runCatching { h.core.remoteCommand(d.deviceId, RemoteCommand(kind, positionMs, level, songIds, uid, toIndex)) }
                .onFailure { Log.w(TAG, "remote command $kind failed: ${it.message}") }
        }
    }

    fun toggle() {
        val d = _device.value ?: return
        send(if (d.state == DeviceState.PLAYING) RemoteCommandKind.PAUSE else RemoteCommandKind.PLAY)
    }

    /**
     * New music while controlling (a song, an album, a playlist, a radio): the core's queue
     * here was just set, so it moves to the controlled device, which plays it.
     */
    fun playQueueThere(onFailure: (Throwable) -> Unit) {
        val d = _device.value ?: return
        val h = host ?: return
        scope.launch {
            runCatching { h.core.handOffTo(d.deviceId) }
                .onSuccess { send(RemoteCommandKind.PLAY) }
                .onFailure(onFailure)
        }
    }

    /** Bring the music back to this device (the capsule's tap). */
    fun playHere(onFailure: (Throwable) -> Unit) {
        val d = _device.value ?: return
        val h = host ?: return
        scope.launch {
            runCatching { h.core.pullFrom(d.deviceId) }
                .onSuccess { stop() }
                .onFailure(onFailure)
        }
    }
}
