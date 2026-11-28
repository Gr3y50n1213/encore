package uk.gcartee.encore.app.playback.tv

import java.io.IOException
import java.io.StringReader
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.HttpURLConnection
import java.net.Inet4Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.SocketTimeoutException
import java.net.URL
import org.xmlpull.v1.XmlPullParser

/*
 * T2 (ADR android-0007, "DLNA renderers"): TVs and speakers that play music sent to them,
 * found through SSDP and driven through UPnP's AVTransport and RenderingControl services.
 * The SOAP is written here by hand and the answers are read with the platform's
 * XmlPullParser: no library. Everything stays on the local network: a renderer whose address
 * isn't a private one (an IPv4 literal in 10/8, 172.16/12, 192.168/16 or 169.254/16) is
 * passed over, and its control addresses must be on the same host.
 */

/** Only addresses on the home network, written as IPv4 literals (never a name to look up). */
object LocalNetwork {
    fun isLocalHost(host: String?): Boolean {
        val parts = host?.split('.') ?: return false
        if (parts.size != 4) return false
        val bytes = parts.map { part ->
            if (part.isEmpty() || part.length > 3 || !part.all(Char::isDigit)) return false
            if (part.length > 1 && part[0] == '0') return false // no octal look-alikes
            part.toInt().takeIf { it in 0..255 } ?: return false
        }
        val (a, b) = bytes
        return a == 10 || (a == 172 && b in 16..31) || (a == 192 && b == 168) || (a == 169 && b == 254)
    }

    fun isLocal(url: URL): Boolean = url.protocol == "http" && isLocalHost(url.host)
}

/** One answer to the search: where the renderer's description is, and its unique name. */
data class SsdpHit(val location: URL, val usn: String)

/** SSDP (UPnP Device Architecture 2.0, 1.3): an M-SEARCH on the Wi-Fi's multicast address. */
object Ssdp {
    const val RENDERER = "urn:schemas-upnp-org:device:MediaRenderer:1"
    private const val GROUP = "239.255.255.250"
    private const val PORT = 1900
    private const val MX_SECONDS = 2

    fun searchMessage(target: String = RENDERER): String =
        "M-SEARCH * HTTP/1.1\r\n" +
            "HOST: $GROUP:$PORT\r\n" +
            "MAN: \"ssdp:discover\"\r\n" +
            "MX: $MX_SECONDS\r\n" +
            "ST: $target\r\n" +
            "USER-AGENT: Android UPnP/1.1 Encore/1\r\n" +
            "\r\n"

    /** A renderer's answer, or null for anything else (another device type, not local). */
    fun parseResponse(text: String, target: String = RENDERER): SsdpHit? {
        val lines = text.split("\r\n", "\n")
        if (lines.firstOrNull()?.trim()?.startsWith("HTTP/1.1 200", ignoreCase = true) != true) return null
        val headers = lines.drop(1).mapNotNull { line ->
            val colon = line.indexOf(':')
            if (colon <= 0) null else line.substring(0, colon).trim().lowercase() to line.substring(colon + 1).trim()
        }.toMap()
        if (headers["st"]?.equals(target, ignoreCase = true) != true) return null
        val location = runCatching { URL(headers["location"]) }.getOrNull() ?: return null
        if (!LocalNetwork.isLocal(location)) return null
        return SsdpHit(location, headers["usn"] ?: location.toString())
    }

