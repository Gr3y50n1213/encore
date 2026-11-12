package uk.gcartee.encore.app

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Phase 9 (WP6): widgets never start the core. Every file in `widgets/` except
 * WidgetSync.kt (which runs only in the app's process where the core already runs) must
 * not mention CoreHost or import the core's bindings. Comments count too, on purpose: the
 * rule stays simple to read.
 */
class WidgetSourceScanTest {
    private val dir = File("src/main/kotlin/uk/gcartee/encore/app/widgets")

    @Test
    fun onlyWidgetSyncTouchesTheCore() {
        assertTrue("no widgets folder at ${dir.absolutePath}", dir.isDirectory)
        val files = dir.walkTopDown().filter { it.isFile && it.extension == "kt" }.toList()
        assertTrue("found the widget sources", files.any { it.name == "NowPlayingWidget.kt" })
        // Phase 9 (WP7): the scan covers the Mixes and Recently played widgets too.
        for (name in listOf("MixesWidget.kt", "RecentWidget.kt", "CollectionUi.kt", "CollectionStates.kt")) {
            assertTrue("found $name", files.any { it.name == name })
        }
        // The widgets redesign (11 November 2026): the sizing model, the field and the shared chrome.
        for (name in listOf("WidgetSizing.kt", "WidgetField.kt", "WidgetChrome.kt")) {
            assertTrue("found $name", files.any { it.name == name })
        }
        val offenders = files
            .filter { it.name != "WidgetSync.kt" }
            .flatMap { file ->
                file.readLines().mapIndexedNotNull { i, line ->
                    val bad = line.contains("CoreHost") || Regex("""^\s*import\s+uk\.gcartee\.encore\.core\.""").containsMatchIn(line)
                    if (bad) "${file.name}:${i + 1}: ${line.trim()}" else null
                }
            }
        assertTrue("widgets must not touch the core:\n" + offenders.joinToString("\n"), offenders.isEmpty())
    }

    @Test
    fun theFieldMathWidgetsDrawWithTouchesNeitherTheCoreNorAndroid() {
        // ui/player/FieldMath.kt is shared by the app's field and every widget's background.
        val file = File("src/main/kotlin/uk/gcartee/encore/app/ui/player/FieldMath.kt")
        assertTrue("no ${file.absolutePath}", file.isFile)
        val text = file.readText()
        assertTrue("FieldMath must not touch the core", !text.contains("CoreHost") && !text.contains("uk.gcartee.encore.core."))
        assertTrue("FieldMath is plain Kotlin", !Regex("""^\s*import\s+(android|androidx)\.""", RegexOption.MULTILINE).containsMatchIn(text))
    }

    @Test
    fun widgetSyncIsTheOneThatDoes() {
        // Guards the guard: if WidgetSync stopped being the bridge, this scan would be moot.
        assertTrue(File(dir, "WidgetSync.kt").readText().contains("CoreHost"))
    }

    @Test
    fun noWidgetFileUsesTheNetworkOrAFileAddress() {
        val words = listOf("java.net.", "okhttp", "HttpURLConnection", "Uri.fromFile", "file://", "http://", "https://")
        val offenders = dir.walkTopDown().filter { it.isFile && it.extension == "kt" }.flatMap { file ->
            file.readLines().mapIndexedNotNull { i, line ->
                if (words.any { line.contains(it) }) "${file.name}:${i + 1}: ${line.trim()}" else null
            }
        }.toList()
        assertTrue("widgets read files only:\n" + offenders.joinToString("\n"), offenders.isEmpty())
    }
}
