package uk.gcartee.encore.app.playback.tv

import java.io.ByteArrayInputStream
import java.io.FileNotFoundException
import java.net.HttpURLConnection
import java.net.InetAddress
import java.net.URL
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * ADR android-0007: the phone's relay. A pass per session, the session's songs only, Range
 * support, and a bare 404 for everything else. Runs a real relay on the loopback address.
 */
class PhoneRelayTest {
    private val song = ByteArray(10_000) { (it % 251).toByte() }
    private val relays = mutableListOf<PhoneRelay>()

    private fun bytes(data: ByteArray, type: String, knownLength: Boolean = true) = PhoneRelay.Source { offset ->
        val from = offset.coerceIn(0, data.size.toLong()).toInt()
        PhoneRelay.Opened(ByteArrayInputStream(data, from, data.size - from), if (knownLength) data.size.toLong() else null, type)
    }

    private fun relay(): PhoneRelay =
        PhoneRelay(InetAddress.getLoopbackAddress()).start().also { relays += it }

    @After
    fun closeAll() = relays.forEach { it.close() }

    private class Answer(val code: Int, val headers: Map<String, String>, val body: ByteArray)

    private fun get(url: String, range: String? = null, method: String = "GET"): Answer {
        val c = URL(url).openConnection() as HttpURLConnection
        c.requestMethod = method
        c.connectTimeout = 5_000
        c.readTimeout = 5_000
        range?.let { c.setRequestProperty("Range", it) }
        val code = c.responseCode
        val headers = c.headerFields.filterKeys { it != null }.mapKeys { it.key.lowercase() }.mapValues { it.value.first() }
        val body = (if (code < 400) c.inputStream else c.errorStream)?.use { it.readBytes() } ?: ByteArray(0)
        c.disconnect()
        return Answer(code, headers, body)
    }

    private fun passOf(relay: PhoneRelay): String = URL(relay.songUrl(0)).path.split('/')[1]

    // --- Range parsing (RFC 9110, 14.1.2). ---

    @Test
    fun rangesAreReadAsOneSpanOrIgnored() {
        assertEquals(ByteRange.Span(0, 499), ByteRange.parse("bytes=0-499"))
        assertEquals(ByteRange.Span(500, null), ByteRange.parse("bytes=500-"))
        assertEquals(ByteRange.Suffix(200), ByteRange.parse("bytes=-200"))
        assertEquals(ByteRange.Span(7, 7), ByteRange.parse(" bytes = 7-7".replace("bytes = ", "bytes=")))
        assertEquals(ByteRange.Span(3, 9), ByteRange.parse("BYTES=3-9"))
        // Not one span: the whole song is sent (200), as the RFC allows.
        assertNull(ByteRange.parse(null))
        assertNull(ByteRange.parse(""))
        assertNull(ByteRange.parse("bytes=0-1,5-6"))
        assertNull(ByteRange.parse("items=0-5"))
        assertNull(ByteRange.parse("bytes=9-3"))
        assertNull(ByteRange.parse("bytes=-"))
        assertNull(ByteRange.parse("bytes=a-b"))
        assertNull(ByteRange.parse("bytes=1-2-3"))
        assertNull(ByteRange.parse("bytes=99999999999999999999-"))
    }

    @Test
    fun aSuffixIsTheLastBytes() {
        assertEquals(ByteRange.Span(9_800, 9_999), ByteRange.Suffix(200).from(10_000))
        assertEquals(ByteRange.Span(0, 9), ByteRange.Suffix(50).from(10))
        assertNull(ByteRange.Suffix(0).from(10))
    }

    // --- The pass and the routes. ---

    @Test
    fun everySessionHasItsOwn128BitPass() {
        val a = relay()
        val b = relay()
        a.add(bytes(song, "audio/flac"), null)
        b.add(bytes(song, "audio/flac"), null)
        val passA = passOf(a)
        val passB = passOf(b)
        assertTrue(passA.matches(Regex("[0-9a-f]{32}")))
        assertNotEquals(passA, passB)
    }

