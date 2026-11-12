package uk.gcartee.encore.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.widgets.CardKind
import uk.gcartee.encore.app.widgets.MainButton
import uk.gcartee.encore.app.widgets.NowPlayingStates
import uk.gcartee.encore.app.widgets.NowPlayingView
import uk.gcartee.encore.app.widgets.SnapshotPrivacy
import uk.gcartee.encore.app.widgets.SnapshotRead
import uk.gcartee.encore.app.widgets.SnapshotReader
import uk.gcartee.encore.app.widgets.WidgetTarget

/**
 * Phase 9 (WP6): the snapshot file's parsing and the Now Playing widget's states (board 34;
 * CORE-API "Widgets"). Made-up data only.
 */
class NowPlayingWidgetStatesTest {
    private val profile = "jf-0123456789abcdef"
    private val now = 1_794_470_400_000L

    private fun json(
        privacy: String = "normal",
        song: String? = song(nowPlaying = true),
        isPlaying: Boolean = true,
        positionMs: Long = 84_000,
        positionAt: Long = now - 10_000,
        written: Long = now - 10_000,
        reachable: Boolean = true,
        nextUp: String? = """{ "mixId": "mix-1", "title": "Rainy Focus", "cover": null, "pinned": false }""",
        pinned: String = """[{ "mixId": "mix-2", "title": "Evening Drive", "cover": "7d793037a0760186574b0282f2f435e7.img", "pinned": true }]""",
        albums: String = """[{ "albumId": 37, "title": "Golden Hour Static", "artist": "Ember Cove", "cover": null }]""",
        schema: Int = 1,
        profileId: String = profile,
    ) = """
        {
          "schema": $schema,
          "profileId": "$profileId",
          "privacy": "$privacy",
          "song": ${song ?: "null"},
          "isPlaying": $isPlaying,
          "positionMs": $positionMs,
          "positionAtUtcMs": $positionAt,
          "pinnedMixes": $pinned,
          "nextUp": ${nextUp ?: "null"},
          "recentAlbums": $albums,
          "reachable": $reachable,
          "writtenAtUtcMs": $written,
          "recap": null
        }
    """.trimIndent()

    private fun song(nowPlaying: Boolean, durationMs: Long = 222_000) = """
        { "songId": 412, "title": "Slow Tide", "artist": "Ember Cove", "albumId": 37,
          "albumTitle": "Golden Hour Static", "cover": "5d41402abc4b2a76b9719d911017c592.img",
          "durationMs": $durationMs, "nowPlaying": $nowPlaying }
    """.trimIndent()

    private fun read(text: String) = SnapshotReader.parse(text, profile)

    private fun decide(
        text: String,
        canControl: Boolean = true,
        lockScreen: Boolean = false,
        hide: Boolean = true,
        at: Long = now,
    ) = NowPlayingStates.decide(read(text), canControl, at, lockScreen, hide)

    // --- Parsing. ---

    @Test
    fun parsesTheCoreApiExample() {
        val r = read(json()) as SnapshotRead.Ok
        val s = r.snapshot
        assertEquals(profile, s.profileId)
        assertEquals(SnapshotPrivacy.NORMAL, s.privacy)
        assertEquals("Slow Tide", s.song?.title)
        assertEquals("Ember Cove", s.song?.artist)
        assertEquals(37L, s.song?.albumId)
        assertEquals(222_000L, s.song?.durationMs)
        assertTrue(s.song!!.nowPlaying)
        assertEquals("Rainy Focus", s.nextUp?.title)
        assertNull("JSON null is null, not the word", s.nextUp?.cover)
        assertEquals(1, s.pinnedMixes.size)
        assertEquals(1, s.recentAlbums.size)
        assertTrue(s.reachable)
    }

    @Test
    fun aNewerSchemaOrBrokenFileIsUnreadable() {
        assertEquals(SnapshotRead.Unreadable, read(json(schema = 2)))
        assertEquals(SnapshotRead.Unreadable, read("{ not json"))
        assertEquals(SnapshotRead.Unreadable, read(json(privacy = "shared")))
        assertEquals(CardKind.OPEN_APP, (decide(json(schema = 2)) as NowPlayingView.Card).kind)
    }

    @Test
    fun anotherProfilesFileIsNeverShown() {
        assertEquals(SnapshotRead.Unreadable, read(json(profileId = "jf-somebody-else")))
        val view = decide(json(profileId = "jf-somebody-else"))
        assertEquals(CardKind.OPEN_APP, (view as NowPlayingView.Card).kind)
    }

