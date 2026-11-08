package uk.gcartee.encore.app.core

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.playback.RollingCache

/**
 * One watcher of the network for the whole app (Phase 8). It tells the core
 * (`set_connectivity`, with the player's fully cached songs) and drives the offline
 * banner (board 20, item 1: in after 2 s without a network, out 2 s after it comes
 * back). Both use the same settled state, so what dims and what the banner says
 * always agree.
 *
 * "Offline" is no network at all (airplane mode, no signal). A network without the
 * internet still reaches a server at home, so it counts as online.
 */
class Connectivity(private val context: Context, private val host: CoreHost) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val cm = context.getSystemService(ConnectivityManager::class.java)
    private val _offline = MutableStateFlow(false)
    /** Offline, settled (2 s each way). */
    val offline: StateFlow<Boolean> = _offline.asStateFlow()
    private var raw = true
    private var settle: Job? = null
    private var started = false

    fun start() {
        if (started) return
        started = true
        raw = hasNetwork()
        _offline.value = !raw
        report()
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) = changed()
            override fun onLost(network: Network) = changed()
            override fun onCapabilitiesChanged(network: Network, caps: NetworkCapabilities) = changed()
        }
        runCatching { cm?.registerDefaultNetworkCallback(callback) }
    }

    private fun hasNetwork(): Boolean {
        val caps = cm?.getNetworkCapabilities(cm.activeNetwork) ?: return false
        return caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
    }

    /** Called on a binder thread: settle on the main thread. */
    private fun changed() {
        scope.launch {
            val now = hasNetwork()
            if (now == raw) {
                // Wi-Fi to cellular and back: the core's quality and Wi-Fi-only rules.
                report()
                return@launch
            }
            raw = now
            settle?.cancel()
            settle = scope.launch {
                delay(SETTLE_MS)
                _offline.value = !raw
                report()
            }
        }
    }

    /** Tell the core what plays now; call again when the profile or the cache changes. */
    fun report() {
        val online = !_offline.value
        val cached = if (online) emptyList() else RollingCache.fullyCachedKeys(context)
        runCatching { host.core.setConnectivity(online, host.network(), cached) }
    }

    private companion object {
        const val SETTLE_MS = 2_000L
    }
}
