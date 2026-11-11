package uk.gcartee.encore.app

import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.ui.EncoreRoot
import uk.gcartee.encore.app.ui.theme.EncoreTheme

class MainActivity : ComponentActivity() {
    private lateinit var host: CoreHost

    override fun onCreate(savedInstanceState: Bundle?) {
        // Edge to edge cannot be turned off at API 36; every screen reads its insets. The
        // navigation bar's background follows the navigation mode
        // (NavigationBarForNavigationMode): clear for gestures, the system scrim for buttons.
        enableEdgeToEdge(
            navigationBarStyle = SystemBarStyle.auto(android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT),
        )
        super.onCreate(savedInstanceState)
        clearSystemBars()
        host = CoreHost.get(this)
        // Later launches go straight to the library (board 22, "First launch only").
        if (!host.profileOpen.value) host.openLastProfile()
        if (intent?.getStringExtra(EXTRA_OPEN) == OPEN_DOWNLOADS) host.openRequest.value = OPEN_DOWNLOADS
        if (intent?.getStringExtra(EXTRA_OPEN) == OPEN_SLEEP_TIMER) host.openRequest.value = OPEN_SLEEP_TIMER
        openFromWidget(intent) // Phase 9 (WP6)
        playFromSearch(intent)
        setContent {
            EncoreTheme {
                EncoreRoot(host)
            }
        }
    }

    /** Again after a rotation (this activity handles it itself): clear bar colours. */
    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        clearSystemBars()
    }

    @Suppress("DEPRECATION")
    private fun clearSystemBars() {
        window.navigationBarColor = android.graphics.Color.TRANSPARENT
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) window.navigationBarDividerColor = android.graphics.Color.TRANSPARENT
    }

    override fun onStart() {
        super.onStart()
        if (host.profileOpen.value) PlaybackHub.connect(this)
        host.sync.onForeground()
    }

    /** The download notification opens Settings › Downloads. */
    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        if (intent.getStringExtra(EXTRA_OPEN) == OPEN_DOWNLOADS) host.openRequest.value = OPEN_DOWNLOADS
        if (intent.getStringExtra(EXTRA_OPEN) == OPEN_SLEEP_TIMER) host.openRequest.value = OPEN_SLEEP_TIMER
        openFromWidget(intent) // Phase 9 (WP6)
        playFromSearch(intent)
    }

    // --- Phase 9 (WP6): a widget's cover or text opens Now Playing; a widget set to
    // another profile, or one with a PIN, opens the profile switcher (EncoreRoot handles
    // both requests). ---
    private fun openFromWidget(intent: android.content.Intent?) {
        // Phase 9 (WP8): touch and hold on the Start a mix tile opens the Mixes tab.
        if (intent?.action == ACTION_QS_TILE_PREFERENCES) host.openRequest.value = OPEN_MIXES
        when (val open = intent?.getStringExtra(EXTRA_OPEN)) {
            OPEN_NOW_PLAYING, OPEN_PROFILES -> host.openRequest.value = open
            // Phase 9 (WP7): a Recently played cover. A request without a valid album or
            // profile ID opens the app and nothing else.
            OPEN_ALBUM -> if (intent?.hasExtra(EXTRA_ALBUM_ID) == true) {
                val profileId = intent.getStringExtra(EXTRA_PROFILE_ID)
                if (profileId != null) {
                    val request = uk.gcartee.encore.app.widgets.AlbumOpenRequest.encode(intent.getLongExtra(EXTRA_ALBUM_ID, 0L), profileId)
                    if (uk.gcartee.encore.app.widgets.AlbumOpenRequest.parse(request) != null) host.openRequest.value = request
                }
            }
        }
    }
    // --- End Phase 9 (WP6). ---

    /** "Play <name> on Encore" from the assistant (Phase 8): the core queues it, then it plays. */
    private fun playFromSearch(intent: android.content.Intent?) {
        if (intent?.action != android.provider.MediaStore.INTENT_ACTION_MEDIA_PLAY_FROM_SEARCH) return
        if (!host.profileOpen.value) return
        val query = intent.getStringExtra(android.app.SearchManager.QUERY)
        if (uk.gcartee.encore.app.playback.VoicePlay.queue(host, query)) PlaybackHub.loadCurrent(this)
    }

    companion object {
        const val EXTRA_OPEN = "uk.gcartee.encore.app.OPEN"
        const val OPEN_DOWNLOADS = "downloads"

        /** The sleep timer's notification opens the sleep timer sheet (EncoreRoot). */
        const val OPEN_SLEEP_TIMER = "sleep-timer"

        /** Phase 9 (WP8): the Start a mix tile's touch and hold (EncoreRoot opens the Mixes tab). */
        const val OPEN_MIXES = "mixes"
        private const val ACTION_QS_TILE_PREFERENCES = "android.service.quicksettings.action.QS_TILE_PREFERENCES"

        /** Phase 9 (WP6): the Now Playing widget's cover or text. */
        const val OPEN_NOW_PLAYING = "now-playing"

        /** Phase 9 (WP6): a widget set to another profile or one with a PIN. */
        const val OPEN_PROFILES = "profiles"

        /** Phase 9 (WP7): a Recently played cover opens that album (with [EXTRA_ALBUM_ID], [EXTRA_PROFILE_ID]). */
        const val OPEN_ALBUM = "album"
        const val EXTRA_ALBUM_ID = "uk.gcartee.encore.app.ALBUM_ID"
        const val EXTRA_PROFILE_ID = "uk.gcartee.encore.app.PROFILE_ID"
    }

    override fun onStop() {
        // Music that is playing keeps the service in the foreground on its own.
        PlaybackHub.disconnect()
        host.sync.onBackground()
        super.onStop()
    }
}
