package uk.gcartee.encore.app.widgets

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import java.io.File
import java.io.IOException
import org.json.JSONException
import org.json.JSONObject

/** The snapshot's `privacy` (CORE-API, "Widgets"). */
enum class SnapshotPrivacy { NORMAL, LOCKED, GUEST }

/**
 * An album's colors (CORE-API, "Widgets", Colors): the palette the app draws that album's
 * gradient from. Opaque ARGB ints: [base], four [stops] (leading color first) and [text],
 * which reads on both. [dark] is the scheme (white text on a dark field, or dark on light).
 */
data class SnapshotPalette(val dark: Boolean, val base: Int, val stops: List<Int>, val text: Int)

/** Now playing (`nowPlaying` true) or the last song played. */
data class SnapshotSong(
    val songId: Long,
    val title: String,
    val artist: String,
    val albumId: Long?,
    val albumTitle: String?,
    val cover: String?,
    val durationMs: Long,
    val nowPlaying: Boolean,
    /** Null until the core has made the album's colors, and for a song with no cover. */
    val palette: SnapshotPalette? = null,
)

data class SnapshotMix(
    val mixId: String,
    val title: String,
    val cover: String?,
    val pinned: Boolean,
    val palette: SnapshotPalette? = null,
)

data class SnapshotAlbum(
    val albumId: Long,
    val title: String,
    val artist: String,
    val cover: String?,
    val palette: SnapshotPalette? = null,
)

/** One profile's `snapshot.json`, schema 1. */
data class Snapshot(
    val profileId: String,
    val privacy: SnapshotPrivacy,
    val song: SnapshotSong?,
    val isPlaying: Boolean,
    val positionMs: Long,
    val positionAtUtcMs: Long,
    val pinnedMixes: List<SnapshotMix>,
    val nextUp: SnapshotMix?,
    val recentAlbums: List<SnapshotAlbum>,
    val reachable: Boolean,
    val writtenAtUtcMs: Long,
)

/** What reading a profile's file gave. */
sealed interface SnapshotRead {
    data class Ok(val snapshot: Snapshot) : SnapshotRead

    /** No file: the profile was removed from this device (its folder is gone). */
    data object Missing : SnapshotRead

    /** A newer schema, another profile's file, or a broken one: show Open Encore. */
    data object Unreadable : SnapshotRead
}

/**
 * Reads the file the core keeps for widgets: `<data_dir>/widgets/<profile_id>/snapshot.json`,
 * covers in `art/` beside it (CORE-API, "Widgets"). `<data_dir>` is `filesDir/core`
 * (the app's core setup). Nothing here starts the core, opens the database or calls the network: a
 * widget reads this file and nothing else.
 */
object SnapshotReader {
    const val SCHEMA = 1
    const val FILE = "snapshot.json"
    const val ART = "art"

    /** Covers are drawn at most about this big (the core saves them at 256 px). */
    const val COVER_PX = 256

    /** The core's data folder is `filesDir/core`; widgets live in `widgets/` there. */
    fun widgetsDir(filesDir: File): File = File(File(filesDir, "core"), "widgets")

    /**
     * Profile IDs and cover names become path parts: only plain ASCII letters, digits,
     * `-`, `_` and `.`, never `.` or `..`, so a name can never leave its profile's folder.
     */
    fun isSafeName(name: String): Boolean =
        name.isNotEmpty() && name.length <= 128 && name != "." && name != ".." &&
            name.all { it in 'a'..'z' || it in 'A'..'Z' || it in '0'..'9' || it == '-' || it == '_' || it == '.' }

    /** The profile's own folder, or null for an ID that isn't a safe name. */
    fun profileDir(widgetsDir: File, profileId: String): File? =
        if (isSafeName(profileId)) File(widgetsDir, profileId) else null

    /** Reads [profileId]'s file, and only that one. */
    fun read(widgetsDir: File, profileId: String): SnapshotRead {
        val dir = profileDir(widgetsDir, profileId) ?: return SnapshotRead.Unreadable
        val file = File(dir, FILE)
        val text = try {
            if (!file.isFile) return SnapshotRead.Missing
            file.readText(Charsets.UTF_8)
        } catch (e: IOException) {
            return SnapshotRead.Unreadable
        } catch (e: SecurityException) {
            return SnapshotRead.Unreadable
        }
        return parse(text, profileId)
    }

