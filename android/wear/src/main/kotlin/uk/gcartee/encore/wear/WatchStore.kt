package uk.gcartee.encore.wear

import android.app.KeyguardManager
import android.content.ComponentName
import android.content.Context
import android.content.SharedPreferences
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.Log
import androidx.wear.tiles.TileService
import androidx.wear.watchface.complications.datasource.ComplicationDataSourceUpdateRequester
import com.google.android.gms.tasks.Tasks
import com.google.android.gms.wearable.DataItem
import com.google.android.gms.wearable.DataMapItem
import com.google.android.gms.wearable.Wearable
import java.io.File
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uk.gcartee.encore.wearlink.Mixes
import uk.gcartee.encore.wearlink.MixesCodec
import uk.gcartee.encore.wearlink.NowCodec
import uk.gcartee.encore.wearlink.NowPlaying
import uk.gcartee.encore.wearlink.Paths
import uk.gcartee.encore.wearlink.WearCommand
import uk.gcartee.encore.wearlink.WearGms
import uk.gcartee.encore.wearlink.WireValues
import uk.gcartee.encore.wearlink.awaitOrNull

/** The watch is locked (the keyguard): nothing personal is shown (ADR android-0008). */
fun watchLocked(context: Context): Boolean =
    context.getSystemService(KeyguardManager::class.java)?.isDeviceLocked == true

/**
 * What the phone last sent, kept in the watch app's own storage so the tile and the
 * complication can draw at once, and as flows for Now Playing. Written by
 * [WatchDataService] when a DataItem changes, and read again from the Data Layer when Now
 * Playing opens. The cover is one small JPEG file, replaced with each song.
 */
object WatchStore {
    private const val TAG = "WatchStore"
    private const val PREFS = "wear-link"
    private const val NOW = "now."
    private const val MIXES = "mixes."
    private const val PHONE = "phone-node"
    private const val COVER_FILE = "cover.jpg"
    private const val ASSET_TIMEOUT_S = 10L

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    @Volatile private var loaded = false

    private val _now = MutableStateFlow<NowPlaying?>(null)
    val now: StateFlow<NowPlaying?> = _now.asStateFlow()

    private val _mixes = MutableStateFlow(Mixes.EMPTY)
    val mixes: StateFlow<Mixes> = _mixes.asStateFlow()

    /** Bumped when the cover file changes. */
    private val _cover = MutableStateFlow(0)
    val coverVersion: StateFlow<Int> = _cover.asStateFlow()

    private fun prefs(context: Context): SharedPreferences =
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /** Reads what was saved, once per process. */
    fun load(context: Context) {
        if (loaded) return
        synchronized(this) {
            if (loaded) return
            val p = prefs(context)
            _now.value = NowCodec.decode(prefValues(p, NOW))
            _mixes.value = MixesCodec.decode(prefValues(p, MIXES)) ?: Mixes.EMPTY
            loaded = true
        }
    }

    /** The node the phone's DataItems came from: where commands go when the capability isn't found. */
    fun phoneNode(context: Context): String? = prefs(context).getString(PHONE, null)

    fun cover(context: Context): Bitmap? {
        val file = File(context.filesDir, COVER_FILE)
        return if (file.isFile) runCatching { BitmapFactory.decodeFile(file.path) }.getOrNull() else null
    }

    /** Reads the phone's DataItems again (Now Playing opening), off the main thread. */
    fun refresh(context: Context) {
        val app = context.applicationContext
        if (!WearGms.available(app)) return
        scope.launch {
            val buffer = Wearable.getDataClient(app).dataItems.awaitOrNull() ?: return@launch
            try {
                buffer.forEach { ingest(app, it) }
            } finally {
                buffer.release()
            }
        }
    }

