package uk.gcartee.encore.app.pebble

import java.util.UUID
import uk.gcartee.encore.app.recap.RecapJson
import uk.gcartee.encore.wearlink.NowPlaying
import uk.gcartee.encore.wearlink.WatchPrivacy
import uk.gcartee.encore.wearlink.WearCommand

/**
 * Stage W2 (docs/adr/android-0009-pebble.md): what the phone and the Encore watchapp on a
 * Pebble (pebble/src/c/encore.c) say to each other through the Pebble app. Pure Kotlin, so
 * the JVM tests cover it; [PebbleLink] does the Android side.
 *
 * PebbleKit Android is broadcast intents to the Pebble app, and this writes them by hand
 * rather than adding the library. The names and the message format are PebbleKit's, from
 * github.com/pebble/pebble-android-sdk, PebbleKit/PebbleKit/src/main/java/com/getpebble/
 * android/kit/Constants.java (the actions and extras), PebbleKit.java
 * (sendDataToPebbleWithTransactionId, sendAckToPebble, PebbleDataReceiver) and
 * util/PebbleDictionary.java (toJsonString and fromJson). Core Devices' Pebble app reads
 * and writes the same (github.com/coredevices/mobileapp, libpebble3, PebbleKitClassic.kt).
 */
object Pebble {
    /** The watchapp's UUID: pebble/package.json, "uuid". */
    val APP_UUID: UUID = UUID.fromString("e82e1a4a-a544-4f75-960c-c78ec6f41bc7")

    /** Core Devices' Pebble app first, then the original Pebble app. */
    val PEBBLE_APPS = listOf("coredevices.coreapp", "com.getpebble.android.basalt")

    // Constants.java: INTENT_APP_SEND, INTENT_APP_RECEIVE, INTENT_APP_ACK, INTENT_APP_NACK,
    // INTENT_APP_RECEIVE_ACK and INTENT_APP_RECEIVE_NACK.
    const val ACTION_SEND = "com.getpebble.action.app.SEND"
    const val ACTION_RECEIVE = "com.getpebble.action.app.RECEIVE"
    const val ACTION_ACK = "com.getpebble.action.app.ACK"
    const val ACTION_NACK = "com.getpebble.action.app.NACK"
    const val ACTION_RECEIVE_ACK = "com.getpebble.action.app.RECEIVE_ACK"
    const val ACTION_RECEIVE_NACK = "com.getpebble.action.app.RECEIVE_NACK"

    // Constants.java: APP_UUID (a java.util.UUID, Serializable), TRANSACTION_ID (an int) and
    // MSG_DATA (the JSON below).
    const val EXTRA_UUID = "uuid"
    const val EXTRA_TRANSACTION_ID = "transaction_id"
    const val EXTRA_MSG_DATA = "msg_data"

    /** The watch's AppMessage transaction IDs are one byte; Core's app keeps only the low byte. */
    fun transactionId(counter: Int): Int = counter and 0xFF
}

/** pebble/package.json "messageKeys": the same numbers on both sides. */
object PebbleKeys {
    const val TITLE = 1
    const val ARTIST = 2
    const val LYRIC = 3
    const val PLAYING = 4
    const val LIKED = 5
    const val PRIVATE = 6
    const val CMD = 7
}

/**
 * One key and value of a PebbleKit dictionary, as PebbleDictionary.toJsonString writes it:
 * `{"key":1,"type":"string","length":0,"value":"..."}`, or for a number
 * `{"key":4,"type":"uint","length":1,"value":1}`. [length] is the integer's width in bytes
 * (1, 2 or 4), and 0 for a string. Byte arrays ("bytes") aren't used here.
 */
sealed interface PebbleTuple {
    val key: Int

    data class Text(override val key: Int, val value: String) : PebbleTuple

    data class Number(override val key: Int, val value: Long, val signed: Boolean, val width: Int) : PebbleTuple

    companion object {
        fun uint8(key: Int, value: Int) = Number(key, value.toLong() and 0xFF, signed = false, width = 1)
        fun flag(key: Int, on: Boolean) = uint8(key, if (on) 1 else 0)
    }
}

/** PebbleKit's `msg_data` JSON, written and read. */
object PebbleDictionary {
    /** The longest `msg_data` read; the watch's messages are a few bytes. */
    const val MAX_JSON = 4096

    fun toJson(tuples: List<PebbleTuple>): String = buildString {
        append('[')
        tuples.forEachIndexed { i, t ->
            if (i > 0) append(',')
            append("{\"key\":").append(t.key)
            when (t) {
                is PebbleTuple.Text -> {
                    append(",\"type\":\"string\",\"length\":0,\"value\":")
                    quote(t.value)
                }
                is PebbleTuple.Number -> {
                    append(",\"type\":\"").append(if (t.signed) "int" else "uint").append("\",\"length\":").append(t.width)
                    append(",\"value\":").append(t.value)
                }
            }
            append('}')
        }
        append(']')
    }

    private fun StringBuilder.quote(s: String) {
        append('"')
        for (c in s) {
            when {
                c == '"' -> append("\\\"")
                c == '\\' -> append("\\\\")
                c == '\n' -> append("\\n")
                c == '\r' -> append("\\r")
                c == '\t' -> append("\\t")
                c < ' ' || c == ' ' || c == ' ' -> append("\\u").append(String.format(java.util.Locale.ROOT, "%04x", c.code))
                else -> append(c)
            }
        }
        append('"')
    }

