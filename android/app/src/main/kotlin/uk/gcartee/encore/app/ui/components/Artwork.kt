package uk.gcartee.encore.app.ui.components

import android.content.Context
import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.size
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.core.ArtworkOf

/**
 * Album art at the exact pixel size the core asks the server for (CORE-API `artwork`).
 * The platform's loader caches it: memory first, then a disk cache named by a hash of
 * the URL (the URL carries the token, so it never becomes a file name). One plain corner
 * radius on all four corners (CLAUDE.md, board 04).
 */
@Composable
fun Artwork(
    of: ArtworkOf,
    id: Long,
    size: Dp,
    modifier: Modifier = Modifier,
    cornerRadius: Dp = 8.dp,
    /** False: transparent until the image arrives, so something drawn beneath shows (a mosaic). */
    placeholder: Boolean = true,
    /** True: fill the frame it is given (a carousel item) instead of keeping a square. */
    fill: Boolean = false,
) {
    val context = LocalContext.current
    val px = with(LocalDensity.current) { size.roundToPx() }.coerceIn(64, 1200)
    var image by remember(of, id, px) { mutableStateOf(ArtworkCache.memory(context, of, id, px)) }
    LaunchedEffect(of, id, px) {
        if (image == null) image = ArtworkCache.load(context, of, id, px)
    }
    val shape = RoundedCornerShape(cornerRadius)
    Box(
        modifier
            .let { if (fill) it.fillMaxSize() else it.aspectRatio(1f) }
            .clip(shape)
            .let { if (placeholder) it.background(MaterialTheme.colorScheme.surfaceContainerHighest) else it }
            .clearAndSetSemantics { },
    ) {
        image?.let {
            Image(bitmap = it, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.matchParentSize())
        }
    }
}

object ArtworkCache {
    private val memory = object : LruCache<String, ImageBitmap>(48 * 1024 * 1024) {
        override fun sizeOf(key: String, value: ImageBitmap) = value.width * value.height * 4
    }

    /** IDs are per profile (each has its own database), so the key names the profile. */
    private fun key(context: Context, of: ArtworkOf, id: Long, px: Int): String {
        val profile = runCatching { CoreHost.get(context).core.currentProfile() }.getOrNull().orEmpty()
        return "$profile-${of.name}-$id-$px"
    }

    fun memory(context: Context, of: ArtworkOf, id: Long, px: Int): ImageBitmap? = memory.get(key(context, of, id, px))

    suspend fun load(context: Context, of: ArtworkOf, id: Long, px: Int): ImageBitmap? = withContext(Dispatchers.IO) {
        val key = key(context, of, id, px)
        memory.get(key)?.let { return@withContext it }
        val dir = File(context.cacheDir, "artwork").apply { mkdirs() }
        val file = File(dir, hash(key))
        val bytes = if (file.isFile) {
            file.readBytes()
        } else {
            val request = runCatching { CoreHost.get(context).core.artwork(of, id, px.toUInt()) }.getOrNull()
                ?: return@withContext null
            fetch(request.url, request.headers.associate { it.name to it.value })?.also { file.writeBytes(it) }
        } ?: return@withContext null
        val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap() ?: return@withContext null
        memory.put(key, bitmap)
        bitmap
    }

    /**
     * A profile's picture from the media server (`ProfileSummary.avatar_url`, served
     * without a token, CORE-API). Cached in memory only, so a changed picture shows on
     * the next launch.
     */
    fun memoryUrl(url: String, px: Int): ImageBitmap? = memory.get("url-$px-$url")

    suspend fun loadUrl(url: String, px: Int): ImageBitmap? = withContext(Dispatchers.IO) {
        val key = "url-$px-$url"
        memory.get(key)?.let { return@withContext it }
        val bytes = fetch(url, emptyMap()) ?: return@withContext null
        val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
        var sample = 1
        while (opts.outWidth / (sample * 2) >= px && opts.outHeight / (sample * 2) >= px) sample *= 2
        val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample })?.asImageBitmap()
            ?: return@withContext null
        memory.put(key, bitmap)
        bitmap
    }

    private fun fetch(url: String, headers: Map<String, String>): ByteArray? = runCatching {
        val conn = URL(url).openConnection() as HttpURLConnection
        try {
            conn.connectTimeout = 10_000
            conn.readTimeout = 15_000
            conn.instanceFollowRedirects = false
            headers.forEach(conn::setRequestProperty)
            if (conn.responseCode != 200) return@runCatching null
            conn.inputStream.use { it.readBytes() }
        } finally {
            conn.disconnect()
        }
    }.getOrNull()

    private fun hash(s: String): String =
        MessageDigest.getInstance("SHA-256").digest(s.toByteArray()).joinToString("") { "%02x".format(it) }
}

/**
 * A profile's circle (boards 05 and 14): the media server's user picture when there is one
 * (`avatar_url`), else the initials on a tonal circle.
 */
@Composable
fun ProfileAvatar(
    initials: String,
    avatarUrl: String?,
    size: Dp,
    modifier: Modifier = Modifier,
    container: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.surfaceContainerHighest,
    content: androidx.compose.ui.graphics.Color = MaterialTheme.colorScheme.onSurface,
) {
    val px = with(LocalDensity.current) { size.roundToPx() }.coerceIn(48, 512)
    var image by remember(avatarUrl, px) { mutableStateOf(avatarUrl?.let { ArtworkCache.memoryUrl(it, px) }) }
    LaunchedEffect(avatarUrl, px) {
        if (image == null && avatarUrl != null) image = ArtworkCache.loadUrl(avatarUrl, px)
    }
    Box(
        modifier.size(size).clip(androidx.compose.foundation.shape.CircleShape).background(container),
        contentAlignment = androidx.compose.ui.Alignment.Center,
    ) {
        val bitmap = image
        if (bitmap != null) {
            Image(bitmap = bitmap, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.matchParentSize())
        } else {
            androidx.compose.material3.Text(initials, style = uk.gcartee.encore.app.ui.theme.LocalEncoreType.current.buttonLabelSelected, color = content)
        }
    }
}