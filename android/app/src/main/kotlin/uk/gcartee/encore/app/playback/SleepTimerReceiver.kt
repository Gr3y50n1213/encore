package uk.gcartee.encore.app.playback

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import uk.gcartee.encore.app.core.CoreHost

/**
 * Phase 9: the sleep timer card's buttons (board 32). Not exported: only the card's own
 * PendingIntents reach it.
 * - Add 15 min: the end moves on 15 minutes (a song or album timer becomes a time); the
 *   countdown shows it at once. An accidental tap only adds time.
 * - The card swiped away: hides the card for this timer; the timer keeps running.
 */
class SleepTimerReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            ACTION_ADD_15 -> {
                val core = CoreHost.get(context).core
                // NotFound (it just ended) or NotAllowed (past 24 hours): show what is true now.
                val timer = runCatching { core.extendSleepTimer(ADD_MINUTES) }
                    .getOrElse { runCatching { core.sleepTimer() }.getOrNull() }
                // SleepTimerChanged redraws it too; this is at once, before the process may go.
                SleepTimerNotifier.update(context.applicationContext, timer)
            }
            ACTION_HIDDEN -> SleepTimerNotifier.hide(context.applicationContext, intent.getLongExtra(EXTRA_SET_UTC_MS, 0L))
        }
    }

    companion object {
        const val ACTION_ADD_15 = "uk.gcartee.encore.app.sleeptimer.ADD_15"
        const val ACTION_HIDDEN = "uk.gcartee.encore.app.sleeptimer.HIDDEN"
        const val EXTRA_SET_UTC_MS = "set-utc-ms"
        private const val ADD_MINUTES = 15u
    }
}
