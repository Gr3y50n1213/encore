package uk.gcartee.encore.app.playback

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.database.Cursor
import android.graphics.Bitmap
import android.graphics.Canvas
import android.net.Uri
import android.os.ParcelFileDescriptor
import androidx.core.content.ContextCompat
import java.io.File
import uk.gcartee.encore.app.R
import uk.gcartee.encore.app.ui.components.ArtworkCache
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.tokens.EncoreTokens.Color.Accents.Iris

/**
 * Covers for Android Auto's browse lists (Phase 8). The car loads them itself and
 * can't send the media server's sign-in header, and a URL carrying the token must
 * never leave the app. So covers go through this provider: not exported, and each
 * address is granted, read-only, to the one controller that asked for it. A
 * downloaded album's kept cover works offline.
 *
 * `content://<package>.artwork/<album|song|playlist|artist>/<id>`, and
 * `content://<package>.artwork/icon/<name>` for the Library's own tiles (Playlists,
 * Albums, Artists, Liked songs, a mix).
 *
 * The car draws a warning triangle on any grid tile with no picture (seen on the
 * Desktop Head Unit, 26 November 2026), so a tile never goes without one: a cover
 * that can't be had (no picture on the server, offline, not reachable) comes back as
 * the icon tile for its kind instead.
 */
class ArtworkProvider : ContentProvider() {
    override fun onCreate(): Boolean = true

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor? {
        if (mode != "r") return null
        val segments = uri.pathSegments
        if (segments.size != 2) return null
        val context = context ?: return null
        if (segments[0] == ICON) return tile(context, segments[1])?.let(::open)
        val of = when (segments[0]) {
            "album" -> ArtworkOf.ALBUM
            "song" -> ArtworkOf.SONG
            "playlist" -> ArtworkOf.PLAYLIST
            "artist" -> ArtworkOf.ARTIST
            else -> return null
        }
        val id = segments[1].toLongOrNull() ?: return null
        val file = runCatching { ArtworkCache.file(context, of, id, SIZE_PX) }.getOrNull()
            ?: tile(context, of.name.lowercase())
            ?: return null
        return open(file)
    }

    private fun open(file: File) = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)

    override fun getType(uri: Uri): String = "image/*"
    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0
    override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?): Int = 0

    companion object {
        private const val SIZE_PX = 480
        private const val ICON = "icon"
        private const val TILE_PX = 256
        private const val GLYPH_PX = 112

        /** The icon each tile shows: the app's own Material Symbols. */
        private val GLYPHS = mapOf(
            "playlists" to R.drawable.sym_queue_music,
            "playlist" to R.drawable.sym_queue_music,
            "albums" to R.drawable.sym_album,
            "album" to R.drawable.sym_album,
            "artists" to R.drawable.sym_person,
            "artist" to R.drawable.sym_person,
            "liked" to R.drawable.sym_favorite_fill,
            "mix" to R.drawable.sym_library_music_fill,
            "song" to R.drawable.sym_music_note,
        )

        /** A Library tile's icon, drawn by [openFile]. */
        fun icon(context: Context, name: String): Uri =
            Uri.Builder()
                .scheme("content")
                .authority("${context.packageName}.artwork")
                .appendPath(ICON)
                .appendPath(name)
                .build()

        /**
         * An icon tile, drawn once and kept: the Iris accent's dark container with its
         * icon colour (design/tokens.json), the colours the car's dark screen suits.
         */
        @Synchronized
        private fun tile(context: Context, name: String): File? {
            val glyph = GLYPHS[name] ?: return null
            val dir = File(context.cacheDir, "artwork").apply { mkdirs() }
            val file = File(dir, "tile-v1-$name.png")
            if (file.isFile) return file
            return runCatching {
                val bitmap = Bitmap.createBitmap(TILE_PX, TILE_PX, Bitmap.Config.ARGB_8888)
                val canvas = Canvas(bitmap)
                canvas.drawColor(argb(Iris.Dark.primaryContainer))
                val drawable = ContextCompat.getDrawable(context, glyph)!!.mutate()
                drawable.setTint(argb(Iris.Dark.onPrimaryContainer))
                val inset = (TILE_PX - GLYPH_PX) / 2
                drawable.setBounds(inset, inset, inset + GLYPH_PX, inset + GLYPH_PX)
                drawable.draw(canvas)
                val part = File(dir, "${file.name}.part")
                part.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
                bitmap.recycle()
                check(part.renameTo(file))
                file
            }.getOrNull()
        }

        private fun argb(hex: String): Int = (0xFF000000 or hex.substring(1).toLong(16)).toInt()

        fun uri(context: Context, of: ArtworkOf, id: Long): Uri =
            Uri.Builder()
                .scheme("content")
                .authority("${context.packageName}.artwork")
                .appendPath(of.name.lowercase())
                .appendPath(id.toString())
                .build()

        /** Let one controller (the car) read one cover. */
        fun grant(context: Context, packageName: String, uri: Uri) {
            runCatching { context.grantUriPermission(packageName, uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
        }
    }
}
