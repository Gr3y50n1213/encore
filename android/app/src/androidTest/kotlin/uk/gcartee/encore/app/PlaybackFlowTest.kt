package uk.gcartee.encore.app

import android.content.ComponentName
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.printToString
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.CorePlayback
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.playback.PlaybackService
import uk.gcartee.encore.app.playback.SleepTimerNotifier
import uk.gcartee.encore.core.AlbumSort
import uk.gcartee.encore.core.SleepTimerMode
import uk.gcartee.encore.core.SleepTimerSpec
import uk.gcartee.encore.core.StartedFrom
import uk.gcartee.encore.core.StreamQuality

/**
 * A song that ends by itself is followed by the next one, never the one after it, and the
 * song after the current one is loaded from the start (so Next is instant). On 8 November
 * 2026 the owner's tablet skipped a song at every natural end; nothing tested a song
 * playing to its end before. Runs after OfflineAndCarTest, signed in to the Jellyfin
 * replay, whose songs are 30 s of generated audio (tools/ci/replay_jellyfin.py).
 */
@RunWith(AndroidJUnit4::class)
class PlaybackFlowTest {
    @get:Rule
    val rule = createAndroidComposeRule<MainActivity>()

    private val replayUrl: String? = InstrumentationRegistry.getArguments().getString("replayUrl")

    @Test
    fun aSongThatEndsPlaysTheNextOneAndTheOneAfterIsAlreadyLoaded() {
        assumeTrue("needs the Jellyfin replay (replayUrl)", replayUrl != null)
        val host = CoreHost.get(rule.activity)
        step("the profile from the sign-in test is open", TIMEOUT) { host.profileOpen.value }

        // An album that streams (not the one the offline test downloaded), three songs or more.
        val saved = host.core.downloadMarks().savedSongIds.toSet()
        val albums = host.core.albums(AlbumSort.TITLE, 0u, 200u)
        val album = albums.firstOrNull { a -> a.songCount >= 3u && host.core.album(a.id).songs.none { it.id in saved } }
            ?: albums.first { it.songCount >= 3u }
        val songs = host.core.album(album.id).songs
        // The file as it is first (the emulator may count as Wi-Fi or as mobile data).
        val network = host.network()
        val quality = host.core.streamQuality(network)
        host.core.setStreamQuality(network, StreamQuality.ORIGINAL)
        host.core.play(songs.map { it.id }, 0u, StartedFrom.ALBUM, null)
        rule.runOnUiThread { PlaybackHub.playCurrent(rule.activity) }
        step("the first song plays", TIMEOUT) { host.core.queue().current?.song?.id == songs[0].id && playing() }

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        val controller = onMain { MediaController.Builder(context, token).buildAsync() }.get(TIMEOUT, TimeUnit.MILLISECONDS)
        try {
            step("the second song is loaded behind the first", TIMEOUT) {
                val next = host.core.queue().upNext.firstOrNull() ?: return@step false
                onMain { controller.mediaItemCount == 2 && controller.getMediaItemAt(1).mediaId == "${CorePlayback.UID_PREFIX}${next.uid}" }
            }

            // Let the first song end by itself, a few seconds from now.
            step("the first song's length is known", TIMEOUT) { onMain { controller.duration } > 0 }
            val length = onMain { controller.duration }
            onMain { controller.seekTo((length - END_IN_MS).coerceAtLeast(0)) }
            step("the second song plays after the first ends", END_TIMEOUT) {
                host.core.queue().current?.song?.id == songs[1].id && playing()
            }
            Thread.sleep(SETTLE_MS)
            val queue = host.core.queue()
            assertEquals("still the second song: the queue didn't move on twice", songs[1].id, queue.current?.song?.id)
            assertEquals("the first song is the last one in History", songs[0].id, queue.history.last().song.id)
            assertFalse("the second song isn't in History", queue.history.any { it.song.id == songs[1].id })
            assertEquals("the player never said a song ended when it hadn't", 0, onMain { PlaybackHub.isConnected.value?.falseEnds ?: -1 })

            // The third song is already loaded, and Next goes to it.
            step("the third song is loaded behind the second", TIMEOUT) {
                val next = host.core.queue().upNext.firstOrNull() ?: return@step false
                onMain { controller.mediaItemCount == 2 && controller.getMediaItemAt(1).mediaId == "${CorePlayback.UID_PREFIX}${next.uid}" }
            }
            onMain { controller.seekToNext() }
            step("Next plays the third song", TIMEOUT) { host.core.queue().current?.song?.id == songs[2].id && playing() }

            // A capped quality: the server transcodes, and sends it with no length (the
            // replay does it the way Jellyfin does), so the player can't seek it at first.
            // A seek lands once the song is whole in the rolling cache, and the song after
            // it is opened from there before it plays: seekable, its length known.
            host.core.setStreamQuality(network, StreamQuality.HIGH)
            host.core.play(songs.map { it.id }, 0u, StartedFrom.ALBUM, null)
            rule.runOnUiThread { PlaybackHub.playCurrent(rule.activity) }
            step("the first song plays, transcoded", TIMEOUT) { host.core.queue().current?.song?.id == songs[0].id && playing() }
            // Now Playing's bar seeks through the app, as here (the session offers no seek for
            // a stream ExoPlayer can't seek).
            onMain { PlaybackHub.seekTo(rule.activity, SEEK_TO_MS) }
            step("the seek lands once the transcoded song is whole", TIMEOUT) {
                onMain { controller.isCurrentMediaItemSeekable && controller.currentPosition >= SEEK_TO_MS - SEEK_SLACK_MS }
            }
            assertEquals("still the first song after the seek", songs[0].id, host.core.queue().current?.song?.id)
            Thread.sleep(SETTLE_MS) // the second song arrives whole and is opened from the cache
            onMain { controller.seekToNext() }
            step("the second song is seekable, its length known, as it starts", TIMEOUT) {
                host.core.queue().current?.song?.id == songs[1].id &&
                    onMain { controller.isCurrentMediaItemSeekable && controller.duration > 0 }
            }
            assertEquals("no false ends on the transcoded songs either", 0, onMain { PlaybackHub.isConnected.value?.falseEnds ?: -1 })
            onMain { controller.pause() }
        } finally {
            host.core.setStreamQuality(network, quality)
            onMain { controller.release() }
        }
    }

