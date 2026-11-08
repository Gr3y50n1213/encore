package uk.gcartee.encore.app.playback

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import uk.gcartee.encore.app.ui.components.ArtworkCache
import uk.gcartee.encore.core.ArtworkOf

/**
 * Covers for Android Auto's browse lists (Phase 8). The car loads them itself and
 * can't send the media server's sign-in header, and a URL carrying the token must
 * never leave the app. So covers go through this provider: not exported, and each
 * address is granted, read-only, to the one controller that asked for it. A
 * downloaded album's kept cover works offline.
 *
 * `content://<package>.artwork/<album|song|playlist|artist>/<id>`
 */
class ArtworkProvider : ContentProvider() {
    override fun onCreate(): Boolean = true

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor? {
        if (mode != "r") return null
        val segments = uri.pathSegments
        if (segments.size != 2) return null
        val of = when (segments[0]) {
            "album" -> ArtworkOf.ALBUM
            "song" -> ArtworkOf.SONG
            "playlist" -> ArtworkOf.PLAYLIST
            "artist" -> ArtworkOf.ARTIST
            else -> return null
        }
        val id = segments[1].toLongOrNull() ?: return null
        val context = context ?: return null
        val file = ArtworkCache.file(context, of, id, SIZE_PX) ?: return null
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun getType(uri: Uri): String = "image/*"
    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0
    override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?): Int = 0

    companion object {
        private const val SIZE_PX = 480

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
