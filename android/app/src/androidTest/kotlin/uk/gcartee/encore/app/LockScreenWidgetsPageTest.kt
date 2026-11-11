package uk.gcartee.encore.app

import android.app.UiAutomation
import android.os.Build
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isToggleable
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodes
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.printToString
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.notifications.LiveUpdates

/**
 * Phase 9 (WP8), board 31: Settings > Lock screen and widgets. Opens the page from the Settings
 * root, checks its rows, flips the sleep timer Live Update switch and reads the preference
 * back, then puts it back. Runs signed in to the Jellyfin replay, after the sleep timer sheet
 * test (a profile has to be open to reach Settings).
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class LockScreenWidgetsPageTest {
    @get:Rule
    val rule = createAndroidComposeRule<MainActivity>()

    private val replayUrl: String? = InstrumentationRegistry.getArguments().getString("replayUrl")
    private val automation: UiAutomation get() = InstrumentationRegistry.getInstrumentation().uiAutomation

    @Test
    fun thePageShowsItsRowsAndATogglePersists() {
        assumeTrue("needs the Jellyfin replay (replayUrl)", replayUrl != null)
        val host = CoreHost.get(rule.activity)
        step("the profile from the sign-in test is open", TIMEOUT) { host.profileOpen.value }
        val context = rule.activity
        val was = LiveUpdates.isOn(context, LiveUpdates.Card.SLEEP_TIMER)

        try {
            rule.onNodeWithContentDescription("Profiles and settings").performClick()
            step("the profile sheet shows Settings", TIMEOUT) { has("Settings") }
            rule.onNodeWithText("Settings").performClick()
            step("the Settings root has its new row", TIMEOUT) { has("Lock screen and widgets") }
            rule.onNodeWithText("Lock screen and widgets").performClick()
            step("the page opens", TIMEOUT) { has("Sleep timer Live Update") }

            // The page's rows (the lower ones scroll into view).
            has("Time left and Add 15 min")
            reach("Download progress")
            reach("Only downloads you start")
            reach("Hide listening history while locked")
            reach("Widget profile")
            reach("Quick Settings tile")
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                reach("Add")
            } else {
                reach("Add it from the Quick Settings edit screen")
            }
            // Where Live Updates don't exist the page says so quietly; the banner is for 16 QPR1+.
            if (!LiveUpdates.apiAllows()) {
                reach("Live Updates need Android 16 or later.", substring = true)
                assertFalse(has("Open settings"))
            }
            rule.onNode(hasScrollAction()).performScrollToNode(hasText("Sleep timer Live Update"))
            shot("19-lock-screen-and-widgets")

            // The first switch is the sleep timer card: flip it, read the preference back.
            rule.onAllNodes(isToggleable())[0].performClick()
            step("the preference changed", TIMEOUT) { LiveUpdates.isOn(context, LiveUpdates.Card.SLEEP_TIMER) == !was }
            assertEquals(!was, LiveUpdates.isOn(context, LiveUpdates.Card.SLEEP_TIMER))
            // Turning a card off never touches the timer; here it only has to survive.
            rule.onAllNodes(isToggleable())[0].performClick()
            step("it flips back", TIMEOUT) { LiveUpdates.isOn(context, LiveUpdates.Card.SLEEP_TIMER) == was }
            assertTrue(LiveUpdates.hideHistoryWhileLocked(context))
        } finally {
            LiveUpdates.setOn(context, LiveUpdates.Card.SLEEP_TIMER, was)
        }
    }

    private fun has(text: String, substring: Boolean = false): Boolean =
        rule.onAllNodes(hasText(text, substring = substring)).fetchSemanticsNodes().isNotEmpty()

    /** Scrolls the page to [text] and checks it is there. */
    private fun reach(text: String, substring: Boolean = false) {
        runCatching { rule.onNode(hasScrollAction()).performScrollToNode(hasText(text, substring = substring)) }
        step("\"$text\" is on the page", TIMEOUT) { has(text, substring) }
    }

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
