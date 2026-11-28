package uk.gcartee.encore.app.playback.tv

import android.content.Context
import android.net.wifi.WifiManager
import android.os.Bundle
import android.util.Log
import android.util.Xml
import androidx.annotation.OptIn
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import kotlin.math.roundToInt
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uk.gcartee.encore.core.QueueEntry
import uk.gcartee.encore.core.StreamInfo

/** The kinds of TV in Play on's "TVs" group (ADR android-0007's order). */
enum class TvKind { CAST, DLNA, ROKU }

/** A TV found on the Wi-Fi. */
data class TvDevice(
    val id: String,
    val name: String,
    val kind: TvKind,
    /** "Chromecast" or the Cast device's model; "Music only" for DLNA (mock-up 50, screen 1). */
    val subtitle: String,
    /** It has a screen (a Cast speaker or a DLNA player has none). */
    val video: Boolean,
)

/** The TV the music is playing on now, for Now Playing's band and TV volume (screen 2). */
data class ActiveTv(val id: String, val name: String, val kind: TvKind, val volume: Float, val canSetVolume: Boolean)

/** A TV that was asked to play and first needs setting up (a Roku). */
data class TvSetup(val id: String, val name: String, val needs: Set<Roku.Need>)

/**
 * Where CorePlayback plays while a TV has the music: the TV's player, and the TV's copy of
 * each song with the relay's addresses.
 */
interface TvTarget {
    val player: Player

    /** The TV's copy of [base] (the phone's item for [entry], built from [info]). */
    fun item(entry: QueueEntry, info: StreamInfo, base: MediaItem): MediaItem?

    /** The TV is playing: the session isn't idle. */
    fun touch()
}

/** Platforms that are a TV running the Encore TV app (CoreHost sends "android-tv" there). */
fun isTvPlatform(platform: String): Boolean {
    val p = platform.lowercase()
    return p == "android-tv" || p == "tv" || p.endsWith("-tv") || p.endsWith("tvos")
}

/**
 * Play on a TV (ADR android-0007, plan stages T1 to T3): finds TVs while Play on is open
 * (Google Cast and DLNA on the Wi-Fi), moves the music to one and back, and keeps the relay's
 * session for it. The playback service attaches itself ([attach]); screens use the state
 * flows and [playOn] / [playHere]. Main thread.
 */
@OptIn(UnstableApi::class)
object TvOutput {
    private const val TAG = "TvOutput"
    private const val SEARCH_MS = 3_000L
    /** Longer than the search's MX (2 s), which a Roku waits up to before answering. */
    private const val ROKU_SEARCH_MS = 3_000L

    /** An error's words without any address in them, for the log. */
    private fun masked(message: String?): String = (message ?: "").replace(Regex("""\d+\.\d+\.\d+\.\d+"""), "<address>").take(160)

    /** This phone's name ("Sam's phone"), which the Roku shows as where the music comes from. */
    private fun phoneName(context: Context): String =
        runCatching { android.provider.Settings.Global.getString(context.contentResolver, android.provider.Settings.Global.DEVICE_NAME) }
            .getOrNull()?.takeIf { it.isNotBlank() } ?: android.os.Build.MODEL
    private const val SEARCH_EVERY_MS = 12_000L

    /** What the playback service gives: the switch, and its data sources for the relay. */
    interface Host {
        val context: Context

        /** Makes [target] the player (null: this phone) at the same song and position. [play] null: as it was. */
        fun move(target: TvTarget?, play: Boolean?)

        /** A queue song for the relay, read the way the phone's own player reads it. */
        fun songSource(info: StreamInfo, type: String): PhoneRelay.Source

        /** The music is playing (or about to). */
        fun playing(): Boolean
    }

    private class Session(val device: TvDevice, val player: Player, val relay: PhoneRelay, val listener: Player.Listener)

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var host: Host? = null
    private var session: Session? = null
    private var watchers = 0
    private var searchJob: Job? = null
    private var castFound = emptyList<TvDevice>()
    private var dlnaFound = emptyList<TvDevice>()
    private var rokuFound = emptyList<TvDevice>()
    /** A Roku's ECP address (`http://<roku>:8060/`) by its id in [TvDevice]. */
    private val rokuById = HashMap<String, java.net.URL>()
    /** Rokus already asked their name, by ECP address. */
    private val rokuInfo = HashMap<String, Roku.Info?>()
    private val dlnaById = HashMap<String, DlnaDevice>()
    private val descriptions = HashMap<String, DlnaDevice?>()

