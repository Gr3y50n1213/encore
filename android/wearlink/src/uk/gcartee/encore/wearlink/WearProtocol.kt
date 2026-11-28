package uk.gcartee.encore.wearlink

/**
 * Stage W1 (docs/adr/android-0008-wear-os-app.md): what the phone and the watch say to each
 * other over Google's Wearable Data Layer. Shared by the phone app and the watch app (both
 * build files add this folder), and pure Kotlin, so the JVM tests cover it.
 *
 * - The phone publishes two DataItems: [Paths.NOW] (the song, its play state and the lyric
 *   line) and [Paths.MIXES] (the pinned mixes for the tile).
 * - The watch sends [Paths.CMD] messages: one [WearCommand] each, as UTF-8 text.
 *
 * The Data Layer only connects apps with the same application ID signed with the same key,
 * so nothing else on the phone or the watch reads these or sends commands.
 */
object Paths {
    const val NOW = "/encore/now"
    const val MIXES = "/encore/mixes"
    const val CMD = "/encore/cmd"

    /** The watch app declares this (res/values/wear.xml): the phone publishes only when it is reachable. */
    const val WATCH_CAPABILITY = "encore_watch"

    /** The phone app declares this: the watch sends its commands to that node. */
    const val PHONE_CAPABILITY = "encore_phone"
}

/** Read access to a DataMap (or, in tests, a plain map). Null when the key is absent. */
interface WireValues {
    fun string(key: String): String?
    fun long(key: String): Long?
    fun boolean(key: String): Boolean?
}

/** A plain map as [WireValues]: what [NowCodec.encode] and [MixesCodec.encode] give, read back. */
class MapValues(private val map: Map<String, Any>) : WireValues {
    override fun string(key: String): String? = map[key] as? String
    override fun long(key: String): Long? = (map[key] as? Number)?.toLong()
    override fun boolean(key: String): Boolean? = map[key] as? Boolean
}

/**
 * The phone's Now Playing for the watch. An empty [title] means nothing is playing or
 * queued: the watch says "Open Encore on your phone".
 *
 * [positionMs] was the position at [positionAtUtcMs] (wall-clock time, which the watch
 * takes from the phone): while [isPlaying] the position now is worked out from them, as
 * the phone's widgets do. [line] is the lyric line being sung, empty when there is none.
 * [upNext] is the next song's title, empty when nothing follows. [isPrivate] is the phone's
 * "Hide listening history while locked" setting.
 */
data class NowPlaying(
    val title: String,
    val artist: String,
    val album: String,
    val line: String,
    val isPlaying: Boolean,
    val positionMs: Long,
    val positionAtUtcMs: Long,
    val durationMs: Long,
    val liked: Boolean,
    val upNext: String,
    val isPrivate: Boolean,
) {
    val isEmpty: Boolean get() = title.isBlank()

    /** The position at [nowUtcMs], kept inside the song. */
    fun positionAt(nowUtcMs: Long): Long {
        val moved = if (isPlaying) (nowUtcMs - positionAtUtcMs).coerceAtLeast(0) else 0
        val at = positionMs + moved
        return if (durationMs > 0) at.coerceIn(0, durationMs) else at.coerceAtLeast(0)
    }

    /** How far into the song, 0 to 1, for the progress ring. */
    fun progressAt(nowUtcMs: Long): Float =
        if (durationMs <= 0) 0f else (positionAt(nowUtcMs).toFloat() / durationMs).coerceIn(0f, 1f)

    /**
     * Whether the watch needs this after [previous]: something besides the position changed,
     * or the position is more than [driftMs] off what [previous] predicts (a seek). The
     * position alone moving on as expected is not sent: the watch works it out.
     */
    fun needsSendAfter(previous: NowPlaying?, driftMs: Long = DRIFT_MS): Boolean {
        if (previous == null) return true
        if (copy(positionMs = 0, positionAtUtcMs = 0) != previous.copy(positionMs = 0, positionAtUtcMs = 0)) return true
        return kotlin.math.abs(positionAt(positionAtUtcMs) - previous.positionAt(positionAtUtcMs)) > driftMs
    }

    companion object {
        val EMPTY = NowPlaying("", "", "", "", false, 0, 0, 0, false, "", true)
        const val DRIFT_MS = 2_000L
    }
}

/** The DataItem [Paths.NOW] as flat keys (the cover is an Asset beside them, [COVER]). */
object NowCodec {
    const val VERSION = 1L
    const val COVER = "cover"

