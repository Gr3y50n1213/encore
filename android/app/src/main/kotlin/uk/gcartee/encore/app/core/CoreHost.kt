package uk.gcartee.encore.app.core

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.os.Build
import android.provider.Settings
import java.io.File
import java.security.SecureRandom
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import uk.gcartee.encore.core.CoreConfig
import uk.gcartee.encore.core.CoreEvent
import uk.gcartee.encore.core.CoreListener
import uk.gcartee.encore.core.EncoreCore
import uk.gcartee.encore.core.EventKind
import uk.gcartee.encore.core.ListeningMode
import uk.gcartee.encore.core.Network
import uk.gcartee.encore.core.QueueState
import uk.gcartee.encore.core.SignedIn
import uk.gcartee.encore.core.SourceToken
import uk.gcartee.encore.core.TranscodeCodec

/**
 * The one core for the whole process: the app's screens and the playback service share
 * it. Events arrive on a core thread and are re-sent as a flow; collect them on the main
 * thread before touching UI (CORE-API, "Events").
 */
class CoreHost private constructor(val context: Context) {
    val tokens = TokenStore(context)
    val core: EncoreCore = EncoreCore(
        CoreConfig(
            dataDir = File(context.filesDir, "core").apply { mkdirs() }.absolutePath,
            deviceId = deviceId(context),
            deviceName = deviceName(context),
            platform = "android",
        ),
    )

    /** The library mirror's sync (sync_library), shared by every screen. */
    val sync: LibrarySync by lazy { LibrarySync(this) }

    /** Downloads' badges and offline dimming (Phase 8). */
    val downloads: Downloads by lazy { Downloads(this) }

    /** The network, settled, for the core and the offline banner (Phase 8). */
    val connectivity: Connectivity by lazy { Connectivity(context, this) }

    /** Board 25: ask for notifications in context, once, before the first download. */
    val notificationPrimer = MutableStateFlow(false)

    /** A screen another part of the app asked for (the download notification). */
    val openRequest = MutableStateFlow<String?>(null)

    private val _events = MutableSharedFlow<CoreEvent>(extraBufferCapacity = 256)
    val events: SharedFlow<CoreEvent> = _events.asSharedFlow()

    private val _queue = MutableStateFlow<QueueState?>(null)
    /** The live queue (board 11): updated by every `QueueChanged`. */
    val queue: StateFlow<QueueState?> = _queue.asStateFlow()

    /** "Playing from Home" on Now Playing (board 09): where the current list was started. */
    val playingFrom = MutableStateFlow<String?>(null)

    /** Private listening (board 09, More): the core records nothing while it is on. */
    val privateListening = MutableStateFlow(false)

    /** Bumped when another device's likes or playlists arrive (`RemoteChange`). */
    val remoteChanges = MutableStateFlow(0)

    fun refreshQueue() {
        _queue.value = runCatching { core.queue() }.getOrNull()
    }

    private val _profileOpen = MutableStateFlow(false)
    /** True once a profile is open: the app shows its library, not Welcome. */
    val profileOpen: StateFlow<Boolean> = _profileOpen.asStateFlow()

    init {
        core.setListener(object : CoreListener {
            override fun onEvent(event: CoreEvent) {
                // A rotated companion token must be stored before anything else: the
                // old one no longer works.
                if (event.kind == EventKind.CREDENTIAL_CHANGED) event.credential?.let(::storeCredential)
                if (event.kind == EventKind.QUEUE_CHANGED) event.queue?.let { _queue.value = it }
                if (event.kind == EventKind.REMOTE_CHANGE) remoteChanges.value++
                if (event.kind == EventKind.DOWNLOADS_CHANGED) downloads.refresh()
                if (event.kind == EventKind.DOWNLOAD_PROGRESS) event.download?.let(downloads::progressed)
                _events.tryEmit(event)
            }
        })
    }

    // Phase 9 (WP4): the sleep timer's card follows the timer for the whole process,
    // whether or not the player or a screen is up (playback/SleepTimerNotifier.kt).
    init {
        uk.gcartee.encore.app.playback.SleepTimerNotifier.start(context, this)
    }

    // Phase 9 (WP6): widgets redraw from their files when the core rewrites them, and learn
    // the open and last active profile here, so a widget never has to start the core
    // (widgets/WidgetSync.kt).
    init {
        uk.gcartee.encore.app.widgets.WidgetSync.attach(this)
    }

    /** At launch: opens the last profile with its tokens from the Keystore. */
    fun openLastProfile(): Boolean {
        val id = core.lastActiveProfile() ?: return false
        val profile = core.profiles().firstOrNull { it.profileId == id } ?: return false
        if (profile.locked) return false
        val sourceTokens = profile.credentials.mapNotNull { key ->
            tokens.read(key.key)?.let { SourceToken(key.sourceId, it) }
        }
        return runCatching { core.openProfile(id, sourceTokens) }
            .onSuccess { profileOpened() }
            .isSuccess
    }

