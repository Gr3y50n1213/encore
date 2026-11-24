package uk.gcartee.encore.app.sound

import android.app.job.JobInfo
import android.app.job.JobScheduler
import android.content.ComponentName
import android.content.Context
import java.util.concurrent.TimeUnit
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.SoundSettings

/**
 * When sound analysis and suggestion training run (Phase 10; docs/adr/core-0019, core-0020).
 * The core does the work and says why it stopped; this only asks Android for the time:
 *
 * - The run job works through the songs waiting while Sound analysis is on. Its conditions are
 *   the person's settings: "Only on Wi-Fi" is an unmetered network, "Only while charging" is
 *   charging, and it never starts on a low battery. Android ends a job after some minutes; the
 *   next one carries on where it stopped.
 * - The daily job, on any network while charging, trains the learned ranker when it is due
 *   (it needs no setting: suggestions learn from plays unless learning is off) and analyses
 *   songs added since.
 * - Nothing is kept across a reboot (no boot permission): the next profile opening schedules
 *   again, like downloads (docs/adr/android-0002).
 */
object SoundScheduler {
    const val JOB_RUN = 48_03
    const val JOB_DAILY = 48_04
    private const val PREFS = "sound-schedule"

    /** At profile open, when the settings change, and when a run stops for its conditions. */
    fun resume(context: Context, replace: Boolean = false) {
        val js = context.getSystemService(JobScheduler::class.java) ?: return
        daily(context, js)
        val status = runCatching { CoreHost.get(context).core.soundStatus() }.getOrNull()
        val settings = status?.settings
        if (status == null || settings == null || !settings.enabled) {
            js.cancel(JOB_RUN)
            return
        }
        val waiting = status.songs > status.analysed + status.unavailable
        if (waiting) run(context, js, settings, replace)
    }

    /** The run job is waiting for its conditions or running (Settings > Sound analysis' states). */
    fun isScheduled(context: Context): Boolean =
        context.getSystemService(JobScheduler::class.java)?.getPendingJob(JOB_RUN) != null

    private fun run(context: Context, js: JobScheduler, s: SoundSettings, replace: Boolean) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val rules = "${s.wifiOnly}/${s.chargingOnly}"
        if (!replace && js.getPendingJob(JOB_RUN) != null && prefs.getString("rules", null) == rules) return
        prefs.edit().putString("rules", rules).apply()
        runCatching {
            js.schedule(
                JobInfo.Builder(JOB_RUN, ComponentName(context, SoundJobService::class.java))
                    .setRequiredNetworkType(if (s.wifiOnly) JobInfo.NETWORK_TYPE_UNMETERED else JobInfo.NETWORK_TYPE_ANY)
                    .setRequiresCharging(s.chargingOnly)
                    .setRequiresBatteryNotLow(true)
                    // Linear: Android ends a long run every few minutes, and each time it
                    // should carry on soon, not wait longer and longer.
                    .setBackoffCriteria(BACKOFF_MS, JobInfo.BACKOFF_POLICY_LINEAR)
                    .build(),
            )
        }
    }

    private fun daily(context: Context, js: JobScheduler) {
        if (js.getPendingJob(JOB_DAILY) != null) return
        runCatching {
            js.schedule(
                JobInfo.Builder(JOB_DAILY, ComponentName(context, SoundJobService::class.java))
                    .setPeriodic(TimeUnit.DAYS.toMillis(1))
                    .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                    .setRequiresCharging(true)
                    .build(),
            )
        }
    }

    private const val BACKOFF_MS = 30_000L
}
