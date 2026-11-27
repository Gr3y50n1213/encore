package uk.gcartee.encore.app.recap

import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import androidx.core.content.FileProvider
import java.io.File
import java.util.Locale
import java.util.TimeZone
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.DropTarget
import uk.gcartee.encore.core.DropTargetKind
import uk.gcartee.encore.core.SongSort
import uk.gcartee.encore.tokens.EncoreTokens.Recap.Variation

/**
 * Phase 11: the year recap on this device. The core builds the document (its `recap` call,
 * counted off the caller's thread); this keeps the last one per profile and year for a few
 * minutes, so Home, Mixes and the story share one build, and keeps every call off the main
 * thread.
 */
object RecapStore {
    private const val CACHE_MS = 10L * 60L * 1000L
    private val cache = HashMap<String, Pair<Long, Recap>>()

    fun southern(): Boolean = RecapRules.southern(Locale.getDefault().country, TimeZone.getDefault().id)

    fun utcOffsetMinutes(nowMs: Long = System.currentTimeMillis()): Int = TimeZone.getDefault().getOffset(nowMs) / 60_000

    private fun profile(host: CoreHost): String = runCatching { host.core.currentProfile() }.getOrNull().orEmpty()

    /** The open profile's recap of [year]; [fresh] builds it again even when one is cached. */
    suspend fun load(host: CoreHost, year: Int, fresh: Boolean = false): Result<Recap> = withContext(Dispatchers.IO) {
        val key = "${profile(host)}:$year"
        val now = System.currentTimeMillis()
        if (!fresh) {
            synchronized(cache) { cache[key] }?.takeIf { now - it.first < CACHE_MS }?.let { return@withContext Result.success(it.second) }
        }
        runCatching {
            parseRecap(host.core.recap(year, Variation.packsAtLaunch, southern(), utcOffsetMinutes(now)))
        }.onSuccess { r -> synchronized(cache) { cache[key] = now to r } }
    }

    /** The years with a kept (locked) recap on this device, newest first. */
    suspend fun pastYears(host: CoreHost): List<Int> = withContext(Dispatchers.IO) {
        runCatching { host.core.recapYears() }.getOrDefault(emptyList())
    }

    /** Guest has no recap (board 41, "Guest or Private"). */
    suspend fun isGuest(host: CoreHost): Boolean = withContext(Dispatchers.IO) {
        runCatching { host.core.guestSession() != null }.getOrDefault(false)
    }

    /** What Save to library did: [saved] of [wanted] songs went into the new playlist. */
    data class Saved(val saved: Int, val wanted: Int, val playlistId: Long?)

    /**
     * Board 39, card 11: the Top Songs playlist into the library. The core has no lookup from
     * a song key to a library song, so the keys are recomputed for the library's songs (the
     * key's own rules, [SongKey]) and matched; a song whose title the recap lists and that the
     * keys missed is found by title and artist. What cannot be found is left out.
     */
    suspend fun saveTopSongs(host: CoreHost, recap: Recap, closer: BeatData.Closer): Result<Saved> = withContext(Dispatchers.IO) {
        runCatching {
            val core = host.core
            val wanted = closer.songKeys
            // Hash -> (seconds, song id) for every library song, read a page at a time.
            val byHash = HashMap<String, MutableList<Pair<Long, Long>>>()
            var offset = 0
            while (offset < LIBRARY_SCAN_MAX) {
                val page = core.songs(SongSort.TITLE, offset.toUInt(), PAGE.toUInt())
                if (page.isEmpty()) break
                for (s in page) {
                    val artists = listOf(s.artist, s.artist.substringBefore(", "), s.albumArtist).filter { it.isNotBlank() }.distinct()
                    for (a in artists) {
                        SongKey.split(SongKey.t1(a, s.title, s.durationMs))?.let { (hash, secs) ->
                            byHash.getOrPut(hash) { mutableListOf() }.add(secs to s.id)
                        }
                    }
                }
                offset += page.size
            }
            val titled = HashMap<String, SongEntry>()
            recap.detail?.topSongs?.forEach { titled[it.key] = it }
            recap.beats.forEach { b -> (b.data as? BeatData.TopSong)?.top?.forEach { titled.putIfAbsent(it.key, it) } }
            val ids = LinkedHashSet<Long>()
            for (key in wanted) {
                val byKey = SongKey.split(key)?.let { (hash, secs) ->
                    byHash[hash]?.firstOrNull { kotlin.math.abs(it.first - secs) <= uk.gcartee.encore.tokens.EncoreTokens.Recap.Counting.sameSongDurationToleranceSeconds }?.second
                }
                val found = byKey ?: titled[key]?.let { e ->
                    runCatching { core.search(e.title).songs }.getOrDefault(emptyList())
                        .firstOrNull { it.title.equals(e.title, ignoreCase = true) && (it.artist.equals(e.artist, ignoreCase = true) || it.artist.startsWith(e.artist, ignoreCase = true)) }
                        ?.id
                }
                found?.let(ids::add)
            }
            if (ids.isEmpty()) return@runCatching Saved(0, wanted.size, null)
            val edit = core.addTo(DropTarget(DropTargetKind.NEW_PLAYLIST, null), ids.toList(), null, false)
            edit.playlistId?.let { pid -> runCatching { core.renamePlaylist(pid, closer.title.ifBlank { "Your Top Songs ${recap.year}" }) } }
            Saved(ids.size, wanted.size, edit.playlistId)
        }
    }

