package uk.gcartee.encore.app.playback.tv

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.Uri
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import java.io.FileNotFoundException
import java.io.IOException
import java.io.InputStream
import java.net.Inet4Address
import uk.gcartee.encore.app.ui.components.ArtworkCache
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.core.StreamInfo

/**
 * A queue song for the relay, read through the player's own data sources: the media server
 * with its sign-in headers (the service keeps them by address, never in the TV's address), the
 * rolling cache (a song played before, or a transcode already whole, comes from there), or a
 * download or a file on the phone.
 */
@OptIn(UnstableApi::class)
class DataSourceSong(
    private val factory: DataSource.Factory,
    private val uri: Uri,
    /** The rolling cache's name for it; null for a file on the phone. */
    private val cacheKey: String?,
    private val fallbackType: String,
) : PhoneRelay.Source {
    override fun open(offset: Long): PhoneRelay.Opened {
        val source = factory.createDataSource()
        val spec = DataSpec.Builder()
            .setUri(uri)
            .setPosition(offset)
            .apply { cacheKey?.let(::setKey) }
            .build()
        val length = try {
            source.open(spec)
        } catch (e: IOException) {
            runCatching { source.close() }
            throw e
        }
        val total = if (length == C.LENGTH_UNSET.toLong()) null else offset + length
        val type = source.responseHeaders.entries
            .firstOrNull { it.key.equals("Content-Type", ignoreCase = true) }
            ?.value?.firstOrNull()
            ?.substringBefore(';')?.trim()
            ?.takeIf { it.startsWith("audio/") }
            ?: fallbackType
        return PhoneRelay.Opened(DataSourceStream(source), total, type)
    }
}

/** A Media3 [DataSource] already open, read as a stream; closing it closes the source. */
@OptIn(UnstableApi::class)
private class DataSourceStream(private val source: DataSource) : InputStream() {
    private val one = ByteArray(1)

    override fun read(): Int = if (read(one, 0, 1) < 0) -1 else one[0].toInt() and 0xff

    override fun read(b: ByteArray, off: Int, len: Int): Int {
        if (len == 0) return 0
        val n = source.read(b, off, len)
        return if (n == C.RESULT_END_OF_INPUT) -1 else n
    }

    override fun close() = source.close()
}

/** The song's cover for the TV, from the app's own cover cache (fetched once from the server). */
class CoverSource(private val context: Context, private val songId: Long) : PhoneRelay.Source {
    override fun open(offset: Long): PhoneRelay.Opened {
        val file = ArtworkCache.file(context, ArtworkOf.SONG, songId, COVER_PX) ?: throw FileNotFoundException("no cover")
        val bytes = file.readBytes()
        val from = offset.coerceIn(0, bytes.size.toLong()).toInt()
        return PhoneRelay.Opened(bytes.inputStream(from, bytes.size - from), bytes.size.toLong(), imageType(bytes))
    }

    private companion object {
        /** Big enough for a TV's music screen, small enough to send at once. */
        const val COVER_PX = 720
    }
}

/** The picture's type from its first bytes. */
internal fun imageType(bytes: ByteArray): String = when {
    bytes.size >= 3 && bytes[0] == 0xFF.toByte() && bytes[1] == 0xD8.toByte() -> "image/jpeg"
    bytes.size >= 8 && bytes[0] == 0x89.toByte() && bytes[1] == 'P'.code.toByte() -> "image/png"
    bytes.size >= 12 && String(bytes, 8, 4, Charsets.US_ASCII) == "WEBP" -> "image/webp"
    else -> "image/jpeg"
}

/**
 * The song's type for the TV before a byte is fetched (Cast needs it up front): from the
 * library's container, or AAC when the server may transcode it (CoreHost asks for AAC). The
 * relay's answer carries the server's own type once it has it.
 */
@OptIn(UnstableApi::class)
internal fun guessAudioType(container: String?, info: StreamInfo): String {
    if (!info.local && info.mayTranscode) return MimeTypes.AUDIO_AAC
    return audioTypeOf(container)
}

@OptIn(UnstableApi::class)
internal fun audioTypeOf(container: String?): String = when (container?.lowercase()?.trim('.')) {
    "flac" -> MimeTypes.AUDIO_FLAC
    "mp3", "mpeg" -> MimeTypes.AUDIO_MPEG
    "m4a", "mp4", "alac" -> MimeTypes.AUDIO_MP4
    "aac", "adts" -> MimeTypes.AUDIO_AAC
    "ogg", "oga", "vorbis" -> MimeTypes.AUDIO_OGG
    "opus" -> MimeTypes.AUDIO_OGG
    "wav", "wave" -> MimeTypes.AUDIO_WAV
    else -> MimeTypes.AUDIO_MPEG
}

/**
 * The phone's IPv4 address on Wi-Fi, which the relay binds to and SSDP searches from; null when
 * the phone isn't on Wi-Fi (a TV can't reach it then).
 */
fun wifiAddress(context: Context): Inet4Address? {
    val connectivity = context.getSystemService(ConnectivityManager::class.java) ?: return null
    @Suppress("DEPRECATION") // allNetworks: the Wi-Fi may not be the default network (a VPN, say).
    val networks = listOfNotNull(connectivity.activeNetwork) + connectivity.allNetworks.toList()
    for (network in networks.distinct()) {
        val caps = connectivity.getNetworkCapabilities(network) ?: continue
        if (!caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) continue
        val address = connectivity.getLinkProperties(network)?.linkAddresses
            ?.map { it.address }
            ?.filterIsInstance<Inet4Address>()
            ?.firstOrNull { !it.isLoopbackAddress && !it.isAnyLocalAddress }
        if (address != null) return address
    }
    return null
}
