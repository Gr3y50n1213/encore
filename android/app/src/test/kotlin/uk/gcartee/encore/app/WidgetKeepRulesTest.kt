package uk.gcartee.encore.app

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * R8 merged the three widget classes into one in the release build (S20 FE, 12 November 2026:
 * the log named all three "yd2"), so a Now Playing redraw was handed the other widgets' ids and
 * drew its content in them. The debug build and the emulator tests never minify, so only this
 * check stands between the keep rule and a release that merges them again.
 */
class WidgetKeepRulesTest {
    private val rules = File("proguard-rules.pro")

    @Test
    fun theWidgetClassesAreKeptSoR8CannotMergeThem() {
        assertTrue("no rules file at ${rules.absolutePath}", rules.isFile)
        val text = rules.readLines().filterNot { it.trim().startsWith("#") }.joinToString("\n")
        assertTrue(
            "keep every GlanceAppWidget subclass",
            Regex("""-keep\s+class\s+\*\s+extends\s+androidx\.glance\.appwidget\.GlanceAppWidget\s*\{""").containsMatchIn(text),
        )
        assertTrue(
            "keep every GlanceAppWidgetReceiver subclass",
            Regex("""-keep\s+class\s+\*\s+extends\s+androidx\.glance\.appwidget\.GlanceAppWidgetReceiver\s*\{""").containsMatchIn(text),
        )
    }
}