    private const val PAGE = 500
    private const val LIBRARY_SCAN_MAX = 400_000

    /**
     * Board 39, item 8: the card as a PNG made on this device and handed to the system share
     * sheet through the export folder (res/xml/export_paths.xml). Nothing is uploaded.
     */
    suspend fun share(context: Context, bitmap: Bitmap, year: Int, card: Int): Result<Unit> = runCatching {
        val file = withContext(Dispatchers.IO) {
            val dir = File(context.cacheDir, "exports").apply { mkdirs() }
            // One recap image at a time in the folder.
            dir.listFiles()?.filter { it.name.startsWith("recap-") }?.forEach { it.delete() }
            val f = File(dir, "recap-$year-${card + 1}.png")
            val soft = if (bitmap.config == Bitmap.Config.HARDWARE) bitmap.copy(Bitmap.Config.ARGB_8888, false) else bitmap
            f.outputStream().use { soft.compress(Bitmap.CompressFormat.PNG, 100, it) }
            f
        }
        val uri = FileProvider.getUriForFile(context, context.packageName + ".exports", file)
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "image/png"
            putExtra(Intent.EXTRA_STREAM, uri)
            clipData = ClipData.newRawUri("", uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        context.startActivity(Intent.createChooser(send, "Share your $year").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }
}

/**
 * Board 41, "Progress": the card the story was left on, whether it was finished (the Home
 * card's "new" dot) and the plays seen last time (the final run's "12 more plays" line), per
 * profile and year, on this device.
 */
class RecapProgress(context: Context, private val profile: String) {
    private val prefs = context.getSharedPreferences("recap-progress", Context.MODE_PRIVATE)

    private fun k(year: Int, what: String) = "$profile:$year:$what"

    fun resumeIndex(year: Int, count: Int, nowMs: Long = System.currentTimeMillis()): Int =
        RecapRules.resumeIndex(prefs.getInt(k(year, "index"), 0), prefs.getLong(k(year, "at"), 0L), nowMs, finished(year) && prefs.getInt(k(year, "index"), 0) >= count - 1, count)

    fun save(year: Int, index: Int, nowMs: Long = System.currentTimeMillis()) {
        prefs.edit().putInt(k(year, "index"), index).putLong(k(year, "at"), nowMs).apply()
    }

    fun finished(year: Int): Boolean = prefs.getBoolean(k(year, "done"), false)

    fun markFinished(year: Int) {
        prefs.edit().putBoolean(k(year, "done"), true).apply()
    }

    fun seenPlays(year: Int): Long = prefs.getLong(k(year, "plays"), -1L)

    fun setSeenPlays(year: Int, plays: Long) {
        prefs.edit().putLong(k(year, "plays"), plays).apply()
    }
}
