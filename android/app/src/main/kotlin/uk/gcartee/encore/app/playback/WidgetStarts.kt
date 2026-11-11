package uk.gcartee.encore.app.playback

import android.annotation.SuppressLint
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.annotation.OptIn
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaSessionService
import uk.gcartee.encore.app.R
import uk.gcartee.encore.app.widgets.WidgetIntents
import uk.gcartee.encore.app.widgets.WidgetToken

/**
 * Phase 9: starts of the playback service from our widgets and tile, and from anything
 * else that names an action (the service is exported for Android Auto).
 *
 * **Private actions.** [WidgetIntents.ACTION_PLAY_MIX] runs only with this install's
 * [WidgetToken]; without it the start is ignored. No intent ever chooses a profile: the
 * service plays the profile it has open, or opens the last active one (CoreHost).
 *
 * **The foreground promise.** A widget tap starts the service with startForegroundService,
 * and Android stops the app (ForegroundServiceDidNotStartInTimeException) unless
 * startForeground follows within seconds. Media3 calls it when playback starts, but a tap
 * may play nothing: Pause, Next while paused, no queue, no mix yet, a wrong token. So
 * [CHECK_MS] after any start that names an action, if Media3 isn't in the foreground:
 * - nothing is about to play: a quiet placeholder goes into the foreground and straight
 *   out again (as Media3's own stopSelfSafely does), and the service stops;
 * - something is about to play (play requested, buffering or ready): the placeholder
 *   stays in the foreground until Media3 replaces it with the media notification (Android
 *   swaps a service's foreground notification and removes the old one), or until nothing
 *   is about to play any more.
 * Media3's own restarts (an explicit intent with no action) are left to Media3.
 */
@OptIn(UnstableApi::class)
internal class WidgetStarts(
    private val service: MediaSessionService,
    private val player: () -> Player?,
) {
    private val handler = Handler(Looper.getMainLooper())
    private var placeholderUp = false
    private var followUps = 0

    /** From onStartCommand, on the main thread, before Media3 handles the intent. */
    fun onStart(intent: Intent?, playMix: (mixId: String?) -> Unit) {
        if (intent == null) return
        if (intent.action == WidgetIntents.ACTION_PLAY_MIX) {
            if (WidgetToken.matches(service, intent.getStringExtra(WidgetIntents.EXTRA_TOKEN))) {
                playMix(intent.getStringExtra(WidgetIntents.EXTRA_MIX_ID))
            } else {
                Log.w(TAG, "Ignored a private action without this install's token")
            }
        }
        if (intent.action != null) {
            handler.removeCallbacks(check)
            handler.postDelayed(check, CHECK_MS)
        }
    }

    fun release() {
        handler.removeCallbacksAndMessages(null)
    }

    private val check = Runnable {
        if (service.isPlaybackOngoing) return@Runnable
        if (!startPlaceholder()) {
            // Not allowed to (a start that wasn't a foreground one): nothing to promise.
            if (!aboutToPlay()) service.stopSelf()
            return@Runnable
        }
        if (aboutToPlay()) {
            placeholderUp = true
            followUps = 0
            handler.postDelayed(followUp, FOLLOW_UP_MS)
        } else {
            stopPlaceholder()
            service.stopSelf()
        }
    }

    private val followUp: Runnable = object : Runnable {
        override fun run() {
            if (!placeholderUp) return
            if (service.isPlaybackOngoing) {
                // Media3's notification took over the foreground (and replaced ours).
                placeholderUp = false
                return
            }
            if (aboutToPlay() && ++followUps < MAX_FOLLOW_UPS) {
                handler.postDelayed(this, FOLLOW_UP_MS)
                return
            }
            stopPlaceholder()
            if (!aboutToPlay()) service.stopSelf()
        }
    }

    private fun aboutToPlay(): Boolean {
        val p = player() ?: return false
        return p.playWhenReady && (p.playbackState == Player.STATE_BUFFERING || p.playbackState == Player.STATE_READY)
    }

    @SuppressLint("InlinedApi") // ServiceCompat ignores the type before Android 10.
    private fun startPlaceholder(): Boolean = try {
        ServiceCompat.startForeground(service, NOTIFICATION_ID, placeholder(), ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        true
    } catch (e: IllegalStateException) {
        // ForegroundServiceStartNotAllowedException (Android 12+) is one.
        Log.w(TAG, "Could not start the placeholder in the foreground: ${e.javaClass.simpleName}")
        false
    } catch (e: SecurityException) {
        Log.w(TAG, "Could not start the placeholder in the foreground: ${e.javaClass.simpleName}")
        false
    }

    private fun stopPlaceholder() {
        placeholderUp = false
        ServiceCompat.stopForeground(service, ServiceCompat.STOP_FOREGROUND_REMOVE)
    }

    private fun placeholder(): android.app.Notification {
        val manager = service.getSystemService(NotificationManager::class.java)
        if (manager?.getNotificationChannel(CHANNEL) == null) {
            manager?.createNotificationChannel(
                NotificationChannel(CHANNEL, service.getString(R.string.playback_start_channel), NotificationManager.IMPORTANCE_LOW).apply {
                    setShowBadge(false)
                },
            )
        }
        // No song, no profile: the words only. Deferred, so Android 12 and later usually
        // never show it at all.
        return NotificationCompat.Builder(service, CHANNEL)
            .setSmallIcon(R.drawable.sym_music_note)
            .setContentTitle(service.getString(R.string.app_name))
            .setContentText(service.getString(R.string.playback_starting))
            .setSilent(true)
            .setOngoing(true)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_DEFERRED)
            .build()
    }

    private companion object {
        const val TAG = "WidgetStarts"
        const val CHANNEL = "playback-start"
        const val NOTIFICATION_ID = 0x5715

        /** Well inside Android's limit (5 to 10 s, by version) for startForeground. */
        const val CHECK_MS = 3_000L
        const val FOLLOW_UP_MS = 5_000L
        const val MAX_FOLLOW_UPS = 12
    }
}