    private val _devices = MutableStateFlow<List<TvDevice>>(emptyList())
    /** The TVs found, Cast first. */
    val devices: StateFlow<List<TvDevice>> = _devices.asStateFlow()

    private val _searching = MutableStateFlow(false)
    /** A search is running and nothing has been found yet. */
    val searching: StateFlow<Boolean> = _searching.asStateFlow()

    private val _onWifi = MutableStateFlow(true)
    /** False when the phone isn't on Wi-Fi, so no TV can be reached. */
    val onWifi: StateFlow<Boolean> = _onWifi.asStateFlow()

    private val _active = MutableStateFlow<ActiveTv?>(null)
    val active: StateFlow<ActiveTv?> = _active.asStateFlow()

    private val _setup = MutableStateFlow<TvSetup?>(null)
    /** The TV last asked for that can't play until it is set up, for Play on to explain. */
    val setup: StateFlow<TvSetup?> = _setup.asStateFlow()

    private val _connecting = MutableStateFlow<String?>(null)
    /** The TV being connected to, for the row's progress. */
    val connecting: StateFlow<String?> = _connecting.asStateFlow()

    /** From PlaybackService.onCreate. */
    fun attach(h: Host) {
        host = h
    }

    /** The playback service stops: the session ends where it is (ADR: the pass ends with it). */
    fun detach() {
        session?.let {
            session = null
            _active.value = null
            finish(it)
        }
        host = null
        _connecting.value = null
    }

    // --- Finding TVs while Play on is open. ---

    fun startDiscovery(context: Context) {
        if (uk.gcartee.encore.app.ui.tv.isTv(context)) return // Encore TV doesn't cast to TVs
        watchers++
        if (watchers > 1) return
        val app = context.applicationContext
        _searching.value = _devices.value.isEmpty()
        CastTvs.discover(app) { found ->
            castFound = found.map { tv ->
                TvDevice("cast:${tv.routeId}", tv.name, TvKind.CAST, tv.model ?: "Chromecast", tv.video)
            }
            publish()
        }
        searchJob = scope.launch {
            while (isActive) {
                val found = withContext(Dispatchers.IO) {
                    runCatching { searchDlna(app) }
                        .onFailure { Log.i(TAG, "the TV search failed: ${it.javaClass.simpleName}") }
                        .getOrDefault(emptyList())
                }
                dlnaFound = found.filter { it.kind == TvKind.DLNA }
                rokuFound = found.filter { it.kind == TvKind.ROKU }
                publish()
                _searching.value = false
                delay(SEARCH_EVERY_MS)
            }
        }
    }

    fun stopDiscovery() {
        if (watchers == 0) return
        watchers--
        if (watchers > 0) return
        searchJob?.cancel()
        searchJob = null
        CastTvs.stopDiscovery()
        _searching.value = false
        _setup.value = null
    }

    private fun publish() {
        _devices.value = (castFound + rokuFound + dlnaFound).distinctBy { it.id }
        if (_devices.value.isNotEmpty()) _searching.value = false
    }

    /** One SSDP search for renderers, then each new one's description (blocking). */
    private fun searchDlna(context: Context): List<TvDevice> {
        val wifi = wifiAddress(context)
        _onWifi.value = wifi != null
        if (wifi == null) return emptyList()
        val manager = context.applicationContext.getSystemService(WifiManager::class.java) ?: return emptyList()
        val lock = manager.createMulticastLock("Encore TV search").apply { setReferenceCounted(false) }
        val (hits, rokus) = try {
            lock.acquire()
            Ssdp.search(wifi, SEARCH_MS) to Ssdp.search(wifi, ROKU_SEARCH_MS, Roku.SEARCH_TARGET)
        } finally {
            if (lock.isHeld) lock.release()
        }
        // Counts only, never addresses: what the search found on this Wi-Fi.
        Log.i(TAG, "TV search: Wi-Fi network ${LocalNet.wifi != null}, private address ${LocalNetwork.isLocalHost(wifi.hostAddress ?: "")}, renderers ${hits.size}, Rokus ${rokus.size}")
        val out = ArrayList<TvDevice>()
        for (hit in hits) {
            val key = hit.location.toString()
            val device = if (descriptions.containsKey(key)) {
                descriptions[key]
            } else {
                runCatching { DlnaDescription.parse(HttpSoap.get(hit.location), hit.location, Xml.newPullParser()) }
                    .onFailure { Log.i(TAG, "a renderer's description couldn't be read: ${it.javaClass.simpleName}") }
                    .getOrNull()
                    .also { descriptions[key] = it }
            } ?: continue
            val id = "dlna:${device.id}"
            synchronized(dlnaById) { dlnaById[id] = device }
            out += TvDevice(id, device.name, TvKind.DLNA, "Music only", video = false)
        }
        // Rokus (plan T5): the Encore channel shows the song; a Roku that is also a DLNA
        // renderer is listed once, as a Roku.
        for (hit in rokus) {
            val base = java.net.URL(hit.location, "/")
            val key = base.toString()
            val info = if (rokuInfo.containsKey(key)) {
                rokuInfo[key]
            } else {
                runCatching { Roku.info(base) }
                    .onFailure { Log.i(TAG, "a Roku's details couldn't be read: ${it.javaClass.simpleName} ${masked(it.message)}") }
                    .getOrNull()
                    .also { rokuInfo[key] = it }
            } ?: continue
            val id = "roku:${info.id}"
            synchronized(rokuById) { rokuById[id] = base }
            out.removeAll { it.kind == TvKind.DLNA && it.name == info.name }
            out += TvDevice(id, info.name, TvKind.ROKU, "Roku · Encore's screen", video = true)
        }
        return out
    }

