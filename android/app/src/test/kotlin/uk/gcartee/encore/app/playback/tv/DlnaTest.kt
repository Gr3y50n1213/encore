package uk.gcartee.encore.app.playback.tv

import java.net.URL
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/**
 * T2 (ADR android-0007): SSDP, a renderer's description, and the AVTransport and
 * RenderingControl SOAP, against a recorded renderer. The renderer is invented ("Kitchen
 * speaker"), and its answers are written in the UPnP specifications' own format.
 */
class DlnaTest {
    private val location = URL("http://192.168.1.40:49152/description.xml")

    private val description = """<?xml version="1.0" encoding="utf-8"?>
        <root xmlns="urn:schemas-upnp-org:device-1-0">
          <specVersion><major>1</major><minor>0</minor></specVersion>
          <device>
            <deviceType>urn:schemas-upnp-org:device:MediaServer:1</deviceType>
            <friendlyName>Shelf box</friendlyName>
            <UDN>uuid:00000000-0000-0000-0000-000000000001</UDN>
            <serviceList>
              <service>
                <serviceType>urn:schemas-upnp-org:service:ContentDirectory:1</serviceType>
                <controlURL>/cd/control</controlURL>
              </service>
            </serviceList>
            <deviceList>
              <device>
                <deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>
                <friendlyName>Kitchen speaker</friendlyName>
                <manufacturer>Example Audio</manufacturer>
                <modelName>Speaker One</modelName>
                <UDN>uuid:00000000-0000-0000-0000-000000000002</UDN>
                <serviceList>
                  <service>
                    <serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType>
                    <serviceId>urn:upnp-org:serviceId:RenderingControl</serviceId>
                    <controlURL>RenderingControl/control</controlURL>
                  </service>
                  <service>
                    <serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType>
                    <serviceId>urn:upnp-org:serviceId:AVTransport</serviceId>
                    <controlURL>/upnp/control/AVTransport</controlURL>
                  </service>
                </serviceList>
              </device>
            </deviceList>
          </device>
        </root>"""

    private fun envelope(body: String) =
        """<?xml version="1.0"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" """ +
            """s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/"><s:Body>$body</s:Body></s:Envelope>"""

    private val av = "urn:schemas-upnp-org:service:AVTransport:1"
    private val rc = "urn:schemas-upnp-org:service:RenderingControl:1"

    private val answers = mapOf(
        "GetTransportInfo" to envelope(
            """<u:GetTransportInfoResponse xmlns:u="$av"><CurrentTransportState>PLAYING</CurrentTransportState>""" +
                """<CurrentTransportStatus>OK</CurrentTransportStatus><CurrentSpeed>1</CurrentSpeed></u:GetTransportInfoResponse>""",
        ),
        "GetPositionInfo" to envelope(
            """<u:GetPositionInfoResponse xmlns:u="$av"><Track>1</Track><TrackDuration>0:03:41</TrackDuration>""" +
                """<TrackMetaData>NOT_IMPLEMENTED</TrackMetaData><TrackURI></TrackURI><RelTime>0:01:24.500</RelTime>""" +
                """<AbsTime>NOT_IMPLEMENTED</AbsTime><RelCount>2147483647</RelCount><AbsCount>2147483647</AbsCount></u:GetPositionInfoResponse>""",
        ),
        "GetVolume" to envelope("""<u:GetVolumeResponse xmlns:u="$rc"><CurrentVolume>35</CurrentVolume></u:GetVolumeResponse>"""),
    )

    private class Sent(val url: URL, val action: String, val body: String)

    private fun renderer(sent: MutableList<Sent>, reply: (String) -> SoapReply = { action ->
        SoapReply(200, answers[action] ?: envelope("<u:${action}Response xmlns:u=\"$av\"/>"))
    }): DlnaRenderer {
        val device = DlnaDescription.parse(description, location, StaxPullParser())!!
        return DlnaRenderer(device, { url, soapAction, body ->
            val action = soapAction.trim('"').substringAfter('#')
            sent += Sent(url, action, body)
            reply(action)
        }) { StaxPullParser() }
    }

    // --- The local network only. ---

    @Test
    fun onlyPrivateIpv4LiteralsAreLocal() {
        for (host in listOf("192.168.1.40", "10.0.0.2", "172.16.5.1", "172.31.255.255", "169.254.10.10")) {
            assertTrue(host, LocalNetwork.isLocalHost(host))
        }
        for (host in listOf("8.8.8.8", "172.32.0.1", "192.169.1.1", "example.org", "tv.local", "192.168.1", "192.168.1.256", "192.168.01.1", "", "::1")) {
            assertFalse(host, LocalNetwork.isLocalHost(host))
        }
        assertFalse(LocalNetwork.isLocal(URL("https://192.168.1.40/x")))
    }

    // --- SSDP. ---

