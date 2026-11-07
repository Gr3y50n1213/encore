package uk.gcartee.encore.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.ui.EncoreRoot
import uk.gcartee.encore.app.ui.theme.EncoreTheme

class MainActivity : ComponentActivity() {
    private lateinit var host: CoreHost

    override fun onCreate(savedInstanceState: Bundle?) {
        // Edge to edge cannot be turned off at API 36; every screen reads its insets.
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        host = CoreHost.get(this)
        // Later launches go straight to the library (board 22, "First launch only").
        if (!host.profileOpen.value) host.openLastProfile()
        setContent {
            EncoreTheme {
                EncoreRoot(host)
            }
        }
    }

    override fun onStart() {
        super.onStart()
        if (host.profileOpen.value) PlaybackHub.connect(this)
    }

    override fun onStop() {
        // Music that is playing keeps the service in the foreground on its own.
        PlaybackHub.disconnect()
        super.onStop()
    }
}