    /**
     * Phase 9: a 15 s sleep timer (the core's testing-only call). The volume ramps down,
     * playback pauses, the volume is back, the play is logged as ended by the system (never
     * a skip), the card is gone, and Play afterwards starts a new play.
     */
    @Test
    fun theSleepTimerFadesOutPausesAndLogsTheEndAsEndedBySystem() {
        assumeTrue("needs the Jellyfin replay (replayUrl)", replayUrl != null)
        val host = CoreHost.get(rule.activity)
        step("the profile from the sign-in test is open", TIMEOUT) { host.profileOpen.value }
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            // So the card can be read (the app asks with its own primer, WP5).
            shell("pm grant ${rule.activity.packageName} android.permission.POST_NOTIFICATIONS")
        }
        val nm = context.getSystemService(android.app.NotificationManager::class.java)
        val songs = albumSongs(host)
        val network = host.network()
        val quality = host.core.streamQuality(network)
        host.core.setStreamQuality(network, StreamQuality.ORIGINAL)
        try {
            host.core.play(songs.map { it.id }, 0u, StartedFrom.ALBUM, null)
            rule.runOnUiThread { PlaybackHub.playCurrent(rule.activity) }
            step("the first song plays", TIMEOUT) { host.core.queue().current?.song?.id == songs[0].id && playing() }
            // From the start: the 30 s song outlasts the 15 s timer, so it fades by the clock.
            onMain { PlaybackHub.seekTo(rule.activity, 0) }
            assertEquals("full volume before", 1f, volume(), 0.001f)
            val setAt = android.os.SystemClock.elapsedRealtime()
            host.core.setSleepTimerSeconds(SLEEP_SECONDS.toUInt())
            if (nm.areNotificationsEnabled()) {
                step("the sleep timer card shows", TIMEOUT) { nm.activeNotifications.any { it.id == SleepTimerNotifier.ID } }
            }
            step("the volume ramps down", SLEEP_TIMEOUT) { volume() < FADING_BELOW }
            assertTrue("still playing while it fades", playing())
            step("playback pauses at the end", SLEEP_TIMEOUT) { !playing() }
            val took = android.os.SystemClock.elapsedRealtime() - setAt
            assertTrue("paused about 15 s after it was set (took $took ms)", took < SLEEP_TIMEOUT)
            step("the volume is back to full", TIMEOUT) { volume() == 1f }
            step("the timer is cleared", TIMEOUT) { host.core.sleepTimer() == null }
            step("the card is gone", TIMEOUT) { nm.activeNotifications.none { it.id == SleepTimerNotifier.ID } }
            assertEquals("the queue stays where it was", songs[0].id, host.core.queue().current?.song?.id)

            val log = plays(host)
            val last = log.getJSONObject(log.length() - 1)
            assertEquals("the play ended by the system", "endedBySystem", last.optString("end_reason"))
            assertEquals("its signal is system, never a skip", "system", last.optString("signal"))

            // Play again: a new play (playback_started), and the ended one stays as it was.
            rule.runOnUiThread { PlaybackHub.togglePlay(rule.activity) }
            step("it plays again", TIMEOUT) { playing() }
            Thread.sleep(SETTLE_MS)
            val after = plays(host)
            assertEquals("Play started a new play", log.length() + 1, after.length())
            val ended = after.getJSONObject(after.length() - 2)
            assertEquals("still ended by the system after Play", "endedBySystem", ended.optString("end_reason"))
            assertTrue("the new play is open", after.getJSONObject(after.length() - 1).isNull("end_reason"))
            rule.runOnUiThread { PlaybackHub.togglePlay(rule.activity) }
            step("paused again", TIMEOUT) { !playing() }
        } finally {
            runCatching { host.core.cancelSleepTimer() }
            host.core.setStreamQuality(network, quality)
        }
    }

    /**
     * Phase 9: End of song plays the song to its end with no fade, and pauses there with the
     * next song loaded and paused; nothing was preloaded behind it. The song is logged as
     * finished. The replay's songs are 30 s, so this seeks near the end.
     */
    @Test
    fun endOfSongPausesAtTheSongsEndWithTheNextSongLoaded() {
        assumeTrue("needs the Jellyfin replay (replayUrl)", replayUrl != null)
        val host = CoreHost.get(rule.activity)
        step("the profile from the sign-in test is open", TIMEOUT) { host.profileOpen.value }
        val songs = albumSongs(host)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        // The file as it is, so its length is known and the seek lands at once.
        val network = host.network()
        val quality = host.core.streamQuality(network)
        host.core.setStreamQuality(network, StreamQuality.ORIGINAL)
        host.core.play(songs.map { it.id }, 0u, StartedFrom.ALBUM, null)
        rule.runOnUiThread { PlaybackHub.playCurrent(rule.activity) }
        step("the first song plays", TIMEOUT) { host.core.queue().current?.song?.id == songs[0].id && playing() }
        val controller = onMain { MediaController.Builder(context, token).buildAsync() }.get(TIMEOUT, TimeUnit.MILLISECONDS)
        try {
            val timer = host.core.setSleepTimer(SleepTimerSpec(SleepTimerMode.END_OF_SONG, 0u))
            assertEquals("End of song stops after the current entry", host.core.queue().current?.uid, timer.stopAfterUid)
            step("nothing is preloaded behind the last song", TIMEOUT) { onMain { controller.mediaItemCount } == 1 }
            step("the first song's length is known", TIMEOUT) { onMain { controller.duration } > 0 }
            val length = onMain { controller.duration }
            onMain { controller.seekTo((length - END_IN_MS).coerceAtLeast(0)) }
            step("paused on the second song, loaded", END_TIMEOUT) {
                val current = host.core.queue().current
                current?.song?.id == songs[1].id && !playing() &&
                    onMain { controller.currentMediaItem?.mediaId == "${CorePlayback.UID_PREFIX}${current?.uid}" && !controller.playWhenReady }
            }
            Thread.sleep(SETTLE_MS)
            assertFalse("still paused", playing())
            assertEquals("still the second song", songs[1].id, host.core.queue().current?.song?.id)
            assertEquals("the first song is the last one in History", songs[0].id, host.core.queue().history.last().song.id)
            assertNull("the timer ended", host.core.sleepTimer())
            assertEquals("no fade: full volume", 1f, volume(), 0.001f)
            val log = plays(host)
            val last = log.getJSONObject(log.length() - 1)
            assertEquals("the last play is the first song", songs[0].title, last.optString("title"))
            assertEquals("it finished", "finished", last.optString("end_reason"))
        } finally {
            runCatching { host.core.cancelSleepTimer() }
            host.core.setStreamQuality(network, quality)
            onMain { controller.release() }
        }
    }

    /** An album that streams, three songs or more (as the test above picks). */
    private fun albumSongs(host: CoreHost): List<uk.gcartee.encore.core.Song> {
        val saved = host.core.downloadMarks().savedSongIds.toSet()
        val albums = host.core.albums(AlbumSort.TITLE, 0u, 200u)
        val album = albums.firstOrNull { a -> a.songCount >= 3u && host.core.album(a.id).songs.none { it.id in saved } }
            ?: albums.first { it.songCount >= 3u }
        return host.core.album(album.id).songs
    }

    /** The player's own volume (the fade ramps it, never the reported volume). */
    private fun volume(): Float = onMain { PlaybackHub.isConnected.value?.playerVolume ?: -1f }

    /** The profile's play log, oldest first, from its export (docs/core/EXPORT-FORMAT.md). */
    private fun plays(host: CoreHost): org.json.JSONArray {
        val file = java.io.File(rule.activity.cacheDir, "sleep-timer-test-export.json")
        try {
            host.core.exportProfile(host.core.currentProfile()!!, file.absolutePath)
            return org.json.JSONObject(file.readText()).getJSONArray("plays")
        } finally {
            file.delete()
        }
    }

    private fun shell(command: String) {
        val pfd = InstrumentationRegistry.getInstrumentation().uiAutomation.executeShellCommand(command)
        android.os.ParcelFileDescriptor.AutoCloseInputStream(pfd).use { it.readBytes() }
    }

    private fun playing(): Boolean = runBlocking { PlaybackHub.state.first().isPlaying }

    private fun <T> onMain(block: () -> T): T {
        var out: T? = null
        InstrumentationRegistry.getInstrumentation().runOnMainSync { out = block() }
        @Suppress("UNCHECKED_CAST")
        return out as T
    }

    private fun step(what: String, timeoutMs: Long, done: () -> Boolean) {
        try {
            rule.waitUntil(timeoutMs) { runCatching(done).getOrDefault(false) }
        } catch (e: androidx.compose.ui.test.ComposeTimeoutException) {
            val screen = runCatching { rule.onRoot().printToString(maxDepth = 20) }.getOrDefault("?")
            throw AssertionError("Not reached in ${timeoutMs / 1000} s: $what.\n$screen", e)
        }
    }

    private companion object {
        const val TIMEOUT = 20_000L
        const val END_TIMEOUT = 30_000L
        const val END_IN_MS = 4_000L
        const val SETTLE_MS = 4_000L
        // Mid-song: the next song is swapped for its cached copy only up to 8 s from the end.
        const val SEEK_TO_MS = 15_000L
        const val SEEK_SLACK_MS = 1_500L
        // Phase 9: a 15 s sleep timer pauses within about 20 s.
        const val SLEEP_SECONDS = 15
        const val SLEEP_TIMEOUT = 22_000L
        const val FADING_BELOW = 0.95f
    }
}
