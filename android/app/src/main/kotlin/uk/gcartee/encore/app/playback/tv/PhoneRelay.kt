package uk.gcartee.encore.app.playback.tv

import java.io.BufferedOutputStream
import java.io.Closeable
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.SocketTimeoutException
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.Collections
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.SynchronousQueue
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit

/**
 * ADR android-0007: the phone relays the music to a TV. A tiny HTTP/1.1 server on the
 * phone's Wi-Fi address only (never every address), on a free port the system picks. Every
 * session has its own random 128-bit pass, and the TV gets nothing but
 * `http://<phone>:<port>/<pass>/song/<n>` and `/<pass>/art/<n>`, where `n` is a song this
 * session was asked to play. The media server's address and key stay on the phone: the
 * phone fetches each song itself ([Source]) and hands the bytes on, with Range support so
 * the TV can seek.
 *
 * Every other path, another session's pass and any other method get a bare 404. Nothing is
 * logged that could open the music: no address, pass, header or token (PUBLIC-REPO-CHECKLIST,
 * item 3), only the song's number and the status.
 *
 * The session ends with [close]: when the music comes back to the phone, stops, or the
 * playback service stops; and by itself after [idleMs] with no request and no [touch]
 * (10 minutes), when [onIdle] says so. Plain Java sockets and threads, so it runs, and is
 * tested, on the JVM.
 */
