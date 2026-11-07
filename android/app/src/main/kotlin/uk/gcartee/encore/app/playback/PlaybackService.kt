package uk.gcartee.encore.app.playback

import android.app.PendingIntent
import android.content.Intent
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.DataSourceBitmapLoader
import androidx.media3.datasource.ResolvingDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.Dispatchers
import uk.gcartee.encore.app.MainActivity
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.SkipOrigin
import uk.gcartee.encore.core.appName

/**
 * Background playback (plan section 8): a Media3 session service of type mediaPlayback.
 * The session gives the notification, the lock screen player, headset and Bluetooth
 * buttons and Android Auto later; every Next and Previous from them goes through the core
 * with its origin, exactly like the app's own buttons (board 42, rule 15).
 */
@OptIn(UnstableApi::class)
class PlaybackService : MediaSessionService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var session: MediaSession? = null
    private var playback: CorePlayback? = null
    /** Request headers by URL: stream and artwork URLs need the server's auth header. */
    private val headers = ConcurrentHashMap<String, Map<String, String>>()


    override fun onCreate() {
        super.onCreate()
        val host = CoreHost.get(this)
        val dataSource = dataSourceFactory()
        val exo = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(dataSource))
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                    .build(),
                /* handleAudioFocus = */ true,
            )
            .setHandleAudioBecomingNoisy(true)
            .setWakeMode(C.WAKE_MODE_NETWORK)
            .build()
        val core = CorePlayback(host, exo, headers, scope).also { it.start() }
        playback = core
        PlaybackHub.attach(core)

        val sessionPlayer = object : ForwardingPlayer(exo) {
            override fun getAvailableCommands(): Player.Commands =
                super.getAvailableCommands().buildUpon()
                    .addAll(
                        Player.COMMAND_SEEK_TO_NEXT,
                        Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
                        Player.COMMAND_SEEK_TO_PREVIOUS,
                        Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
                    )
                    .build()

            override fun isCommandAvailable(command: Int): Boolean =
                command in TRANSPORT || super.isCommandAvailable(command)

            override fun seekToNext() = core.next(origin())
            override fun seekToNextMediaItem() = core.next(origin())
            override fun seekToPrevious() = core.previous(origin())
            override fun seekToPreviousMediaItem() = core.previous(origin())
            override fun play() = core.play()
        }

        session = MediaSession.Builder(this, sessionPlayer)
            .setSessionActivity(
                PendingIntent.getActivity(
                    this,
                    0,
                    Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
                    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                ),
            )
            .setBitmapLoader(DataSourceBitmapLoader.Builder(this).setDataSourceFactory(dataSource).build())
            .build()
    }

    /** Who sent the transport command being handled, for the skip's origin (board 42, rule 15). */
    private fun origin(): SkipOrigin {
        val s = session ?: return SkipOrigin.LOCK_SCREEN
        val controller = s.controllerForCurrentRequest ?: return SkipOrigin.HEADSET
        return when {
            s.isAutomotiveController(controller) || s.isAutoCompanionController(controller) -> SkipOrigin.CAR
            s.isMediaNotificationController(controller) -> SkipOrigin.LOCK_SCREEN
            controller.packageName == packageName -> SkipOrigin.BUTTON
            else -> SkipOrigin.HEADSET
        }
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Swiped away from Recents while paused: nothing should keep running.
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) stopSelf()
    }

    override fun onDestroy() {
        PlaybackHub.detach()
        playback?.stop()
        session?.run {
            player.release()
            release()
        }
        session = null
        scope.cancel()
        super.onDestroy()
    }

    private fun dataSourceFactory(): DataSource.Factory {
        val http = DefaultHttpDataSource.Factory()
            .setUserAgent("${appName()} Android")
            .setAllowCrossProtocolRedirects(false)
        return ResolvingDataSource.Factory(DefaultDataSource.Factory(this, http)) { spec ->
            headers[spec.uri.toString()]?.let(spec::withAdditionalHeaders) ?: spec
        }
    }

    private companion object {
        val TRANSPORT = setOf(
            Player.COMMAND_SEEK_TO_NEXT,
            Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_PREVIOUS,
            Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
        )
    }
}
