package uk.gcartee.encore.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.widgets.AlbumOpenRequest
import uk.gcartee.encore.app.widgets.CardKind
import uk.gcartee.encore.app.widgets.CollectionStates
import uk.gcartee.encore.app.widgets.MixesView
import uk.gcartee.encore.app.widgets.NowPlayingStates
import uk.gcartee.encore.app.widgets.RecentView
import uk.gcartee.encore.app.widgets.SnapshotRead
import uk.gcartee.encore.app.widgets.SnapshotReader
import uk.gcartee.encore.app.widgets.WidgetMixChoice
import uk.gcartee.encore.app.widgets.WidgetTarget

/**
 * Phase 9 (WP7): the Mixes and Recently played widgets' states (board 34, items 3 and 4),
 * on the snapshot file's real org.json parsing. Made-up data only.
 */
class CollectionWidgetStatesTest {
    private val profile = "jf-0123456789abcdef"
    private val now = 1_794_470_400_000L

    private fun mix(id: String, title: String, pinned: Boolean = true) =
        """{ "mixId": "$id", "title": "$title", "cover": "$id.img", "pinned": $pinned }"""

    private fun album(id: Int, title: String) =
        """{ "albumId": $id, "title": "$title", "artist": "Ember Cove", "cover": "a$id.img" }"""

    private fun json(
        privacy: String = "normal",
        reachable: Boolean = true,
        written: Long = now - 10_000,
        nextUp: String? = mix("next-1", "Rainy Focus", pinned = false),
        pinned: List<String> = listOf(mix("m-evening", "Evening Drive"), mix("m-home", "Drive Home"), mix("m-slow", "Slow Morning")),
        albums: List<String> = listOf(album(1, "Golden Hour"), album(2, "Night Market")),
        profileId: String = profile,
    ) = """
        {
          "schema": 1, "profileId": "$profileId", "privacy": "$privacy",
          "song": null, "isPlaying": false, "positionMs": 0, "positionAtUtcMs": 0,
          "pinnedMixes": [${pinned.joinToString(",")}],
          "nextUp": ${nextUp ?: "null"},
          "recentAlbums": [${albums.joinToString(",")}],
          "reachable": $reachable, "writtenAtUtcMs": $written, "recap": null
        }
    """.trimIndent()

    private fun read(text: String) = SnapshotReader.parse(text, profile)

    private fun mixes(text: String, choice: WidgetMixChoice = WidgetMixChoice.NextUp, canControl: Boolean = true) =
        CollectionStates.mixes(read(text), choice, canControl, now)

    private fun recent(text: String, canControl: Boolean = true) = CollectionStates.recent(read(text), canControl, now)

    // --- Mixes. ---

    @Test
    fun nextUpLeadsThenThePinnedMixes() {
        val v = mixes(json()) as MixesView.Tiles
        assertEquals(listOf("Rainy Focus", "Evening Drive", "Drive Home", "Slow Morning"), v.tiles.map { it.title })
        assertNull("the Next up tile follows the daily set", v.tiles[0].mixId)
        assertEquals("m-evening", v.tiles[1].mixId)
        assertNull("one tap plays", v.tilesOpen)
        assertEquals(false, v.stale)
    }

    @Test
    fun aPinnedChoiceLeadsWhileItIsStillPinned() {
        val v = mixes(json(), WidgetMixChoice.Pinned("m-home")) as MixesView.Tiles
        assertEquals(listOf("Drive Home", "Rainy Focus", "Evening Drive", "Slow Morning"), v.tiles.map { it.title })
    }

    @Test
    fun aPinnedChoiceThatIsGoneFallsBackToNextUp() {
        val v = mixes(json(), WidgetMixChoice.Pinned("m-gone")) as MixesView.Tiles
        assertEquals("Rainy Focus", v.tiles.first().title)
        assertEquals(4, v.tiles.size)
    }

    @Test
    fun atMostSixTilesAndNoMixTwice() {
        // Six is the most the biggest layout (three across, two down) shows.
        val many = (1..8).map { mix("m-$it", "Mix $it") } + mix("next-1", "Rainy Focus")
        val v = mixes(json(pinned = many)) as MixesView.Tiles
        assertEquals(6, v.tiles.size)
        assertEquals(v.tiles.size, v.tiles.map { it.mixId }.distinct().size)
        // The pinned copy of the Next up mix is dropped: Next up already shows it.
        assertEquals(1, mixes(json(pinned = many)).let { (it as MixesView.Tiles).tiles }.count { it.title == "Rainy Focus" })
    }

    @Test
    fun noNextUpShowsJustThePinnedMixes() {
        val v = mixes(json(nextUp = null, pinned = listOf(mix("m-1", "Evening Drive")))) as MixesView.Tiles
        assertEquals(listOf("Evening Drive"), v.tiles.map { it.title })
    }

