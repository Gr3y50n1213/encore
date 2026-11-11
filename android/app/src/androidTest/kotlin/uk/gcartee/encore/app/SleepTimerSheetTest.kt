package uk.gcartee.encore.app

import android.app.UiAutomation
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.printToString
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.core.AlbumSort
import uk.gcartee.encore.core.SleepTimerMode
import uk.gcartee.encore.core.StartedFrom

/**
 * Board 31's sleep timer sheet, as a person uses it: open it, pick 15 minutes and see when the
 * music would pause, Start, see the running sheet, Cancel, then Undo from the toast. It opens
 * the way the timer's notification does (an open request), so that path is covered too. Runs
 * after PlaybackFlowTest, signed in to the Jellyfin replay (tools/ci/replay_jellyfin.py).
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class SleepTimerSheetTest {
    @get:Rule
    val rule = createAndroidComposeRule<MainActivity>()

    private val replayUrl: String? = InstrumentationRegistry.getArguments().getString("replayUrl")
    private val automation: UiAutomation get() = InstrumentationRegistry.getInstrumentation().uiAutomation

    @Test
    fun theSheetSetsShowsCancelsAndRestoresATimer() {
        assumeTrue("needs the Jellyfin replay (replayUrl)", replayUrl != null)
        val host = CoreHost.get(rule.activity)
        step("the profile from the sign-in test is open", TIMEOUT) { host.profileOpen.value }
        host.core.cancelSleepTimer()

        // Something is playing, so End of song would work too.
        val album = host.core.albums(AlbumSort.TITLE, 0u, 200u).first { it.songCount >= 2u }
        val songs = host.core.album(album.id).songs
        host.core.play(songs.map { it.id }, 0u, StartedFrom.ALBUM, null)
        rule.runOnUiThread { PlaybackHub.playCurrent(rule.activity) }
        step("the first song plays", TIMEOUT) { host.core.queue().current != null && playing() }

        try {
            // Not running: the presets and when the music would pause.
            rule.runOnUiThread { host.openRequest.value = MainActivity.OPEN_SLEEP_TIMER }
            step("the sheet opens", TIMEOUT) { has("Sleep timer") && has("End of this album") }
            rule.onNodeWithText("15 min").performClick()
            step("the footnote says when the music pauses", TIMEOUT) { has("Music pauses at", substring = true) }
            rule.onNodeWithText("Start for 15 minutes").assertIsDisplayed()
            shot("18-sleep-timer-presets")

            // Start: the running sheet. The notification primer may come first (Android 13+).
            rule.onNodeWithText("Start for 15 minutes").performClick()
            step("the running sheet shows", TIMEOUT) {
                if (has("Not now")) rule.onNodeWithText("Not now").performClick()
                has("Music fades out at", substring = true)
            }
            step("the countdown is read out", TIMEOUT) {
                rule.onAllNodes(hasContentDescription("Sleep timer, ", substring = true)).fetchSemanticsNodes().isNotEmpty()
            }
            rule.onNodeWithText("Cancel timer").assertIsDisplayed()
            rule.onNodeWithText("Add 15 min").assertIsDisplayed()
            val running = host.core.sleepTimer()
            assertNotNull("the core is running the timer", running)
            assertEquals(SleepTimerMode.MINUTES, running!!.mode)
            shot("18-sleep-timer")

            // Cancel: no question, a toast with Undo, and the timer is gone.
            rule.onNodeWithText("Cancel timer").performClick()
            step("Timer cancelled is shown with Undo", TIMEOUT) { has("Timer cancelled") && has("Undo") }
            assertNull("the timer is cancelled", host.core.sleepTimer())

            // Undo: the timer is back.
            rule.onNodeWithText("Undo").performClick()
            step("Undo puts the timer back", TIMEOUT) { host.core.sleepTimer() != null }
        } finally {
            runCatching { host.core.cancelSleepTimer() }
            if (runCatching { playing() }.getOrDefault(false)) rule.runOnUiThread { PlaybackHub.togglePlay(rule.activity) }
        }
    }

    private fun has(text: String, substring: Boolean = false): Boolean =
        rule.onAllNodes(hasText(text, substring = substring)).fetchSemanticsNodes().isNotEmpty()

    private fun playing(): Boolean = runBlocking { PlaybackHub.state.first().isPlaying }

    /** A screenshot for the review, taken by the shell (device-check.sh pulls them). */
    private fun shot(name: String) {
        rule.waitForIdle()
        Thread.sleep(SETTLE_MS)
        val pfd = automation.executeShellCommand("screencap -p /data/local/tmp/encore-shot-$name.png")
        android.os.ParcelFileDescriptor.AutoCloseInputStream(pfd).use { it.readBytes() }
    }

    private fun step(what: String, timeoutMs: Long, done: () -> Boolean) {
        try {
            rule.waitUntil(timeoutMs) { runCatching(done).getOrDefault(false) }
        } catch (e: androidx.compose.ui.test.ComposeTimeoutException) {
            val screen = runCatching { rule.onRoot().printToString(maxDepth = 30) }.getOrDefault("?")
            throw AssertionError("Not reached in ${timeoutMs / 1000} s: $what.\n$screen", e)
        }
    }

    private companion object {
        const val TIMEOUT = 20_000L
        const val SETTLE_MS = 800L
    }
}
