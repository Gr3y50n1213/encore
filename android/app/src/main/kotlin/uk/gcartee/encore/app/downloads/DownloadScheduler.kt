package uk.gcartee.encore.app.downloads

import android.app.job.JobInfo
import android.app.job.JobScheduler
import android.content.ComponentName
import android.content.Context
import android.os.Build
import java.util.concurrent.TimeUnit
import uk.gcartee.encore.app.core.CoreHost

/**
 * When downloads run (Phase 8; docs/adr/android-0002). The core does the work in
 * `run_downloads`; this only asks Android for the time and the network to do it:
 *
 * - A download the person started is a user-initiated data transfer job on Android
 *   14 and later (plan section 20), with its progress notification; on older
 *   versions, and when the app isn't on screen, a normal job.
 * - "Download only on Wi-Fi" is the job's network constraint (unmetered), so a job
 *   never wakes up on cellular to find it can't run.
 * - Smart downloads are a daily job on an unmetered network while charging.
 * - Nothing is persisted across a reboot (no boot permission): the app reschedules
 *   what is waiting the next time a profile opens.
 */
object DownloadScheduler {
    const val JOB_NOW = 48_01
    const val JOB_SMART = 48_02
    private const val PREFS = "downloads-schedule"

    /** Start the transfer job, or leave the one already going (it picks up new songs). */
    fun start(context: Context, byUser: Boolean) {
        val js = context.getSystemService(JobScheduler::class.java) ?: return
        val wifiOnly = runCatching { CoreHost.get(context).core.downloadSettings().wifiOnly }.getOrDefault(true)
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val sameNetwork = prefs.getBoolean("wifiOnly", wifiOnly) == wifiOnly
        if (js.getPendingJob(JOB_NOW) != null && sameNetwork) return
        prefs.edit().putBoolean("wifiOnly", wifiOnly).apply()
        fun job(userInitiated: Boolean): JobInfo {
            val b = JobInfo.Builder(JOB_NOW, ComponentName(context, DownloadJobService::class.java))
                .setRequiredNetworkType(if (wifiOnly) JobInfo.NETWORK_TYPE_UNMETERED else JobInfo.NETWORK_TYPE_ANY)
                .setBackoffCriteria(BACKOFF_MS, JobInfo.BACKOFF_POLICY_EXPONENTIAL)
            if (userInitiated && Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) b.setUserInitiated(true)
            return b.build()
        }
        // A user-initiated job needs the app on screen; when it isn't (Android Auto,
        // a job rescheduling itself) a normal job does the same work.
        val scheduled = byUser && runCatching { js.schedule(job(userInitiated = true)) == JobScheduler.RESULT_SUCCESS }.getOrDefault(false)
        if (!scheduled) runCatching { js.schedule(job(userInitiated = false)) }
    }

    /** At profile open: whatever was waiting starts again, and Smart downloads keep their night. */
    fun resume(context: Context) {
        val host = CoreHost.get(context)
        val overview = runCatching { host.core.downloadsOverview() }.getOrNull() ?: return
        if (overview.queued > 0u) start(context, byUser = true)
        smart(context, overview.settings.smartDownloads)
    }

    /** Smart downloads on or off (section 21: nightly, on Wi-Fi while charging). */
    fun smart(context: Context, on: Boolean) {
        val js = context.getSystemService(JobScheduler::class.java) ?: return
        if (!on) {
            js.cancel(JOB_SMART)
            return
        }
        if (js.getPendingJob(JOB_SMART) != null) return
        runCatching {
            js.schedule(
                JobInfo.Builder(JOB_SMART, ComponentName(context, DownloadJobService::class.java))
                    .setPeriodic(TimeUnit.DAYS.toMillis(1))
                    .setRequiredNetworkType(JobInfo.NETWORK_TYPE_UNMETERED)
                    .setRequiresCharging(true)
                    .build(),
            )
        }
    }

    /** The person cancelled (the notification's Cancel): stop without retrying. */
    fun cancel(context: Context) {
        context.getSystemService(JobScheduler::class.java)?.cancel(JOB_NOW)
    }

    private const val BACKOFF_MS = 30_000L
}