    /** After sign-in: the token goes to the Keystore; the core already opened the profile. */
    fun signedIn(signed: SignedIn) {
        tokens.save(signed.credentialKey, signed.token)
        profileOpened()
    }

    /** Board 19, item 2: "Learn from my listening" off, kept per profile and applied when it opens. */
    fun learningOff(): Boolean = context.getSharedPreferences("privacy-${core.currentProfile().orEmpty()}", Context.MODE_PRIVATE).getBoolean("learning-off", false)

    fun setLearningOff(off: Boolean) {
        context.getSharedPreferences("privacy-${core.currentProfile().orEmpty()}", Context.MODE_PRIVATE).edit().putBoolean("learning-off", off).apply()
        runCatching { core.setListeningMode(if (off) ListeningMode.LEARNING_OFF else ListeningMode.NORMAL) }
    }

    private fun profileOpened() {
        uk.gcartee.encore.app.ui.theme.Appearance.load(context, core.currentProfile())
        if (learningOff()) runCatching { core.setListeningMode(ListeningMode.LEARNING_OFF) }
        // ExoPlayer plays these as they are; the rest is transcoded to AAC. ALAC is left
        // out: Android has no platform decoder for it. The core keeps this per profile
        // (it refuses with NoProfile before one is open), so it is set on every opening.
        core.setPlaybackCapabilities(
            listOf("flac", "mp3", "m4a", "aac", "ogg", "opus", "wav"),
            TranscodeCodec.AAC,
        )
        refreshQueue()
        _profileOpen.value = true
        afterProfileChange()
    }

    /**
     * Phase 8, after any profile opens: its cached songs and downloads, its rolling
     * cache size, and the downloads it was waiting on.
     */
    private fun afterProfileChange() {
        connectivity.start()
        connectivity.report()
        downloads.reset()
        runCatching { core.downloadSettings() }.onSuccess {
            uk.gcartee.encore.app.playback.RollingCache.setLimit(context, it.cacheLimitBytes.toLong())
        }
        runCatching { core.removeUnplayedDownloads() }
        uk.gcartee.encore.app.downloads.DownloadScheduler.resume(context)
    }

    /** Board 14: after a switch, the other profile's queue and settings load. */
    fun profileSwitched(queue: QueueState) {
        uk.gcartee.encore.app.ui.theme.Appearance.load(context, core.currentProfile())
        _queue.value = queue
        playingFrom.value = null
        privateListening.value = false
        core.setPlaybackCapabilities(listOf("flac", "mp3", "m4a", "aac", "ogg", "opus", "wav"), TranscodeCodec.AAC)
        afterProfileChange()
    }

    /** Signed out of every source, or the profile removed: back to Welcome. */
    fun profileClosed() {
        _queue.value = null
        _profileOpen.value = false
        downloads.reset()
    }

    /**
     * Opens the last profile when something other than the app's screen started the
     * process (Android Auto, a download job). False when there is none, or it is
     * locked (it opens only from the app).
     */
    fun ensureProfile(): Boolean = profileOpen.value || openLastProfile()

    private fun storeCredential(credential: SourceToken) {
        val profileId = core.currentProfile() ?: return
        val key = core.profiles().firstOrNull { it.profileId == profileId }
            ?.credentials?.firstOrNull { it.sourceId == credential.sourceId }?.key ?: return
        tokens.save(key, credential.token)
    }

    /** Wi-Fi quality on unmetered networks, cellular quality otherwise (CORE-API `stream_for`). */
    fun network(): Network {
        val cm = context.getSystemService(ConnectivityManager::class.java)
        val caps = cm?.getNetworkCapabilities(cm.activeNetwork)
        val unmetered = caps?.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED) == true
        return if (unmetered) Network.WIFI else Network.CELLULAR
    }

    companion object {
        @Volatile private var instance: CoreHost? = null

        fun get(context: Context): CoreHost =
            instance ?: synchronized(this) {
                instance ?: CoreHost(context.applicationContext).also { instance = it }
            }

        /** Stable per installation (CORE-API `CoreConfig.device_id`); not backed up. */
        private fun deviceId(context: Context): String {
            val file = File(context.noBackupFilesDir, "device-id")
            file.takeIf { it.isFile }?.readText()?.trim()?.takeIf { it.isNotEmpty() }?.let { return it }
            val bytes = ByteArray(16).also { SecureRandom().nextBytes(it) }
            val id = bytes.joinToString("") { "%02x".format(it) }
            file.writeText(id)
            return id
        }

        /** The name the owner gave the phone, as the companion's Devices page shows it. */
        private fun deviceName(context: Context): String =
            Settings.Global.getString(context.contentResolver, Settings.Global.DEVICE_NAME)
                ?.takeIf { it.isNotBlank() }
                ?: Build.MODEL
    }
}