    /** Longer text is cut: a DataItem has a 100 KB limit and a watch shows a line or two. */
    const val MAX_TEXT = 200

    fun encode(n: NowPlaying): Map<String, Any> = mapOf(
        "v" to VERSION,
        "title" to n.title.take(MAX_TEXT),
        "artist" to n.artist.take(MAX_TEXT),
        "album" to n.album.take(MAX_TEXT),
        "line" to n.line.take(MAX_TEXT),
        "playing" to n.isPlaying,
        "pos" to n.positionMs,
        "posAt" to n.positionAtUtcMs,
        "dur" to n.durationMs,
        "liked" to n.liked,
        "next" to n.upNext.take(MAX_TEXT),
        "private" to n.isPrivate,
    )

    /** Null for another version: the watch then shows the empty state until the phone updates. */
    fun decode(v: WireValues): NowPlaying? {
        if (v.long("v") != VERSION) return null
        return NowPlaying(
            title = v.string("title").orEmpty(),
            artist = v.string("artist").orEmpty(),
            album = v.string("album").orEmpty(),
            line = v.string("line").orEmpty(),
            isPlaying = v.boolean("playing") ?: false,
            positionMs = (v.long("pos") ?: 0).coerceAtLeast(0),
            positionAtUtcMs = v.long("posAt") ?: 0,
            durationMs = (v.long("dur") ?: 0).coerceAtLeast(0),
            liked = v.boolean("liked") ?: false,
            upNext = v.string("next").orEmpty(),
            // Missing means private: the stricter reading.
            isPrivate = v.boolean("private") ?: true,
        )
    }
}

/** A pinned mix for the tile. [colour] and [text] are opaque ARGB, or 0 for the theme's. */
data class MixChip(val id: String, val title: String, val colour: Int, val text: Int)

/**
 * The DataItem [Paths.MIXES]. [likedSongs] offers the Liked songs chip. Under Guest, Private
 * listening or a locked profile the phone sends no mixes.
 */
data class Mixes(val mixes: List<MixChip>, val likedSongs: Boolean) {
    /** The tile's chips: up to four, the pinned mixes first, then Liked songs if there is room. */
    fun chips(): List<TileChip> {
        val out = mixes.take(MixesCodec.MAX).map<MixChip, TileChip> { TileChip.Mix(it) }.toMutableList()
        if (likedSongs && out.size < MixesCodec.MAX) out += TileChip.Liked
        return out
    }

    companion object {
        val EMPTY = Mixes(emptyList(), false)
    }
}

sealed interface TileChip {
    /** What a tap sends to the phone. */
    val command: WearCommand

    data class Mix(val mix: MixChip) : TileChip {
        override val command: WearCommand get() = WearCommand.PlayMix(mix.id)
    }

    data object Liked : TileChip {
        override val command: WearCommand get() = WearCommand.PlayLiked
    }
}

object MixesCodec {
    const val VERSION = 1L
    const val MAX = 4

    fun encode(m: Mixes): Map<String, Any> {
        val out = linkedMapOf<String, Any>("v" to VERSION, "liked" to m.likedSongs)
        val list = m.mixes.filter { WearCommand.isSafeId(it.id) }.take(MAX)
        out["n"] = list.size.toLong()
        list.forEachIndexed { i, mix ->
            out["id$i"] = mix.id
            out["title$i"] = mix.title.take(NowCodec.MAX_TEXT)
            out["colour$i"] = mix.colour.toLong() and 0xFFFFFFFFL
            out["text$i"] = mix.text.toLong() and 0xFFFFFFFFL
        }
        return out
    }

    fun decode(v: WireValues): Mixes? {
        if (v.long("v") != VERSION) return null
        val n = (v.long("n") ?: 0).coerceIn(0, MAX.toLong()).toInt()
        val mixes = (0 until n).mapNotNull { i ->
            val id = v.string("id$i")?.takeIf(WearCommand::isSafeId) ?: return@mapNotNull null
            MixChip(id, v.string("title$i").orEmpty(), (v.long("colour$i") ?: 0).toInt(), (v.long("text$i") ?: 0).toInt())
        }
        return Mixes(mixes, v.boolean("liked") ?: false)
    }
}

/** A watch-to-phone message on [Paths.CMD]. */
sealed interface WearCommand {
    val wire: String