class PhoneRelay(
    private val bind: InetAddress,
    private val now: () -> Long = System::currentTimeMillis,
    private val log: (String) -> Unit = {},
    private val idleMs: Long = IDLE_MS,
    /** How often the idle time is checked. */
    private val watchMs: Long = WATCH_MS,
) : Closeable {
    /** Opens one song or cover at a byte offset: from the media server, a download or a file. */
    fun interface Source {
        @Throws(IOException::class)
        fun open(offset: Long): Opened
    }

    /** An open song: its bytes from the asked offset, the whole length if known, and its type. */
    class Opened(val stream: InputStream, val total: Long?, val contentType: String) : Closeable {
        override fun close() = stream.close()
    }

    private class Entry(val song: Source, val art: Source?)

    /** The pass, 32 hex digits from 16 random bytes; a new one for every relay. */
    private val pass: String = newPass()
    private val passBytes = pass.toByteArray(Charsets.US_ASCII)
    private val entries = CopyOnWriteArrayList<Entry>()
    private val open = Collections.newSetFromMap(java.util.concurrent.ConcurrentHashMap<Socket, Boolean>())
    private val workers = ThreadPoolExecutor(0, MAX_CONNECTIONS, 30, TimeUnit.SECONDS, SynchronousQueue()) { r ->
        Thread(r, "EncoreRelay").apply { isDaemon = true }
    }
    private var server: ServerSocket? = null
    private var acceptor: Thread? = null

    @Volatile
    private var lastUse = now()

    @Volatile
    var closed = false
        private set

    /** Called once, on the relay's own thread, when the session ended by itself (idle). */
    @Volatile
    var onIdle: (() -> Unit)? = null

    /** The port the TV connects to; 0 until [start]. */
    val port: Int get() = server?.localPort ?: 0

    /** Opens the port on the Wi-Fi address and starts answering. */
    fun start(): PhoneRelay {
        check(server == null && !closed) { "already started" }
        val socket = ServerSocket()
        socket.reuseAddress = true
        // Port 0: the system picks a free one. Bound to the one address, so no other
        // network the phone is on (mobile data, a VPN) can reach it.
        socket.bind(InetSocketAddress(bind, 0), BACKLOG)
        socket.soTimeout = watchMs.toInt().coerceAtLeast(1)
        server = socket
        lastUse = now()
        acceptor = Thread({ acceptLoop(socket) }, "EncoreRelayAccept").apply {
            isDaemon = true
            start()
        }
        return this
    }

    /** Adds a song of this session's queue; returns its number in the addresses. */
    fun add(song: Source, art: Source?): Int {
        entries.add(Entry(song, art))
        return entries.size - 1
    }

    /** How many songs this session may serve. */
    val size: Int get() = entries.size

    private val base: String
        get() = "http://${bind.hostAddress}:$port/$pass"

    fun songUrl(n: Int): String = "$base/song/$n"

    fun artUrl(n: Int): String = "$base/art/$n"

    /** The TV is still playing (a long song read in one go asks for nothing for a while). */
    fun touch() {
        lastUse = now()
    }

    override fun close() {
        if (closed) return
        closed = true
        runCatching { server?.close() }
        open.forEach { runCatching { it.close() } }
        open.clear()
        workers.shutdownNow()
        entries.clear()
    }

    private fun acceptLoop(socket: ServerSocket) {
        while (!closed) {
            val client = try {
                socket.accept()
            } catch (timeout: SocketTimeoutException) {
                if (!closed && open.isEmpty() && now() - lastUse >= idleMs) {
                    log("relay: idle for ${idleMs / 60_000} min, session ended")
                    close()
                    onIdle?.invoke()
                }
                continue
            } catch (gone: IOException) {
                break // closed
            }
            lastUse = now()
            open.add(client)
            try {
                workers.execute { serve(client) }
            } catch (busy: Exception) {
                // Too many at once (or closing): the TV tries again.
                open.remove(client)
                runCatching { client.close() }
            }
        }
    }

    private fun serve(client: Socket) {
        try {
            client.soTimeout = READ_TIMEOUT_MS
            client.tcpNoDelay = true
            val request = readRequest(java.io.BufferedInputStream(client.getInputStream(), MAX_HEAD)) ?: return
            val out = BufferedOutputStream(client.getOutputStream(), BUFFER)
            respond(request, out)
            out.flush()
        } catch (hungUp: IOException) {
            // The TV hung up, or the session closed: nothing to report.
        } finally {
            lastUse = now()
            open.remove(client)
            runCatching { client.close() }
        }
    }

    /** What the TV asked for: one request per connection (every answer closes it). */
    internal class Request(val method: String, val target: String, val headers: Map<String, String>)

    internal sealed class Route {
        data class Song(val n: Int) : Route()
        data class Art(val n: Int) : Route()
    }

    /** The song or cover a request may have; null for anything else (a 404). */
    internal fun route(method: String, target: String): Route? {
        if (method != "GET" && method != "HEAD") return null
        val parts = target.split('/')
        // "", pass, kind, n: nothing more, nothing less (no query, no trailing slash).
        if (parts.size != 4 || parts[0].isNotEmpty()) return null
        if (!MessageDigest.isEqual(parts[1].toByteArray(Charsets.US_ASCII), passBytes)) return null
        val digits = parts[3]
        if (digits.isEmpty() || digits.length > 6 || !digits.all { it in '0'..'9' }) return null
        val n = digits.toInt()
        if (n >= entries.size) return null
        return when (parts[2]) {
            "song" -> Route.Song(n)
            "art" -> if (entries[n].art != null) Route.Art(n) else null
            else -> null
        }
    }

    private fun respond(request: Request, out: OutputStream) {
        val route = route(request.method, request.target)
        val head = request.method == "HEAD"
        val source = when (route) {
            is Route.Song -> entries.getOrNull(route.n)?.song
            is Route.Art -> entries.getOrNull(route.n)?.art
            null -> null
        }
        if (route == null || source == null) {
            status(out, 404, "Not Found")
            return
        }
        val label = when (route) {
            is Route.Song -> "song ${route.n}"
            is Route.Art -> "art ${route.n}"
        }
        val dlna = DlnaHeaders(request.headers, route is Route.Song)
        val asked = ByteRange.parse(request.headers["range"])
        val code = try {
            sendBody(source, asked, head, dlna, out)
        } catch (e: IOException) {
            if (e is java.io.FileNotFoundException) {
                status(out, 404, "Not Found")
                404
            } else {
                throw e
            }
        }
        log("relay: $label ${request.method} $code${if (asked != null) " (range)" else ""}")
    }

    /** Sends the bytes asked for; returns the status code sent. */
    private fun sendBody(source: Source, asked: ByteRange?, head: Boolean, dlna: DlnaHeaders, out: OutputStream): Int {
        // A suffix range ("the last n bytes") needs the length first.
        var range = asked
        if (range is ByteRange.Suffix) {
            val total = source.open(0).use { it.total }
            range = if (total == null) null else range.from(total)
            if (range == null && total != null) {
                status(out, 416, "Range Not Satisfiable", listOf("Content-Range" to "bytes */$total"))
                return 416
            }
        }
        val span = range as ByteRange.Span?
        val start = span?.start ?: 0L
        source.open(start).use { opened ->
            val total = opened.total
            val headers = mutableListOf(
                "Content-Type" to opened.contentType,
                "Accept-Ranges" to "bytes",
            )
            headers += dlna.answer(total != null)
            val code: Int
            val length: Long?
            when {
                span == null || (start == 0L && span.end == null && total == null) -> {
                    // The whole song; its length when the source knows it.
                    code = 200
                    length = total
                }
                total != null && start >= total -> {
                    status(out, 416, "Range Not Satisfiable", listOf("Content-Range" to "bytes */$total"))
                    return 416
                }
                total != null -> {
                    val end = minOf(span.end ?: (total - 1), total - 1)
                    code = 206
                    length = end - start + 1
                    headers += "Content-Range" to "bytes $start-$end/$total"
                }
                span.end != null -> {
                    // A transcode still arriving has no length yet: the asked span, of a whole unknown.
                    code = 206
                    length = span.end - start + 1
                    headers += "Content-Range" to "bytes $start-${span.end}/*"
                }
                else -> {
                    // "From here to the end" of a song whose end isn't known can't be answered.
                    status(out, 416, "Range Not Satisfiable", listOf("Content-Range" to "bytes */*"))
                    return 416
                }
            }
            if (length != null) headers += "Content-Length" to length.toString()
            writeHead(out, code, if (code == 200) "OK" else "Partial Content", headers)
            if (!head) copy(opened.stream, out, length)
            return code
        }
    }

    private fun copy(from: InputStream, to: OutputStream, limit: Long?) {
        val buffer = ByteArray(BUFFER)
        var left = limit ?: Long.MAX_VALUE
        while (left > 0 && !closed) {
            val read = from.read(buffer, 0, minOf(buffer.size.toLong(), left).toInt())
            if (read < 0) break
            to.write(buffer, 0, read)
            left -= read
            lastUse = now()
        }
    }

    private fun status(out: OutputStream, code: Int, reason: String, extra: List<Pair<String, String>> = emptyList()) {
        writeHead(out, code, reason, extra + ("Content-Length" to "0"))
    }

    private fun writeHead(out: OutputStream, code: Int, reason: String, headers: List<Pair<String, String>>) {
        val text = StringBuilder("HTTP/1.1 $code $reason\r\n")
        for ((name, value) in headers) text.append(name).append(": ").append(value).append("\r\n")
        text.append("Connection: close\r\nCache-Control: no-store\r\n\r\n")
        out.write(text.toString().toByteArray(Charsets.ISO_8859_1))
    }

    /** DLNA renderers ask for these to know they may seek (DLNA guidelines, 7.4.1.3). */
    private class DlnaHeaders(private val asked: Map<String, String>, private val song: Boolean) {
        fun answer(lengthKnown: Boolean): List<Pair<String, String>> {
            val out = mutableListOf<Pair<String, String>>()
            if (asked.containsKey("getcontentfeatures.dlna.org")) {
                // OP=01: byte seeks, when the length is known. FLAGS: streaming transfer mode.
                val op = if (lengthKnown) "01" else "00"
                out += "contentFeatures.dlna.org" to "DLNA.ORG_OP=$op;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000"
            }
            if (asked.containsKey("transfermode.dlna.org") || asked.containsKey("getcontentfeatures.dlna.org")) {
                out += "transferMode.dlna.org" to if (song) "Streaming" else "Interactive"
            }
            return out
        }
    }

    internal companion object {
        /** The ADR's 10 minutes. */
        const val IDLE_MS = 10 * 60_000L
        private const val WATCH_MS = 15_000L
        private const val BACKLOG = 8
        private const val MAX_CONNECTIONS = 6
        private const val READ_TIMEOUT_MS = 20_000
        private const val BUFFER = 64 * 1024
        private const val MAX_HEAD = 16 * 1024

        private val random = SecureRandom()

        fun newPass(): String {
            val bytes = ByteArray(16)
            random.nextBytes(bytes)
            val hex = "0123456789abcdef"
            return buildString(32) {
                for (b in bytes) {
                    append(hex[(b.toInt() shr 4) and 0xf])
                    append(hex[b.toInt() and 0xf])
                }
            }
        }

        /** The request line and headers (names lower-cased); null if it isn't one. */
        fun readRequest(input: InputStream): Request? {
            val head = StringBuilder()
            var last4 = 0
            while (head.length < MAX_HEAD) {
                val b = input.read()
                if (b < 0) return null
                head.append(b.toChar())
                last4 = (last4 shl 8) or b
                if (last4 == 0x0d0a0d0a) break
            }
            val lines = head.toString().split("\r\n")
            val first = lines.firstOrNull()?.split(' ') ?: return null
            if (first.size != 3 || !first[2].startsWith("HTTP/1.")) return null
            val headers = HashMap<String, String>()
            for (line in lines.drop(1)) {
                val colon = line.indexOf(':')
                if (colon <= 0) continue
                headers[line.substring(0, colon).trim().lowercase()] = line.substring(colon + 1).trim()
            }
            return Request(first[0], first[1], headers)
        }
    }
}