    /**
     * Sends the search three times from [bind] (the Wi-Fi address) and collects answers for
     * [timeoutMs]. Blocking: never on the main thread. The caller holds a multicast lock.
     */
    fun search(bind: Inet4Address, timeoutMs: Long, target: String = RENDERER): List<SsdpHit> {
        val found = LinkedHashMap<String, SsdpHit>()
        DatagramSocket(null).use { socket ->
            socket.reuseAddress = true
            socket.bind(InetSocketAddress(bind, 0))
            val message = searchMessage(target).toByteArray(Charsets.US_ASCII)
            val group = InetSocketAddress(InetAddress.getByName(GROUP), PORT)
            repeat(3) { socket.send(DatagramPacket(message, message.size, group)) }
            val end = System.currentTimeMillis() + timeoutMs
            val buffer = ByteArray(4096)
            while (true) {
                val left = end - System.currentTimeMillis()
                if (left <= 0) break
                socket.soTimeout = left.toInt().coerceAtLeast(1)
                val packet = DatagramPacket(buffer, buffer.size)
                try {
                    socket.receive(packet)
                } catch (e: SocketTimeoutException) {
                    break
                }
                val text = String(packet.data, 0, packet.length, Charsets.ISO_8859_1)
                parseResponse(text, target)?.let { found.putIfAbsent(it.usn.substringBefore("::"), it) }
            }
        }
        return found.values.toList()
    }
}

/** A renderer from its description: its name and where to send AVTransport and volume commands. */
data class DlnaDevice(
    val id: String,
    val name: String,
    val model: String?,
    val manufacturer: String?,
    val avTransport: URL,
    val renderingControl: URL?,
)

/** A small element tree read with XmlPullParser (names without their namespace prefix). */
class XmlNode(val name: String) {
    val children = mutableListOf<XmlNode>()
    internal val textBuilder = StringBuilder()
    val text: String get() = textBuilder.toString().trim()

    fun child(name: String): XmlNode? = children.firstOrNull { it.name == name }

    fun childText(name: String): String? = child(name)?.text?.takeIf { it.isNotEmpty() }

    /** Every element below this one, depth first, in document order. */
    fun descendants(): Sequence<XmlNode> = sequence {
        for (c in children) {
            yield(c)
            yieldAll(c.descendants())
        }
    }

    companion object {
        private const val MAX_DEPTH = 32
        private const val MAX_NODES = 5_000

        /** Reads [xml] with [parser] (namespaces processed, no DTD); the document's root element. */
        fun parse(xml: String, parser: XmlPullParser): XmlNode {
            parser.setFeature(XmlPullParser.FEATURE_PROCESS_NAMESPACES, true)
            parser.setInput(StringReader(xml))
            val stack = ArrayDeque<XmlNode>()
            var root: XmlNode? = null
            var nodes = 0
            var event = parser.eventType
            while (event != XmlPullParser.END_DOCUMENT) {
                when (event) {
                    XmlPullParser.START_TAG -> {
                        if (stack.size >= MAX_DEPTH || ++nodes > MAX_NODES) throw IOException("XML too deep or too long")
                        val node = XmlNode(parser.name)
                        stack.lastOrNull()?.children?.add(node) ?: run { root = node }
                        stack.addLast(node)
                    }
                    XmlPullParser.TEXT -> stack.lastOrNull()?.textBuilder?.append(parser.text)
                    XmlPullParser.END_TAG -> stack.removeLastOrNull()
                }
                event = parser.next()
            }
            return root ?: throw IOException("empty XML")
        }
    }
}

/** Reads a renderer's description document (UPnP Device Architecture, 2.3). */
object DlnaDescription {
    const val AV_TRANSPORT = "urn:schemas-upnp-org:service:AVTransport:1"
    const val RENDERING_CONTROL = "urn:schemas-upnp-org:service:RenderingControl:1"