    @Test
    fun theSearchAsksForMediaRenderers() {
        val message = Ssdp.searchMessage()
        assertTrue(message.startsWith("M-SEARCH * HTTP/1.1\r\n"))
        assertTrue(message.contains("HOST: 239.255.255.250:1900\r\n"))
        assertTrue(message.contains("MAN: \"ssdp:discover\"\r\n"))
        assertTrue(message.contains("ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n"))
        assertTrue(message.endsWith("\r\n\r\n"))
    }

    @Test
    fun aRenderersAnswerIsReadAndOthersPassedOver() {
        val answer = "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=1800\r\nEXT:\r\n" +
            "LOCATION: http://192.168.1.40:49152/description.xml\r\nSERVER: Linux/4.9 UPnP/1.0 Example/1.0\r\n" +
            "ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n" +
            "USN: uuid:00000000-0000-0000-0000-000000000002::urn:schemas-upnp-org:device:MediaRenderer:1\r\n\r\n"
        val hit = Ssdp.parseResponse(answer)
        assertNotNull(hit)
        assertEquals(location, hit!!.location)
        assertTrue(hit.usn.startsWith("uuid:00000000-0000-0000-0000-000000000002"))
        // Somewhere off the home network, another device type, not an answer.
        assertNull(Ssdp.parseResponse(answer.replace("192.168.1.40", "203.0.113.9")))
        assertNull(Ssdp.parseResponse(answer.replace("http://192.168.1.40", "http://tv.example.org")))
        assertNull(Ssdp.parseResponse(answer.replace("MediaRenderer:1\r\nUSN", "MediaServer:1\r\nUSN")))
        assertNull(Ssdp.parseResponse(answer.replace("HTTP/1.1 200 OK", "NOTIFY * HTTP/1.1")))
    }

    // --- The description. ---

    @Test
    fun theDescriptionGivesTheNameAndControlAddresses() {
        val device = DlnaDescription.parse(description, location, StaxPullParser())
        assertNotNull(device)
        assertEquals("Kitchen speaker", device!!.name)
        assertEquals("Speaker One", device.model)
        assertEquals("Example Audio", device.manufacturer)
        assertEquals("uuid:00000000-0000-0000-0000-000000000002", device.id)
        assertEquals(URL("http://192.168.1.40:49152/upnp/control/AVTransport"), device.avTransport)
        assertEquals(URL("http://192.168.1.40:49152/RenderingControl/control"), device.renderingControl)
    }

    @Test
    fun urlBaseIsUsedButNeverAnotherHost() {
        val withBase = description.replace("<specVersion>", "<URLBase>http://192.168.1.40:8080/</URLBase><specVersion>")
        val device = DlnaDescription.parse(withBase, location, StaxPullParser())!!
        assertEquals(URL("http://192.168.1.40:8080/upnp/control/AVTransport"), device.avTransport)
        // A description that sends control elsewhere (another host, the internet) is refused.
        val elsewhere = description.replace("<controlURL>/upnp/control/AVTransport</controlURL>", "<controlURL>http://203.0.113.9/av</controlURL>")
        assertNull(DlnaDescription.parse(elsewhere, location, StaxPullParser()))
        val otherLocal = description.replace("<controlURL>/upnp/control/AVTransport</controlURL>", "<controlURL>http://192.168.1.41/av</controlURL>")
        assertNull(DlnaDescription.parse(otherLocal, location, StaxPullParser()))
    }

    @Test
    fun aDeviceWithoutAvTransportIsNotARenderer() {
        val server = description.substringBefore("<deviceList>") + "</device></root>"
        assertNull(DlnaDescription.parse(server, location, StaxPullParser()))
    }

    // --- SOAP. ---

    @Test
    fun setAvTransportUriCarriesTheRelayAddressAndDidlLite() {
        val sent = mutableListOf<Sent>()
        val r = renderer(sent)
        val meta = TvMeta("Slow & Steady", "Marigold Ave", "Field <Notes>", "http://192.168.1.20:40000/0123/art/3", "audio/flac", 221_000)
        r.setUri("http://192.168.1.20:40000/0123/song/3", meta)
        val call = sent.single()
        assertEquals("SetAVTransportURI", call.action)
        assertEquals(URL("http://192.168.1.40:49152/upnp/control/AVTransport"), call.url)
        assertTrue(call.body.contains("<u:SetAVTransportURI xmlns:u=\"$av\">"))
        assertTrue(call.body.contains("<InstanceID>0</InstanceID>"))
        assertTrue(call.body.contains("<CurrentURI>http://192.168.1.20:40000/0123/song/3</CurrentURI>"))
        // The DIDL-Lite is itself escaped inside the argument.
        assertTrue(call.body.contains("&lt;dc:title&gt;Slow &amp;amp; Steady&lt;/dc:title&gt;"))
        assertTrue(call.body.contains("&lt;upnp:album&gt;Field &amp;lt;Notes&amp;gt;&lt;/upnp:album&gt;"))
        assertTrue(call.body.contains("http://192.168.1.20:40000/0123/art/3"))
        assertTrue(call.body.contains("protocolInfo=&quot;http-get:*:audio/flac:*&quot;"))
        assertTrue(call.body.contains("duration=&quot;0:03:41.000&quot;"))
        // The DIDL-Lite on its own parses as XML, with the cover.
        val didl = XmlNode.parse(Didl.item("http://x/song/3", meta), StaxPullParser())
        assertEquals("Slow & Steady", didl.descendants().first { it.name == "title" }.text)
        assertEquals("http://192.168.1.20:40000/0123/art/3", didl.descendants().first { it.name == "albumArtURI" }.text)
    }

