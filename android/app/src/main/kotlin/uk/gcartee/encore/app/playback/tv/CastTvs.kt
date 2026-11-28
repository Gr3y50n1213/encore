package uk.gcartee.encore.app.playback.tv

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.annotation.OptIn
import androidx.core.content.ContextCompat
import androidx.media3.cast.DefaultMediaItemConverter
import androidx.media3.cast.MediaItemConverter
import androidx.media3.cast.RemoteCastPlayer
import androidx.media3.cast.SessionAvailabilityListener
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.util.UnstableApi
import androidx.mediarouter.media.MediaRouteSelector
import androidx.mediarouter.media.MediaRouter
import com.google.android.gms.cast.CastDevice
import com.google.android.gms.cast.CastMediaControlIntent
import com.google.android.gms.cast.MediaQueueItem
import com.google.android.gms.cast.framework.CastContext
import com.google.android.gms.cast.framework.CastOptions
import com.google.android.gms.cast.framework.OptionsProvider
import com.google.android.gms.cast.framework.SessionProvider
import com.google.android.gms.cast.framework.media.CastMediaOptions
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailability

/**
 * T3 (ADR android-0007, "Google Cast"): Google's Default Media Receiver, which needs no
 * registration and shows the cover and title. Named in the manifest
 * (`com.google.android.gms.cast.framework.OPTIONS_PROVIDER_CLASS_NAME`). Media3 owns the media
 * session and the notification, so the Cast framework's own are off, and nothing resumes
 * or reconnects by itself: the listener chooses a TV in Play on every time.
 */
class EncoreCastOptions : OptionsProvider {
    override fun getCastOptions(context: Context): CastOptions = CastOptions.Builder()
        .setReceiverApplicationId(CastMediaControlIntent.DEFAULT_MEDIA_RECEIVER_APPLICATION_ID)
        .setResumeSavedSession(false)
        .setEnableReconnectionService(false)
        .setStopReceiverApplicationWhenEndingSession(true)
        .setRemoteToLocalEnabled(false)
        .setCastMediaOptions(
            CastMediaOptions.Builder()
                .setMediaSessionEnabled(false)
                .setNotificationOptions(null)
                .build(),
        )
        .build()

    override fun getAdditionalSessionProviders(context: Context): List<SessionProvider> = emptyList()
}

/** A Cast device found on the Wi-Fi. */
data class CastTv(val routeId: String, val name: String, val model: String?, val video: Boolean)

/**
 * Finding Cast devices (MediaRouter, with the Cast framework's route provider) and connecting
 * to one, which gives Media3's Cast player. Only where Google Play services are, and never on
 * Encore TV itself. Main thread only.
 */
@OptIn(UnstableApi::class)
internal object CastTvs {
    private const val TAG = "CastTvs"
    private const val CONNECT_TIMEOUT_MS = 25_000L

    private val selector: MediaRouteSelector by lazy {
        MediaRouteSelector.Builder()
            .addControlCategory(CastMediaControlIntent.categoryForCast(CastMediaControlIntent.DEFAULT_MEDIA_RECEIVER_APPLICATION_ID))
            .build()
    }
    private val main = Handler(Looper.getMainLooper())
    private var ready = false
    private var failed = false
    private var router: MediaRouter? = null
    private var callback: MediaRouter.Callback? = null
    private var player: RemoteCastPlayer? = null
    private var endedCallback: (() -> Unit)? = null

    fun usable(context: Context): Boolean =
        !failed && runCatching {
            GoogleApiAvailability.getInstance().isGooglePlayServicesAvailable(context) == ConnectionResult.SUCCESS
        }.getOrDefault(false)

    /** Starts the Cast framework once; [then] runs when it is ready (never if it can't start). */
    private fun init(context: Context, then: () -> Unit) {
        if (ready) {
            then()
            return
        }
        if (!usable(context)) return
        runCatching {
            CastContext.getSharedInstance(context.applicationContext, ContextCompat.getMainExecutor(context))
                .addOnSuccessListener {
                    ready = true
                    then()
                }
                .addOnFailureListener {
                    failed = true
                    Log.w(TAG, "Cast can't start here: ${it.javaClass.simpleName}")
                }
        }.onFailure {
            failed = true
            Log.w(TAG, "Cast can't start here: ${it.javaClass.simpleName}")
        }
    }