    /** The renderer in [xml] fetched from [location]; null if it can't play music sent to it. */
    fun parse(xml: String, location: URL, parser: XmlPullParser): DlnaDevice? {
        val root = XmlNode.parse(xml, parser)
        val base = root.childText("URLBase")?.let { runCatching { URL(it) }.getOrNull() } ?: location
        val devices = (if (root.name == "device") sequenceOf(root) else emptySequence()) + root.descendants().filter { it.name == "device" }
        val list = devices.toList()
        fun servicesOf(device: XmlNode) = device.child("serviceList")?.children.orEmpty().filter { it.name == "service" }
        fun find(device: XmlNode, type: String) = servicesOf(device).firstOrNull { it.childText("serviceType")?.startsWith(type.substringBeforeLast(':')) == true }
        val renderer = list.firstOrNull { it.childText("deviceType")?.contains(":MediaRenderer:") == true && find(it, AV_TRANSPORT) != null }
            ?: list.firstOrNull { find(it, AV_TRANSPORT) != null }
            ?: return null
        val av = find(renderer, AV_TRANSPORT)?.childText("controlURL")?.let { resolve(base, location, it) } ?: return null
        val rc = (find(renderer, RENDERING_CONTROL) ?: list.firstNotNullOfOrNull { find(it, RENDERING_CONTROL) })
            ?.childText("controlURL")?.let { resolve(base, location, it) }
        val name = renderer.childText("friendlyName") ?: list.firstNotNullOfOrNull { it.childText("friendlyName") } ?: "TV"
        return DlnaDevice(
            id = renderer.childText("UDN") ?: location.toString(),
            name = name.take(MAX_NAME),
            model = renderer.childText("modelName")?.take(MAX_NAME),
            manufacturer = renderer.childText("manufacturer")?.take(MAX_NAME),
            avTransport = av,
            renderingControl = rc,
        )
    }

    /** A control address, only if it is on the renderer's own host. */
    private fun resolve(base: URL, location: URL, path: String): URL? {
        val url = runCatching { URL(base, path) }.getOrNull() ?: return null
        return url.takeIf { LocalNetwork.isLocal(it) && it.host == location.host }
    }

    private const val MAX_NAME = 80
}

/** What a renderer said back: the HTTP status and the body. */
data class SoapReply(val code: Int, val body: String)

/** Posts one SOAP action. The real one is [HttpSoap]; tests give recorded answers. */
fun interface SoapTransport {
    @Throws(IOException::class)
    fun post(url: URL, soapAction: String, body: String): SoapReply
}

/** A renderer refused an action (a SOAP fault, with UPnP's error code when it gave one). */
class DlnaException(val action: String, val httpCode: Int, val upnpCode: Int?) :
    IOException("$action failed: HTTP $httpCode${upnpCode?.let { ", UPnP error $it" } ?: ""}")

/** SOAP 1.1 for UPnP actions (UPnP Device Architecture, 3.2), by hand. */
object Soap {
    fun envelope(serviceType: String, action: String, args: List<Pair<String, String>>): String {
        val body = StringBuilder()
        body.append("<?xml version=\"1.0\" encoding=\"utf-8\"?>")
        body.append("<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">")
        body.append("<s:Body><u:").append(action).append(" xmlns:u=\"").append(serviceType).append("\">")
        for ((name, value) in args) body.append('<').append(name).append('>').append(escape(value)).append("</").append(name).append('>')
        body.append("</u:").append(action).append("></s:Body></s:Envelope>")
        return body.toString()
    }

    /** The action's out arguments by name, or a [DlnaException] for a fault. */
    fun result(action: String, reply: SoapReply, parser: XmlPullParser): Map<String, String> {
        val root = runCatching { XmlNode.parse(reply.body, parser) }.getOrNull()
        val body = root?.child("Body")
        if (reply.code !in 200..299 || body == null || body.child("Fault") != null) {
            val code = root?.descendants()?.firstOrNull { it.name == "errorCode" }?.text?.toIntOrNull()
            throw DlnaException(action, reply.code, code)
        }
        val response = body.children.firstOrNull { it.name == "${action}Response" } ?: body.children.firstOrNull()
        return response?.children.orEmpty().associate { it.name to it.text }
    }

    fun escape(text: String): String {
        val out = StringBuilder(text.length + 16)
        for (c in text) {
            when (c) {
                '&' -> out.append("&amp;")
                '<' -> out.append("&lt;")
                '>' -> out.append("&gt;")
                '"' -> out.append("&quot;")
                '\'' -> out.append("&apos;")
                else -> if (c >= ' ' || c == '\t' || c == '\n' || c == '\r') out.append(c)
            }
        }
        return out.toString()
    }
}

/** UPnP's H+:MM:SS[.F+] times (AVTransport, 2.2.15). */
object UpnpTime {
    fun format(ms: Long): String {
        val total = (ms.coerceAtLeast(0) / 1000)
        // Locale.ROOT: ASCII digits whatever the phone's language.
        return String.format(java.util.Locale.ROOT, "%d:%02d:%02d", total / 3600, (total / 60) % 60, total % 60)
    }