    // --- Moving the music. ---

    /** Moves the music to [device]; [onFailure] gets a sentence to show. */
    fun playOn(device: TvDevice, onFailure: (String) -> Unit) {
        val h = host ?: return onFailure("Play something first, then choose a TV")
        if (session?.device?.id == device.id || _connecting.value != null) return
        val wasPlaying = h.playing()
        _setup.value = null
        _connecting.value = device.id
        scope.launch {
            fun fail(message: String) {
                _connecting.value = null
                onFailure(message)
            }
            val wifi = withContext(Dispatchers.IO) { wifiAddress(h.context) }
            if (wifi == null) return@launch fail("Connect to Wi-Fi to play on a TV")
            // A Roku says whether it can play at all; ask before the music leaves where it is.
            if (device.kind == TvKind.ROKU) {
                val base = synchronized(rokuById) { rokuById[device.id] }
                val needs = base?.let { withContext(Dispatchers.IO) { runCatching { Roku.check(it) }.getOrNull() } }
                    ?: return@launch fail("${device.name} can't be reached")
                if (needs.isNotEmpty()) {
                    Log.i(TAG, "the Roku isn't set up yet: ${needs.joinToString()}")
                    _connecting.value = null
                    _setup.value = TvSetup(device.id, device.name, needs)
                    return@launch
                }
            }
            // From one TV to another: back here (paused) first, then on to the new one.
            if (session != null) playHere(play = false)
            val relay = withContext(Dispatchers.IO) {
                runCatching { PhoneRelay(wifi, log = { Log.i(TAG, it) }).start() }.getOrNull()
            } ?: return@launch fail("Encore couldn't share the music on this Wi-Fi")
            when (device.kind) {
                TvKind.DLNA -> {
                    val dlna = synchronized(dlnaById) { dlnaById[device.id] }
                    if (dlna == null) {
                        relay.close()
                        return@launch fail("${device.name} can't be reached")
                    }
                    val renderer = DlnaRenderer(dlna, HttpSoap) { Xml.newPullParser() }
                    val player = TvPlayer(renderer, onLost = { lost(device.id) }, onRefused = { refused(device, onFailure) })
                    begin(device, player, relay, wasPlaying)
                }
                TvKind.ROKU -> {
                    val base = synchronized(rokuById) { rokuById[device.id] }
                    if (base == null) {
                        relay.close()
                        return@launch fail("${device.name} can't be reached")
                    }
                    val renderer = RokuRenderer(base, device.name, from = phoneName(h.context))
                    val player = TvPlayer(renderer, onLost = { lost(device.id) }, onRefused = { refused(device, onFailure) })
                    begin(device, player, relay, wasPlaying)
                }
                TvKind.CAST -> CastTvs.connect(
                    h.context,
                    device.id.removePrefix("cast:"),
                    onConnected = { player -> begin(device, player, relay, wasPlaying) },
                    onFailed = {
                        relay.close()
                        fail("${device.name} didn't answer")
                    },
                    onEnded = { ended(device.id) },
                )
            }
        }
    }

