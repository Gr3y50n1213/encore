package uk.gcartee.encore.app.playback.tv

import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder

/**
 * Roku (ADR android-0007, plan T5): the Encore channel (`roku/`, installed in the Roku's
 * developer mode, app id "dev") plays the relay's address and shows the song. Encore drives
 * it through Roku's External Control Protocol on port 8060: `launch/dev?url=…` starts it on a
 * song, `input?action=…` drives it, and `query/media-player` is Roku's own report of what is
 * playing and where. Everything stays on the local network.
 */
object Roku {
    const val SEARCH_TARGET = "roku:ecp"
    private const val CHANNEL = "dev"
    private const val TIMEOUT_MS = 4_000

    /** A Roku's name and model, from `query/device-info`. */
    data class Info(val id: String, val name: String, val model: String?)

    fun info(base: URL): Info {
        val xml = get(URL(base, "query/device-info"))
        fun tag(name: String) = Regex("<$name>([^<]*)</$name>").find(xml)?.groupValues?.get(1)?.trim()?.takeIf { it.isNotEmpty() }
        val id = tag("serial-number") ?: tag("device-id") ?: base.host
        val name = tag("user-device-name") ?: tag("friendly-device-name") ?: tag("default-device-name") ?: "Roku"
        return Info(id, unescape(name), tag("model-name")?.let(::unescape))
    }

    /** Whether the Encore channel is the one on screen now. */
    fun channelOpen(base: URL): Boolean =
        Regex("<app id=\"$CHANNEL\"").containsMatchIn(get(URL(base, "query/active-app")))

    fun launch(base: URL, params: Map<String, String>) = post(URL(base, "launch/$CHANNEL?${query(params)}"))

    fun input(base: URL, params: Map<String, String>) = post(URL(base, "input?${query(params)}"))

    fun key(base: URL, key: String) = post(URL(base, "keypress/$key"))

    /** Roku's `query/media-player`: the state, and the position and length in ms. */
    fun mediaPlayer(base: URL): Triple<RendererState, Long, Long> = parseMediaPlayer(get(URL(base, "query/media-player")))

    /** Pure, for tests. */
    fun parseMediaPlayer(xml: String): Triple<RendererState, Long, Long> {
        val state = Regex("<player[^>]*\\bstate=\"([a-z]+)\"").find(xml)?.groupValues?.get(1)
        fun ms(name: String) = Regex("<$name>\\s*(\\d+)\\s*ms\\s*</$name>").find(xml)?.groupValues?.get(1)?.toLongOrNull() ?: 0L
        val s = when (state) {
            "play" -> RendererState.PLAYING
            "pause" -> RendererState.PAUSED
            "buffer", "startup", "open" -> RendererState.TRANSITIONING
            "stop", "finished" -> RendererState.STOPPED
            else -> RendererState.NO_MEDIA
        }
        return Triple(s, ms("position"), ms("duration"))
    }

    fun query(params: Map<String, String>): String =
        params.entries.joinToString("&") { (k, v) -> "${enc(k)}=${enc(v)}" }

    private fun enc(s: String) = URLEncoder.encode(s, "UTF-8").replace("+", "%20")

    private fun unescape(s: String) =
        s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")

    /** The Roku said no (403): a report its settings don't allow. */
    class Refused : IOException("the Roku refused")

    /** Whether the Roku answers at all (its active app), for when reports are refused. */
    fun reachable(base: URL): Boolean = runCatching { get(URL(base, "query/active-app")) }.isSuccess

    private fun get(url: URL): String {
        check(LocalNetwork.isLocal(url))
        val c = url.openConnection() as HttpURLConnection
        try {
            c.connectTimeout = TIMEOUT_MS
            c.readTimeout = TIMEOUT_MS
            if (c.responseCode == HttpURLConnection.HTTP_FORBIDDEN) throw Refused()
            if (c.responseCode !in 200..299) throw IOException("the Roku answered ${c.responseCode}")
            return c.inputStream.use { it.readBytes().toString(Charsets.UTF_8) }
        } finally {
            c.disconnect()
        }
    }

