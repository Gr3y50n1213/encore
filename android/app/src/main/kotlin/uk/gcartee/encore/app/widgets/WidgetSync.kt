package uk.gcartee.encore.app.widgets

import androidx.glance.appwidget.updateAll
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.EventKind

/**
 * Keeps widgets current from the app's process. The only file in `widgets/` that touches
 * CoreHost or the core (WidgetSourceScanTest checks): it runs only where the core already
 * runs, started by CoreHost, and never starts it.
 *
 * On `WidgetSnapshotChanged` (which the core also sends when a profile opens, closes
 * on a switch, is removed, gets a PIN or turns Guest) it saves what widgets need to
 * choose a file without the core (the open profile, the last active one, the profile
 * names) and redraws every widget. Bursts are folded into one redraw.
 */
object WidgetSync {
    private const val DEBOUNCE_MS = 300L

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private var pending: Job? = null
    @Volatile private var attached = false

    /** From CoreHost, once the core exists. */
    fun attach(host: CoreHost) {
        if (attached) return
        attached = true
        scope.launch {
            host.events.collect { event ->
                if (event.kind == EventKind.WIDGET_SNAPSHOT_CHANGED) request(host)
            }
        }
        // Phase 9 (WP8): the Start a mix tile shows "Playing" while playback runs in this process.
        scope.launch {
            uk.gcartee.encore.app.playback.PlaybackHub.state
                .map { it.isPlaying }
                .distinctUntilChanged()
                .collect { uk.gcartee.encore.app.tile.StartMixTileService.refresh(host.context) }
        }
        request(host)
    }

    /** Redraw soon; also from the app after something widgets show changed outside the core. */
    fun request(host: CoreHost) {
        synchronized(this) {
            pending?.cancel()
            pending = scope.launch {
                delay(DEBOUNCE_MS)
                sync(host)
            }
        }
    }

    private suspend fun sync(host: CoreHost) {
        val core = host.core
        val state = ProfileState(
            current = runCatching { core.currentProfile() }.getOrNull(),
            lastActive = runCatching { core.lastActiveProfile() }.getOrNull(),
            profiles = runCatching { core.profiles() }.getOrDefault(emptyList())
                .map { KnownProfile(it.profileId, it.displayName, it.locked) },
        )
        WidgetConfig.saveProfileState(host.context, state)
        WidgetRedraws.bump()
        runCatching { NowPlayingWidget().updateAll(host.context) }
        runCatching { MixesWidget().updateAll(host.context) } // Phase 9 (WP7)
        runCatching { RecentWidget().updateAll(host.context) } // Phase 9 (WP7)
        uk.gcartee.encore.app.tile.StartMixTileService.refresh(host.context) // Phase 9 (WP8)
    }
}
