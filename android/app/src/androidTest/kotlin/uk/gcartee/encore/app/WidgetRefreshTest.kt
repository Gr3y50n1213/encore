package uk.gcartee.encore.app

import android.appwidget.AppWidgetHost
import android.appwidget.AppWidgetHostView
import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.os.Build
import android.os.Bundle
import android.os.ParcelFileDescriptor
import android.util.Log
import android.util.SizeF
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.glance.appwidget.updateAll
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uk.gcartee.encore.app.widgets.MixesWidget
import uk.gcartee.encore.app.widgets.MixesWidgetReceiver
import uk.gcartee.encore.app.widgets.NowPlayingWidget
import uk.gcartee.encore.app.widgets.NowPlayingWidgetReceiver
import uk.gcartee.encore.app.widgets.RecentWidget
import uk.gcartee.encore.app.widgets.RecentWidgetReceiver
import uk.gcartee.encore.app.widgets.SnapshotReader
import uk.gcartee.encore.app.widgets.WidgetConfig
import uk.gcartee.encore.app.widgets.WidgetProfileChoice
import uk.gcartee.encore.app.widgets.WidgetRedraws
import uk.gcartee.encore.app.widgets.WidgetRefresh

/**
 * Found on the S20 FE on 12 November 2026: once the app had run, the Mixes and Recently played
 * widgets drew Now Playing's content. The app redraws every widget through [WidgetRefresh]
 * (what WidgetSync calls), so this places all three at once, as a home screen does, redraws
 * them that way and checks each one shows its own words. The gallery test could not catch it:
 * it hosts each widget on its own and never redraws them together.
 */
@RunWith(AndroidJUnit4::class)
class WidgetRefreshTest {
    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context: Context get() = instrumentation.targetContext
    private val density get() = context.resources.displayMetrics.density

    private class Placed(val name: String, val receiver: Class<*>, val w: Int, val h: Int, val mustShow: String, val mustNotShow: List<String>)

    @After
    fun cleanUp() {
        File(SnapshotReader.widgetsDir(context.filesDir), PROFILE).deleteRecursively()
    }

    @Test
    fun redrawingAllThreeLeavesEachWithItsOwnContent() {
        val problems = placeAndDraw { WidgetRefresh.updateAll(context) }
        assertTrue("${problems.size} problem(s):\n" + problems.joinToString("\n"), problems.isEmpty())
    }

    /** What the app did before: one updateAll per class. Recorded in the log, never failing, so CI shows whether that was the cause. */
    @Test
    fun theOldPerClassRedrawIsRecorded() {
        val problems = placeAndDraw {
            NowPlayingWidget().updateAll(context)
            MixesWidget().updateAll(context)
            RecentWidget().updateAll(context)
        }
        Log.i("WidgetRefreshTest", "old per-class redraw: ${problems.size} problem(s): ${problems.joinToString("; ")}")
    }