    @Test
    fun onlyTheSessionsSongsAndCoversHaveARoute() {
        val r = relay()
        r.add(bytes(song, "audio/flac"), bytes(byteArrayOf(1, 2), "image/jpeg"))
        r.add(bytes(song, "audio/mpeg"), null)
        val pass = passOf(r)
        assertEquals(PhoneRelay.Route.Song(0), r.route("GET", "/$pass/song/0"))
        assertEquals(PhoneRelay.Route.Song(1), r.route("HEAD", "/$pass/song/1"))
        assertEquals(PhoneRelay.Route.Art(0), r.route("GET", "/$pass/art/0"))
        // Beyond the session's songs, a song with no cover, other shapes and methods: nothing.
        assertNull(r.route("GET", "/$pass/song/2"))
        assertNull(r.route("GET", "/$pass/art/1"))
        assertNull(r.route("GET", "/$pass/song/-1"))
        assertNull(r.route("GET", "/$pass/song/+1"))
        assertNull(r.route("GET", "/$pass/song/0/"))
        assertNull(r.route("GET", "/$pass/song/0?x=1"))
        assertNull(r.route("GET", "/$pass/song"))
        assertNull(r.route("GET", "/$pass/lyrics/0"))
        assertNull(r.route("GET", "/song/0"))
        assertNull(r.route("GET", "/"))
        assertNull(r.route("POST", "/$pass/song/0"))
        assertNull(r.route("DELETE", "/$pass/song/0"))
        assertNull(r.route("GET", "/${pass.uppercase()}/song/0"))
    }

    @Test
    fun anUnknownPathGetsABare404() {
        val r = relay()
        r.add(bytes(song, "audio/flac"), null)
        val base = "http://127.0.0.1:${r.port}"
        for (path in listOf("/", "/favicon.ico", "/${passOf(r)}/song/1", "/${passOf(r)}/../song/0", "/${passOf(r)}/art/0")) {
            val answer = get(base + path)
            assertEquals(path, 404, answer.code)
            assertEquals(path, 0, answer.body.size)
        }
    }

    @Test
    fun anOldPassIsRefused() {
        val old = relay()
        old.add(bytes(song, "audio/flac"), null)
        val oldPass = passOf(old)
        old.close()
        val now = relay()
        now.add(bytes(song, "audio/flac"), null)
        val answer = get("http://127.0.0.1:${now.port}/$oldPass/song/0")
        assertEquals(404, answer.code)
        assertEquals(200, get(now.songUrl(0).replace(InetAddress.getLoopbackAddress().hostAddress!!, "127.0.0.1")).code)
    }

    // --- Serving bytes. ---

    private fun url(r: PhoneRelay, n: Int, art: Boolean = false) =
        (if (art) r.artUrl(n) else r.songUrl(n)).replace(InetAddress.getLoopbackAddress().hostAddress!!, "127.0.0.1")

    @Test
    fun theWholeSongWithItsLengthAndType() {
        val r = relay()
        r.add(bytes(song, "audio/flac"), null)
        val answer = get(url(r, 0))
        assertEquals(200, answer.code)
        assertEquals("10000", answer.headers["content-length"])
        assertEquals("audio/flac", answer.headers["content-type"])
        assertEquals("bytes", answer.headers["accept-ranges"])
        assertArrayEquals(song, answer.body)
    }

    @Test
    fun aRangeGets206WithContentRange() {
        val r = relay()
        r.add(bytes(song, "audio/flac"), null)
        val answer = get(url(r, 0), "bytes=100-199")
        assertEquals(206, answer.code)
        assertEquals("bytes 100-199/10000", answer.headers["content-range"])
        assertEquals("100", answer.headers["content-length"])
        assertArrayEquals(song.copyOfRange(100, 200), answer.body)

        val open = get(url(r, 0), "bytes=9990-")
        assertEquals(206, open.code)
        assertEquals("bytes 9990-9999/10000", open.headers["content-range"])
        assertArrayEquals(song.copyOfRange(9_990, 10_000), open.body)

        val past = get(url(r, 0), "bytes=9000-20000")
        assertEquals("bytes 9000-9999/10000", past.headers["content-range"])
        assertEquals(1_000, past.body.size)

        val suffix = get(url(r, 0), "bytes=-10")
        assertEquals(206, suffix.code)
        assertEquals("bytes 9990-9999/10000", suffix.headers["content-range"])
    }

