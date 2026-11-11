package uk.gcartee.encore.app

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Phase 9 (WP7): the history widgets can never be placed on the lock screen. Mixes and
 * Recently played declare `not_keyguard` (PLAN-ANDROID section 20) and never `keyguard`.
 * Read from the sources, so a change to the provider XML fails here, not on a phone.
 */
class WidgetProviderXmlTest {
    private val xml = File("src/main/res/xml")
    private val android = "http://schemas.android.com/apk/res/android"

    private fun category(name: String): Set<String> {
        val file = File(xml, name)
        assertTrue("no ${file.absolutePath}", file.isFile)
        val doc = DocumentBuilderFactory.newInstance().apply { isNamespaceAware = true }.newDocumentBuilder().parse(file)
        val value = doc.documentElement.getAttributeNS(android, "widgetCategory")
        return value.split('|').map { it.trim() }.filter { it.isNotEmpty() }.toSet()
    }

    @Test
    fun mixesAndRecentlyPlayedAreNeverOnTheLockScreen() {
        for (name in listOf("mixes_widget_info.xml", "recent_widget_info.xml")) {
            val c = category(name)
            assertTrue("$name declares not_keyguard", "not_keyguard" in c)
            assertTrue("$name is a home screen widget", "home_screen" in c)
            assertFalse("$name must not declare keyguard", "keyguard" in c)
        }
    }

    @Test
    fun nowPlayingMayBeOnTheLockScreen() {
        // Guards the guard: the one widget that may be on the lock screen still says so.
        assertEquals(setOf("home_screen", "keyguard"), category("now_playing_widget_info.xml"))
    }

    @Test
    fun everyWidgetReceiverIsExportedWithItsProvider() {
        val manifest = File("src/main/AndroidManifest.xml").readText()
        for ((receiver, info) in listOf(
            ".widgets.MixesWidgetReceiver" to "@xml/mixes_widget_info",
            ".widgets.RecentWidgetReceiver" to "@xml/recent_widget_info",
        )) {
            val block = manifest.substringAfter("android:name=\"$receiver\"").substringBefore("</receiver>")
            assertTrue("$receiver is exported", block.contains("android:exported=\"true\""))
            assertTrue("$receiver has the update filter", block.contains("android.appwidget.action.APPWIDGET_UPDATE"))
            assertTrue("$receiver names $info", block.contains(info))
        }
    }
}