    /** The tuples in [json], or null when it isn't a PebbleKit dictionary. Unknown types are skipped. */
    fun fromJson(json: String?): List<PebbleTuple>? {
        if (json.isNullOrEmpty() || json.length > MAX_JSON) return null
        val array = runCatching { RecapJson.parse(json) }.getOrNull() as? List<*> ?: return null
        val out = ArrayList<PebbleTuple>(array.size)
        for (item in array) {
            val o = item as? Map<*, *> ?: return null
            val key = (o["key"] as? Long)?.takeIf { it in 0..0xFFFFFFFFL }?.toInt() ?: return null
            when (o["type"]) {
                "string" -> out += PebbleTuple.Text(key, o["value"] as? String ?: return null)
                "int", "uint" -> {
                    val width = (o["length"] as? Long)?.toInt()?.takeIf { it == 1 || it == 2 || it == 4 } ?: return null
                    val value = o["value"] as? Long ?: return null
                    out += PebbleTuple.Number(key, value, signed = o["type"] == "int", width = width)
                }
                else -> Unit
            }
        }
        return out
    }
}

/**
 * Now Playing for the watchapp: the whole state each time (a missing key reads as empty).
 * Texts are cut to the watch's buffers, in UTF-8 bytes, on a character boundary.
 */
object PebbleNow {
    const val MAX_TITLE_BYTES = 100
    const val MAX_ARTIST_BYTES = 100
    const val MAX_LYRIC_BYTES = 160

    /**
     * What the watch shows for [now]. The Pebble has no lock, and the watchapp is on the
     * wrist for anyone to see, so it follows the phone's lock-screen rule, as the Wear OS
     * watch face does (WatchPrivacy, onWatchFace): with "Hide listening history while
     * locked" on, only a song playing now is shown, never the last one played. Otherwise
     * `private` is set and no title, artist or line leaves the phone.
     */
    fun view(now: NowPlaying): View {
        if (now.isEmpty) return View("", "", "", playing = false, liked = false, isPrivate = false)
        if (!WatchPrivacy.showSong(watchLocked = false, now = now, onWatchFace = true)) {
            return View("", "", "", playing = now.isPlaying, liked = false, isPrivate = true)
        }
        return View(
            title = utf8Take(now.title, MAX_TITLE_BYTES),
            artist = utf8Take(now.artist, MAX_ARTIST_BYTES),
            lyric = utf8Take(now.line, MAX_LYRIC_BYTES),
            playing = now.isPlaying,
            liked = now.liked,
            isPrivate = false,
        )
    }

    data class View(
        val title: String,
        val artist: String,
        val lyric: String,
        val playing: Boolean,
        val liked: Boolean,
        val isPrivate: Boolean,
    ) {
        fun tuples(): List<PebbleTuple> = listOf(
            PebbleTuple.Text(PebbleKeys.TITLE, title),
            PebbleTuple.Text(PebbleKeys.ARTIST, artist),
            PebbleTuple.Text(PebbleKeys.LYRIC, lyric),
            PebbleTuple.flag(PebbleKeys.PLAYING, playing),
            PebbleTuple.flag(PebbleKeys.LIKED, liked),
            PebbleTuple.flag(PebbleKeys.PRIVATE, isPrivate),
        )
    }

    /** [s] cut to at most [maxBytes] of UTF-8, never inside a character. */
    fun utf8Take(s: String, maxBytes: Int): String {
        var bytes = 0
        var i = 0
        while (i < s.length) {
            val cp = s.codePointAt(i)
            val n = when {
                cp < 0x80 -> 1
                cp < 0x800 -> 2
                cp < 0x10000 -> 3
                else -> 4
            }
            if (bytes + n > maxBytes) break
            bytes += n
            i += Character.charCount(cp)
        }
        return s.substring(0, i)
    }
}

/** A button on the watch: the "cmd" key's values (pebble/src/c/encore.c, CMD_*). */
enum class PebbleCommand(val wire: Int) {
    /** The watchapp opened (and asks again until the phone answers). */
    HELLO(1),
    PREVIOUS(2),
    PLAY_PAUSE(3),
    NEXT(4),

    /** A long press of select: Like, or Unlike when the song is liked already. */
    LIKE(5),
    ;

    /**
     * What the phone does, as the Wear OS watch's commands ([WearCommand]), from what it
     * last sent the watch. Null for [HELLO], which only starts the updates.
     */
    fun toWear(isPlaying: Boolean, isLiked: Boolean): WearCommand? = when (this) {
        HELLO -> null
        PREVIOUS -> WearCommand.Previous
        NEXT -> WearCommand.Next
        PLAY_PAUSE -> if (isPlaying) WearCommand.Pause else WearCommand.Play
        LIKE -> if (isLiked) WearCommand.Unlike else WearCommand.Like
    }

    companion object {
        /** The command in a message from the watch, or null when it carries none. */
        fun of(tuples: List<PebbleTuple>): PebbleCommand? {
            val cmd = tuples.firstOrNull { it.key == PebbleKeys.CMD } as? PebbleTuple.Number ?: return null
            return entries.firstOrNull { it.wire.toLong() == cmd.value }
        }
    }
}
