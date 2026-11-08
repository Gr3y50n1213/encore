package uk.gcartee.encore.app

import android.app.UiAutomation
import android.content.ComponentName
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.printToString
import androidx.media3.common.MediaItem
import androidx.media3.session.MediaBrowser
import androidx.media3.session.SessionToken
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.playback.PlaybackService
import uk.gcartee.encore.core.AlbumSort
import uk.gcartee.encore.core.DownloadState
import uk.gcartee.encore.core.EncoreException
import uk.gcartee.encore.core.StartedFrom

/**
 * Phase 8's "done when", as far as an emulator shows it: a downloaded album plays with
 * no network, and the car's library (Android Auto's browse tree) plays through the core.
 * Runs after SignInAndBrowseTest, signed in to the Jellyfin replay, which serves made-up
 * audio (tools/ci/replay_jellyfin.py). The network is turned off with `svc` and always
 * turned back on. The car itself and a real airplane mode are for the owner's phone.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class OfflineAndCarTest {
    @get:Rule
    val rule = createAndroidComposeRule<MainActivity>()

    private val replayUrl: String? = InstrumentationRegistry.getArguments().getString("replayUrl")
    private val automation: UiAutomation get() = InstrumentationRegistry.getInstrumentation().uiAutomation

    @Test
    fun aDownloadedAlbumPlaysOfflineAndTheCarBrowsesAndPlays() {
        assumeTrue("needs the Jellyfin replay (replayUrl)", replayUrl != null)
        val host = CoreHost.get(rule.activity)
        step("the profile from the sign-in test is open", TIMEOUT) { host.profileOpen.value }
        // The emulator's network may be metered: allow downloads on it for the test.
        host.core.setDownloadSettings(host.core.downloadSettings().copy(wifiOnly = false))

        // An album with two songs or more, downloaded from its page (board 08).
        val album = host.core.albums(AlbumSort.TITLE, 0u, 200u).first { it.songCount >= 2u }
        val songs = host.core.album(album.id).songs
        rule.onNodeWithText("Library").performClick()
        rule.waitUntilAtLeastOneExists(hasText("Downloaded"), TIMEOUT)
        rule.onAllNodesWithText("Albums").onFirst().performClick()
        openAlbum(album.title)
        rule.onNodeWithContentDescription("Download").performClick()
        step("every song of the album downloaded (the job ran)", DOWNLOAD_TIMEOUT) {
            host.core.downloadMarks().items.any { it.target.id == album.id && it.state == DownloadState.SAVED }
        }
        val saved = host.core.downloadMarks().savedSongIds.toSet()
        assertTrue("the album's songs are saved", songs.all { it.id in saved })
        assertTrue("a saved song plays from its file", host.core.streamFor(songs[0].id, host.network()).local)
        rule.waitUntilAtLeastOneExists(hasContentDescription("Downloaded. Remove download"), TIMEOUT)
        shot("12-album-downloaded")

        // Library's Downloaded chip lists it.
        Espresso.pressBack()
        rule.onNodeWithText("Downloaded").performClick()
        rule.waitUntilAtLeastOneExists(hasText(album.title), TIMEOUT)
        shot("13-library-downloaded")

        // Settings › Downloads, in portrait and landscape.
        rule.onNodeWithContentDescription("Profiles and settings").performClick()
        rule.waitUntilAtLeastOneExists(hasText("Settings"), TIMEOUT)
        rule.onNodeWithText("Settings").performClick()
        rule.waitUntilAtLeastOneExists(hasText("Downloads"), TIMEOUT)
        rule.onNodeWithText("Downloads").performClick()
        rule.waitUntilAtLeastOneExists(hasText("Using", substring = true), TIMEOUT)
        shot("14-settings-downloads")
        automation.setRotation(UiAutomation.ROTATION_FREEZE_90)
        rule.waitForIdle()
        shot("15-settings-downloads-landscape")
        automation.setRotation(UiAutomation.ROTATION_FREEZE_0)
        automation.setRotation(UiAutomation.ROTATION_UNFREEZE)
        rule.waitForIdle()
        Espresso.pressBack()
        Espresso.pressBack()

        try {
            // No network: the banner, the dimming, and the downloaded album still plays.
            shell("svc wifi disable")
            shell("svc data disable")
            step("the app says it's offline (board 20: after 2 s)", TIMEOUT) { host.connectivity.offline.value }
            rule.waitUntilAtLeastOneExists(hasText("Offline · Showing downloaded music"), TIMEOUT)
            step("the core knows", TIMEOUT) { !host.core.downloadMarks().online }
            shot("16-offline-library")

            openAlbum(album.title)
            rule.onNodeWithText("Play").performScrollTo().performClick()
            step("the downloaded album plays with no network", TIMEOUT) {
                host.core.queue().current?.song?.id in saved && runBlocking { PlaybackHub.state.first().isPlaying }
            }
            shot("17-offline-playing")

            // A song that isn't downloaded is refused with the reason, and dims.
            val other = host.core.albums(AlbumSort.TITLE, 0u, 200u)
                .firstNotNullOfOrNull { a -> host.core.album(a.id).songs.firstOrNull { it.id !in saved } }
            if (other != null) {
                val refused = runCatching { host.core.play(listOf(other.id), 0u, StartedFrom.ALBUM, null) }.exceptionOrNull()
                assertTrue("a song that isn't downloaded is refused offline: $refused", refused is EncoreException.NotAllowed)
            }
            Espresso.pressBack()
        } finally {
            shell("svc wifi enable")
            shell("svc data enable")
        }
        step("back online", TIMEOUT) { !host.connectivity.offline.value }

        // Android Auto's side: the library session, browsed and played like a car does.
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        val browser = onMain { MediaBrowser.Builder(context, token).buildAsync() }.get(TIMEOUT, TimeUnit.MILLISECONDS)
        try {
            val root = onMain { browser.getLibraryRoot(null) }.get(TIMEOUT, TimeUnit.MILLISECONDS).value!!
            val tabs = onMain { browser.getChildren(root.mediaId, 0, 10, null) }.get(TIMEOUT, TimeUnit.MILLISECONDS).value!!
            assertEquals("Home, Library and Downloads", listOf("Home", "Library", "Downloads"), tabs.map { it.mediaMetadata.title.toString() })
            val inAlbum = onMain { browser.getChildren("album:${album.id}", 0, 100, null) }.get(TIMEOUT, TimeUnit.MILLISECONDS).value!!
            assertEquals("the album's songs", songs.size, inAlbum.size)
            val downloads = onMain { browser.getChildren("downloads", 0, 100, null) }.get(TIMEOUT, TimeUnit.MILLISECONDS).value!!
            assertTrue("Downloads lists the album", downloads.any { it.mediaId == "album:${album.id}" })
            // A tap in the car on the second song plays it with its album.
            val chosen: MediaItem = inAlbum[1]
            onMain {
                browser.setMediaItem(chosen)
                browser.prepare()
                browser.play()
            }
            step("the car's choice plays through the core", TIMEOUT) { host.core.queue().current?.song?.id == songs[1].id }
            assertEquals("its album follows it", songs.size - 2, host.core.queue().upNext.size)
        } finally {
            onMain { browser.release() }
        }
    }

    private fun openAlbum(title: String) {
        rule.waitUntilAtLeastOneExists(hasText(title), TIMEOUT)
        rule.onAllNodesWithText(title).onFirst().performClick()
        rule.waitUntilAtLeastOneExists(hasText("Play"), TIMEOUT)
    }

    private fun shell(command: String) {
        val pfd = automation.executeShellCommand(command)
        android.os.ParcelFileDescriptor.AutoCloseInputStream(pfd).use { it.readBytes() }
    }

    private fun <T> onMain(block: () -> T): T {
        var out: T? = null
        InstrumentationRegistry.getInstrumentation().runOnMainSync { out = block() }
        @Suppress("UNCHECKED_CAST")
        return out as T
    }

    /** A screenshot for the review, taken by the shell (device-check.sh pulls them). */
    private fun shot(name: String) {
        rule.mainClock.advanceTimeBy(SETTLE_MS)
        rule.waitForIdle()
        Thread.sleep(400)
        shell("screencap -p /data/local/tmp/encore-shot-$name.png")
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
        const val DOWNLOAD_TIMEOUT = 90_000L
        const val SETTLE_MS = 1_500L
    }
}

