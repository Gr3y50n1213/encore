package uk.gcartee.encore.app

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.printToString
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.AlbumSort

/**
 * Phase 2's "done when", as far as an emulator can show it: sign in to a Jellyfin server
 * through the real Connect screen (Quick Connect), let the library sync, browse it, and
 * start an album. The server is the owner's anonymized Jellyfin recording, replayed on
 * the CI host (tools/ci/replay_jellyfin.py) and reached through `adb reverse`; the run
 * passes its address as the `replayUrl` argument. The recording holds no audio, so the
 * sound itself is checked on a phone.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class SignInAndBrowseTest {
    @get:Rule
    val rule = createAndroidComposeRule<MainActivity>()

    private val replayUrl: String? = InstrumentationRegistry.getArguments().getString("replayUrl")

    @Test
    fun signInBrowseAndPlayAnAlbum() {
        assumeTrue("needs the Jellyfin replay (replayUrl)", replayUrl != null)

        // Welcome: Get started ignores taps for its first 300 ms (board 22, item 3).
        rule.waitUntilAtLeastOneExists(hasText("Get started"), TIMEOUT)
        // Compose tests run on a virtual clock: move it past the guard.
        rule.mainClock.advanceTimeBy(GUARD_MS)
        rule.onNodeWithText("Get started").performClick()

        // Connect: the address is checked 600 ms after typing stops (board 21, item 3).
        rule.waitUntilAtLeastOneExists(hasTestTag("address"), TIMEOUT)
        rule.onNodeWithTag("address").performTextInput(replayUrl!!)
        rule.mainClock.advanceTimeBy(CHECK_AFTER_MS)
        rule.waitUntilAtLeastOneExists(hasText("Found Jellyfin", substring = true), TIMEOUT)
        rule.onNodeWithTag("quick-connect").performScrollTo().performClick()

        // Quick Connect approves (the recording's approval) and the profile opens. Each
        // step is waited for on its own, so a failure says which one stalled and why.
        val host = CoreHost.get(rule.activity)
        step("signed in (the profile opened)", SIGN_IN_TIMEOUT) { host.profileOpen.value }
        step("the first library sync finished", SYNC_TIMEOUT) {
            val s = host.sync.state.value
            !s.running && s.generation > 0
        }
        val sync = host.sync.state.value
        assertNull("the library sync failed: ${sync.problem}", sync.problem)
        // Library opens on Albums, Recently added, as a grid: its first tile is on screen.
        val albums = host.core.albums(AlbumSort.RECENTLY_ADDED, 0u, 10u)
        assertTrue("the synced library has no albums", albums.isNotEmpty())
        val album = albums.first()
        shot("01-home")

        // Library: the album is listed from the on-device mirror.
        rule.onNodeWithText("Library").performClick()
        rule.waitUntilAtLeastOneExists(hasText(album.title), TIMEOUT)
        shot("02-library")
        rule.onAllNodesWithText(album.title).onFirst().performClick()

        // Album: Play replaces the queue with it; the mini player shows its first song.
        rule.waitUntilAtLeastOneExists(hasText("Play"), TIMEOUT)
        shot("03-album")
        rule.onNodeWithText("Play").performScrollTo().performClick()
        val first = host.core.album(album.id).songs.first()
        rule.waitUntil(TIMEOUT) { host.core.queue().current?.song?.id == first.id }
        rule.waitUntilAtLeastOneExists(hasTestTag("mini-player"), TIMEOUT)

        // Now Playing, its lyrics and its queue (boards 09, 10, 11).
        val current = host.core.queue().current!!.song
        rule.onNodeWithContentDescription("${current.title}, ${current.artist}").performClick()
        rule.waitUntilAtLeastOneExists(hasContentDescription("Close Now Playing"), TIMEOUT)
        shot("04-now-playing")
        rule.onNodeWithText("Lyrics").performClick()
        shot("05-lyrics")
        rule.onNodeWithText("Queue").performClick()
        rule.waitUntilAtLeastOneExists(hasText("Autoplay"), TIMEOUT)
        shot("06-queue")
        rule.onNodeWithContentDescription("Close Now Playing").performClick()
        rule.waitUntilDoesNotExist(hasContentDescription("Close Now Playing"), TIMEOUT)

        // A playlist page lists its songs (the owner found it broken on 7 November 2026).
        val playlist = host.core.playlists(0u, 50u).firstOrNull { it.songCount > 0u }
        if (playlist != null) {
            val firstSong = host.core.playlist(playlist.id).entries.firstNotNullOf { it.song }
            // Back from the album to Library's root.
            Espresso.pressBack()
            rule.waitUntilAtLeastOneExists(hasText("Playlists"), TIMEOUT)
            rule.onAllNodesWithText("Playlists").onFirst().performClick()
            rule.waitUntilAtLeastOneExists(hasText(playlist.name), TIMEOUT)
            rule.onAllNodesWithText(playlist.name).onFirst().performClick()
            rule.waitUntilAtLeastOneExists(hasText(firstSong.title), TIMEOUT)
            rule.waitUntilAtLeastOneExists(hasText("Shuffle"), TIMEOUT)
            shot("07-playlist")
        }

        // Search, Mixes, Settings (boards 07, 23, 17).
        rule.onNodeWithText("Search").performClick()
        rule.onNode(hasSetTextAction()).performTextInput(album.title.split(" ").first())
        // The search key saves the query and puts the keyboard away (board 07).
        rule.onNode(hasSetTextAction()).performImeAction()
        rule.waitUntilAtLeastOneExists(hasText(album.title), TIMEOUT)
        shot("08-search")
        rule.onNodeWithText("Mixes").performClick()
        shot("09-mixes")
        // Library still shows the playlist opened above; tapping its tab again returns to its root (board 05, item 6).
        rule.onNodeWithText("Library").performClick()
        rule.onAllNodesWithText("Library").onFirst().performClick()
        rule.waitUntilAtLeastOneExists(hasContentDescription("Settings"), TIMEOUT)
        rule.onNodeWithContentDescription("Settings").performClick()
        rule.waitUntilAtLeastOneExists(hasText("Appearance"), TIMEOUT)
        shot("10-settings")
        rule.onNodeWithText("Appearance").performClick()
        rule.waitUntilAtLeastOneExists(hasText("Accent colour"), TIMEOUT)
        shot("11-appearance")
    }

    /**
     * A screenshot for the review, taken by the shell (it can write /data/local/tmp);
     * android/ci/device-check.sh pulls them. The clock is moved past the transitions first.
     */
    private fun shot(name: String) {
        rule.mainClock.advanceTimeBy(SETTLE_MS)
        rule.waitForIdle()
        Thread.sleep(400)
        val pfd = InstrumentationRegistry.getInstrumentation().uiAutomation
            .executeShellCommand("screencap -p /data/local/tmp/encore-shot-$name.png")
        android.os.ParcelFileDescriptor.AutoCloseInputStream(pfd).use { it.readBytes() }
    }

    /** Waits for [done], pumping the UI; fails naming the step if it never comes. */
    private fun step(what: String, timeoutMs: Long, done: () -> Boolean) {
        try {
            rule.waitUntil(timeoutMs) { done() }
        } catch (e: androidx.compose.ui.test.ComposeTimeoutException) {
            // What the screen said at that moment (an error line, a spinner, a code).
            val screen = runCatching { rule.onRoot().printToString(maxDepth = 30) }.getOrDefault("?")
            throw AssertionError("Not reached in ${timeoutMs / 1000} s: $what.\n$screen", e)
        }
    }

    private companion object {
        const val SIGN_IN_TIMEOUT = 30_000L
        const val GUARD_MS = 400L
        const val SETTLE_MS = 900L
        const val CHECK_AFTER_MS = 700L
        const val TIMEOUT = 10_000L
        const val SYNC_TIMEOUT = 60_000L
    }
}
