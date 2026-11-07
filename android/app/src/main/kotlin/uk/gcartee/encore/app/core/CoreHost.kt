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
import uk.gcartee.encore.core.Network
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

    private val _events = MutableSharedFlow<CoreEvent>(extraBufferCapacity = 256)
    val events: SharedFlow<CoreEvent> = _events.asSharedFlow()

    private val _profileOpen = MutableStateFlow(false)
    /** True once a profile is open: the app shows its library, not Welcome. */
    val profileOpen: StateFlow<Boolean> = _profileOpen.asStateFlow()

    init {
        core.setListener(object : CoreListener {
            override fun onEvent(event: CoreEvent) {
                // A rotated companion token must be stored before anything else: the
                // old one no longer works.
                if (event.kind == EventKind.CREDENTIAL_CHANGED) event.credential?.let(::storeCredential)
                _events.tryEmit(event)
            }
        })
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

    private fun profileOpened() {
        // ExoPlayer plays these as they are; the rest is transcoded to AAC. ALAC is left
        // out: Android has no platform decoder for it. The core keeps this per profile
        // (it refuses with NoProfile before one is open), so it is set on every opening.
        core.setPlaybackCapabilities(
            listOf("flac", "mp3", "m4a", "aac", "ogg", "opus", "wav"),
            TranscodeCodec.AAC,
        )
        _profileOpen.value = true
    }

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