    @Test
    fun coverNamesAndProfileIdsCannotLeaveTheirFolder() {
        assertTrue(SnapshotReader.isSafeName("5d41402abc4b2a76b9719d911017c592.img"))
        assertTrue(SnapshotReader.isSafeName(profile))
        for (bad in listOf("", ".", "..", "../x", "a/b", "a\\b", "x\u0000", "é")) {
            assertFalse("\"$bad\" is not a safe name", SnapshotReader.isSafeName(bad))
        }
        val dir = java.io.File("widgets")
        assertNull(SnapshotReader.coverFile(dir, profile, "../../core.db"))
        assertNull(SnapshotReader.coverFile(dir, "..", "a.img"))
        assertEquals(java.io.File(java.io.File(java.io.File(dir, profile), "art"), "a.img"), SnapshotReader.coverFile(dir, profile, "a.img"))
    }

    @Test
    fun coversAreDownsampledToAbout256() {
        assertEquals(1, SnapshotReader.sampleSize(256, 256, 256))
        assertEquals(1, SnapshotReader.sampleSize(300, 300, 256))
        assertEquals(2, SnapshotReader.sampleSize(600, 600, 256))
        assertEquals(4, SnapshotReader.sampleSize(1024, 1024, 256))
        assertEquals(8, SnapshotReader.sampleSize(3000, 2000, 256))
    }

    // --- Normal states. ---

    @Test
    fun playingShowsTheSongWithPause() {
        val v = decide(json()) as NowPlayingView.Song
        assertTrue(v.playing)
        assertTrue(v.current)
        assertEquals(MainButton.PAUSE, v.button)
        assertEquals(WidgetTarget.NOW_PLAYING, v.tap)
        assertNull("buttons go straight to the service", v.buttonsOpen)
        assertFalse(v.stale)
    }

    @Test
    fun pausedShowsPlay() {
        val v = decide(json(isPlaying = false)) as NowPlayingView.Song
        assertFalse(v.playing)
        assertEquals(MainButton.PLAY, v.button)
    }

    @Test
    fun aPlayingFileWhoseSongMustHaveEndedReadsAsPaused() {
        // Written 10 minutes ago, 138 s left of the song: it can't still be playing.
        val v = decide(json(written = now - 600_000, positionAt = now - 600_000)) as NowPlayingView.Song
        assertFalse(v.playing)
        assertEquals(MainButton.PLAY, v.button)
    }

    @Test
    fun theLevelBarsChangeWithEveryStateChange() {
        // A widget can't animate: the bars are three drawings, and the file's position time
        // moves with every start, pause, resume and song change, so each redraw differs.
        val variants = (0L..5L).map { s -> (decide(json(positionAt = now - 60_000 + s * 1000)) as NowPlayingView.Song).barsVariant }
        assertTrue("always one of the drawings: $variants", variants.all { it in 0 until NowPlayingStates.BARS_VARIANTS })
        assertEquals("every drawing comes up: $variants", NowPlayingStates.BARS_VARIANTS, variants.distinct().size)
        for ((a, b) in variants.zipWithNext()) assertTrue("a second later it looks different: $variants", a != b)
    }

    @Test
    fun idleShowsTheLastPlayedSongWithPlay() {
        val v = decide(json(song = song(nowPlaying = false), isPlaying = false, positionMs = 0, positionAt = 0)) as NowPlayingView.Song
        assertEquals("Slow Tide", v.title)
        assertFalse(v.current)
        assertFalse(v.playing)
        assertEquals(MainButton.PLAY, v.button)
    }

    @Test
    fun nothingYetSaysSoWithPlay() {
        val v = decide(json(song = null, isPlaying = false, nextUp = null, pinned = "[]", albums = "[]")) as NowPlayingView.Card
        assertEquals(CardKind.NOTHING_YET, v.kind)
        assertEquals(MainButton.START_NEXT_UP, v.button)
    }

    @Test
    fun noSongButMixesOffersNextUp() {
        val v = decide(json(song = null, isPlaying = false)) as NowPlayingView.Card
        assertEquals(CardKind.MIX_READY, v.kind)
        assertEquals("Rainy Focus", v.mixName)
        assertEquals(MainButton.START_NEXT_UP, v.button)
    }

    // --- Privacy and the other cards. ---

    @Test
    fun aLockedProfileShowsOnlyOpenEncore() {
        val v = decide(json(privacy = "locked", song = null, nextUp = null, pinned = "[]", albums = "[]", isPlaying = false))
        assertEquals(CardKind.LOCKED, (v as NowPlayingView.Card).kind)
        assertEquals(WidgetTarget.APP, v.tap)
        assertNull(v.button)
    }

    @Test
    fun evenALockedFileWithASongNeverShowsIt() {
        // The core empties it; the widget still never shows a song for a locked profile.
        val v = decide(json(privacy = "locked"))
        assertEquals(CardKind.LOCKED, (v as NowPlayingView.Card).kind)
        assertNull(v.mixName)
    }

    @Test
    fun guestShowsTheNeutralCard() {
        val v = decide(json(privacy = "guest", song = null, nextUp = null, pinned = "[]", albums = "[]", isPlaying = false))
        assertEquals(CardKind.OPEN_APP, (v as NowPlayingView.Card).kind)
        assertNull(v.button)
    }

