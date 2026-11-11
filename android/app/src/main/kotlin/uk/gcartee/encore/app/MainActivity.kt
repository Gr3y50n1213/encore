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
        playFromSearch(intent)
    }

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
    }

    override fun onStop() {
        // Music that is playing keeps the service in the foreground on its own.
        PlaybackHub.disconnect()
        host.sync.onBackground()
        super.onStop()
    }
}
