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
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.CorePlayback
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.playback.PlaybackService
import uk.gcartee.encore.core.AlbumSort
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
            onMain { controller.seekTo(SEEK_TO_MS) }
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
        const val SEEK_TO_MS = 20_000L
        const val SEEK_SLACK_MS = 1_500L
    }
}
