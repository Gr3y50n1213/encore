package uk.gcartee.encore.wear

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import kotlinx.coroutines.flow.MutableStateFlow

/**
 * Mock-up 50, screen 4: Now Playing on the watch, opened from the app list or the
 * complication. Everything comes from the phone (WatchStore); a locked watch shows only
 * "Encore" and the controls.
 */
class MainActivity : ComponentActivity() {
    private val locked = MutableStateFlow(false)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        WatchStore.load(this)
        setContent {
            EncoreWatchTheme {
                NowPlayingApp(
                    locked = locked,
                    checkLocked = { locked.value = watchLocked(this) },
                    send = { command ->
                        WatchStore.expect(command)
                        PhoneLink.send(this, command)
                    },
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        locked.value = watchLocked(this)
        WatchStore.refresh(this)
    }
}
