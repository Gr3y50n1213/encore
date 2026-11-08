package uk.gcartee.encore.app.downloads

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.DownloadState

/**
 * The notification's Cancel (boards 32 and 34): the transfer stops and what was
 * still waiting is paused, so nothing starts again by itself; the ring on each item
 * resumes it. What was saved stays.
 */
class DownloadActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != ACTION_CANCEL) return
        val host = CoreHost.get(context)
        host.core.stopDownloads()
        runCatching {
            host.core.downloadMarks().items
                .filter { it.state != DownloadState.SAVED && !it.auto }
                .forEach { host.core.pauseDownload(it.target, true) }
        }
        DownloadScheduler.cancel(context)
        DownloadNotifications(context).clearProgress()
        host.downloads.refresh()
    }

    companion object {
        const val ACTION_CANCEL = "uk.gcartee.encore.app.downloads.CANCEL"
    }
}