    @Test
    fun nothingYetWhenThereAreNoMixes() {
        val v = mixes(json(nextUp = null, pinned = emptyList())) as MixesView.Card
        assertEquals(CardKind.NOTHING_YET, v.kind)
    }

    @Test
    fun anotherProfileOrAPinMakesATapOpenTheSwitcher() {
        val v = mixes(json(), canControl = false) as MixesView.Tiles
        assertEquals(WidgetTarget.PROFILES, v.tilesOpen)
    }

    @Test
    fun overADayOldDimsAndOpensTheApp() {
        val v = mixes(json(written = now - NowPlayingStates.STALE_MS - 1)) as MixesView.Tiles
        assertTrue(v.stale)
        assertEquals(WidgetTarget.APP, v.tilesOpen)
        val fresh = mixes(json(written = now - NowPlayingStates.STALE_MS + 60_000)) as MixesView.Tiles
        assertEquals(false, fresh.stale)
    }

    @Test
    fun unreachableShowsSavedMixes() {
        val v = mixes(json(reachable = false)) as MixesView.Card
        assertEquals(CardKind.UNREACHABLE, v.kind)
    }

    @Test
    fun lockedGuestRemovedAndBrokenFilesShowOnlyWords() {
        assertEquals(CardKind.LOCKED, (mixes(json(privacy = "locked")) as MixesView.Card).kind)
        assertEquals(CardKind.OPEN_APP, (mixes(json(privacy = "guest")) as MixesView.Card).kind)
        assertEquals(CardKind.OPEN_APP, (mixes(json(profileId = "jf-somebody-else")) as MixesView.Card).kind)
        val removed = CollectionStates.mixes(SnapshotRead.Missing, WidgetMixChoice.NextUp, true, now) as MixesView.Card
        assertEquals(CardKind.REMOVED, removed.kind)
        assertEquals(WidgetTarget.PROFILES, removed.tap)
        val none = CollectionStates.mixes(null, WidgetMixChoice.NextUp, false, now) as MixesView.Card
        assertEquals(CardKind.OPEN_APP, none.kind)
    }

    // --- Recently played. ---

    @Test
    fun recentShowsUpToEightAlbumsAndOpensThem() {
        val ten = (1..10).map { album(it, "Album $it") }
        val v = recent(json(albums = ten)) as RecentView.Albums
        assertEquals(8, v.albums.size)
        assertEquals(profile, v.profileId)
        assertNull("a tap opens the album", v.albumsOpen)
    }

    @Test
    fun recentNothingYet() {
        assertEquals(CardKind.NOTHING_YET, (recent(json(albums = emptyList())) as RecentView.Card).kind)
    }

    @Test
    fun recentForAnotherProfileOrAPinOpensTheSwitcher() {
        assertEquals(WidgetTarget.PROFILES, (recent(json(), canControl = false) as RecentView.Albums).albumsOpen)
    }

    @Test
    fun recentStaleDimsAndOpensTheApp() {
        val v = CollectionStates.recent(read(json(written = now - NowPlayingStates.STALE_MS - 1)), true, now) as RecentView.Albums
        assertTrue(v.stale)
        assertEquals(WidgetTarget.APP, v.albumsOpen)
    }

    @Test
    fun recentNeverShowsListeningForLockedGuestRemovedOrAnotherProfile() {
        assertEquals(CardKind.LOCKED, (recent(json(privacy = "locked")) as RecentView.Card).kind)
        assertEquals(CardKind.OPEN_APP, (recent(json(privacy = "guest")) as RecentView.Card).kind)
        assertEquals(CardKind.OPEN_APP, (recent(json(profileId = "jf-somebody-else")) as RecentView.Card).kind)
        assertEquals(CardKind.REMOVED, (CollectionStates.recent(SnapshotRead.Missing, true, now) as RecentView.Card).kind)
    }

    // --- The album request the app gets. ---

    @Test
    fun theAlbumRequestRoundTripsAndRejectsBadOnes() {
        val parsed = AlbumOpenRequest.parse(AlbumOpenRequest.encode(37, profile))
        assertEquals(37L, parsed?.albumId)
        assertEquals(profile, parsed?.profileId)
        assertNull(AlbumOpenRequest.parse(null))
        assertNull(AlbumOpenRequest.parse("downloads"))
        assertNull(AlbumOpenRequest.parse("album:abc:$profile"))
        assertNull(AlbumOpenRequest.parse("album:37"))
        assertNull(AlbumOpenRequest.parse("album:37:../x"))
        assertNull(AlbumOpenRequest.parse("album:37:"))
    }
}
