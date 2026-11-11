package uk.gcartee.encore.app

import org.junit.Assert.assertEquals
import org.junit.Test
import uk.gcartee.encore.app.tile.StartMixTileModel
import uk.gcartee.encore.app.tile.TileKind
import uk.gcartee.encore.app.tile.TileTap
import uk.gcartee.encore.app.tile.TileWords
import uk.gcartee.encore.app.widgets.KnownProfile
import uk.gcartee.encore.app.widgets.ProfileState
import uk.gcartee.encore.app.widgets.Snapshot
import uk.gcartee.encore.app.widgets.SnapshotMix
import uk.gcartee.encore.app.widgets.SnapshotPrivacy
import uk.gcartee.encore.app.widgets.SnapshotRead

/** Phase 9 (WP8): the Start a mix tile's state and tap, from what it is allowed to know. */
class StartMixTileModelTest {
    private val words = TileWords(playing = "Playing", unlock = "Unlock Encore", nextUp = "Next up", openApp = "Open Encore")

    private fun mix(id: String, title: String, pinned: Boolean = true) = SnapshotMix(id, title, null, pinned)

    private fun snapshot(
        profile: String = "p1",
        privacy: SnapshotPrivacy = SnapshotPrivacy.NORMAL,
        pinned: List<SnapshotMix> = emptyList(),
        nextUp: SnapshotMix? = null,
    ) = SnapshotRead.Ok(
        Snapshot(profile, privacy, null, false, 0, 0, pinned, nextUp, emptyList(), true, 0),
    )

    private fun state(current: String? = "p1", lastActive: String? = "p1", locked: Boolean = false) =
        ProfileState(current, lastActive, listOf(KnownProfile("p1", "Ada", locked), KnownProfile("p2", "Bo", false)))

    @Test
    fun theFirstPinnedMixIsNamedAndStartedByID() {
        val v = StartMixTileModel.decide(
            state(), snapshot(pinned = listOf(mix("m1", "Evening Drive"), mix("m2", "Gym"))), false, false, words,
        )
        assertEquals(TileKind.INACTIVE, v.kind)
        assertEquals("Evening Drive", v.subtitle)
        assertEquals(TileTap.StartMix("m1"), v.tap)
    }

    @Test
    fun withNothingPinnedNextUpIsNamedAndStartedWithoutAnID() {
        val v = StartMixTileModel.decide(
            state(), snapshot(nextUp = mix("n", "Made for you", pinned = false)), false, false, words,
        )
        assertEquals("Made for you", v.subtitle)
        assertEquals(TileTap.StartMix(null), v.tap)
    }

    @Test
    fun withNoMixesOrNoFileItSaysNextUpAndStillStarts() {
        for (read in listOf(snapshot(), SnapshotRead.Missing, SnapshotRead.Unreadable, null)) {
            val v = StartMixTileModel.decide(state(), read, false, false, words)
            assertEquals(TileKind.INACTIVE, v.kind)
            assertEquals("Next up", v.subtitle)
            assertEquals(TileTap.StartMix(null), v.tap)
        }
    }

    @Test
    fun whilePlayingItIsActiveAndOpensNowPlaying() {
        val v = StartMixTileModel.decide(state(), snapshot(pinned = listOf(mix("m1", "Gym"))), true, false, words)
        assertEquals(TileKind.ACTIVE, v.kind)
        assertEquals("Playing", v.subtitle)
        assertEquals(TileTap.OpenNowPlaying, v.tap)
    }

    @Test
    fun aPinOnALockedPhoneIsUnavailable() {
        val v = StartMixTileModel.decide(state(locked = true), snapshot(pinned = listOf(mix("m1", "Gym"))), false, true, words)
        assertEquals(TileKind.UNAVAILABLE, v.kind)
        assertEquals("Unlock Encore", v.subtitle)
    }

    @Test
    fun aLockedPhoneWithoutAPinStillStarts() {
        val v = StartMixTileModel.decide(state(), snapshot(pinned = listOf(mix("m1", "Gym"))), false, true, words)
        assertEquals(TileKind.INACTIVE, v.kind)
        assertEquals(TileTap.StartMix("m1"), v.tap)
    }

    @Test
    fun aPinOnAnUnlockedPhoneOpensTheAppInsteadOfStarting() {
        val v = StartMixTileModel.decide(state(locked = true), snapshot(pinned = listOf(mix("m1", "Gym"))), false, false, words)
        assertEquals(TileKind.INACTIVE, v.kind)
        assertEquals("Gym", v.subtitle)
        assertEquals(TileTap.OpenProfiles, v.tap)
    }

    @Test
    fun aSnapshotTheCoreCallsLockedIsUnavailable() {
        val v = StartMixTileModel.decide(state(), snapshot(privacy = SnapshotPrivacy.LOCKED), false, false, words)
        assertEquals(TileKind.UNAVAILABLE, v.kind)
        assertEquals("Unlock Encore", v.subtitle)
    }

    @Test
    fun anotherOpenProfileOpensTheAppNotAMixOfTheirs() {
        // p2 is open, so the service would not play p1 without a switch; the tile shows p2's mix.
        val v = StartMixTileModel.decide(
            state(current = "p2", lastActive = "p1"), snapshot("p2", pinned = listOf(mix("m9", "Bo's mix"))), false, false, words,
        )
        assertEquals("Bo's mix", v.subtitle)
        assertEquals(TileTap.OpenProfiles, v.tap)
    }

    @Test
    fun theLastActiveProfileWithNothingOpenStartsHeadlessly() {
        val v = StartMixTileModel.decide(state(current = null, lastActive = "p1"), snapshot(pinned = listOf(mix("m1", "Gym"))), false, false, words)
        assertEquals(TileTap.StartMix("m1"), v.tap)
    }

    @Test
    fun withNoProfileKnownItOpensTheApp() {
        val v = StartMixTileModel.decide(ProfileState(null, null, emptyList()), null, false, false, words)
        assertEquals(TileKind.INACTIVE, v.kind)
        assertEquals("Open Encore", v.subtitle)
        assertEquals(TileTap.OpenApp, v.tap)
    }

    @Test
    fun aBlankMixTitleFallsBackToNextUp() {
        val v = StartMixTileModel.decide(state(), snapshot(pinned = listOf(mix("m1", " "))), false, false, words)
        assertEquals("Next up", v.subtitle)
        assertEquals(TileTap.StartMix("m1"), v.tap)
    }

    @Test
    fun onAndroid9TheSubtitleGoesIntoTheLabel() {
        assertEquals("Start a mix, Evening Drive", StartMixTileModel.labelWithSubtitle("Start a mix", "Evening Drive"))
    }
}