    /**
     * Pure parsing (JVM-tested). A schema other than 1, or a file whose `profileId` isn't
     * [expectedProfileId], is [SnapshotRead.Unreadable]: a widget never shows another
     * profile's listening.
     */
    fun parse(text: String, expectedProfileId: String): SnapshotRead {
        val o = try {
            JSONObject(text)
        } catch (e: JSONException) {
            return SnapshotRead.Unreadable
        }
        if (o.optInt("schema", -1) != SCHEMA) return SnapshotRead.Unreadable
        val profileId = o.str("profileId") ?: return SnapshotRead.Unreadable
        if (profileId != expectedProfileId) return SnapshotRead.Unreadable
        val privacy = when (o.str("privacy")) {
            "normal" -> SnapshotPrivacy.NORMAL
            "locked" -> SnapshotPrivacy.LOCKED
            "guest" -> SnapshotPrivacy.GUEST
            else -> return SnapshotRead.Unreadable
        }
        return try {
            SnapshotRead.Ok(
                Snapshot(
                    profileId = profileId,
                    privacy = privacy,
                    song = o.optJSONObject("song")?.let(::song),
                    isPlaying = o.optBoolean("isPlaying", false),
                    positionMs = o.optLong("positionMs", 0L),
                    positionAtUtcMs = o.optLong("positionAtUtcMs", 0L),
                    pinnedMixes = o.objects("pinnedMixes").mapNotNull(::mix),
                    nextUp = o.optJSONObject("nextUp")?.let(::mix),
                    recentAlbums = o.objects("recentAlbums").mapNotNull(::album),
                    reachable = o.optBoolean("reachable", true),
                    writtenAtUtcMs = o.optLong("writtenAtUtcMs", 0L),
                ),
            )
        } catch (e: JSONException) {
            SnapshotRead.Unreadable
        }
    }

    /** A cover named by the snapshot, inside the profile's own `art/` folder, or null. */
    fun coverFile(widgetsDir: File, profileId: String, name: String?): File? {
        if (name == null || !isSafeName(name)) return null
        val dir = profileDir(widgetsDir, profileId) ?: return null
        return File(File(dir, ART), name)
    }

    /**
     * Pure: the power-of-two step BitmapFactory takes to bring [width] x [height] to about
     * [maxPx] on its longer side.
     */
    fun sampleSize(width: Int, height: Int, maxPx: Int): Int {
        var sample = 1
        val longer = maxOf(width, height)
        while (longer / (sample * 2) >= maxPx) sample *= 2
        return sample
    }

    /**
     * A cover as a downsampled bitmap for `ImageProvider(bitmap)`, decoded by content (the
     * core saves JPEG or PNG under one name). Never a file or web address: the
     * launcher reads nothing of ours.
     */
    fun loadCover(file: File, maxPx: Int = COVER_PX): Bitmap? = try {
        if (!file.isFile) {
            null
        } else {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeFile(file.path, bounds)
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) {
                null
            } else {
                val options = BitmapFactory.Options().apply {
                    inSampleSize = sampleSize(bounds.outWidth, bounds.outHeight, maxPx)
                }
                BitmapFactory.decodeFile(file.path, options)
            }
        }
    } catch (e: OutOfMemoryError) {
        null
    } catch (e: SecurityException) {
        null
    }

    private fun song(o: JSONObject): SnapshotSong? {
        val title = o.str("title") ?: return null
        return SnapshotSong(
            songId = o.optLong("songId", 0L),
            title = title,
            artist = o.str("artist").orEmpty(),
            albumId = if (o.isNull("albumId")) null else o.optLong("albumId"),
            albumTitle = o.str("albumTitle"),
            cover = o.str("cover"),
            durationMs = o.optLong("durationMs", 0L).coerceAtLeast(0L),
            nowPlaying = o.optBoolean("nowPlaying", false),
            palette = palette(o),
        )
    }

    private fun mix(o: JSONObject): SnapshotMix? {
        val id = o.str("mixId") ?: return null
        return SnapshotMix(id, o.str("title").orEmpty(), o.str("cover"), o.optBoolean("pinned", false), palette(o))
    }

    private fun album(o: JSONObject): SnapshotAlbum? {
        if (o.isNull("albumId")) return null
        return SnapshotAlbum(
            o.optLong("albumId"),
            o.str("title").orEmpty(),
            o.str("artist").orEmpty(),
            o.str("cover"),
            palette(o),
        )
    }

    private val HEX_COLOUR = Regex("^[0-9A-Fa-f]{6}$")

    /** An `RRGGBB` string as an opaque ARGB int, or null for anything else. */
    private fun colour(s: String?): Int? =
        if (s != null && HEX_COLOUR.matches(s)) 0xFF000000.toInt() or s.toInt(16) else null

    /**
     * The `palette` key of a song, mix or album. Anything malformed (a wrong type, a bad
     * hex, not exactly four stops) is no palette, never an unreadable file: the widget just
     * draws in the theme's colors.
     */
    internal fun palette(o: JSONObject): SnapshotPalette? {
        val p = o.optJSONObject("palette") ?: return null
        val base = colour(p.str("base")) ?: return null
        val text = colour(p.str("text")) ?: return null
        val array = p.optJSONArray("stops") ?: return null
        if (array.length() != 4) return null
        val stops = (0 until 4).map { colour(array.optString(it, null)) ?: return null }
        return SnapshotPalette(dark = p.str("scheme") != "light", base = base, stops = stops, text = text)
    }

    /** A string, or null when the key is missing or JSON null (optString would say "null"). */
    private fun JSONObject.str(key: String): String? = if (isNull(key)) null else optString(key)

    private fun JSONObject.objects(key: String): List<JSONObject> {
        val array = optJSONArray(key) ?: return emptyList()
        return (0 until array.length()).mapNotNull { array.optJSONObject(it) }
    }
}
