package uk.gcartee.encore.app.playback.tv

import java.net.Inet4Address
import java.net.InetAddress
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/** The search sent to each address when a router doesn't pass multicast (ADR android-0007). */
class SsdpSweepTest {
    @Test
    fun the_sweep_covers_the_network_but_not_the_phone() {
        val own = InetAddress.getByName("192.168.1.37") as Inet4Address
        val targets = Ssdp.sweepTargets(own).map { it.hostAddress }
        assertEquals(253, targets.size)
        assertEquals("192.168.1.1", targets.first())
        assertEquals("192.168.1.254", targets.last())
        assertFalse("192.168.1.37" in targets)
        assertFalse("192.168.1.0" in targets || "192.168.1.255" in targets)
    }
}