    @Test
    fun aRemovedProfileSaysSoAndOpensTheSwitcher() {
        val v = NowPlayingStates.decide(SnapshotRead.Missing, true, now, false, true) as NowPlayingView.Card
        assertEquals(CardKind.REMOVED, v.kind)
        assertEquals(WidgetTarget.PROFILES, v.tap)
    }

    @Test
    fun noProfileYetOpensTheApp() {
        val v = NowPlayingStates.decide(null, false, now, false, true) as NowPlayingView.Card
        assertEquals(CardKind.OPEN_APP, v.kind)
    }

    @Test
    fun unreachableShowsSavedMixesUnlessSomethingPlays() {
        val idle = decide(json(reachable = false, isPlaying = false))
        assertEquals(CardKind.UNREACHABLE, (idle as NowPlayingView.Card).kind)
        // A downloaded song playing offline still shows.
        assertTrue(decide(json(reachable = false)) is NowPlayingView.Song)
    }

    @Test
    fun overADayOldDimsAndOpensTheApp() {
        val text = json(isPlaying = false, written = now - NowPlayingStates.STALE_MS - 1)
        val v = decide(text) as NowPlayingView.Song
        assertTrue(v.stale)
        assertEquals(WidgetTarget.APP, v.tap)
        assertEquals(WidgetTarget.APP, v.buttonsOpen)
        val fresh = decide(json(isPlaying = false, written = now - NowPlayingStates.STALE_MS + 60_000)) as NowPlayingView.Song
        assertFalse(fresh.stale)
    }

    @Test
    fun anotherProfileOrAPinMakesButtonsOpenTheSwitcher() {
        val v = decide(json(), canControl = false) as NowPlayingView.Song
        assertEquals(WidgetTarget.PROFILES, v.buttonsOpen)
        assertEquals(WidgetTarget.PROFILES, v.tap)
        val card = decide(json(song = null, isPlaying = false), canControl = false) as NowPlayingView.Card
        assertEquals(WidgetTarget.PROFILES, card.buttonsOpen)
    }

    // --- The lock screen. ---

    @Test
    fun onTheLockScreenWithHideOnTheLastPlayedSongNeverShows() {
        val text = json(song = song(nowPlaying = false), isPlaying = false, positionMs = 0, positionAt = 0)
        val v = decide(text, lockScreen = true, hide = true)
        assertTrue("no song view on the lock screen", v is NowPlayingView.Card)
        v as NowPlayingView.Card
        assertEquals(CardKind.LOCK_SCREEN_IDLE, v.kind)
        assertEquals(MainButton.PLAY, v.button)
        assertEquals("only the Next up mix's name", "Rainy Focus", v.mixName)
    }

    @Test
    fun onTheLockScreenWithHideOnAPausedSongDoesntShowEither() {
        val v = decide(json(isPlaying = false), lockScreen = true, hide = true)
        assertEquals(CardKind.LOCK_SCREEN_IDLE, (v as NowPlayingView.Card).kind)
    }

    @Test
    fun onTheLockScreenWithHideOnWhatIsPlayingNowMayShow() {
        assertTrue(decide(json(), lockScreen = true, hide = true) is NowPlayingView.Song)
    }

    @Test
    fun onTheLockScreenWithHideOffTheLastPlayedSongShows() {
        val text = json(song = song(nowPlaying = false), isPlaying = false, positionMs = 0, positionAt = 0)
        val v = decide(text, lockScreen = true, hide = false) as NowPlayingView.Song
        assertEquals("Slow Tide", v.title)
    }

    @Test
    fun onTheLockScreenWithNoMixTheCardHasNoName() {
        val text = json(song = song(nowPlaying = false), isPlaying = false, nextUp = null)
        val v = decide(text, lockScreen = true, hide = true) as NowPlayingView.Card
        assertNull(v.mixName)
    }

    @Test
    fun theLockScreenNeverOverridesLockedOrGuest() {
        assertEquals(CardKind.LOCKED, (decide(json(privacy = "locked"), lockScreen = true, hide = false) as NowPlayingView.Card).kind)
        assertEquals(CardKind.OPEN_APP, (decide(json(privacy = "guest"), lockScreen = true, hide = false) as NowPlayingView.Card).kind)
    }

    @Test
    fun theSongCarriesItsAlbumsColors() {
        // A palette in the file reaches the view (the widget then wears the album's colors).
        val palette = """{"scheme":"dark","base":"3D0000","stops":["CF4630","1B1030","C02D56","986E00"],"text":"FFFFFF"}"""
        val withColours = song(nowPlaying = true).replace("\"nowPlaying\": true", "\"nowPlaying\": true, \"palette\": $palette")
        val v = decide(json(song = withColours)) as NowPlayingView.Song
        assertEquals(0xFF3D0000.toInt(), v.palette!!.base)
        assertNull("no palette yet: the theme's colors", (decide(json()) as NowPlayingView.Song).palette)
    }
}
