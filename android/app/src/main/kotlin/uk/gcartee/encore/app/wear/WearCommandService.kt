package uk.gcartee.encore.app.wear

import android.content.Context
import android.content.Intent
import android.media.AudioManager
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.KeyEvent
import com.google.android.gms.wearable.MessageEvent
import com.google.android.gms.wearable.WearableListenerService
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.playback.RemoteControl
import uk.gcartee.encore.app.widgets.WidgetIntents
import uk.gcartee.encore.core.MixRef
import uk.gcartee.encore.core.MixRefKind
import uk.gcartee.encore.core.RemoteCommandKind
import uk.gcartee.encore.core.SkipOrigin
import uk.gcartee.encore.core.StartedFrom
import uk.gcartee.encore.wearlink.Paths
import uk.gcartee.encore.wearlink.WearCommand

/**
 * Stage W1: the watch's commands ([Paths.CMD]). Google Play services binds this service
 * when a message arrives and checks that the caller is itself; the Data Layer only
 * delivers messages from the same app, signed with the same key, on a paired watch.
 */
class WearCommandService : WearableListenerService() {
    override fun onMessageReceived(event: MessageEvent) {
        if (event.path != Paths.CMD) return
        val command = WearCommand.parse(event.data)
        if (command == null) {
            Log.w(TAG, "Ignored a message that isn't a command")
            return
        }
        WearCommands.run(applicationContext, command)
    }

    private companion object {
        const val TAG = "WearCommand"
    }
}

/**
 * What each command does, through the same paths as the app's own buttons: PlaybackHub and
 * RemoteControl while the player runs; with it stopped, a start of the playback service the
 * way the widgets and the Quick Settings tile start it (startForegroundService with a media
 * button or the private Start a mix action, [WidgetIntents]). Android may refuse that start
 * from the background; then nothing plays and the reason is logged.
 */
internal object WearCommands {
    private const val TAG = "WearCommand"
    private const val REMOTE_VOLUME_STEP = 1.0 / 16

    private val main = Handler(Looper.getMainLooper())
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    /** The player is running in this process (the core is open, too). */
    private fun running(): Boolean = PlaybackHub.isConnected.value != null

    fun run(context: Context, command: WearCommand) {
        when (command) {
            WearCommand.Play -> main.post {
                when {
                    RemoteControl.active -> RemoteControl.send(RemoteCommandKind.PLAY)
                    running() -> PlaybackHub.withPlayback(context) { it.play() }
                    else -> startPlayback(context, WidgetIntents.mediaButton(context, KeyEvent.KEYCODE_MEDIA_PLAY))
                }
            }
            WearCommand.Pause -> main.post {
                if (RemoteControl.active) RemoteControl.send(RemoteCommandKind.PAUSE) else PlaybackHub.isConnected.value?.pause()
            }
            WearCommand.Next -> main.post {
                if (RemoteControl.active) RemoteControl.send(RemoteCommandKind.NEXT) else PlaybackHub.isConnected.value?.next(SkipOrigin.HEADSET)
            }
            WearCommand.Previous -> main.post {
                if (RemoteControl.active) RemoteControl.send(RemoteCommandKind.PREVIOUS) else PlaybackHub.isConnected.value?.previous(SkipOrigin.HEADSET)
            }
            WearCommand.Like -> setLiked(context, true)
            WearCommand.Unlike -> setLiked(context, false)
            is WearCommand.Volume -> main.post { volume(context, command.up) }
            is WearCommand.PlayMix -> playMix(context, command.mixId)
            WearCommand.PlayLiked -> playLiked(context)
        }
    }

    private fun setLiked(context: Context, like: Boolean) {
        // Only for the song playing here: a like never starts the core.
        val songId = PlaybackHub.isConnected.value?.state?.value?.entry?.song?.id ?: return
        scope.launch {
            val core = CoreHost.get(context).core
            runCatching { if (like) core.like(listOf(songId)) else core.unlike(listOf(songId)) }
                .onFailure { Log.w(TAG, "Like from the watch failed: ${it.javaClass.simpleName}") }
            WearLink.likedChanged()
        }
    }

    private fun volume(context: Context, up: Boolean) {
        if (RemoteControl.active) {
            val device = RemoteControl.device.value ?: return
            val step = if (up) REMOTE_VOLUME_STEP else -REMOTE_VOLUME_STEP
            RemoteControl.send(RemoteCommandKind.SET_VOLUME, level = (device.volume + step).coerceIn(0.0, 1.0))
            return
        }
        val audio = context.getSystemService(AudioManager::class.java) ?: return
        // No slider pops up on the phone: the watch is where the person is looking.
        runCatching {
            audio.adjustStreamVolume(AudioManager.STREAM_MUSIC, if (up) AudioManager.ADJUST_RAISE else AudioManager.ADJUST_LOWER, 0)
        }
    }

    private fun playMix(context: Context, mixId: String) {
        if (!running()) {
            // As the tile does: the service opens the profile and plays the mix (WidgetStarts).
            startPlayback(context, WidgetIntents.playMix(context, mixId))
            return
        }
        scope.launch {
            val host = CoreHost.get(context)
            if (!host.ensureProfile()) return@launch
            runCatching { host.core.playMix(MixRef(MixRefKind.ID, mixId)) }
                .onSuccess { main.post { PlaybackHub.playCurrent(context) } }
                .onFailure { Log.w(TAG, "Start a mix from the watch failed: ${it.javaClass.simpleName}") }
        }
    }

    private fun playLiked(context: Context) {
        scope.launch {
            val host = CoreHost.get(context)
            // A profile with a PIN, or none: nothing opens without the phone.
            if (!host.ensureProfile()) return@launch
            val view = host.downloads.view.value
            val ids = runCatching { host.core.likedSongs(0u, LIKED_LIMIT).map { it.id } }.getOrDefault(emptyList()).filter(view::canPlay)
            if (ids.isEmpty()) return@launch
            val queued = runCatching { host.core.play(ids, 0u, StartedFrom.PLAYLIST, null) }.isSuccess
            if (!queued) return@launch
            host.playingFrom.value = LIKED_LABEL
            main.post {
                if (running()) {
                    PlaybackHub.playCurrent(context)
                } else {
                    // The service picks up the core's queue (onPlaybackResumption in AutoLibrary).
                    startPlayback(context, WidgetIntents.mediaButton(context, KeyEvent.KEYCODE_MEDIA_PLAY))
                }
            }
        }
    }

    private fun startPlayback(context: Context, intent: Intent) {
        try {
            context.startForegroundService(intent)
        } catch (e: IllegalStateException) {
            // ForegroundServiceStartNotAllowedException (Android 12+) is one.
            Log.w(TAG, "The foreground start was refused: ${e.javaClass.simpleName}")
        } catch (e: SecurityException) {
            Log.w(TAG, "The start was refused: ${e.javaClass.simpleName}")
        }
    }

    private val LIKED_LIMIT = 5000u
    private const val LIKED_LABEL = "Liked songs"
}
