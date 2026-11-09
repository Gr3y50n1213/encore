package uk.gcartee.encore.app.playback

import android.content.Context
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.database.StandaloneDatabaseProvider
import androidx.media3.datasource.cache.Cache
import androidx.media3.datasource.cache.CacheEvictor
import androidx.media3.datasource.cache.CacheSpan
import androidx.media3.datasource.cache.ContentMetadata
import androidx.media3.datasource.cache.SimpleCache
import java.io.File
import java.util.TreeSet

/**
 * The player's rolling cache of streamed songs (plan section 8; Phase 8): songs are
 * kept as they play, the oldest go first past the size set in Settings › Downloads
 * (`DownloadSettings.cache_limit_bytes`, 1 GB by default), and a song held whole plays
 * offline too (board 20: "downloaded and already cached"). Downloads never go
 * through it: they are files of their own.
 *
 * One instance for the whole process (SimpleCache allows one per folder). Keys are
 * the core's `StreamInfo.cache_key`s, which carry no token.
 */
@OptIn(UnstableApi::class)
object RollingCache {
    @Volatile private var cache: SimpleCache? = null
    private val evictor = ResizableEvictor()

    fun get(context: Context): SimpleCache =
        cache ?: synchronized(this) {
            cache ?: SimpleCache(
                File(context.applicationContext.cacheDir, "media"),
                evictor,
                StandaloneDatabaseProvider(context.applicationContext),
            ).also { cache = it }
        }

    /** The size the person chose; 0 empties the cache and keeps nothing more. */
    fun setLimit(context: Context, bytes: Long) {
        evictor.maxBytes = bytes
        runCatching { evictor.trim(get(context)) }
    }

    fun usedBytes(context: Context): Long = runCatching { get(context).cacheSpace }.getOrDefault(0L)

    /** The songs held whole, for `set_connectivity` (a part of a song can't play offline). */
    fun fullyCachedKeys(context: Context): List<String> = runCatching {
        val c = get(context)
        c.keys.filter { key ->
            val length = ContentMetadata.getContentLength(c.getContentMetadata(key))
            length > 0 && c.isCached(key, 0, length)
        }
    }.getOrDefault(emptyList())

    /**
     * The song under [key] is held whole, its length known. A transcoded stream arrives
     * with no length (so the player can't seek it); opened again from here, it can.
     */
    fun isWhole(context: Context, key: String): Boolean = runCatching {
        val c = get(context)
        val length = ContentMetadata.getContentLength(c.getContentMetadata(key))
        length > 0 && c.isCached(key, 0, length)
    }.getOrDefault(false)

    fun clear(context: Context) {
        runCatching {
            val c = get(context)
            c.keys.toList().forEach(c::removeResource)
        }
    }
}

/**
 * Least recently used, like Media3's own evictor, with a size that can change while
 * the player runs (Settings changes apply at once, board 17).
 */
@OptIn(UnstableApi::class)
private class ResizableEvictor : CacheEvictor {
    @Volatile var maxBytes: Long = DEFAULT_BYTES
    private val spans = TreeSet<CacheSpan> { a, b ->
        val t = a.lastTouchTimestamp.compareTo(b.lastTouchTimestamp)
        if (t != 0) t else a.compareTo(b)
    }
    private var size = 0L

    override fun requiresCacheSpanTouches(): Boolean = true

    override fun onCacheInitialized() {}

    override fun onStartFile(cache: Cache, key: String, position: Long, length: Long) {
        if (length != androidx.media3.common.C.LENGTH_UNSET.toLong()) evict(cache, length)
    }

    override fun onSpanAdded(cache: Cache, span: CacheSpan) {
        synchronized(this) {
            spans.add(span)
            size += span.length
        }
        evict(cache, 0)
    }

    override fun onSpanRemoved(cache: Cache, span: CacheSpan) {
        synchronized(this) {
            if (spans.remove(span)) size -= span.length
        }
    }

    override fun onSpanTouched(cache: Cache, oldSpan: CacheSpan, newSpan: CacheSpan) {
        onSpanRemoved(cache, oldSpan)
        onSpanAdded(cache, newSpan)
    }

    fun trim(cache: Cache) = evict(cache, 0)

    private fun evict(cache: Cache, required: Long) {
        while (true) {
            val oldest = synchronized(this) {
                if (size + required <= maxBytes || spans.isEmpty()) null else spans.first()
            } ?: return
            runCatching { cache.removeSpan(oldest) }.onFailure {
                synchronized(this) { if (spans.remove(oldest)) size -= oldest.length }
            }
        }
    }

    private companion object {
        const val DEFAULT_BYTES = 1_000_000_000L
    }
}
