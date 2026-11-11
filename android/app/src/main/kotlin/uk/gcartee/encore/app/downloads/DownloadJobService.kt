package uk.gcartee.encore.app.downloads

import android.app.job.JobParameters
import android.app.job.JobService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.StatFs
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.DownloadStop
import uk.gcartee.encore.core.EventKind

/**
 * Runs the core's downloads while Android gives the app the time (Phase 8). The
 * core fetches one song at a time and says why it stopped; this keeps the progress
 * notification current (boards 32 and 34) and decides whether to try again later.
 */
class DownloadJobService : JobService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val running = ConcurrentHashMap<Int, Job>()

    override fun onStartJob(params: JobParameters): Boolean {
        val host = CoreHost.get(this)
        // Started with the app closed (a reschedule, the nightly job): the last profile.
        if (!host.ensureProfile()) return false
        val notes = DownloadNotifications(this)
        val userInitiated = Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE && params.isUserInitiatedJob
        if (userInitiated && Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            setNotification(params, DownloadNotifications.PROGRESS_ID, notes.starting(userInitiated = true), JOB_END_NOTIFICATION_POLICY_REMOVE)
        }
        // Smart downloads are quiet: no progress notification (board 32).
        val quiet = params.jobId == DownloadScheduler.JOB_SMART
        val watch = scope.launch(Dispatchers.Main) {
            host.events.collect { e ->
                if (e.kind == EventKind.DOWNLOAD_PROGRESS && !quiet) e.download?.let { notes.progress(it, userInitiated) }
            }
        }
        running[params.jobId] = scope.launch {
            var again = false
            try {
                if (params.jobId == DownloadScheduler.JOB_SMART) runCatching { host.core.refreshSmartDownloads() }
                runCatching { host.core.removeUnplayedDownloads() }
                val run = host.core.runDownloads(host.network(), freeBytes())
                watch.cancel()
                notes.finished(run, quiet)
                again = run.stop == DownloadStop.OFFLINE || run.stop == DownloadStop.SERVER_UNREACHABLE
                if (run.stop == DownloadStop.DONE && run.remaining > 0u) {
                    // Songs were added while the last one finished: another round.
                    Handler(Looper.getMainLooper()).post { DownloadScheduler.start(applicationContext, byUser = false) }
                }
                if (run.stop == DownloadStop.WAITING_FOR_WIFI) {
                    // The Wi-Fi setting changed under the job: schedule with the new rule.
                    Handler(Looper.getMainLooper()).post { DownloadScheduler.start(applicationContext, byUser = false) }
                }
            } catch (e: Exception) {
                watch.cancel()
                notes.clearProgress()
                again = true
            } finally {
                running.remove(params.jobId)
                host.downloads.refresh()
                jobFinished(params, again)
            }
        }
        return true
    }

    override fun onStopJob(params: JobParameters): Boolean {
        // The run ends after the chunk it is writing; the part is kept and resumed.
        CoreHost.get(this).core.stopDownloads()
        // The person cancelled it (the notification's Cancel): no retry.
        val byApp = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && params.stopReason == JobParameters.STOP_REASON_CANCELLED_BY_APP
        return !byApp
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private fun freeBytes(): ULong = runCatching { StatFs(filesDir.absolutePath).availableBytes.toULong() }.getOrDefault(0uL)
}
