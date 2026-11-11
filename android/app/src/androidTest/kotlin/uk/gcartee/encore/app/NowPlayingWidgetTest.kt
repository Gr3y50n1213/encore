package uk.gcartee.encore.app

import android.app.PendingIntent
import android.app.UiAutomation
import android.appwidget.AppWidgetHost
import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.UUID
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.playback.PlaybackService
import uk.gcartee.encore.app.widgets.NowPlayingWidget
import uk.gcartee.encore.app.widgets.NowPlayingWidgetReceiver
import uk.gcartee.encore.app.widgets.WidgetConfig
import uk.gcartee.encore.app.widgets.WidgetIntents
import uk.gcartee.encore.core.AlbumSort
import uk.gcartee.encore.core.StartedFrom

/**
 * Phase 9 (WP6) on an emulator, with no activity open:
 * 1. the Now Playing widget, bound through a test AppWidgetHost, updates without a crash
 *    and without starting the core (checked when this process hasn't started it already:
 *    android/ci/device-check.sh runs this class in a fresh process first);
 * 2. with the Jellyfin replay (replayUrl), the widget's own intents sent as the launcher
 *    would: a private action with a wrong token plays nothing and the service still keeps
 *    Android's foreground promise; then Play / pause reaches PLAYING. No
 *    ForegroundServiceDidNotStartInTimeException in the log either way.
 *
 * What an instrumented test can't do: kill the app's process (the test runs in it), tap a
 * real launcher (the PendingIntent is sent from this process, with the temporary allowlist
 * a widget tap gets standing in for the tap), or host the widget on a lock screen.
 */
