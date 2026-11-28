package uk.gcartee.encore.app

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.pebble.Pebble
import uk.gcartee.encore.app.pebble.PebbleCommand
import uk.gcartee.encore.app.pebble.PebbleDictionary
import uk.gcartee.encore.app.pebble.PebbleKeys
import uk.gcartee.encore.app.pebble.PebbleNow
import uk.gcartee.encore.app.pebble.PebbleTuple
import uk.gcartee.encore.wearlink.NowPlaying
import uk.gcartee.encore.wearlink.WearCommand

/**
 * Stage W2: the Pebble link's PebbleKit `msg_data` JSON, the privacy rule, and what each
 * button on the watch does (pebble/PebbleProtocol.kt).
 */
class PebbleProtocolTest {
    private val song = NowPlaying(
        title = "Slow Burn Season",
        artist = "Marigold Ave",
        album = "Golden Hour Static",
        line = "we stay till the summer is gone",
        isPlaying = true,
        positionMs = 84_000,
        positionAtUtcMs = 1_794_470_400_000,
        durationMs = 222_000,
        liked = true,
        upNext = "Rainy Focus",
        isPrivate = true,
    )

    @Test
    fun writesPebbleKitsFormat() {
        val json = PebbleDictionary.toJson(
            listOf(PebbleTuple.Text(PebbleKeys.TITLE, "Slow Burn"), PebbleTuple.flag(PebbleKeys.PLAYING, true)),
        )
        // PebbleDictionary.toJsonString: key, type, length (0 for a string) and value.
        assertEquals(
            """[{"key":1,"type":"string","length":0,"value":"Slow Burn"},{"key":4,"type":"uint","length":1,"value":1}]""",
            json,
        )
    }

    @Test
    fun textIsEscapedAndRoundTrips() {
        val text = "Say \"hi\"\\ now\n\ttab \u0001 café   🎵"
        val tuples = listOf(
            PebbleTuple.Text(PebbleKeys.LYRIC, text),
            PebbleTuple.Number(PebbleKeys.CMD, -3, signed = true, width = 4),
            PebbleTuple.uint8(PebbleKeys.LIKED, 255),
        )
        val json = PebbleDictionary.toJson(tuples)
        assertFalse("no raw control characters", json.any { it < ' ' })
        assertEquals(tuples, PebbleDictionary.fromJson(json))
    }

    @Test
    fun readsWhatThePebbleAppSends() {
        // A button press as Core's app broadcasts it (PebbleClassicDictionary.toJsonString).
        val json = """[ {"key": 7, "type": "uint", "length": 1, "value": 3} ]"""
        val tuples = PebbleDictionary.fromJson(json)!!
        assertEquals(listOf(PebbleTuple.Number(7, 3, signed = false, width = 1)), tuples)
        assertEquals(PebbleCommand.PLAY_PAUSE, PebbleCommand.of(tuples))
    }

    @Test
    fun refusesWhatIsntADictionary() {
        for (bad in listOf(
            null,
            "",
            "{}",
            "[1]",
            "not json",
            """[{"type":"uint","length":1,"value":3}]""",
            """[{"key":7,"type":"uint","length":3,"value":3}]""",
            """[{"key":7,"type":"uint","length":1,"value":"3"}]""",
            """[{"key":1,"type":"string","length":0,"value":4}]""",
            """[{"key":-1,"type":"uint","length":1,"value":3}]""",
            "[" + "{\"key\":7,\"type\":\"uint\",\"length\":1,\"value\":3},".repeat(200) + "]",
        )) {
            assertNull(bad?.take(40), PebbleDictionary.fromJson(bad))
        }
        // Byte arrays aren't used: skipped, not refused.
        assertEquals(emptyList<PebbleTuple>(), PebbleDictionary.fromJson("""[{"key":9,"type":"bytes","length":0,"value":"AAE="}]"""))
    }

    @Test
    fun commandsMapToTheWearWatchs() {
        assertNull(PebbleCommand.HELLO.toWear(isPlaying = true, isLiked = false))
        assertEquals(WearCommand.Previous, PebbleCommand.PREVIOUS.toWear(isPlaying = true, isLiked = false))
        assertEquals(WearCommand.Next, PebbleCommand.NEXT.toWear(isPlaying = false, isLiked = false))
        assertEquals(WearCommand.Pause, PebbleCommand.PLAY_PAUSE.toWear(isPlaying = true, isLiked = false))
        assertEquals(WearCommand.Play, PebbleCommand.PLAY_PAUSE.toWear(isPlaying = false, isLiked = false))
        assertEquals(WearCommand.Like, PebbleCommand.LIKE.toWear(isPlaying = true, isLiked = false))
        assertEquals(WearCommand.Unlike, PebbleCommand.LIKE.toWear(isPlaying = true, isLiked = true))
        assertEquals(PebbleCommand.LIKE, PebbleCommand.of(listOf(PebbleTuple.uint8(PebbleKeys.CMD, 5))))
        assertNull(PebbleCommand.of(listOf(PebbleTuple.uint8(PebbleKeys.CMD, 9))))
        assertNull(PebbleCommand.of(listOf(PebbleTuple.Text(PebbleKeys.CMD, "3"))))
        assertNull(PebbleCommand.of(listOf(PebbleTuple.uint8(PebbleKeys.TITLE, 3))))
    }