    /** A changed DataItem, from [WatchDataService] or [refresh]. Blocks on the cover: never on the main thread. */
    fun ingest(context: Context, item: DataItem) {
        load(context)
        val path = item.uri.path ?: return
        val map = runCatching { DataMapItem.fromDataItem(item).dataMap }.getOrNull() ?: return
        when (path) {
            Paths.NOW -> {
                val now = NowCodec.decode(WearGms.values(map)) ?: return
                val asset = map.getAsset(NowCodec.COVER)
                val bytes = asset?.let {
                    runCatching {
                        val fd = Tasks.await(Wearable.getDataClient(context).getFdForAsset(it), ASSET_TIMEOUT_S, TimeUnit.SECONDS)
                        fd.inputStream.use { stream -> stream.readBytes() }
                    }.onFailure { e -> Log.w(TAG, "No cover: ${e.javaClass.simpleName}") }.getOrNull()
                }
                saveCover(context, bytes)
                val p = prefs(context)
                p.edit().also { e ->
                    clear(p, e, NOW)
                    putAll(e, NOW, NowCodec.encode(now))
                    item.uri.host?.let { e.putString(PHONE, it) }
                }.apply()
                val before = _now.value
                _now.value = now
                if (before == null || before.title != now.title || before.artist != now.artist || before.isPlaying != now.isPlaying || before.isPrivate != now.isPrivate) {
                    updateComplication(context)
                }
            }
            Paths.MIXES -> {
                val mixes = MixesCodec.decode(WearGms.values(map)) ?: return
                val p = prefs(context)
                p.edit().also { e ->
                    clear(p, e, MIXES)
                    putAll(e, MIXES, MixesCodec.encode(mixes))
                }.apply()
                _mixes.value = mixes
                updateTile(context)
            }
        }
    }

    /** The phone deleted an item (it never does now): forget it. */
    fun forget(context: Context, path: String?) {
        load(context)
        val p = prefs(context)
        when (path) {
            Paths.NOW -> {
                p.edit().also { clear(p, it, NOW) }.apply()
                saveCover(context, null)
                _now.value = null
                updateComplication(context)
            }
            Paths.MIXES -> {
                p.edit().also { clear(p, it, MIXES) }.apply()
                _mixes.value = Mixes.EMPTY
                updateTile(context)
            }
        }
    }

    /**
     * Shown at once after a tap, before the phone's own update arrives (which then wins):
     * Play and Pause flip, Like and Unlike set the heart.
     */
    fun expect(command: WearCommand) {
        val n = _now.value ?: return
        val at = System.currentTimeMillis()
        _now.value = when (command) {
            WearCommand.Play -> n.copy(isPlaying = true, positionMs = n.positionAt(at), positionAtUtcMs = at)
            WearCommand.Pause -> n.copy(isPlaying = false, positionMs = n.positionAt(at), positionAtUtcMs = at)
            WearCommand.Like -> n.copy(liked = true)
            WearCommand.Unlike -> n.copy(liked = false)
            else -> n
        }
    }

    private fun saveCover(context: Context, bytes: ByteArray?) {
        val file = File(context.filesDir, COVER_FILE)
        runCatching {
            if (bytes == null) {
                if (file.exists()) file.delete()
            } else {
                val tmp = File(context.filesDir, "$COVER_FILE.tmp")
                tmp.writeBytes(bytes)
                tmp.renameTo(file)
            }
        }
        _cover.value++
    }

    private fun updateTile(context: Context) {
        runCatching { TileService.getUpdater(context).requestUpdate(MixesTileService::class.java) }
    }

    private fun updateComplication(context: Context) {
        runCatching {
            ComplicationDataSourceUpdateRequester
                .create(context, ComponentName(context, NowPlayingComplicationService::class.java))
                .requestUpdateAll()
        }
    }

    private fun clear(p: SharedPreferences, e: SharedPreferences.Editor, prefix: String) {
        p.all.keys.filter { it.startsWith(prefix) }.forEach(e::remove)
    }

    private fun putAll(e: SharedPreferences.Editor, prefix: String, values: Map<String, Any>) {
        for ((key, value) in values) {
            when (value) {
                is String -> e.putString(prefix + key, value)
                is Long -> e.putLong(prefix + key, value)
                is Boolean -> e.putBoolean(prefix + key, value)
            }
        }
    }

    private fun prefValues(p: SharedPreferences, prefix: String): WireValues = object : WireValues {
        private val all = p.all
        override fun string(key: String): String? = all[prefix + key] as? String
        override fun long(key: String): Long? = all[prefix + key] as? Long
        override fun boolean(key: String): Boolean? = all[prefix + key] as? Boolean
    }
}