    data object Play : WearCommand { override val wire = "play" }
    data object Pause : WearCommand { override val wire = "pause" }
    data object Next : WearCommand { override val wire = "next" }
    data object Previous : WearCommand { override val wire = "previous" }
    data object Like : WearCommand { override val wire = "like" }
    data object Unlike : WearCommand { override val wire = "unlike" }

    /** One step of the phone's media volume, up (+1) or down (-1): the crown or bezel. */
    data class Volume(val up: Boolean) : WearCommand {
        override val wire get() = if (up) "volume:+1" else "volume:-1"
    }

    /** Start a pinned mix (CORE-API `play_mix`). */
    data class PlayMix(val mixId: String) : WearCommand {
        override val wire get() = "mix:$mixId"
    }

    /** Start Liked songs. */
    data object PlayLiked : WearCommand { override val wire = "liked" }

    companion object {
        /** Commands are short; anything longer is not one. */
        const val MAX_BYTES = 200

        /** A mix ID: plain ASCII letters, digits, '-', '_' and '.', as the core makes them. */
        fun isSafeId(id: String): Boolean =
            id.isNotEmpty() && id.length <= 128 &&
                id.all { it in 'a'..'z' || it in 'A'..'Z' || it in '0'..'9' || it == '-' || it == '_' || it == '.' }

        fun parse(text: String): WearCommand? = when (text) {
            "play" -> Play
            "pause" -> Pause
            "next" -> Next
            "previous" -> Previous
            "like" -> Like
            "unlike" -> Unlike
            "volume:+1" -> Volume(up = true)
            "volume:-1" -> Volume(up = false)
            "liked" -> PlayLiked
            else -> text.removePrefix("mix:").takeIf { text.startsWith("mix:") && isSafeId(it) }?.let(::PlayMix)
        }

        fun parse(bytes: ByteArray?): WearCommand? {
            if (bytes == null || bytes.isEmpty() || bytes.size > MAX_BYTES) return null
            return parse(bytes.toString(Charsets.UTF_8))
        }
    }
}

fun WearCommand.toBytes(): ByteArray = wire.toByteArray(Charsets.UTF_8)

/**
 * At most one send per [minIntervalMs] (the phone's limit of two updates a second). The
 * caller asks [delayFor] before each send and reports [sent]: a change that arrives too soon
 * waits for the rest of the interval and is sent then, so the last change always goes out.
 */
class Throttle(private val minIntervalMs: Long) {
    private var lastSentAt: Long? = null

    /** How long to wait at [nowMs] before the next send; 0 sends at once. */
    fun delayFor(nowMs: Long): Long {
        val last = lastSentAt ?: return 0
        return (last + minIntervalMs - nowMs).coerceIn(0, minIntervalMs)
    }

    fun sent(nowMs: Long) {
        lastSentAt = nowMs
    }
}

/**
 * Which lyric line is being sung, from the lines' start times (null for a line without
 * one). The same rule as the phone's lyrics view: a line lights [leadMs] early.
 */
object LyricTiming {
    const val LEAD_MS = 150L

    /** The index of the line sung at [positionMs], or -1 before the first. */
    fun indexAt(starts: List<Long?>, positionMs: Long, leadMs: Long = LEAD_MS): Int =
        starts.indexOfLast { (it ?: Long.MAX_VALUE) <= positionMs + leadMs }

    /** Milliseconds until the line after [positionMs]'s starts, or null when none follows. */
    fun msUntilNext(starts: List<Long?>, positionMs: Long, leadMs: Long = LEAD_MS): Long? {
        val next = starts.filterNotNull().filter { it > positionMs + leadMs }.minOrNull() ?: return null
        return next - leadMs - positionMs
    }
}

/**
 * Stage W1's privacy rule on the watch (the phone's lock-screen rule, PLAN-PRODUCT section
 * 12): a locked watch shows no title anywhere, only "Encore" and the controls. On the watch
 * face (the complication), with the phone's "Hide listening history while locked" on, the
 * last song played is not shown once it stops: only a song playing now.
 */
object WatchPrivacy {
    fun showSong(watchLocked: Boolean, now: NowPlaying?, onWatchFace: Boolean = false): Boolean {
        if (now == null || now.isEmpty || watchLocked) return false
        return !onWatchFace || now.isPlaying || !now.isPrivate
    }

    /** The tile shows mix names only on an unlocked watch. */
    fun showMixes(watchLocked: Boolean): Boolean = !watchLocked
}
