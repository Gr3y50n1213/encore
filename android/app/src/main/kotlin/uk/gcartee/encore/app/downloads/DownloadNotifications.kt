package uk.gcartee.encore.app.downloads

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import uk.gcartee.encore.app.MainActivity
import uk.gcartee.encore.app.R
import uk.gcartee.encore.core.DownloadProgress
import uk.gcartee.encore.core.DownloadRun
import uk.gcartee.encore.core.DownloadStop

/**
 * Boards 32 and 34: one notification for downloads the person started, updated in
 * place, with no sound: "Downloading Road trip", "15 of 24 songs", a bar and
 * Cancel. When they are done, or something stops them, one line says so. Live
 * Updates (Android 16 QPR1) come with Phase 9; this is the plain ongoing
 * notification the plan names as the fallback.
 */
class DownloadNotifications(private val context: Context) {
    private val manager = NotificationManagerCompat.from(context)

    init {
        val channel = NotificationChannel(CHANNEL, "Downloads", NotificationManager.IMPORTANCE_LOW).apply {
            description = "Progress of downloads you start, and when they finish"
            setShowBadge(false)
            setSound(null, null)
            enableVibration(false)
        }
        context.getSystemService(NotificationManager::class.java)?.createNotificationChannel(channel)
    }

    private fun base(): NotificationCompat.Builder =
        NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.sym_download)
            .setSilent(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(openDownloads())
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)

    /** The user-initiated job must show a notification before work starts. */
    fun starting(): Notification =
        base()
            .setContentTitle("Getting downloads ready")
            .setProgress(0, 0, true)
            .setOngoing(true)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .addAction(0, "Cancel", cancel())
            .build()

    fun progress(p: DownloadProgress) {
        val total = p.itemTotal.toInt().coerceAtLeast(1)
        val part = p.bytesTotal?.takeIf { it > 0uL }?.let { (p.bytesDone.toDouble() / it.toDouble()).coerceIn(0.0, 1.0) } ?: 0.0
        val done = (p.itemSaved.toInt() * SCALE + (part * SCALE).toInt()).coerceAtMost(total * SCALE)
        val n = base()
            .setContentTitle("Downloading ${p.itemTitle}")
            .setContentText("${p.itemSaved} of ${p.itemTotal} songs")
            .setSubText(if (p.songsTotal > p.itemTotal) "${p.songsDone} of ${p.songsTotal} in all" else null)
            .setProgress(total * SCALE, done, false)
            .setOngoing(true)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .addAction(0, "Cancel", cancel())
            .build()
        post(PROGRESS_ID, n)
    }

    /** The run ended: say so in one line, or nothing when there is nothing to say. */
    fun finished(run: DownloadRun, quiet: Boolean) {
        clearProgress()
        val (title, text) = when (run.stop) {
            DownloadStop.DONE -> if (run.saved > 0u && !quiet) {
                (if (run.saved == 1u) "1 song downloaded" else "${run.saved} songs downloaded") to "Ready to play without a connection"
            } else {
                return
            }
            DownloadStop.STORAGE_LIMIT -> "Downloads paused" to "Storage limit reached. Raise it in Settings › Downloads"
            DownloadStop.LOW_SPACE -> "Downloads paused" to "Your device is nearly full"
            DownloadStop.SIGN_IN_NEEDED -> "Downloads paused" to "Sign in to your server again to keep downloading"
            else -> return
        }
        post(DONE_ID, base().setContentTitle(title).setContentText(text).setAutoCancel(true).build())
    }

    fun clearProgress() = manager.cancel(PROGRESS_ID)

    private fun post(id: Int, n: Notification) {
        // Without the permission (Android 13+, declined) nothing shows; downloads go on.
        if (!manager.areNotificationsEnabled()) return
        runCatching { manager.notify(id, n) }
    }

    private fun openDownloads(): PendingIntent =
        PendingIntent.getActivity(
            context,
            0,
            Intent(context, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
                .putExtra(MainActivity.EXTRA_OPEN, MainActivity.OPEN_DOWNLOADS),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    private fun cancel(): PendingIntent =
        PendingIntent.getBroadcast(
            context,
            1,
            Intent(context, DownloadActionReceiver::class.java).setAction(DownloadActionReceiver.ACTION_CANCEL),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    companion object {
        const val CHANNEL = "downloads"
        const val PROGRESS_ID = 48_10
        const val DONE_ID = 48_11
        private const val SCALE = 100
    }
}