/**
 * A `Range: bytes=...` header (RFC 9110, 14.1.2): one span only. Anything else, several
 * spans or another unit, is treated as no range at all, which the RFC allows: the whole song
 * is sent with 200.
 */
sealed class ByteRange {
    /** `bytes=a-b`, or `bytes=a-` (end null: to the end). */
    data class Span(val start: Long, val end: Long?) : ByteRange()

    /** `bytes=-n`: the last n bytes. */
    data class Suffix(val length: Long) : ByteRange() {
        /** The span it is in a song of [total] bytes; null when it is empty. */
        fun from(total: Long): Span? =
            if (length <= 0 || total <= 0) null else Span(maxOf(0, total - length), total - 1)
    }

    companion object {
        fun parse(header: String?): ByteRange? {
            val value = header?.trim() ?: return null
            if (!value.startsWith("bytes=", ignoreCase = true)) return null
            val spec = value.substring(6).trim()
            if (spec.contains(',')) return null
            val dash = spec.indexOf('-')
            if (dash < 0) return null
            val a = spec.substring(0, dash).trim()
            val b = spec.substring(dash + 1).trim()
            if (!a.all(Char::isDigit) || !b.all(Char::isDigit) || a.length > 18 || b.length > 18) return null
            return when {
                a.isEmpty() && b.isEmpty() -> null
                a.isEmpty() -> Suffix(b.toLong())
                b.isEmpty() -> Span(a.toLong(), null)
                else -> {
                    val start = a.toLong()
                    val end = b.toLong()
                    if (end < start) null else Span(start, end)
                }
            }
        }
    }
}