    private fun placeAndDraw(redraw: suspend () -> Unit): List<String> {
        assumeTrue("needs app widgets", context.packageManager.hasSystemFeature(PackageManager.FEATURE_APP_WIDGETS))
        writeProfile()
        val placed = listOf(
            Placed("Now Playing", NowPlayingWidgetReceiver::class.java, 276, 102, "Slow Tide", listOf("Evening Drive", "Golden Hour")),
            Placed("Mixes", MixesWidgetReceiver::class.java, 276, 220, "Evening Drive", listOf("Slow Tide")),
            Placed("Recently played", RecentWidgetReceiver::class.java, 276, 220, "Golden Hour", listOf("Slow Tide", "Evening Drive")),
        )
        shell("appwidget grantbind --package ${context.packageName}")
        val host = AppWidgetHost(rule.activity, HOST_ID)
        runCatching { host.startListening() }
        val ids = mutableListOf<Int>()
        val views = mutableListOf<Pair<Placed, AppWidgetHostView>>()
        try {
            rule.runOnUiThread {
                val root = FrameLayout(rule.activity).apply { setPadding(0, (32 * density).toInt(), 0, 0) }
                val col = LinearLayout(rule.activity).apply { orientation = LinearLayout.VERTICAL }
                root.addView(col, FrameLayout.LayoutParams(-1, -1))
                rule.activity.setContentView(root)
                for (p in placed) {
                    val id = host.allocateAppWidgetId()
                    ids += id
                    check(AppWidgetManager.getInstance(context).bindAppWidgetIdIfAllowed(id, ComponentName(context, p.receiver))) { "${p.name} could not be bound" }
                    WidgetConfig.setChoice(context, id, WidgetProfileChoice.Fixed(PROFILE))
                    val view = host.createView(rule.activity, id, AppWidgetManager.getInstance(context).getAppWidgetInfo(id))
                    view.setPadding(0, 0, 0, 0)
                    val params = LinearLayout.LayoutParams((p.w * density).toInt(), (p.h * density).toInt())
                    params.bottomMargin = (8 * density).toInt()
                    col.addView(view, params)
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                        view.updateAppWidgetSize(Bundle(), listOf(SizeF(p.w.toFloat(), p.h.toFloat())))
                    } else {
                        @Suppress("DEPRECATION")
                        view.updateAppWidgetSize(null, p.w, p.h, p.w, p.h)
                    }
                    views += p to view
                }
            }
            // The widgets start drawing on their own; let those sessions run, then redraw the way the app does.
            Thread.sleep(SETTLE_MS)
            WidgetRedraws.bump()
            runBlocking { redraw() }
            Thread.sleep(SETTLE_MS)
            rule.waitForIdle()

            val problems = mutableListOf<String>()
            for ((p, view) in views) {
                val words = mutableListOf<String>()
                collect(view, words)
                if (words.none { it.contains(p.mustShow) }) problems += "${p.name} does not show \"${p.mustShow}\" (it shows: ${words.take(8)})"
                for (other in p.mustNotShow) {
                    if (words.any { it.contains(other) }) problems += "${p.name} shows \"$other\", which belongs to another widget"
                }
            }
            return problems
        } finally {
            rule.runOnUiThread { (rule.activity.window.decorView as? ViewGroup)?.removeAllViews() }
            WidgetConfig.forget(context, ids.toIntArray())
            ids.forEach { runCatching { host.deleteAppWidgetId(it) } }
            runCatching { host.stopListening() }
        }
    }

    /** Every piece of text and every content description in the drawn widget. */
    private fun collect(v: View, out: MutableList<String>) {
        (v as? TextView)?.text?.toString()?.takeIf { it.isNotBlank() }?.let(out::add)
        v.contentDescription?.toString()?.takeIf { it.isNotBlank() }?.let(out::add)
        if (v is ViewGroup) for (i in 0 until v.childCount) collect(v.getChildAt(i), out)
    }

    private fun writeProfile() {
        val dir = SnapshotReader.widgetsDir(context.filesDir)
        val folder = File(dir, PROFILE)
        folder.deleteRecursively()
        File(folder, "art").mkdirs()
        val bmp = Bitmap.createBitmap(64, 64, Bitmap.Config.ARGB_8888).apply { eraseColor(0xFFC01B66.toInt()) }
        File(folder, "art/c.img").outputStream().use { bmp.compress(Bitmap.CompressFormat.PNG, 100, it) }
        bmp.recycle()
        val palette = """{"scheme":"dark","base":"3D0000","stops":["CF4630","1B1030","C02D56","986E00"],"text":"FFFFFF"}"""
        fun mix(i: Int, name: String) = """{"mixId":"mix-$i","title":"$name","cover":"c.img","palette":$palette,"pinned":true}"""
        val albums = (0 until 6).joinToString(",") { """{"albumId":${it + 1},"title":"Golden Hour $it","artist":"Ember Cove","cover":"c.img","palette":$palette}""" }
        val now = System.currentTimeMillis()
        File(folder, "snapshot.json").writeText(
            """{"schema":1,"profileId":"$PROFILE","privacy":"normal",""" +
                """"song":{"songId":412,"title":"Slow Tide","artist":"Ember Cove","albumId":1,"albumTitle":"Static","cover":"c.img","palette":$palette,"durationMs":222000,"nowPlaying":true},""" +
                """"isPlaying":true,"positionMs":84000,"positionAtUtcMs":$now,""" +
                """"pinnedMixes":[${mix(1, "Evening Drive")},${mix(2, "Rainy Focus")},${mix(3, "Drive Home")},${mix(4, "Slow Morning")}],""" +
                """"nextUp":${mix(0, "Evening Drive").replace("true", "false")},"recentAlbums":[$albums],""" +
                """"reachable":true,"writtenAtUtcMs":$now,"recap":null}""",
        )
    }

    private fun shell(command: String): String {
        val pfd = instrumentation.uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(pfd).use { it.readBytes().toString(Charsets.UTF_8) }
    }

    private companion object {
        const val PROFILE = "refresh-demo"
        const val HOST_ID = 0x5159
        const val SETTLE_MS = 4_000L
    }
}
