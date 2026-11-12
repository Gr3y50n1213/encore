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
import androidx.glance.appwidget.GlanceAppWidget
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
import uk.gcartee.encore.app.widgets.MixesWidget
import uk.gcartee.encore.app.widgets.MixesWidgetReceiver
import uk.gcartee.encore.app.widgets.RecentWidget
import uk.gcartee.encore.app.widgets.RecentWidgetReceiver
import uk.gcartee.encore.app.widgets.WidgetConfig
import uk.gcartee.encore.app.widgets.WidgetIntents

/**
 * Phase 9 (WP7) on an emulator, with no activity open:
 * 1. the Mixes and Recently played widgets, bound through a test AppWidgetHost, update
 *    without a crash and without starting the core (checked when this process hasn't
 *    started it already: android/ci/device-check.sh runs this class in a fresh process first);
 * 2. with the Jellyfin replay (replayUrl), a Mixes tile's own PendingIntent (the private
 *    play-mix action with the install's token, for Next up), sent as the launcher would
 *    with no activity open, starts playback.
 *
 * What an instrumented test can't do: kill the app's process, tap a real launcher (the
 * PendingIntent is sent from this process with the temporary allowlist a tap gets), or host
 * a widget on the lock screen (these two declare not_keyguard: WidgetProviderXmlTest).
 */
@RunWith(AndroidJUnit4::class)
class CollectionWidgetsTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context: Context get() = instrumentation.targetContext
    private val automation: UiAutomation get() = instrumentation.uiAutomation
    private val replayUrl: String? = InstrumentationRegistry.getArguments().getString("replayUrl")

    @Test
    fun boundMixesAndRecentWidgetsUpdateWithoutStartingTheCore() {
        assumeTrue("needs app widgets", context.packageManager.hasSystemFeature(PackageManager.FEATURE_APP_WIDGETS))
        val coreBefore = coreStarted()
        val marker = mark()
        val host = AppWidgetHost(context, HOST_ID)
        runCatching { host.startListening() }
        shell("appwidget grantbind --package ${context.packageName}")
        val ids = listOf(
            bind(host, MixesWidgetReceiver::class.java) to (MixesWidget() as GlanceAppWidget),
            bind(host, RecentWidgetReceiver::class.java) to (RecentWidget() as GlanceAppWidget),
        )
        try {
            for ((id, widget) in ids) {
                runBlocking { widget.update(context, GlanceAppWidgetManager(context).getGlanceIdBy(id)) }
            }
            // And the way Android asks (updatePeriodMillis), through each receiver.
            context.sendBroadcast(
                Intent(AppWidgetManager.ACTION_APPWIDGET_UPDATE)
                    .setComponent(ComponentName(context, MixesWidgetReceiver::class.java))
                    .putExtra(AppWidgetManager.EXTRA_APPWIDGET_IDS, intArrayOf(ids[0].first)),
            )
            context.sendBroadcast(
                Intent(AppWidgetManager.ACTION_APPWIDGET_UPDATE)
                    .setComponent(ComponentName(context, RecentWidgetReceiver::class.java))
                    .putExtra(AppWidgetManager.EXTRA_APPWIDGET_IDS, intArrayOf(ids[1].first)),
            )
            Thread.sleep(SETTLE_MS)
            if (!coreBefore) assertFalse("a widget update started the core", coreStarted())
            assertNoCrash(marker)
        } finally {
            ids.forEach { runCatching { host.deleteAppWidgetId(it.first) } }
            runCatching { host.stopListening() }
        }
    }

    @Test
    fun aMixesTileStartsPlaybackWithNoActivityOpen() {
        assumeTrue("needs the Jellyfin replay (replayUrl)", replayUrl != null)
        val marker = mark()
        val host = CoreHost.get(context)
        assertTrue("the profile from the sign-in test opens", host.ensureProfile())
        // WidgetSync has saved the open profile, so the widget's tiles play directly.
        val profile = host.core.currentProfile()!!
        waitFor("WidgetSync saved the profile", TIMEOUT) { WidgetConfig.canControl(profile, WidgetConfig.profileState(context)) }
        // The tile can only start a mix the profile has. The core builds the mixes in the background
        // after a sign-in, so give it a minute before calling the replay's history too short (this
        // failed once on the API 36 phone and passed on a rerun with nothing changed).
        val end = System.currentTimeMillis() + MIXES_BUILD_MS
        while (System.currentTimeMillis() < end) {
            val s = runCatching { host.core.widgetSnapshot() }.getOrNull()
            if (s != null && (s.nextUp != null || s.pinnedMixes.isNotEmpty())) break
            Thread.sleep(POLL_MS * 5)
        }
        val snapshot = host.core.widgetSnapshot()
        Log.i(TAG, "Next up: ${snapshot.nextUp?.title}; pinned mixes: ${snapshot.pinnedMixes.size}")
        assertTrue(
            "the profile has a mix to start (Next up or a pinned mix); the replay's history may be too short",
            snapshot.nextUp != null || snapshot.pinnedMixes.isNotEmpty(),
        )
        instrumentation.runOnMainSync { PlaybackHub.isConnected.value?.pause() }
        Thread.sleep(POLL_MS * 5)

        // The Next up tile's PendingIntent, as the launcher sends it (a foreground service
        // start, the widget tap's allowlist standing in for the tap).
        allowForegroundStart()
        PendingIntent.getForegroundService(context, REQUEST_TILE, WidgetIntents.playMix(context, null), PendingIntent.FLAG_IMMUTABLE).send()
        waitFor("the mix plays", TIMEOUT) { playing() }
        Thread.sleep(SETTLE_MS)
        assertNoCrash(marker)
        instrumentation.runOnMainSync { PlaybackHub.isConnected.value?.pause() }
    }

    // --- Helpers. ---

    private fun coreStarted(): Boolean {
        val field = CoreHost::class.java.getDeclaredField("instance").apply { isAccessible = true }
        return field.get(null) != null
    }

    private fun bind(host: AppWidgetHost, receiver: Class<*>): Int {
        val id = host.allocateAppWidgetId()
        val bound = AppWidgetManager.getInstance(context).bindAppWidgetIdIfAllowed(id, ComponentName(context, receiver))
        assertTrue("${receiver.simpleName} could be bound (appwidget grantbind)", bound)
        return id
    }

    private fun allowForegroundStart() {
        shell("cmd deviceidle tempwhitelist ${context.packageName}")
    }

    private fun playing(): Boolean = runBlocking { PlaybackHub.state.first().isPlaying }

    private fun mark(): String = "collection-widget-test-${UUID.randomUUID()}".also { Log.i(TAG, it) }

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
        const val TAG = "CollectionWidgetsTest"
        const val HOST_ID = 0x5158
        const val REQUEST_TILE = 3
        const val TIMEOUT = 20_000L
        const val SETTLE_MS = 3_000L
        const val POLL_MS = 200L
        const val MIXES_BUILD_MS = 60_000L
    }
}
