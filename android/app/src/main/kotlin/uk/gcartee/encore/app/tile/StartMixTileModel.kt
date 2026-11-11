package uk.gcartee.encore.app.tile

import uk.gcartee.encore.app.widgets.ProfileState
import uk.gcartee.encore.app.widgets.SnapshotPrivacy
import uk.gcartee.encore.app.widgets.SnapshotRead
import uk.gcartee.encore.app.widgets.WidgetConfig
import uk.gcartee.encore.app.widgets.WidgetProfileChoice

/**
 * Phase 9 (WP8), board 34 item 4: what the "Start a mix" Quick Settings tile shows and does.
 * Pure (no Android classes), so the JVM tests cover every state.
 *
 * Like the widgets, it never starts the core: it reads the profile state WidgetSync saved and
 * that profile's snapshot file, and "playing" is what the playback service in this process
 * says (nothing running means not playing).
 */
enum class TileKind { INACTIVE, ACTIVE, UNAVAILABLE }

/** What a tap does. */
sealed interface TileTap {
    /** Start [mixId] (Next up when null) through the playback service, with no screen. */
    data class StartMix(val mixId: String?) : TileTap

    /** Open the app on Now Playing: the mix is already playing. */
    data object OpenNowPlaying : TileTap

    /** Open the app at the profile switcher: another profile, or one with a PIN. */
    data object OpenProfiles : TileTap

    /** Open the app: nothing known yet. */
    data object OpenApp : TileTap
}

/** The words the tile shows; the service fills them from string resources. */
data class TileWords(val playing: String, val unlock: String, val nextUp: String, val openApp: String)

data class TileView(val kind: TileKind, val subtitle: String, val tap: TileTap)

object StartMixTileModel {
    /**
     * [snapshot] is the headless profile's file ([SnapshotRead.Missing] when none was read),
     * [playing] whether this process is playing right now, [deviceLocked] the keyguard.
     */
    fun decide(
        state: ProfileState,
        snapshot: SnapshotRead?,
        playing: Boolean,
        deviceLocked: Boolean,
        words: TileWords,
    ): TileView {
        val profile = WidgetConfig.resolve(WidgetProfileChoice.Active, state)
            ?: return TileView(TileKind.INACTIVE, words.openApp, TileTap.OpenApp)
        val hasPin = state.profiles.any { it.id == profile && it.locked }
        val ok = (snapshot as? SnapshotRead.Ok)?.snapshot
        // A PIN on a locked phone, or a profile the core says is locked: the tile can't start it.
        if ((hasPin && deviceLocked) || ok?.privacy == SnapshotPrivacy.LOCKED) {
            return TileView(TileKind.UNAVAILABLE, words.unlock, TileTap.OpenProfiles)
        }
        if (playing) return TileView(TileKind.ACTIVE, words.playing, TileTap.OpenNowPlaying)
        val pinned = ok?.pinnedMixes?.firstOrNull()
        // The name shown: the first pinned mix, else Next up's title, else the plain words.
        val subtitle = (pinned ?: ok?.nextUp)?.title?.takeIf { it.isNotBlank() } ?: words.nextUp
        // A tap starts the pinned mix by ID, else Next up (no ID).
        val tap = if (WidgetConfig.canControl(profile, state)) {
            TileTap.StartMix(pinned?.mixId)
        } else {
            TileTap.OpenProfiles
        }
        return TileView(TileKind.INACTIVE, subtitle, tap)
    }

    /** A label for API 28, where a tile has no subtitle: "Start a mix, Evening Drive". */
    fun labelWithSubtitle(label: String, subtitle: String): String = "$label, $subtitle"
}