    /** Milliseconds, or 0 for "NOT_IMPLEMENTED" and anything else that isn't a time. */
    fun parse(text: String?): Long {
        val parts = text?.trim()?.split(':') ?: return 0
        if (parts.size != 3) return 0
        val h = parts[0].toLongOrNull() ?: return 0
        val m = parts[1].toLongOrNull() ?: return 0
        val secs = parts[2].split('.')
        val s = secs[0].toLongOrNull() ?: return 0
        val fraction = secs.getOrNull(1)?.takeWhile(Char::isDigit)?.take(3)?.padEnd(3, '0')?.toLongOrNull() ?: 0
        return ((h * 60 + m) * 60 + s) * 1000 + fraction
    }
}

/** DIDL-Lite metadata for SetAVTransportURI: title, artist, album and the relay's cover. */
object Didl {
    fun item(url: String, meta: TvMeta): String {
        val e = Soap::escape
        val sb = StringBuilder()
        sb.append("<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"")
        sb.append(" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\" xmlns:dlna=\"urn:schemas-dlna-org:metadata-1-0/\">")
        sb.append("<item id=\"encore\" parentID=\"0\" restricted=\"1\">")
        sb.append("<dc:title>").append(e(meta.title)).append("</dc:title>")
        meta.artist?.let {
            sb.append("<dc:creator>").append(e(it)).append("</dc:creator>")
            sb.append("<upnp:artist>").append(e(it)).append("</upnp:artist>")
        }
        meta.album?.let { sb.append("<upnp:album>").append(e(it)).append("</upnp:album>") }
        meta.coverUrl?.let { sb.append("<upnp:albumArtURI dlna:profileID=\"JPEG_TN\">").append(e(it)).append("</upnp:albumArtURI>") }
        sb.append("<upnp:class>object.item.audioItem.musicTrack</upnp:class>")
        sb.append("<res protocolInfo=\"http-get:*:").append(e(meta.mimeType)).append(":*\"")
        if (meta.durationMs > 0) sb.append(" duration=\"").append(UpnpTime.format(meta.durationMs)).append(".000\"")
        sb.append('>').append(e(url)).append("</res>")
        sb.append("</item></DIDL-Lite>")
        return sb.toString()
    }
}

/** A DLNA renderer as a [Renderer]: AVTransport for the song, RenderingControl for the volume. */
class DlnaRenderer(
    private val device: DlnaDevice,
    private val transport: SoapTransport,
    private val parser: () -> XmlPullParser,
) : Renderer {
    override val name: String get() = device.name
    override val maxVolume: Int = 100

    private fun av(action: String, vararg args: Pair<String, String>): Map<String, String> =
        call(device.avTransport, DlnaDescription.AV_TRANSPORT, action, listOf("InstanceID" to "0") + args)

    private fun rc(action: String, vararg args: Pair<String, String>): Map<String, String>? {
        val url = device.renderingControl ?: return null
        return call(url, DlnaDescription.RENDERING_CONTROL, action, listOf("InstanceID" to "0", "Channel" to "Master") + args)
    }

    private fun call(url: URL, service: String, action: String, args: List<Pair<String, String>>): Map<String, String> {
        val reply = transport.post(url, "\"$service#$action\"", Soap.envelope(service, action, args))
        return Soap.result(action, reply, parser())
    }

    override fun setUri(url: String, meta: TvMeta) {
        av("SetAVTransportURI", "CurrentURI" to url, "CurrentURIMetaData" to Didl.item(url, meta))
    }

    override fun play() {
        av("Play", "Speed" to "1")
    }

    override fun pause() {
        av("Pause")
    }

    override fun seek(positionMs: Long) {
        av("Seek", "Unit" to "REL_TIME", "Target" to UpnpTime.format(positionMs))
    }

    override fun stop() {
        av("Stop")
    }

    override fun setVolume(level: Int) {
        rc("SetVolume", "DesiredVolume" to level.coerceIn(0, maxVolume).toString())
    }

    override fun poll(withVolume: Boolean): RendererStatus {
        val transportInfo = av("GetTransportInfo")
        val position = av("GetPositionInfo")
        val volume = if (withVolume) runCatching { rc("GetVolume")?.get("CurrentVolume")?.toIntOrNull() }.getOrNull() else null
        val state = when (transportInfo["CurrentTransportState"]?.uppercase()) {
            "PLAYING" -> RendererState.PLAYING
            "PAUSED_PLAYBACK", "PAUSED_RECORDING" -> RendererState.PAUSED
            "TRANSITIONING" -> RendererState.TRANSITIONING
            "NO_MEDIA_PRESENT" -> RendererState.NO_MEDIA
            else -> RendererState.STOPPED
        }
        return RendererStatus(
            state = state,
            positionMs = UpnpTime.parse(position["RelTime"]),
            durationMs = UpnpTime.parse(position["TrackDuration"]),
            volume = volume,
        )
    }
}

/** SOAP over HttpURLConnection, to a renderer on the local network only. */
object HttpSoap : SoapTransport {
    private const val TIMEOUT_MS = 4_000
    private const val MAX_BYTES = 256 * 1024