@RunWith(AndroidJUnit4::class)
class NowPlayingWidgetTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context: Context get() = instrumentation.targetContext
    private val automation: UiAutomation get() = instrumentation.uiAutomation
    private val replayUrl: String? = InstrumentationRegistry.getArguments().getString("replayUrl")

    @Test
    fun aBoundWidgetUpdatesWithoutStartingTheCore() {
        assumeTrue("needs app widgets", context.packageManager.hasSystemFeature(PackageManager.FEATURE_APP_WIDGETS))
        val coreBefore = coreStarted()
        val marker = mark()
        val host = AppWidgetHost(context, HOST_ID)
        runCatching { host.startListening() }
        val id = bind(host)
        try {
            runBlocking {
                NowPlayingWidget().update(context, GlanceAppWidgetManager(context).getGlanceIdBy(id))
            }
            // And the way Android asks (updatePeriodMillis), through the receiver.
            context.sendBroadcast(
                Intent(AppWidgetManager.ACTION_APPWIDGET_UPDATE)
                    .setComponent(ComponentName(context, NowPlayingWidgetReceiver::class.java))
                    .putExtra(AppWidgetManager.EXTRA_APPWIDGET_IDS, intArrayOf(id)),
            )
            Thread.sleep(SETTLE_MS)
            if (!coreBefore) assertFalse("a widget update started the core", coreStarted())
            assertNoCrash(marker)
        } finally {
            runCatching { host.deleteAppWidgetId(id) }
            runCatching { host.stopListening() }
        }
    }

    @Test
    fun widgetButtonsPlayWithTheAppClosedAndKeepTheForegroundPromise() {
        assumeTrue("needs the Jellyfin replay (replayUrl)", replayUrl != null)
        val marker = mark()
        val host = CoreHost.get(context)
        assertTrue("the profile from the sign-in test opens", host.ensureProfile())
        // A queue to resume, nothing playing.
        val album = host.core.albums(AlbumSort.TITLE, 0u, 50u).first { it.songCount >= 1u }
        host.core.play(host.core.album(album.id).songs.map { it.id }, 0u, StartedFrom.ALBUM, null)
        // WidgetSync has saved the open profile, so the widget's buttons play directly.
        val profile = host.core.currentProfile()!!
        waitFor("WidgetSync saved the profile", TIMEOUT) { WidgetConfig.canControl(profile, WidgetConfig.profileState(context)) }

        // A start that plays nothing (a private action with a wrong token): the service
        // must still reach startForeground in time, then stop.
        allowForegroundStart()
        val forged = Intent(WidgetIntents.ACTION_PLAY_MIX)
            .setComponent(ComponentName(context, PlaybackService::class.java))
            .putExtra(WidgetIntents.EXTRA_TOKEN, "not-the-token")
        PendingIntent.getForegroundService(context, REQUEST_FORGED, forged, PendingIntent.FLAG_IMMUTABLE).send()
        Thread.sleep(FOREGROUND_DEADLINE_MS)
        assertFalse("a forged token started playback", playing())
        assertNoCrash(marker)

        // The widget's Play / pause, as the launcher sends it.
        allowForegroundStart()
        PendingIntent.getForegroundService(
            context,
            REQUEST_PLAY,
            WidgetIntents.mediaButton(context, WidgetIntents.KEY_PLAY_PAUSE),
            PendingIntent.FLAG_IMMUTABLE,
        ).send()
        waitFor("the session plays", TIMEOUT) { playing() }
        Thread.sleep(SETTLE_MS)
        assertNoCrash(marker)
        instrumentation.runOnMainSync { PlaybackHub.isConnected.value?.pause() }
    }

    // --- Helpers. ---

    /** Whether CoreHost (and so the core) exists in this process. */
    private fun coreStarted(): Boolean {
        val field = CoreHost::class.java.getDeclaredField("instance").apply { isAccessible = true }
        return field.get(null) != null
    }

    private fun bind(host: AppWidgetHost): Int {
        shell("appwidget grantbind --package ${context.packageName}")
        val id = host.allocateAppWidgetId()
        val bound = AppWidgetManager.getInstance(context)
            .bindAppWidgetIdIfAllowed(id, ComponentName(context, NowPlayingWidgetReceiver::class.java))
        assertTrue("the widget could be bound (appwidget grantbind)", bound)
        return id
    }

    /** What a widget tap gives the app: a short window to start a foreground service. */
    private fun allowForegroundStart() {
        shell("cmd deviceidle tempwhitelist ${context.packageName}")
    }

    private fun playing(): Boolean = runBlocking { PlaybackHub.state.first().isPlaying }

    /** A line in the log; only what comes after it counts. */
    private fun mark(): String = "widget-test-${UUID.randomUUID()}".also { Log.i(TAG, it) }

    private fun assertNoCrash(marker: String) {
        val log = shell("logcat -d -v brief")
        val after = log.substringAfter(marker, missingDelimiterValue = "")
        assertTrue("the marker is in the log", log.contains(marker))
        assertFalse("ForegroundServiceDidNotStartInTimeException:\n$after", after.contains("ForegroundServiceDidNotStartInTimeException"))
        assertFalse("a crash:\n$after", after.contains("FATAL EXCEPTION"))
    }

    private fun shell(command: String): String {
        val pfd = automation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(pfd).use { it.readBytes().toString(Charsets.UTF_8) }
    }

    private fun waitFor(what: String, timeoutMs: Long, done: () -> Boolean) {
        val end = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < end) {
            if (runCatching(done).getOrDefault(false)) return
            Thread.sleep(POLL_MS)
        }
        throw AssertionError("Not reached in ${timeoutMs / 1000} s: $what")
    }

    private companion object {
        const val TAG = "NowPlayingWidgetTest"
        const val HOST_ID = 0x5157
        const val REQUEST_FORGED = 1
        const val REQUEST_PLAY = 2
        const val TIMEOUT = 20_000L
        const val SETTLE_MS = 3_000L
        const val POLL_MS = 200L

        /** Longer than Android's limit for startForeground (10 s from Android 10). */
        const val FOREGROUND_DEADLINE_MS = 12_000L
    }
}