    @Test
    fun transportAndVolumeActions() {
        val sent = mutableListOf<Sent>()
        val r = renderer(sent)
        r.play()
        r.pause()
        r.seek(84_500)
        r.stop()
        r.setVolume(140)
        assertEquals(listOf("Play", "Pause", "Seek", "Stop", "SetVolume"), sent.map { it.action })
        assertTrue(sent[0].body.contains("<Speed>1</Speed>"))
        assertTrue(sent[2].body.contains("<Unit>REL_TIME</Unit><Target>0:01:24</Target>"))
        assertEquals(URL("http://192.168.1.40:49152/RenderingControl/control"), sent[4].url)
        assertTrue(sent[4].body.contains("<Channel>Master</Channel><DesiredVolume>100</DesiredVolume>"))
    }

    @Test
    fun aPollReadsStatePositionAndVolume() {
        val sent = mutableListOf<Sent>()
        val status = renderer(sent).poll(withVolume = true)
        assertEquals(RendererState.PLAYING, status.state)
        assertEquals(84_500, status.positionMs)
        assertEquals(221_000, status.durationMs)
        assertEquals(35, status.volume)
        assertEquals(listOf("GetTransportInfo", "GetPositionInfo", "GetVolume"), sent.map { it.action })
        val quiet = renderer(mutableListOf()).poll(withVolume = false)
        assertNull(quiet.volume)
    }

    @Test
    fun transportStatesMap() {
        for ((upnp, ours) in listOf(
            "PAUSED_PLAYBACK" to RendererState.PAUSED,
            "STOPPED" to RendererState.STOPPED,
            "TRANSITIONING" to RendererState.TRANSITIONING,
            "NO_MEDIA_PRESENT" to RendererState.NO_MEDIA,
        )) {
            val r = renderer(mutableListOf()) { action ->
                SoapReply(200, (answers[action] ?: "").replace("<CurrentTransportState>PLAYING<", "<CurrentTransportState>$upnp<"))
            }
            assertEquals(upnp, ours, r.poll(withVolume = false).state)
        }
    }

    @Test
    fun aFaultIsAnErrorWithItsUpnpCode() {
        val fault = envelope(
            """<s:Fault><faultcode>s:Client</faultcode><faultstring>UPnPError</faultstring><detail>""" +
                """<UPnPError xmlns="urn:schemas-upnp-org:control-1-0"><errorCode>701</errorCode>""" +
                """<errorDescription>Transition not available</errorDescription></UPnPError></detail></s:Fault>""",
        )
        val r = renderer(mutableListOf()) { SoapReply(500, fault) }
        try {
            r.play()
            fail("a fault was taken for success")
        } catch (e: DlnaException) {
            assertEquals("Play", e.action)
            assertEquals(500, e.httpCode)
            assertEquals(701, e.upnpCode)
        }
        val garbage = renderer(mutableListOf()) { SoapReply(200, "not xml at all") }
        assertTrue(runCatching { garbage.stop() }.exceptionOrNull() is DlnaException)
    }

    @Test
    fun upnpTimes() {
        assertEquals("0:00:00", UpnpTime.format(0))
        assertEquals("0:03:41", UpnpTime.format(221_999))
        assertEquals("1:01:01", UpnpTime.format(3_661_000))
        assertEquals(84_500, UpnpTime.parse("0:01:24.500"))
        assertEquals(84_500, UpnpTime.parse("00:01:24.5"))
        assertEquals(3_661_000, UpnpTime.parse("1:01:01"))
        assertEquals(0, UpnpTime.parse("NOT_IMPLEMENTED"))
        assertEquals(0, UpnpTime.parse(null))
    }

    @Test
    fun audioTypesFromTheLibrarysContainer() {
        assertEquals("audio/flac", audioTypeOf("flac"))
        assertEquals("audio/mpeg", audioTypeOf("MP3"))
        assertEquals("audio/mp4", audioTypeOf("m4a"))
        assertEquals("audio/ogg", audioTypeOf("opus"))
        assertEquals("audio/mpeg", audioTypeOf(null))
        assertEquals("image/png", imageType(byteArrayOf(0x89.toByte(), 'P'.code.toByte(), 'N'.code.toByte(), 'G'.code.toByte(), 13, 10, 26, 10)))
        assertEquals("image/jpeg", imageType(byteArrayOf(0xFF.toByte(), 0xD8.toByte(), 0xFF.toByte())))
    }

    @Test
    fun deviceTypesCountAsTvs() {
        assertTrue(isTvPlatform("android-tv"))
        assertFalse(isTvPlatform("android"))
        assertFalse(isTvPlatform("ios"))
        assertFalse(isTvPlatform("linux"))
    }
}