    @Test
    fun aPlayingSongIsShown() {
        val view = PebbleNow.view(song)
        assertEquals(PebbleNow.View(song.title, song.artist, song.line, playing = true, liked = true, isPrivate = false), view)
        val keys = view.tuples().map { it.key }
        assertEquals(listOf(1, 2, 3, 4, 5, 6), keys)
    }

    @Test
    fun aStoppedSongStaysPrivateUnlessHistoryMayShow() {
        val stopped = song.copy(isPlaying = false)
        // "Hide listening history while locked" on: no title leaves the phone.
        assertEquals(PebbleNow.View("", "", "", playing = false, liked = false, isPrivate = true), PebbleNow.view(stopped))
        // Off: the paused song shows.
        assertEquals(stopped.title, PebbleNow.view(stopped.copy(isPrivate = false)).title)
        // Nothing playing or queued: the empty state, not private.
        assertEquals(PebbleNow.View("", "", "", playing = false, liked = false, isPrivate = false), PebbleNow.view(NowPlaying.EMPTY))
    }

    @Test
    fun textIsCutToTheWatchsBuffersInUtf8() {
        assertEquals("abc", PebbleNow.utf8Take("abc", 3))
        assertEquals("ab", PebbleNow.utf8Take("abc", 2))
        // é is two bytes, a note four: never cut inside one.
        assertEquals("caf", PebbleNow.utf8Take("café", 4))
        assertEquals("café", PebbleNow.utf8Take("café", 5))
        assertEquals("a", PebbleNow.utf8Take("a🎵", 4))
        val long = PebbleNow.view(song.copy(title = "é".repeat(80), line = "x".repeat(300)))
        assertEquals(PebbleNow.MAX_TITLE_BYTES, long.title.toByteArray(Charsets.UTF_8).size)
        assertEquals(PebbleNow.MAX_LYRIC_BYTES, long.lyric.toByteArray(Charsets.UTF_8).size)
    }

    @Test
    fun aFullMessageFitsTheWatchsInbox() {
        val longest = PebbleNow.View(
            title = "é".repeat(50),
            artist = "é".repeat(50),
            lyric = "é".repeat(80),
            playing = true,
            liked = true,
            isPrivate = false,
        )
        // An AppMessage dictionary: a count byte, then 7 bytes a tuple and its value; each
        // string with its terminating zero. pebble/src/c/encore.c opens a 512-byte inbox.
        val size = 1 + longest.tuples().sumOf { t ->
            7 + when (t) {
                is PebbleTuple.Text -> t.value.toByteArray(Charsets.UTF_8).size + 1
                is PebbleTuple.Number -> t.width
            }
        }
        assertTrue("$size bytes", size <= 512)
    }

    @Test
    fun theUuidAndKeysMatchTheWatchapp() {
        val pkg = File("../../pebble/package.json").readText()
        assertTrue(pkg.contains("\"uuid\": \"${Pebble.APP_UUID}\""))
        for ((name, key) in listOf(
            "title" to PebbleKeys.TITLE,
            "artist" to PebbleKeys.ARTIST,
            "lyric" to PebbleKeys.LYRIC,
            "playing" to PebbleKeys.PLAYING,
            "liked" to PebbleKeys.LIKED,
            "private" to PebbleKeys.PRIVATE,
            "cmd" to PebbleKeys.CMD,
        )) {
            assertTrue("$name is $key", pkg.contains("\"$name\": $key"))
        }
        val c = File("../../pebble/src/c/encore.c").readText()
        for (command in PebbleCommand.entries) {
            assertTrue("$command is ${command.wire}", c.contains("CMD_$command = ${command.wire},"))
        }
        assertTrue(c.contains("#define TITLE_SIZE ${PebbleNow.MAX_TITLE_BYTES + 1}"))
        assertTrue(c.contains("#define ARTIST_SIZE ${PebbleNow.MAX_ARTIST_BYTES + 1}"))
        assertTrue(c.contains("#define LYRIC_SIZE ${PebbleNow.MAX_LYRIC_BYTES + 1}"))
        assertEquals(0x34, Pebble.transactionId(0x1234))
    }
}
