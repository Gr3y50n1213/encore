package uk.gcartee.encore.app.playback

import android.content.ComponentName
import android.content.Context
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import uk.gcartee.encore.core.SkipOrigin

/**
 * The app's screens reach the playback service through here. The service runs in the
 * same process, so screens call it directly; the MediaController exists only to start
 * and bind the service, as Media3 expects.
 */
object PlaybackHub {
    private val playback = MutableStateFlow<CorePlayback?>(null)
    private var controller: ListenableFuture<MediaController>? = null

    @OptIn(ExperimentalCoroutinesApi::class)
    val state: Flow<PlayerState> = playback.flatMapLatest { it?.state ?: flowOf(PlayerState()) }

    val isConnected: StateFlow<CorePlayback?> = playback.asStateFlow()

    internal fun attach(p: CorePlayback) {
        playback.value = p
    }

    internal fun detach() {
        playback.value = null
    }

    /** Starts and binds the service; call from the activity's onStart. */
    fun connect(context: Context) {
        if (controller != null) return
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        controller = MediaController.Builder(context, token).buildAsync()
    }

    /** From the activity's onStop: playing music keeps the service in the foreground. */
    fun disconnect() {
        controller?.let(MediaController::releaseFuture)
        controller = null
    }

    /** Runs [action] once the service is up; at once when it already is. */
    fun withPlayback(context: Context, action: (CorePlayback) -> Unit) {
        playback.value?.let {
            action(it)
            return
        }
        connect(context)
        val future = controller ?: return
        future.addListener({ playback.value?.let(action) }, context.mainExecutor)
    }

    fun playCurrent(context: Context) = withPlayback(context) { it.playCurrent() }
    fun togglePlay(context: Context) = withPlayback(context) { it.togglePlay() }
    fun next(context: Context) = withPlayback(context) { it.next(SkipOrigin.BUTTON) }
    fun previous(context: Context) = withPlayback(context) { it.previous(SkipOrigin.BUTTON) }
    fun seekTo(context: Context, ms: Long) = withPlayback(context) { it.seekTo(ms) }

    /** Board 42: a completed swipe is a skip exactly like the button, with origin Swipe. */
    fun swipeNext(context: Context) = withPlayback(context) { it.next(SkipOrigin.SWIPE) }
    fun swipePrevious(context: Context) = withPlayback(context) { it.previous(SkipOrigin.SWIPE) }
    fun startScan(context: Context, forward: Boolean) = withPlayback(context) { it.startScan(forward) }
    fun stopScan(context: Context) = withPlayback(context) { it.stopScan() }
}