    private fun post(url: URL) {
        check(LocalNetwork.isLocal(url))
        val c = url.openConnection() as HttpURLConnection
        try {
            c.requestMethod = "POST"
            c.connectTimeout = TIMEOUT_MS
            c.readTimeout = TIMEOUT_MS
            c.doOutput = true
            c.setFixedLengthStreamingMode(0)
            c.outputStream.close()
            if (c.responseCode !in 200..299) throw IOException("the Roku answered ${c.responseCode}")
        } finally {
            c.disconnect()
        }
    }
}

/**
 * A Roku running the Encore channel, behind [TvPlayer]. Roku has no volume to set, only the
 * remote's volume keys, so the level is Encore's own count of the presses it sent.
 */
class RokuRenderer(
    private val base: URL,
    override val name: String,
    /** "Sam's phone": shown on the TV as where the music comes from. */
    private val from: String,
) : Renderer {
    override val maxVolume: Int = MAX_VOLUME

    private var level = MAX_VOLUME / 2
    private var lastUri: String? = null

    /**
     * Some Rokus refuse `query/media-player` ("Control by mobile apps" set to Limited, seen on
     * the owner's: 403). Then Encore keeps its own clock of what it told the channel: playing
     * or paused, and the position from when it last started or moved.
     */
    private var reportRefused = false
    private var ownState = RendererState.NO_MEDIA
    private var ownPositionMs = 0L
    private var ownSinceMs = 0L
    private var ownDurationMs = 0L

    private fun own(state: RendererState, positionMs: Long = ownNow()) {
        ownPositionMs = positionMs
        ownSinceMs = android.os.SystemClock.elapsedRealtime()
        ownState = state
    }

    private fun ownNow(): Long =
        if (ownState == RendererState.PLAYING) ownPositionMs + (android.os.SystemClock.elapsedRealtime() - ownSinceMs) else ownPositionMs

    override fun setUri(url: String, meta: TvMeta) {
        val params = buildMap {
            put("url", url)
            put("title", meta.title)
            meta.artist?.let { put("artist", it) }
            meta.album?.let { put("album", it) }
            meta.coverUrl?.let { put("art", it) }
            put("from", from)
        }
        // A new song goes to the open channel as input; the first one (or after the
        // listener left the channel) starts it.
        if (lastUri != null && runCatching { Roku.channelOpen(base) }.getOrDefault(false)) {
            Roku.input(base, params)
        } else {
            Roku.launch(base, params)
        }
        lastUri = url
        ownDurationMs = meta.durationMs
        own(RendererState.PLAYING, 0L)
    }

    override fun play() {
        Roku.input(base, mapOf("action" to "play"))
        own(RendererState.PLAYING)
    }

    override fun pause() {
        Roku.input(base, mapOf("action" to "pause"))
        own(RendererState.PAUSED)
    }

    override fun seek(positionMs: Long) {
        Roku.input(base, mapOf("action" to "seek", "pos" to positionMs.toString()))
        own(ownState, positionMs)
    }

    override fun stop() {
        lastUri = null
        Roku.input(base, mapOf("action" to "stop"))
        own(RendererState.STOPPED, 0L)
    }

    override fun setVolume(level: Int) {
        val target = level.coerceIn(0, MAX_VOLUME)
        val steps = (target - this.level).coerceIn(-MAX_STEPS, MAX_STEPS)
        repeat(kotlin.math.abs(steps)) { Roku.key(base, if (steps > 0) "VolumeUp" else "VolumeDown") }
        this.level += steps
    }

    override fun poll(withVolume: Boolean): RendererStatus {
        if (!reportRefused) {
            try {
                val (state, pos, dur) = Roku.mediaPlayer(base)
                return RendererStatus(state, pos, dur, if (withVolume) level else null)
            } catch (e: Roku.Refused) {
                reportRefused = true
            }
        }
        // Encore's own clock; the song ends when its length has played.
        if (!Roku.reachable(base)) throw java.io.IOException("the Roku can't be reached")
        var now = ownNow()
        if (ownState == RendererState.PLAYING && ownDurationMs > 0 && now >= ownDurationMs) {
            own(RendererState.STOPPED, ownDurationMs)
            now = ownDurationMs
        }
        return RendererStatus(ownState, now, ownDurationMs, if (withVolume) level else null)
    }

    private companion object {
        /** Roku TVs have about 100 volume steps; one press is one step. */
        const val MAX_VOLUME = 100
        /** At most this many presses for one change, so a slider drag can't flood the TV. */
        const val MAX_STEPS = 10
    }
}
