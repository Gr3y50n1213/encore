package uk.gcartee.encore.app.tile

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.drawable.Icon
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.util.Log
import uk.gcartee.encore.app.R
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.widgets.SnapshotRead
import uk.gcartee.encore.app.widgets.SnapshotReader
import uk.gcartee.encore.app.widgets.WidgetConfig
import uk.gcartee.encore.app.widgets.WidgetIntents
import uk.gcartee.encore.app.widgets.WidgetProfileChoice
import uk.gcartee.encore.app.widgets.WidgetTarget

/**
 * Phase 9 (WP8), board 34 item 4: the "Start a mix" Quick Settings tile.
 *
 * - Inactive: the subtitle is the mix a tap starts (the first pinned mix, else Next up).
 * - Active: "Playing". Unavailable: "Unlock Encore", for a profile with a PIN on a locked phone.
 * - A tap starts the mix in the background through the playback service, with the same
 *   token-protected private action the widgets use ([WidgetIntents.playMix]). Android may
 *   refuse a foreground start (Android 12 and later); then the app opens on Now Playing.
 *   A profile that isn't the one that opens headlessly, or one with a PIN, opens the app.
 * - Touch and hold opens the app on Mixes (MainActivity answers QS_TILE_PREFERENCES).
 *
 * Drawing it never starts the core: the saved profile state and the profile's snapshot file
 * are all it reads (the same rule as the widgets). It listens only while visible and is
 * told to refresh by [refresh] (META_DATA_ACTIVE_TILE).
 */
class StartMixTileService : TileService() {

    override fun onStartListening() = draw()

    override fun onClick() {
        when (val tap = currentView().tap) {
            is TileTap.StartMix -> startMix(tap.mixId)
            TileTap.OpenNowPlaying -> open(WidgetTarget.NOW_PLAYING)
            TileTap.OpenProfiles -> open(WidgetTarget.PROFILES)
            TileTap.OpenApp -> open(WidgetTarget.APP)
        }
    }

    private fun draw() {
        val tile = qsTile ?: return
        val view = currentView()
        val label = getString(R.string.tile_start_mix_label)
        tile.state = when (view.kind) {
            TileKind.INACTIVE -> Tile.STATE_INACTIVE
            TileKind.ACTIVE -> Tile.STATE_ACTIVE
            TileKind.UNAVAILABLE -> Tile.STATE_UNAVAILABLE
        }
        tile.icon = Icon.createWithResource(this, R.drawable.sym_graphic_eq)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            tile.label = label
            tile.subtitle = view.subtitle
        } else {
            // Android 9 has no subtitle: the state goes into the label.
            tile.label = StartMixTileModel.labelWithSubtitle(label, view.subtitle)
        }
        tile.contentDescription = StartMixTileModel.labelWithSubtitle(label, view.subtitle)
        tile.updateTile()
    }

    private fun currentView(): TileView {
        val state = WidgetConfig.profileState(this)
        val profile = WidgetConfig.resolve(WidgetProfileChoice.Active, state)
        val snapshot = profile?.let { SnapshotReader.read(SnapshotReader.widgetsDir(filesDir), it) }
        // In this process only: a started service has attached its player to the hub.
        val playing = PlaybackHub.isConnected.value?.state?.value?.isPlaying == true
        val name = getString(R.string.app_name)
        return StartMixTileModel.decide(
            state = state,
            snapshot = snapshot,
            playing = playing,
            deviceLocked = isLocked,
            words = TileWords(
                playing = getString(R.string.tile_playing),
                unlock = getString(R.string.tile_unlock, name),
                nextUp = getString(R.string.tile_next_up),
                openApp = getString(R.string.tile_open_app, name),
            ),
        )
    }

    private fun startMix(mixId: String?) {
        try {
            startForegroundService(WidgetIntents.playMix(this, mixId))
        } catch (e: IllegalStateException) {
            // ForegroundServiceStartNotAllowedException (Android 12+) is one: say it with the app.
            Log.w(TAG, "The foreground start was refused: ${e.javaClass.simpleName}")
            open(WidgetTarget.NOW_PLAYING)
        } catch (e: SecurityException) {
            Log.w(TAG, "The start was refused: ${e.javaClass.simpleName}")
            open(WidgetTarget.NOW_PLAYING)
        }
    }

    @SuppressLint("StartActivityAndCollapseDeprecated")
    @Suppress("DEPRECATION")
    private fun open(target: WidgetTarget) {
        val intent = WidgetIntents.open(this, target).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            // Required when targeting Android 14 and later: the Intent overload throws.
            val pending = PendingIntent.getActivity(this, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
            startActivityAndCollapse(pending)
        } else {
            startActivityAndCollapse(intent)
        }
    }

    companion object {
        private const val TAG = "StartMixTile"

        fun component(context: Context): ComponentName = ComponentName(context, StartMixTileService::class.java)

        /**
         * Ask Android to bind the tile once and redraw it (it declares META_DATA_ACTIVE_TILE,
         * so it isn't bound otherwise). From WidgetSync when the snapshot changes and when
         * playback starts or stops. Never throws.
         */
        fun refresh(context: Context) {
            runCatching { TileService.requestListeningState(context, component(context)) }
        }
    }
}