    /** Looks for Cast devices while Play on is open; [onChange] gets the list each time it changes. */
    fun discover(context: Context, onChange: (List<CastTv>) -> Unit) {
        init(context) {
            val r = MediaRouter.getInstance(context.applicationContext)
            router = r
            callback?.let(r::removeCallback)
            val cb = object : MediaRouter.Callback() {
                override fun onRouteAdded(router: MediaRouter, route: MediaRouter.RouteInfo) = onChange(routes())
                override fun onRouteRemoved(router: MediaRouter, route: MediaRouter.RouteInfo) = onChange(routes())
                override fun onRouteChanged(router: MediaRouter, route: MediaRouter.RouteInfo) = onChange(routes())
            }
            callback = cb
            r.addCallback(selector, cb, MediaRouter.CALLBACK_FLAG_REQUEST_DISCOVERY or MediaRouter.CALLBACK_FLAG_PERFORM_ACTIVE_SCAN)
            onChange(routes())
        }
    }

    fun stopDiscovery() {
        val r = router ?: return
        callback?.let(r::removeCallback)
        callback = null
    }

    private fun routes(): List<CastTv> {
        val r = router ?: return emptyList()
        return r.routes
            .filter { !it.isDefault && !it.isBluetooth && it.isEnabled && it.matchesSelector(selector) }
            .map { route ->
                val device = route.extras?.let { extras -> runCatching { CastDevice.getFromBundle(extras) }.getOrNull() }
                CastTv(
                    routeId = route.id,
                    name = route.name.ifBlank { device?.friendlyName ?: "TV" },
                    model = device?.modelName?.takeIf { it.isNotBlank() },
                    video = device?.hasCapability(CastDevice.CAPABILITY_VIDEO_OUT) ?: true,
                )
            }
    }

    /**
     * Connects to the Cast device [routeId]: the Default Media Receiver starts on it, then
     * [onConnected] gets Media3's Cast player. [onEnded] is called if the session ends from
     * elsewhere (the TV's remote, Google Home); [onFailed] if it never starts.
     */
    fun connect(
        context: Context,
        routeId: String,
        onConnected: (RemoteCastPlayer) -> Unit,
        onFailed: () -> Unit,
        onEnded: () -> Unit,
    ) {
        init(context) {
            val r = router ?: MediaRouter.getInstance(context.applicationContext).also { router = it }
            val route = r.routes.firstOrNull { it.id == routeId }
            if (route == null) {
                onFailed()
                return@init
            }
            val p = player ?: RemoteCastPlayer.Builder(context.applicationContext)
                .setMediaItemConverter(RelayCoverConverter())
                .build()
                .also { player = it }
            endedCallback = onEnded
            var settled = false
            val timeout = Runnable {
                if (!settled) {
                    settled = true
                    Log.w(TAG, "the Cast device didn't start the receiver in time")
                    onFailed()
                }
            }
            p.setSessionAvailabilityListener(object : SessionAvailabilityListener {
                override fun onCastSessionAvailable() {
                    if (settled) return
                    settled = true
                    main.removeCallbacks(timeout)
                    onConnected(p)
                }

                override fun onCastSessionUnavailable() {
                    main.removeCallbacks(timeout)
                    if (!settled) {
                        settled = true
                        onFailed()
                    } else {
                        endedCallback?.invoke()
                    }
                }
            })
            if (p.isCastSessionAvailable && route.isSelected) {
                settled = true
                onConnected(p)
                return@init
            }
            main.postDelayed(timeout, CONNECT_TIMEOUT_MS)
            r.selectRoute(route)
        }
    }

    /** Ends the Cast session and stops the receiver on the TV (the music came back here). */
    fun disconnect(context: Context) {
        endedCallback = null
        player?.setSessionAvailabilityListener(null)
        if (!ready) return
        runCatching { CastContext.getSharedInstance(context.applicationContext, ContextCompat.getMainExecutor(context)).addOnSuccessListener { it.sessionManager.endCurrentSession(true) } }
    }
}

/**
 * What the Cast device gets for each song: the Default Media Receiver draws the cover from the
 * metadata's image, so that is the relay's cover address (kept in the item's extras), never the
 * app's own content address that the phone's lock screen and car read. Media3 keeps the
 * session's items as they were given, so nothing else sees the relay's addresses.
 */
@OptIn(UnstableApi::class)
internal class RelayCoverConverter : MediaItemConverter {
    private val base = DefaultMediaItemConverter()

    override fun toMediaQueueItem(mediaItem: MediaItem): MediaQueueItem {
        val cover = mediaItem.mediaMetadata.extras?.getString(TvPlayer.EXTRA_COVER)
        val metadata = mediaItem.mediaMetadata.buildUpon()
            .setArtworkUri(cover?.let(Uri::parse))
            .setArtworkData(null, null)
            .setExtras(null)
            .setMediaType(MediaMetadata.MEDIA_TYPE_MUSIC)
            .build()
        return base.toMediaQueueItem(mediaItem.buildUpon().setMediaMetadata(metadata).build())
    }

    override fun toMediaItem(mediaQueueItem: MediaQueueItem): MediaItem = base.toMediaItem(mediaQueueItem)
}
