package uk.gcartee.encore.wearlink

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Stage W1: the phone-watch protocol (wearlink/src), shared by both apps. */
class WearProtocolTest {
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
        isPrivate = false,
    )

    @Test
    fun nowPlayingRoundTrips() {
        val map = NowCodec.encode(song)
        assertEquals(NowCodec.VERSION, map["v"])
        assertEquals(song, NowCodec.decode(MapValues(map)))
        assertEquals(NowPlaying.EMPTY, NowCodec.decode(MapValues(NowCodec.encode(NowPlaying.EMPTY))))
    }

    @Test
    fun nowPlayingUsesOnlyFlatValues() {
        // DataMap and SharedPreferences take these three; WearGms and WatchStore copy nothing else.
        assertTrue(NowCodec.encode(song).values.all { it is String || it is Long || it is Boolean })
        assertTrue(MixesCodec.encode(Mixes(listOf(MixChip("a", "A", 1, 2)), true)).values.all { it is String || it is Long || it is Boolean })
    }

    @Test
    fun anotherVersionIsNotRead() {
        val map = NowCodec.encode(song).toMutableMap()
        map["v"] = 2L
        assertNull(NowCodec.decode(MapValues(map)))
        map.remove("v")
        assertNull(NowCodec.decode(MapValues(map)))
    }

    @Test
    fun missingPrivacyIsPrivate() {
        val map = NowCodec.encode(song).toMutableMap()
        map.remove("private")
        assertTrue(NowCodec.decode(MapValues(map))!!.isPrivate)
    }

    @Test
    fun longTextIsCut() {
        val long = "x".repeat(1_000)
        val map = NowCodec.encode(song.copy(title = long, line = long))
        assertEquals(NowCodec.MAX_TEXT, (map["title"] as String).length)
        assertEquals(NowCodec.MAX_TEXT, (map["line"] as String).length)
    }

    @Test
    fun positionMovesOnOnlyWhilePlaying() {
        val at = song.positionAtUtcMs
        assertEquals(94_000, song.positionAt(at + 10_000))
        assertEquals(84_000, song.copy(isPlaying = false).positionAt(at + 10_000))
        // Kept inside the song, and never before its start (a watch clock a little behind).
        assertEquals(222_000, song.positionAt(at + 1_000_000))
        assertEquals(84_000, song.positionAt(at - 5_000))
        assertEquals(0.5f, song.copy(positionMs = 111_000).progressAt(at), 0.0001f)
        assertEquals(0f, song.copy(durationMs = 0).progressAt(at), 0f)
    }

    @Test
    fun onlyRealChangesAreSent() {
        assertTrue(song.needsSendAfter(null))
        // The position moving on as predicted: nothing to send.
        val later = song.copy(positionMs = 94_000, positionAtUtcMs = song.positionAtUtcMs + 10_000)
        assertFalse(later.needsSendAfter(song))
        // A seek: the position is off what the last update predicts.
        assertTrue(later.copy(positionMs = 30_000).needsSendAfter(song))
        // Anything else changing.
        assertTrue(later.copy(line = "next line").needsSendAfter(song))
        assertTrue(later.copy(liked = false).needsSendAfter(song))
        assertTrue(later.copy(isPlaying = false, positionMs = 94_000).needsSendAfter(song))
    }

    @Test
    fun mixesRoundTripUpToFour() {
        val five = (1..5).map { MixChip("mix-$it", "Mix $it", 0xFF3B82C4.toInt(), 0xFFFFFFFF.toInt()) }
        val map = MixesCodec.encode(Mixes(five, likedSongs = true))
        assertEquals(4L, map["n"])
        val back = MixesCodec.decode(MapValues(map))!!
        assertEquals(five.take(4), back.mixes)
        assertTrue(back.likedSongs)
        assertEquals(Mixes.EMPTY, MixesCodec.decode(MapValues(MixesCodec.encode(Mixes.EMPTY))))
    }

    @Test
    fun unsafeMixIdsAreDropped() {
        val map = MixesCodec.encode(Mixes(listOf(MixChip("../x", "Bad", 0, 0), MixChip("ok", "Good", 0, 0)), false))
        assertEquals(listOf("ok"), MixesCodec.decode(MapValues(map))!!.mixes.map { it.id })
        val forged = MixesCodec.encode(Mixes(listOf(MixChip("ok", "Good", 0, 0)), false)).toMutableMap()
        forged["id0"] = "mix id with spaces"
        assertTrue(MixesCodec.decode(MapValues(forged))!!.mixes.isEmpty())
    }

    @Test
    fun tileChipsArePinnedMixesThenLiked() {
        val mix = { i: Int -> MixChip("m$i", "M$i", 0, 0) }
        assertEquals(
            listOf(TileChip.Mix(mix(1)), TileChip.Mix(mix(2)), TileChip.Liked),
            Mixes(listOf(mix(1), mix(2)), likedSongs = true).chips(),
        )
        assertEquals(4, Mixes((1..4).map(mix), likedSongs = true).chips().size)
        assertTrue(Mixes((1..4).map(mix), likedSongs = true).chips().none { it == TileChip.Liked })
        assertEquals(listOf(TileChip.Liked), Mixes(emptyList(), likedSongs = true).chips())
        assertTrue(Mixes.EMPTY.chips().isEmpty())
        assertEquals(WearCommand.PlayMix("m1"), TileChip.Mix(mix(1)).command)
        assertEquals(WearCommand.PlayLiked, TileChip.Liked.command)
    }

    @Test
    fun commandsRoundTrip() {
        val all = listOf(
            WearCommand.Play, WearCommand.Pause, WearCommand.Next, WearCommand.Previous,
            WearCommand.Like, WearCommand.Unlike, WearCommand.Volume(up = true), WearCommand.Volume(up = false),
            WearCommand.PlayMix("mix-20403-strongest"), WearCommand.PlayLiked,
        )
        for (c in all) assertEquals(c, WearCommand.parse(c.toBytes()))
        assertEquals(
            listOf("play", "pause", "next", "previous", "like", "unlike", "volume:+1", "volume:-1", "mix:mix-20403-strongest", "liked"),
            all.map { it.wire },
        )
    }

    @Test
    fun anythingElseIsNotACommand() {
        for (bad in listOf("", "PLAY", "play ", "volume:+2", "volume:", "mix:", "mix:a/b", "mix:../x", "seek:100", "liked:1")) {
            assertNull(bad, WearCommand.parse(bad))
        }
        assertNull(WearCommand.parse(null as ByteArray?))
        assertNull(WearCommand.parse(ByteArray(0)))
        assertNull(WearCommand.parse(("mix:" + "a".repeat(300)).toByteArray()))
        assertNull(WearCommand.parse(byteArrayOf(0xC3.toByte(), 0x28)))
    }

    @Test
    fun lyricLineFollowsThePosition() {
        val starts = listOf(1_000L, null, 5_000L, 9_000L)
        assertEquals(-1, LyricTiming.indexAt(starts, 0))
        assertEquals(0, LyricTiming.indexAt(starts, 1_000))
        // A line lights 150 ms early, as on the phone.
        assertEquals(2, LyricTiming.indexAt(starts, 4_850))
        assertEquals(0, LyricTiming.indexAt(starts, 4_849))
        assertEquals(3, LyricTiming.indexAt(starts, 60_000))
        assertEquals(3_850L, LyricTiming.msUntilNext(starts, 1_000))
        assertEquals(850L, LyricTiming.msUntilNext(starts, 0))
        assertNull(LyricTiming.msUntilNext(starts, 9_000))
    }

    @Test
    fun lockedWatchShowsNoSong() {
        assertTrue(WatchPrivacy.showSong(watchLocked = false, now = song))
        assertFalse(WatchPrivacy.showSong(watchLocked = true, now = song))
        assertFalse(WatchPrivacy.showSong(watchLocked = false, now = NowPlaying.EMPTY))
        assertFalse(WatchPrivacy.showSong(watchLocked = false, now = null))
        assertFalse(WatchPrivacy.showMixes(watchLocked = true))
        assertTrue(WatchPrivacy.showMixes(watchLocked = false))
    }

    @Test
    fun watchFaceHidesTheLastSongWhenPrivate() {
        val stopped = song.copy(isPlaying = false)
        assertTrue(WatchPrivacy.showSong(false, song.copy(isPrivate = true), onWatchFace = true))
        assertFalse(WatchPrivacy.showSong(false, stopped.copy(isPrivate = true), onWatchFace = true))
        assertTrue(WatchPrivacy.showSong(false, stopped.copy(isPrivate = false), onWatchFace = true))
        // The app itself, opened on an unlocked watch, still shows it.
        assertTrue(WatchPrivacy.showSong(false, stopped.copy(isPrivate = true)))
        assertFalse(WatchPrivacy.showSong(true, song, onWatchFace = true))
    }
}
