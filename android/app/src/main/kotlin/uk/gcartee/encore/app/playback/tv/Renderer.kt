package uk.gcartee.encore.app.playback.tv

import java.io.IOException

/** What a TV is told about the song it plays: the relay's addresses, never the server's. */
data class TvMeta(
    val title: String,
    val artist: String?,
    val album: String?,
    /** The cover through the relay (`/<pass>/art/<n>`), or null. */
    val coverUrl: String?,
    val mimeType: String,
    val durationMs: Long,
)

/** What the TV says it is doing. */
enum class RendererState { PLAYING, PAUSED, STOPPED, TRANSITIONING, NO_MEDIA }

data class RendererStatus(
    val state: RendererState,
    val positionMs: Long,
    /** The song's length as the TV knows it; 0 when it doesn't say. */
    val durationMs: Long,
    /** The TV's volume, 0 to [Renderer.maxVolume], when this poll asked for it. */
    val volume: Int?,
)

/**
 * A TV Encore drives itself (ADR android-0007: DLNA now, Roku later), behind [TvPlayer].
 * Every call is blocking network I/O on the local network: [TvPlayer] makes them on its own
 * thread, never the main one.
 */
interface Renderer {
    /** The TV's own name, as Play on shows it. */
    val name: String

    /** The top of [setVolume]'s scale (DLNA: 100). */
    val maxVolume: Int

    @Throws(IOException::class)
    fun setUri(url: String, meta: TvMeta)

    @Throws(IOException::class)
    fun play()

    @Throws(IOException::class)
    fun pause()

    @Throws(IOException::class)
    fun seek(positionMs: Long)

    @Throws(IOException::class)
    fun stop()

    @Throws(IOException::class)
    fun setVolume(level: Int)

    /** The state and position now; [withVolume] also asks for the volume. */
    @Throws(IOException::class)
    fun poll(withVolume: Boolean): RendererStatus
}