    override fun post(url: URL, soapAction: String, body: String): SoapReply {
        if (!LocalNetwork.isLocal(url)) throw IOException("not on the local network")
        val connection = url.openConnection() as HttpURLConnection
        try {
            connection.instanceFollowRedirects = false
            connection.connectTimeout = TIMEOUT_MS
            connection.readTimeout = TIMEOUT_MS
            connection.requestMethod = "POST"
            connection.doOutput = true
            connection.useCaches = false
            val bytes = body.toByteArray(Charsets.UTF_8)
            connection.setRequestProperty("Content-Type", "text/xml; charset=\"utf-8\"")
            connection.setRequestProperty("SOAPACTION", soapAction)
            connection.setFixedLengthStreamingMode(bytes.size)
            connection.outputStream.use { it.write(bytes) }
            val code = connection.responseCode
            val stream = if (code in 200..299) connection.inputStream else connection.errorStream
            val text = stream?.use { readCapped(it) }.orEmpty()
            return SoapReply(code, text)
        } finally {
            connection.disconnect()
        }
    }

    /** GETs a description document from the local network. */
    fun get(url: URL): String {
        if (!LocalNetwork.isLocal(url)) throw IOException("not on the local network")
        val connection = url.openConnection() as HttpURLConnection
        try {
            connection.instanceFollowRedirects = false
            connection.connectTimeout = TIMEOUT_MS
            connection.readTimeout = TIMEOUT_MS
            connection.useCaches = false
            if (connection.responseCode !in 200..299) throw IOException("description: HTTP ${connection.responseCode}")
            return connection.inputStream.use { readCapped(it) }
        } finally {
            connection.disconnect()
        }
    }

    private fun readCapped(input: java.io.InputStream): String {
        val out = java.io.ByteArrayOutputStream()
        val buffer = ByteArray(8192)
        while (true) {
            val n = input.read(buffer)
            if (n < 0) break
            out.write(buffer, 0, n)
            if (out.size() > MAX_BYTES) throw IOException("answer too long")
        }
        return out.toString("UTF-8")
    }
}
