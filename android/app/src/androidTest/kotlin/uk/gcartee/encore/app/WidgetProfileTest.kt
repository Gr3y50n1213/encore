package uk.gcartee.encore.app

import java.io.File
import java.nio.file.Files
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.widgets.KnownProfile
import uk.gcartee.encore.app.widgets.ProfileState
import uk.gcartee.encore.app.widgets.SnapshotRead
import uk.gcartee.encore.app.widgets.SnapshotReader
import uk.gcartee.encore.app.widgets.WidgetConfig
import uk.gcartee.encore.app.widgets.WidgetProfileChoice
import uk.gcartee.encore.app.widgets.WidgetToken

/**
 * Phase 9 (WP6): which profile's file a widget reads, and when its buttons may play
 * directly. The done-when: a widget set to one profile never shows another's history.
 * Made-up profiles only.
 */
class WidgetProfileTest {
    private val ada = "jf-aaaaaaaaaaaaaaaa"
    private val ben = "jf-bbbbbbbbbbbbbbbb"
    private val root: File = Files.createTempDirectory("widgets-test").toFile()

    @After
    fun tidy() {
        root.deleteRecursively()
    }

    private fun write(profileId: String, title: String, fileProfileId: String = profileId) {
        val dir = File(root, profileId).apply { mkdirs() }
        File(dir, SnapshotReader.FILE).writeText(
            """
            { "schema": 1, "profileId": "$fileProfileId", "privacy": "normal",
              "song": { "songId": 1, "title": "$title", "artist": "Ember Cove", "albumId": 2,
                        "albumTitle": "Golden Hour Static", "cover": null, "durationMs": 1000, "nowPlaying": false },
              "isPlaying": false, "positionMs": 0, "positionAtUtcMs": 0, "pinnedMixes": [], "nextUp": null,
              "recentAlbums": [], "reachable": true, "writtenAtUtcMs": 1, "recap": null }
            """.trimIndent(),
        )
    }

    private fun titleFor(profileId: String): String? =
        ((SnapshotReader.read(root, profileId) as? SnapshotRead.Ok)?.snapshot?.song?.title)

    @Test
    fun theFileComesFromTheCoresDataFolder() {
        assertEquals(File(File(File("files"), "core"), "widgets"), SnapshotReader.widgetsDir(File("files")))
    }

    @Test
    fun activeFollowsTheOpenProfileThenTheLastActive() {
        val state = ProfileState(current = ben, lastActive = ada, profiles = emptyList())
        assertEquals(ben, WidgetConfig.resolve(WidgetProfileChoice.Active, state))
        assertEquals(ada, WidgetConfig.resolve(WidgetProfileChoice.Active, state.copy(current = null)))
        assertNull(WidgetConfig.resolve(WidgetProfileChoice.Active, ProfileState(null, null, emptyList())))
    }

    @Test
    fun aFixedProfileIsAlwaysItselfWhateverIsOpen() {
        val state = ProfileState(current = ben, lastActive = ben, profiles = emptyList())
        assertEquals(ada, WidgetConfig.resolve(WidgetProfileChoice.Fixed(ada), state))
    }

    @Test
    fun aWidgetSetToOneProfileReadsOnlyThatProfilesFile() {
        write(ada, "Ada's song")
        write(ben, "Ben's song")
        val state = ProfileState(current = ben, lastActive = ben, profiles = emptyList())
        val id = WidgetConfig.resolve(WidgetProfileChoice.Fixed(ada), state)!!
        assertEquals("Ada's song", titleFor(id))
        // Ada's folder gone: "Profile removed", never Ben's history in its place.
        File(root, ada).deleteRecursively()
        assertEquals(SnapshotRead.Missing, SnapshotReader.read(root, id))
    }

    @Test
    fun aFileInTheWrongFolderIsNotShown() {
        // Ben's snapshot copied into Ada's folder (it must never happen; the widget still refuses).
        write(ada, "Ben's song", fileProfileId = ben)
        assertEquals(SnapshotRead.Unreadable, SnapshotReader.read(root, ada))
    }

    @Test
    fun anUnsafeProfileIdReadsNothing() {
        write(ada, "Ada's song")
        assertEquals(SnapshotRead.Unreadable, SnapshotReader.read(root, "../${root.name}/$ada"))
        assertEquals(SnapshotRead.Unreadable, SnapshotReader.read(root, ".."))
    }

    @Test
    fun buttonsPlayDirectlyOnlyForTheProfileTheServiceWouldOpen() {
        val both = ProfileState(current = ada, lastActive = ada, profiles = listOf(KnownProfile(ada, "Ada", false), KnownProfile(ben, "Ben", false)))
        assertTrue(WidgetConfig.canControl(ada, both))
        assertTrue("app closed, Ada last", WidgetConfig.canControl(ada, both.copy(current = null)))
        assertFalse("would need a switch", WidgetConfig.canControl(ben, both))
        assertFalse("Guest open", WidgetConfig.canControl(ada, both.copy(current = "guest")))
        val pin = both.copy(profiles = listOf(KnownProfile(ada, "Ada", true)))
        assertFalse("a PIN opens the app", WidgetConfig.canControl(ada, pin))
    }

    @Test
    fun theProfileListRoundTrips() {
        val list = listOf(KnownProfile(ada, "Ada \"A\"", false), KnownProfile(ben, "Ben", true))
        assertEquals(list, WidgetConfig.decodeProfiles(WidgetConfig.encodeProfiles(list)))
        assertEquals(emptyList<KnownProfile>(), WidgetConfig.decodeProfiles("not json"))
        assertEquals(emptyList<KnownProfile>(), WidgetConfig.decodeProfiles(null))
    }

    @Test
    fun theTokenMustMatchExactly() {
        val token = "0123456789abcdef0123456789abcdef"
        assertTrue(WidgetToken.matches(token, token))
        assertFalse(WidgetToken.matches(token, null))
        assertFalse(WidgetToken.matches(token, ""))
        assertFalse(WidgetToken.matches(token, token.dropLast(1)))
        assertFalse(WidgetToken.matches(token, token.uppercase()))
        assertFalse("an empty expected token never matches", WidgetToken.matches("", ""))
    }
}
