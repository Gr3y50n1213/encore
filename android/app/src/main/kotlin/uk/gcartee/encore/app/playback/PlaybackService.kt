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
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.DataSourceBitmapLoader
import androidx.media3.datasource.ResolvingDataSource
import androidx.media3.datasource.cache.CacheDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.exoplayer.util.EventLogger
import androidx.media3.extractor.DefaultExtractorsFactory
import androidx.media3.common.MediaItem
import androidx.media3.session.MediaLibraryService
import androidx.media3.session.MediaSession
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
 * Background playback (plan section 8): a Media3 library session service of type
 * mediaPlayback. The session gives the notification, the lock screen player, headset
 * and Bluetooth buttons, and Android Auto's browse lists and voice (Phase 8,
 * AutoLibrary); every Next and Previous from them goes through the core with its
 * origin, exactly like the app's own buttons (board 42, rule 15).
 */
@OptIn(UnstableApi::class)
class PlaybackService : MediaLibraryService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var session: MediaLibrarySession? = null
    private var playback: CorePlayback? = null
    /** Request headers by URL: stream and artwork URLs need the server's auth header. */
    private val headers = ConcurrentHashMap<String, Map<String, String>>()


    override fun onCreate() {
        super.onCreate()
        val host = CoreHost.get(this)
        // Started by the car or Bluetooth with the app closed: the last profile.
        host.ensureProfile()
        val dataSource = dataSourceFactory()
        // Constant-bitrate seeking: a song downloaded at a capped quality is a plain AAC
        // (ADTS) file with no index, and seeks by its byte rate.
        val extractors = DefaultExtractorsFactory().setConstantBitrateSeekingEnabled(true)
        val exo = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(dataSource, extractors).setLoadErrorHandlingPolicy(PatientRetries()))
            .setLoadControl(wholeSongLoadControl())
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
        // Debug builds (and CI's emulators) log every player event: no addresses, no titles.
        if (applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE != 0) {
            exo.addAnalyticsListener(EventLogger("EncorePlayer"))
        }
        val core = CorePlayback(host, exo, headers, scope) { key -> RollingCache.isWhole(this, key) }.also { it.start() }
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

            // The lock screen's, the car's and Bluetooth's seeks go the app's way
            // (CorePlayback.seekTo). The session offers them only when ExoPlayer can seek:
            // for a transcode that's from the second song on, opened from the cache.
            override fun seekTo(positionMs: Long) = core.seekTo(positionMs)
            override fun seekTo(mediaItemIndex: Int, positionMs: Long) {
                if (mediaItemIndex == exo.currentMediaItemIndex) core.seekTo(positionMs) else super.seekTo(mediaItemIndex, positionMs)
            }

            // A car or another controller chose something: AutoLibrary has already
            // built the queue in the core; load its current song (never the list).
            override fun setMediaItems(mediaItems: MutableList<MediaItem>) = core.loadCurrent()
            override fun setMediaItems(mediaItems: MutableList<MediaItem>, resetPosition: Boolean) = core.loadCurrent()
            override fun setMediaItems(mediaItems: MutableList<MediaItem>, startIndex: Int, startPositionMs: Long) = core.loadCurrent()
            override fun setMediaItem(mediaItem: MediaItem) = core.loadCurrent()
            override fun setMediaItem(mediaItem: MediaItem, resetPosition: Boolean) = core.loadCurrent()
            override fun setMediaItem(mediaItem: MediaItem, startPositionMs: Long) = core.loadCurrent()
        }

        session = MediaLibrarySession.Builder(this, sessionPlayer, AutoLibrary(this, host, scope) { playback })
            .setSessionActivity(
                PendingIntent.getActivity(
                    this,
                    0,
                    Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
                    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                ),
            )
            .setBitmapLoader(DataSourceBitmapLoader.Builder(this).setDataSourceFactory(dataSourceFactory(cached = false)).build())
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

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaLibrarySession? = session

    // --- Phase 9 (WP6): widget and tile starts (WidgetStarts.kt). Media buttons go on to
    // Media3 as before; the private Start a mix action needs this install's token; every
    // start that names an action reaches startForeground in time, or stops. ---
    private val widgetStarts = WidgetStarts(this) { session?.player }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        widgetStarts.onStart(intent, ::playMixFromWidget)
        return super.onStartCommand(intent, flags, startId)
    }

    /** Start a mix (CORE-API `play_mix`) on the profile already open or the last active one; never another. */
    private fun playMixFromWidget(mixId: String?) {
        val host = CoreHost.get(this)
        if (!host.ensureProfile()) return
        val ref = if (mixId == null) {
            uk.gcartee.encore.core.MixRef(uk.gcartee.encore.core.MixRefKind.NEXT_UP, null)
        } else {
            uk.gcartee.encore.core.MixRef(uk.gcartee.encore.core.MixRefKind.ID, mixId)
        }
        if (runCatching { host.core.playMix(ref) }.isFailure) return
        // Media3 shows the notification (and goes into the foreground) only for a session
        // it knows; a controller usually adds it, a private action doesn't.
        session?.let { if (!isSessionAdded(it)) addSession(it) }
        runCatching { playback?.playCurrent() }
    }
    // --- End Phase 9 (WP6). ---

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Swiped away from Recents while paused: nothing should keep running.
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) stopSelf()
    }

    override fun onDestroy() {
        widgetStarts.release() // Phase 9 (WP6)
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

    /**
     * Read a song through in one go (up to [WHOLE_SONG_MS] or [BUFFER_BYTES]) instead of
     * ExoPlayer's 50 s, then the next one right after it (CorePlayback keeps it loaded).
     * A transcoded stream (a capped quality) can't be resumed: when the player paused
     * reading it, the idle connection was dropped, and each reopen started a new
     * transcode on the server. The owner's Jellyfin got 3-4 per song and crashed killing
     * one (8 November 2026). One read per song also lets the radio sleep sooner. 64 MB
     * holds a CD-quality FLAC and the next song.
     */
    private fun wholeSongLoadControl(): DefaultLoadControl = DefaultLoadControl.Builder()
        .setBufferDurationsMs(
            WHOLE_SONG_MS,
            WHOLE_SONG_MS,
            DefaultLoadControl.DEFAULT_BUFFER_FOR_PLAYBACK_MS,
            DefaultLoadControl.DEFAULT_BUFFER_FOR_PLAYBACK_AFTER_REBUFFER_MS,
        )
        .setTargetBufferBytes(BUFFER_BYTES)
        .setPrioritizeTimeOverSizeThresholds(false)
        .build()

    /**
     * Streams go through the rolling cache (Phase 8), keyed by the core's
     * `cache_key`; a downloaded song is a `file://` and goes straight to the file.
     * [cached] false is for artwork, which the image loaders keep themselves.
     */
    private fun dataSourceFactory(cached: Boolean = true): DataSource.Factory {
        // A busy server can take a while to start a transcode (over a minute, on the owner's
        // PC under load): a short timeout made the player retry, and every retry was another
        // transcode. The read timeout is long; a server that can't be reached still fails
        // fast on the connect timeout.
        val http = DefaultHttpDataSource.Factory()
            .setUserAgent("${appName()} Android")
            .setAllowCrossProtocolRedirects(false)
            .setConnectTimeoutMs(CONNECT_TIMEOUT_MS)
            .setReadTimeoutMs(READ_TIMEOUT_MS)
        val limit = runCatching { CoreHost.get(this).core.downloadSettings().cacheLimitBytes.toLong() }
            .getOrDefault(DEFAULT_CACHE_BYTES)
        val network: DataSource.Factory = if (cached && limit > 0) {
            RollingCache.setLimit(this, limit)
            CacheDataSource.Factory()
                .setCache(RollingCache.get(this))
                .setUpstreamDataSourceFactory(http)
                .setFlags(CacheDataSource.FLAG_IGNORE_CACHE_ON_ERROR)
        } else {
            http
        }
        return ResolvingDataSource.Factory(DefaultDataSource.Factory(this, network)) { asked ->
            // ExoPlayer tells the cache to skip a stream whose length is unknown (meant for
            // endless radio). A transcoded song (a capped quality) is one, and it must be
            // kept: it's what makes it seekable and playable offline once whole.
            val spec = if (asked.flags and DataSpec.FLAG_DONT_CACHE_IF_LENGTH_UNKNOWN != 0) {
                asked.buildUpon().setFlags(asked.flags and DataSpec.FLAG_DONT_CACHE_IF_LENGTH_UNKNOWN.inv()).build()
            } else {
                asked
            }
            headers[spec.uri.toString()]?.let(spec::withAdditionalHeaders) ?: spec
        }
    }

    /**
     * A failed song load is tried again twice, after 5 s and then 15 s, not ExoPlayer's
     * up to six quick retries for a stream of unknown length (a transcode). Every retry is a
     * new request, and on Jellyfin a new transcode while the abandoned one keeps running: on
     * the owner's busy server (9 November 2026, a minute to answer) one song was asked for
     * five times, 20 s apart.
     */
    private class PatientRetries : androidx.media3.exoplayer.upstream.DefaultLoadErrorHandlingPolicy(MEDIA_RETRIES) {
        override fun getRetryDelayMsFor(info: androidx.media3.exoplayer.upstream.LoadErrorHandlingPolicy.LoadErrorInfo): Long {
            val asked = super.getRetryDelayMsFor(info)
            if (asked == C.TIME_UNSET) return asked // not worth retrying (a 404, say)
            return RETRY_DELAYS_MS.getOrElse(info.errorCount - 1) { RETRY_DELAYS_MS.last() }
        }
    }

    private companion object {
        const val MEDIA_RETRIES = 2
        val RETRY_DELAYS_MS = longArrayOf(5_000, 15_000).toList()
        const val DEFAULT_CACHE_BYTES = 1_000_000_000L
        const val WHOLE_SONG_MS = 15 * 60_000
        const val BUFFER_BYTES = 64 * 1024 * 1024
        const val CONNECT_TIMEOUT_MS = 10_000
        const val READ_TIMEOUT_MS = 90_000
        val TRANSPORT = setOf(
            Player.COMMAND_SEEK_TO_NEXT,
            Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_PREVIOUS,
            Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
        )
    }
}