    @Test
    fun aRangePastTheEndIs416() {
        val r = relay()
        r.add(bytes(song, "audio/flac"), null)
        val answer = get(url(r, 0), "bytes=10000-")
        assertEquals(416, answer.code)
        assertEquals("bytes */10000", answer.headers["content-range"])
    }

    @Test
    fun headSendsTheHeadersOnly() {
        val r = relay()
        r.add(bytes(song, "audio/mpeg"), null)
        val answer = get(url(r, 0), method = "HEAD")
        assertEquals(200, answer.code)
        assertEquals("10000", answer.headers["content-length"])
        assertEquals(0, answer.body.size)
    }

    @Test
    fun aSongOfUnknownLengthStreamsWhole() {
        val r = relay()
        r.add(bytes(song, "audio/aac", knownLength = false), null)
        val answer = get(url(r, 0))
        assertEquals(200, answer.code)
        assertNull(answer.headers["content-length"])
        assertArrayEquals(song, answer.body)
        val span = get(url(r, 0), "bytes=10-19")
        assertEquals(206, span.code)
        assertEquals("bytes 10-19/*", span.headers["content-range"])
        assertArrayEquals(song.copyOfRange(10, 20), span.body)
    }

    @Test
    fun theCoverAndAMissingCover() {
        val r = relay()
        val cover = byteArrayOf(0xFF.toByte(), 0xD8.toByte(), 1, 2, 3)
        r.add(bytes(song, "audio/flac"), bytes(cover, "image/jpeg"))
        r.add(bytes(song, "audio/flac"), PhoneRelay.Source { throw FileNotFoundException("none") })
        val answer = get(url(r, 0, art = true))
        assertEquals(200, answer.code)
        assertEquals("image/jpeg", answer.headers["content-type"])
        assertArrayEquals(cover, answer.body)
        assertEquals(404, get(url(r, 1, art = true)).code)
    }

    @Test
    fun dlnaRenderersAreToldTheyMaySeek() {
        val r = relay()
        r.add(bytes(song, "audio/flac"), null)
        val c = URL(url(r, 0)).openConnection() as HttpURLConnection
        c.setRequestProperty("getcontentFeatures.dlna.org", "1")
        assertEquals(200, c.responseCode)
        assertTrue(c.getHeaderField("contentFeatures.dlna.org").startsWith("DLNA.ORG_OP=01"))
        assertEquals("Streaming", c.getHeaderField("transferMode.dlna.org"))
        c.inputStream.use { it.readBytes() }
        c.disconnect()
    }

    @Test
    fun aClosedSessionServesNothing() {
        val r = relay()
        r.add(bytes(song, "audio/flac"), null)
        val address = url(r, 0)
        r.close()
        assertTrue(r.closed)
        assertEquals(0, r.size)
        val refused = runCatching { get(address) }
        assertTrue("a closed relay still answered", refused.isFailure || refused.getOrNull()?.code == 404)
    }

    @Test
    fun theSessionEndsAfterBeingIdle() {
        var clock = 0L
        val ended = CountDownLatch(1)
        val r = PhoneRelay(InetAddress.getLoopbackAddress(), now = { clock }, idleMs = 1_000, watchMs = 20).also { relays += it }
        r.onIdle = { ended.countDown() }
        r.start()
        r.add(bytes(song, "audio/flac"), null)
        // Played on: touched, not idle.
        Thread.sleep(80)
        r.touch()
        assertTrue(!r.closed)
        clock = 1_001
        assertTrue("the idle session didn't end", ended.await(5, TimeUnit.SECONDS))
        assertTrue(r.closed)
    }
}
