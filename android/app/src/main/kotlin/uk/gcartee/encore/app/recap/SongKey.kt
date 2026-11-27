package uk.gcartee.encore.app.recap

import java.security.MessageDigest
import java.text.Normalizer
import java.util.Locale
import uk.gcartee.encore.tokens.EncoreTokens.Recap.Counting

/**
 * The stable song key the recap counts by (docs/adr/core-0004-song-key.md,
 * core/encore-sources/src/song_key.rs), recomputed here because the core's API gives no
 * song for a key: Save to library matches the Top Songs playlist's keys to library songs.
 * `t1:<first 32 hex digits of SHA-256(artist 0x1F title)>:<seconds>`, names normalized
 * (NFKC, lower case, whitespace collapsed).
 */
object SongKey {
    fun normalize(s: String): String =
        Normalizer.normalize(s, Normalizer.Form.NFKC).lowercase(Locale.ROOT).trim().split(Regex("(?U)\\s+")).filter { it.isNotEmpty() }.joinToString(" ")

    fun t1(artist: String, title: String, durationMs: Long): String {
        val md = MessageDigest.getInstance("SHA-256")
        md.update(normalize(artist).toByteArray(Charsets.UTF_8))
        md.update(0x1F.toByte())
        md.update(normalize(title).toByteArray(Charsets.UTF_8))
        val hex = md.digest().take(16).joinToString("") { b -> (b.toInt() and 0xFF).toString(16).padStart(2, '0') }
        return "t1:$hex:${(durationMs + 500) / 1000}"
    }

    /** A `t1` key's hash and seconds; null for an `mb:` key or anything else. */
    fun split(key: String): Pair<String, Long>? {
        val rest = key.removePrefix("t1:").takeIf { key.startsWith("t1:") } ?: return null
        val hash = rest.substringBefore(':', "")
        val secs = rest.substringAfter(':', "").toLongOrNull() ?: return null
        return if (hash.isEmpty()) null else hash to secs
    }

    /** The same song: equal keys, or `t1` keys whose lengths differ by 2 s or less. */
    fun same(a: String, b: String): Boolean {
        if (a == b) return true
        val x = split(a) ?: return false
        val y = split(b) ?: return false
        return x.first == y.first && kotlin.math.abs(x.second - y.second) <= Counting.sameSongDurationToleranceSeconds
    }
}
