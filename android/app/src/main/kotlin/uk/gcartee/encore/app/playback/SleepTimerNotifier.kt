package uk.gcartee.encore.app.playback

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import java.util.Date
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import uk.gcartee.encore.app.MainActivity
import uk.gcartee.encore.app.R
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.notifications.LiveUpdates
import uk.gcartee.encore.core.EventKind
import uk.gcartee.encore.core.SleepTimer
import uk.gcartee.encore.core.SleepTimerMode

/**
 * Phase 9: the sleep timer's card (boards 31 and 32; PLAN-UX "Sleep timer"). One ongoing,
 * silent notification while a timer runs: "Sleep timer", "Music fades out at 10:05 PM",
 * the system's own countdown to the end, and Add 15 min. On Android 16 QPR1 and later it
 * asks to be a Live Update (LiveUpdates, card SLEEP_TIMER) with a short chip such as
 * "24m". Process-wide: started once by CoreHost, it follows `SleepTimerChanged`, so it
 * works whether or not the player or the app's screen is up.
 *
 * - The countdown is the system's (`setWhen` + a count-down chronometer), and the card
 *   times out by itself just after the end, so it is right even if the app is closed.
 * - The chip text is refreshed once a minute while the process runs.
 * - A tap opens the sleep timer sheet (`MainActivity.OPEN_SLEEP_TIMER`).
 * - Swiping it away (Android 14 and later) hides this timer's card only; the timer runs on,
 *   and the card isn't posted again for it (Add 15 min keeps it hidden). A new timer posts.
 * - Without notification permission nothing is posted; the timer still works.
 * - It names no profile and nothing played, so it is the same on the lock screen.
 */
object SleepTimerNotifier {
    const val CHANNEL = "sleep-timer"
    const val ID = 48_20
    private const val PREFS = "sleep-timer-card"
    private const val KEY_HIDDEN = "hidden-set-utc-ms"
    private const val MINUTE_MS = 60_000L
    /** The card stays this long past the end: the fade, and the player's last tick. */
    private const val TIMEOUT_MARGIN_MS = 15_000L

    private var host: CoreHost? = null
    private var scope: CoroutineScope? = null
    private var refresh: Job? = null

    /** From CoreHost, once per process. */
    @Synchronized
    fun start(context: Context, coreHost: CoreHost) {
        if (scope != null) return
        val app = context.applicationContext
        val s = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
        scope = s
        host = coreHost
        val channel = NotificationChannel(CHANNEL, "Sleep timer", NotificationManager.IMPORTANCE_LOW).apply {
            description = "Time left on the sleep timer, with Add 15 min"
            setShowBadge(false)
            setSound(null, null)
            enableVibration(false)
        }
        app.getSystemService(NotificationManager::class.java)?.createNotificationChannel(channel)
        s.launch {
            coreHost.events.collect { event ->
                if (event.kind == EventKind.SLEEP_TIMER_CHANGED) update(app, event.sleepTimer)
            }
        }
        // A timer that was running when the process started (it survives the app closing).
        s.launch { update(app, runCatching { coreHost.core.sleepTimer() }.getOrNull()) }
    }

