package uk.gcartee.encore.app.sound

import android.app.job.JobParameters
import android.app.job.JobService
import android.content.Context
import android.os.BatteryManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.PowerState
import uk.gcartee.encore.core.SoundStop

/**
 * Runs suggestion training and sound analysis while Android gives the app the time (Phase 10).
 * The core analyses one song at a time and says why it stopped; this decides what comes next.
 */
class SoundJobService : JobService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    override fun onStartJob(params: JobParameters): Boolean {
        val host = CoreHost.get(this)
        // Started with the app closed: the last profile (a locked one opens only from the app).
        if (!host.ensureProfile()) return false
        scope.launch {
            var again = false
            var reschedule = false
            try {
                // Suggestions learn from the plays first: it needs no setting, and with a
                // companion server it is the server that trains.
                runCatching { host.core.trainRankerIfDue() }
                val enabled = runCatching { host.core.soundSettings().enabled }.getOrDefault(false)
                if (enabled && SoundModel.ensure(applicationContext, host.core)) {
                    val run = host.core.runSoundAnalysis(host.network(), power(applicationContext))
                    when (run.stop) {
                        // Battery saver, heat or no network: try again later.
                        SoundStop.PAUSED, SoundStop.OFFLINE -> again = params.jobId == SoundScheduler.JOB_RUN
                        // The settings changed under the job: schedule with them as they are now.
                        // (Android ending the job is onStopJob's; Stop from the app stays stopped.)
                        SoundStop.WAITING_FOR_WIFI, SoundStop.WAITING_FOR_POWER -> reschedule = true
                        // The daily job may leave songs for the run job.
                        SoundStop.DONE -> reschedule = run.remaining > 0u
                        // The server failed three songs in a row (the daily job tries again);
                        // not enabled, no model, sign-in needed, already running: nothing to do.
                        else -> {}
                    }
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                again = params.jobId == SoundScheduler.JOB_RUN
            } finally {
                if (reschedule) Handler(Looper.getMainLooper()).post { SoundScheduler.resume(applicationContext, replace = true) }
                jobFinished(params, again)
            }
        }
        return true
    }

    override fun onStopJob(params: JobParameters): Boolean {
        // Android ended the job (its time is up, or the charger or Wi-Fi went): the run ends after
        // the song it is on, keeping everything done, and Android runs it again when it can.
        CoreHost.get(this).core.stopSoundAnalysis()
        return params.jobId == SoundScheduler.JOB_RUN
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private fun power(context: Context): PowerState {
        val battery = context.getSystemService(BatteryManager::class.java)
        val pm = context.getSystemService(PowerManager::class.java)
        val hot = Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q &&
            (pm?.currentThermalStatus ?: PowerManager.THERMAL_STATUS_NONE) >= PowerManager.THERMAL_STATUS_MODERATE
        return PowerState(
            charging = battery?.isCharging == true,
            batterySaver = pm?.isPowerSaveMode == true,
            hot = hot,
        )
    }
}