    private fun begin(device: TvDevice, player: Player, relay: PhoneRelay, play: Boolean) {
        val h = host
        if (h == null) {
            relay.close()
            if (device.kind != TvKind.CAST) player.release()
            _connecting.value = null
            return
        }
        relay.onIdle = { scope.launch { if (session?.relay === relay) playHere(play = false) } }
        h.move(RelayTarget(h, player, relay), play)
        val listener = object : Player.Listener {
            override fun onDeviceVolumeChanged(volume: Int, muted: Boolean) = showVolume(player)
            override fun onDeviceInfoChanged(deviceInfo: androidx.media3.common.DeviceInfo) = showVolume(player)
            override fun onPlaybackStateChanged(playbackState: Int) {
                // The queue ran out or was cleared: the music stops, and so does the session.
                if (playbackState == Player.STATE_ENDED || playbackState == Player.STATE_IDLE) {
                    scope.launch {
                        val s = session ?: return@launch
                        if (s.player !== player) return@launch
                        val stopped = player.playbackState == Player.STATE_IDLE && player.mediaItemCount == 0
                        if (player.playbackState == Player.STATE_ENDED || stopped) playHere(play = false)
                    }
                }
            }
        }
        player.addListener(listener)
        session = Session(device, player, relay, listener)
        _connecting.value = null
        showVolume(player)
        Log.i(TAG, "playing on a ${device.kind.name.lowercase()} TV")
    }

    private fun showVolume(player: Player) {
        val s = session ?: return
        if (s.player !== player) return
        val max = player.deviceInfo.maxVolume.coerceAtLeast(1)
        val min = player.deviceInfo.minVolume
        _active.value = ActiveTv(
            id = s.device.id,
            name = s.device.name,
            kind = s.device.kind,
            volume = ((player.deviceVolume - min).toFloat() / (max - min).coerceAtLeast(1)).coerceIn(0f, 1f),
            canSetVolume = player.isCommandAvailable(Player.COMMAND_SET_DEVICE_VOLUME_WITH_FLAGS),
        )
    }

    /** The TV's volume, 0 to 1 (Now Playing's slider). */
    fun setVolume(level: Float) {
        val s = session ?: return
        val info = s.player.deviceInfo
        val max = info.maxVolume.coerceAtLeast(1)
        val value = (info.minVolume + level.coerceIn(0f, 1f) * (max - info.minVolume)).roundToInt()
        if (s.player.isCommandAvailable(Player.COMMAND_SET_DEVICE_VOLUME_WITH_FLAGS)) s.player.setDeviceVolume(value, 0)
        _active.value = _active.value?.copy(volume = level.coerceIn(0f, 1f))
    }

    /** Brings the music back to this phone at the TV's position. [play] null: as it was. */
    fun playHere(play: Boolean? = null) {
        val s = session ?: return
        session = null
        _active.value = null
        s.player.removeListener(s.listener)
        host?.move(null, play)
        finish(s)
    }

    /** The TV stopped answering (DLNA): the music comes back here, paused. */
    private fun lost(id: String) {
        if (session?.device?.id == id) playHere(play = false)
    }

    /** The TV wouldn't take the first song: the music comes back here, paused, and says so. */
    private fun refused(device: TvDevice, onFailure: (String) -> Unit) {
        if (session?.device?.id != device.id) return
        playHere(play = false)
        onFailure("${device.name} wouldn't start the music, so it's back here, paused.")
    }

    /** The Cast session ended from elsewhere (the TV's remote, Google Home). */
    private fun ended(id: String) {
        if (session?.device?.id == id) playHere(play = false)
    }

    private fun finish(s: Session) {
        s.player.removeListener(s.listener)
        s.relay.close()
        when (s.device.kind) {
            TvKind.DLNA, TvKind.ROKU -> s.player.release()
            TvKind.CAST -> host?.context?.let(CastTvs::disconnect)
        }
    }

    /** A handoff moved the music to another Encore device: this TV's session ends. */
    fun handedOff() {
        if (session != null) playHere(play = false)
    }

    /** The relay's copy of each queue song: one number per queue entry, for this session only. */
    private class RelayTarget(private val host: Host, override val player: Player, private val relay: PhoneRelay) : TvTarget {
        private val numbers = HashMap<Long, Int>()

        override fun item(entry: QueueEntry, info: StreamInfo, base: MediaItem): MediaItem? {
            if (relay.closed) return null
            val type = guessAudioType(entry.song.container, info)
            val n = numbers.getOrPut(entry.uid) {
                relay.add(host.songSource(info, type), CoverSource(host.context.applicationContext, entry.song.id))
            }
            val metadata = base.mediaMetadata.buildUpon()
                .setExtras(Bundle().apply { putString(TvPlayer.EXTRA_COVER, relay.artUrl(n)) })
                .setMediaType(MediaMetadata.MEDIA_TYPE_MUSIC)
                .build()
            return base.buildUpon()
                .setUri(relay.songUrl(n))
                .setMimeType(type)
                .setCustomCacheKey(null)
                .setMediaMetadata(metadata)
                .build()
        }

        override fun touch() = relay.touch()
    }
}