    /** Posts, refreshes or removes the card for [timer] (null: no timer). Main thread. */
    fun update(context: Context, timer: SleepTimer?) {
        refresh?.cancel()
        refresh = null
        val manager = NotificationManagerCompat.from(context)
        val now = System.currentTimeMillis()
        if (timer == null || (timer.mode == SleepTimerMode.MINUTES && timer.endsAtUtcMs <= now)) {
            manager.cancel(ID)
            return
        }
        if (prefs(context).getLong(KEY_HIDDEN, Long.MIN_VALUE) == timer.setUtcMs) return
        val left = (timer.endsAtUtcMs - now).coerceAtLeast(0L)
        val at = android.text.format.DateFormat.getTimeFormat(context).format(Date(timer.endsAtUtcMs))
        val text = when (timer.mode) {
            SleepTimerMode.MINUTES -> "Music fades out at $at"
            SleepTimerMode.END_OF_SONG -> "Music stops after this song, about $at"
            SleepTimerMode.END_OF_ALBUM -> "Music stops after this album, about $at"
        }
        val b = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.sym_bedtime)
            .setContentTitle("Sleep timer")
            .setContentText(text)
            .setSilent(true)
            .setOnlyAlertOnce(true)
            .setOngoing(true)
            .setCategory(NotificationCompat.CATEGORY_STOPWATCH)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setWhen(timer.endsAtUtcMs)
            .setShowWhen(true)
            .setUsesChronometer(true)
            .setChronometerCountDown(true)
            .setTimeoutAfter(left + timer.fadeMs + TIMEOUT_MARGIN_MS)
            .setContentIntent(openSheet(context))
            .setDeleteIntent(hidden(context, timer.setUtcMs))
            .addAction(0, "Add 15 min", addFifteen(context))
        LiveUpdates.applyPromotion(context, b, LiveUpdates.Card.SLEEP_TIMER, userInitiated = true, chip = chipText(left))
        // Without the permission (Android 13+, declined) nothing shows; the timer goes on.
        if (!manager.areNotificationsEnabled()) return
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU &&
            androidx.core.content.ContextCompat.checkSelfPermission(context, android.Manifest.permission.POST_NOTIFICATIONS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED
        ) return
        runCatching { manager.notify(ID, b.build()) }
        scheduleRefresh(context, left)
    }

    /** The card was swiped away: hide this timer's card, never cancel the timer. */
    fun hide(context: Context, setUtcMs: Long) {
        refresh?.cancel()
        refresh = null
        prefs(context).edit().putLong(KEY_HIDDEN, setUtcMs).apply()
    }

    /**
     * The status bar chip: whole minutes left, like "24m" or "1h 5m"; "<1m" in the last
     * minute. Always [LiveUpdates.CHIP_MAX_CHARS] characters or fewer (24 hours at most).
     */
    fun chipText(leftMs: Long): String {
        val minutes = leftMs.coerceAtLeast(0L) / MINUTE_MS
        return when {
            minutes < 1 -> "<1m"
            minutes < 60 -> "${minutes}m"
            minutes % 60 == 0L -> "${minutes / 60}h"
            else -> "${minutes / 60}h ${minutes % 60}m".let { if (it.length > LiveUpdates.CHIP_MAX_CHARS) it.replace(" ", "") else it }
        }.take(LiveUpdates.CHIP_MAX_CHARS)
    }

    /** Again when the chip's minute changes, with the timer read fresh (song estimates move). */
    private fun scheduleRefresh(context: Context, leftMs: Long) {
        val s = scope ?: return
        val coreHost = host ?: return
        if (leftMs <= 0L) return
        val wait = (leftMs % MINUTE_MS).let { if (it == 0L) MINUTE_MS else it } + 50L
        refresh = s.launch {
            delay(wait)
            update(context, runCatching { coreHost.core.sleepTimer() }.getOrNull())
        }
    }

    private fun openSheet(context: Context): PendingIntent =
        PendingIntent.getActivity(
            context,
            REQUEST_OPEN,
            Intent(context, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
                .putExtra(MainActivity.EXTRA_OPEN, MainActivity.OPEN_SLEEP_TIMER),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    private fun addFifteen(context: Context): PendingIntent =
        PendingIntent.getBroadcast(
            context,
            REQUEST_ADD,
            Intent(context, SleepTimerReceiver::class.java).setAction(SleepTimerReceiver.ACTION_ADD_15),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    private fun hidden(context: Context, setUtcMs: Long): PendingIntent =
        PendingIntent.getBroadcast(
            context,
            REQUEST_HIDDEN,
            Intent(context, SleepTimerReceiver::class.java)
                .setAction(SleepTimerReceiver.ACTION_HIDDEN)
                .putExtra(SleepTimerReceiver.EXTRA_SET_UTC_MS, setUtcMs),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    // Distinct from the download notification's request codes (0 and 1), so neither
    // PendingIntent replaces the other's extras.
    private const val REQUEST_OPEN = 20
    private const val REQUEST_ADD = 21
    private const val REQUEST_HIDDEN = 22
}
