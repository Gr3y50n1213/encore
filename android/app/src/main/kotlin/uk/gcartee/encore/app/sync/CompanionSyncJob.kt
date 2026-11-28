package uk.gcartee.encore.app.sync

import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.SyncState

/**
 * Keeps a profile with a companion server in step while the app isn't open: about once
 * an hour, on any network, what is waiting is sent (to the companion and the media
 * server) and what other devices did is fetched. While the app or its player runs, the
 * core syncs within seconds by itself (docs/adr/core-0010); this covers the rest of the
 * time. Not kept across a reboot (no boot permission): the next profile opening
 * schedules it again, like downloads (docs/adr/android-0002).
 */
object CompanionSync {
    const val JOB = 48_05

    /** At profile open, and after connecting to or leaving a companion server. */
    fun resume(context: Context) {
        val js = context.getSystemService(JobScheduler::class.java) ?: return
        val state = runCatching { CoreHost.get(context).core.syncStatus().state }.getOrNull() ?: return
        if (state == SyncState.NOT_CONNECTED) {
            js.cancel(JOB)
            return
        }
        if (js.getPendingJob(JOB) != null) return
        runCatching {
            js.schedule(
                JobInfo.Builder(JOB, ComponentName(context, CompanionSyncJobService::class.java))
                    .setPeriodic(TimeUnit.HOURS.toMillis(1))
                    .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                    .build(),
            )
        }
    }
}

class CompanionSyncJobService : JobService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override fun onStartJob(params: JobParameters): Boolean {
        val host = CoreHost.get(this)
        // Started with the app closed: the last profile (a locked one opens only from the app).
        if (!host.ensureProfile()) return false
        scope.launch {
            try {
                // Each stands alone: one server being away mustn't stop the other. What
                // doesn't go now stays waiting for the next time.
                runCatching { host.core.sendEdits() }
                runCatching { host.core.syncNow() }
            } finally {
                jobFinished(params, false)
            }
        }
        return true
    }

    override fun onStopJob(params: JobParameters): Boolean = false

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }
}
