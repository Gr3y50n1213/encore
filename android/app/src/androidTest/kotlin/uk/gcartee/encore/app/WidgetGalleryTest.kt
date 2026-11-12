package uk.gcartee.encore.app

import android.app.UiAutomation
import android.appwidget.AppWidgetHost
import android.appwidget.AppWidgetHostView
import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.BitmapDrawable
import android.graphics.Color
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Shader
import android.os.Build
import android.os.Bundle
import android.os.ParcelFileDescriptor
import android.util.SizeF
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import kotlin.math.abs
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uk.gcartee.encore.app.widgets.MixesSizing
import uk.gcartee.encore.app.widgets.MixesWidget
import uk.gcartee.encore.app.widgets.MixesWidgetReceiver
import uk.gcartee.encore.app.widgets.NowPlayingWidget
import uk.gcartee.encore.app.widgets.NowPlayingWidgetReceiver
import uk.gcartee.encore.app.widgets.NpLayout
import uk.gcartee.encore.app.widgets.RecentSizing
import uk.gcartee.encore.app.widgets.RecentWidget
import uk.gcartee.encore.app.widgets.RecentWidgetReceiver
import uk.gcartee.encore.app.widgets.SnapshotReader
import uk.gcartee.encore.app.widgets.WidgetConfig
import uk.gcartee.encore.app.widgets.WidgetProfileChoice

/**
 * The widgets redesign (11 November 2026), on an emulator: every widget, at every size it
 * declares and at the sizes a phone reaches, hosted for real (an AppWidgetHost, the
 * provider's own RemoteViews) and looked at.
 *
 * - **Nothing is cropped.** Every view of the drawn widget lies inside its parent: the
 *   test that would have caught the play button cut off at its top and bottom.
 * - **Buttons are touchable.** Play, Pause, Previous and Next are at least 48 dp each way.
 * - **The corners are the system's.** The pixel in each widget's corner is the page behind
 *   it, so the album field is clipped to the rounded tile.
 * - **Pages of screenshots** (encore-shot-widgets-*.png) for review.
 *
 * Demo data only: three made-up profiles written into the widget folder (a dark and a light
 * album, a stale file and a locked profile), never the person's library. Every problem is
 * collected and reported at the end, so one run shows them all.
 */
@RunWith(AndroidJUnit4::class)
class WidgetGalleryTest {
    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context: Context get() = instrumentation.targetContext
    private val automation: UiAutomation get() = instrumentation.uiAutomation
    private val density get() = context.resources.displayMetrics.density
    private val problems = mutableListOf<String>()
    private val demoProfiles = listOf("gallery-dark", "gallery-light", "gallery-stale", "gallery-locked", "gallery-few")

    private class Kind(val name: String, val receiver: Class<*>, val widget: GlanceAppWidget)

    private class Spec(val kind: Kind, val w: Int, val h: Int, val profile: String, val note: String = "")

    @After
    fun cleanUp() {
        val dir = SnapshotReader.widgetsDir(context.filesDir)
        demoProfiles.forEach { File(dir, it).deleteRecursively() }
    }

    @Test
    fun everyWidgetIsDrawnWholeAtEverySizeItMayBePlaced() {
        assumeTrue("needs app widgets", context.packageManager.hasSystemFeature(PackageManager.FEATURE_APP_WIDGETS))
        writeDemoProfiles()
        val nowPlaying = Kind("Now Playing", NowPlayingWidgetReceiver::class.java, NowPlayingWidget())
        val mixes = Kind("Mixes", MixesWidgetReceiver::class.java, MixesWidget())
        val recent = Kind("Recently played", RecentWidgetReceiver::class.java, RecentWidget())

        val specs = mutableListOf<Spec>()
        // Every bucket, and the sizes a 4-column phone reaches (Google's cell table; One UI's are for the owner's phones).
        for (l in NpLayout.entries) specs += Spec(nowPlaying, l.w, l.h, "gallery-dark", l.name)
        for ((w, h) in listOf(130 to 102, 276 to 102, 130 to 220, 276 to 220, 412 to 51, 412 to 117)) specs += Spec(nowPlaying, w, h, "gallery-dark", "phone")
        // The states and the light album.
        for (l in listOf(NpLayout.COMPACT, NpLayout.TILE, NpLayout.FULL_TALL)) specs += Spec(nowPlaying, l.w, l.h, "gallery-light", "light album")
        specs += Spec(nowPlaying, NpLayout.FULL_TALL.w, NpLayout.FULL_TALL.h, "gallery-stale", "stale")
        for (l in listOf(NpLayout.COMPACT, NpLayout.ROW, NpLayout.TILE, NpLayout.FULL)) specs += Spec(nowPlaying, l.w, l.h, "gallery-locked", "locked")
        for (b in MixesSizing.buckets) specs += Spec(mixes, b.w, b.h, "gallery-dark", "${b.cols}x${b.rows}${if (b.hero) " hero" else ""}")
        for ((w, h) in listOf(276 to 220, 276 to 337, 412 to 117, 530 to 184)) specs += Spec(mixes, w, h, "gallery-dark", "phone")
        specs += Spec(mixes, 250, 200, "gallery-stale", "stale")
        for (b in RecentSizing.buckets) specs += Spec(recent, b.w, b.h, "gallery-dark", "${b.cols}x${b.rows}")
        for ((w, h) in listOf(276 to 220, 276 to 337, 412 to 117)) specs += Spec(recent, w, h, "gallery-dark", "phone")
        specs += Spec(recent, 250, 190, "gallery-locked", "locked")
        // Fewer tiles than columns: the strip is centered, not left with a hole.
        specs += Spec(mixes, 310, 200, "gallery-few", "hero, one tile")
        specs += Spec(mixes, 410, 110, "gallery-few", "one mix")
        specs += Spec(recent, 410, 110, "gallery-few", "two albums")

        // Pages of widgets that fit one screen.
        val screenDp = context.resources.displayMetrics.heightPixels / density
        val pages = mutableListOf<MutableList<Spec>>()
        var used = 0f
        for (s in specs) {
            val need = s.h + CAPTION_DP + GAP_DP
            if (pages.isEmpty() || used + need > screenDp - TOP_DP - 40) {
                pages += mutableListOf<Spec>()
                used = 0f
            }
            pages.last() += s
            used += need
        }

        shell("appwidget grantbind --package ${context.packageName}")
        val host = AppWidgetHost(rule.activity, HOST_ID)
        runCatching { host.startListening() }
        try {
            pages.forEachIndexed { n, page -> drawPage(host, page, "widgets-%02d".format(n + 1)) }
        } finally {
            runCatching { host.stopListening() }
        }
        assertTrue("${problems.size} problem(s):\n" + problems.joinToString("\n"), problems.isEmpty())
    }

    // --- A page. ---

    private fun drawPage(host: AppWidgetHost, page: List<Spec>, shotName: String) {
        val ids = mutableListOf<Int>()
        val views = mutableListOf<Triple<Spec, AppWidgetHostView, Int>>()
        var column: LinearLayout? = null
        rule.runOnUiThread {
            // Under the status bar, so the clock doesn't draw over the first widget.
            val root = FrameLayout(rule.activity).apply {
                setBackgroundColor(MAGENTA)
                setPadding(0, (TOP_DP * density).toInt(), 0, 0)
            }
            val col = LinearLayout(rule.activity).apply { orientation = LinearLayout.VERTICAL }
            root.addView(col, FrameLayout.LayoutParams(-1, -1))
            rule.activity.setContentView(root)
            column = col
        }
        for (spec in page) {
            val id = host.allocateAppWidgetId()
            ids += id
            val bound = AppWidgetManager.getInstance(context).bindAppWidgetIdIfAllowed(id, ComponentName(context, spec.kind.receiver))
            if (!bound) {
                problems += "${label(spec)}: could not be bound"
                continue
            }
            WidgetConfig.setChoice(context, id, WidgetProfileChoice.Fixed(spec.profile))
            val info = AppWidgetManager.getInstance(context).getAppWidgetInfo(id)
            rule.runOnUiThread {
                val caption = TextView(rule.activity).apply {
                    text = label(spec)
                    textSize = 11f
                    setTextColor(Color.WHITE)
                    setBackgroundColor(Color.BLACK)
                }
                column!!.addView(caption, LinearLayout.LayoutParams(-2, (CAPTION_DP * density).toInt()))
                val view = host.createView(rule.activity, id, info)
                view.setPadding(0, 0, 0, 0)
                val params = LinearLayout.LayoutParams((spec.w * density).toInt(), (spec.h * density).toInt())
                params.bottomMargin = (GAP_DP * density).toInt()
                column!!.addView(view, params)
                resize(view, spec)
                views += Triple(spec, view, id)
            }
        }
        // Ask each provider to draw at the size it was just told.
        for ((spec, _, id) in views) {
            runCatching { runBlocking { spec.kind.widget.update(context, GlanceAppWidgetManager(context).getGlanceIdBy(id)) } }
        }
        Thread.sleep(SETTLE_MS)
        rule.waitForIdle()

        for ((spec, view, _) in views) check(spec, view)
        shot(shotName)
        val screen = automation.takeScreenshot()
        for ((spec, view, _) in views) checkCorner(spec, view, screen)
        screen.recycle()

        rule.runOnUiThread { column?.removeAllViews() }
        WidgetConfig.forget(context, ids.toIntArray())
        ids.forEach { runCatching { host.deleteAppWidgetId(it) } }
    }

    /** Tell the provider the size this widget is drawn at, as a launcher does when it is placed or resized. */
    private fun resize(view: AppWidgetHostView, spec: Spec) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            view.updateAppWidgetSize(Bundle(), listOf(SizeF(spec.w.toFloat(), spec.h.toFloat())))
        } else {
            @Suppress("DEPRECATION")
            view.updateAppWidgetSize(null, spec.w, spec.h, spec.w, spec.h)
        }
    }

    private fun label(s: Spec) = "${s.kind.name} ${s.w}x${s.h} dp (${s.note})"

    // --- What is checked. ---

    private fun check(spec: Spec, host: AppWidgetHostView) {
        var count = 0
        fun walk(v: View, parent: View?, path: String) {
            count++
            if (parent != null && v.visibility == View.VISIBLE && parent.width > 0 && parent.height > 0) {
                val slack = 2
                if (v.left < -slack || v.top < -slack || v.right > parent.width + slack || v.bottom > parent.height + slack) {
                    problems += "${label(spec)}: cropped. $path ${describe(v)} sits at (${v.left},${v.top})-(${v.right},${v.bottom}) in a ${parent.width}x${parent.height} px parent"
                }
            }
            if (v is ViewGroup) for (i in 0 until v.childCount) walk(v.getChildAt(i), v, "$path>${v.javaClass.simpleName}")
        }
        walk(host, null, "host")
        if (count < 4) problems += "${label(spec)}: was not drawn ($count views)"
        // Buttons: at least 48 dp each way.
        val names = listOf(R.string.widget_play, R.string.widget_pause, R.string.widget_previous, R.string.widget_next).map { context.getString(it) }
        fun buttons(v: View) {
            val d = v.contentDescription?.toString()
            if (d != null && d in names && v.visibility == View.VISIBLE) {
                val wDp = v.width / density
                val hDp = v.height / density
                if (wDp < 47.5f || hDp < 47.5f) problems += "${label(spec)}: the \"$d\" button is ${"%.0f".format(wDp)}x${"%.0f".format(hDp)} dp, under 48"
            }
            if (v is ViewGroup) for (i in 0 until v.childCount) buttons(v.getChildAt(i))
        }
        buttons(host)
        // The album field is drawn at the widget's own shape, not stretched (owner, 12 Nov 2026).
        fun fields(v: View) {
            val bmp = (v as? ImageView)?.drawable as? BitmapDrawable
            if (bmp != null && bmp.intrinsicWidth > 0 && v.width >= host.width * 0.9f && v.height >= host.height * 0.9f) {
                val viewAspect = v.height.toFloat() / v.width
                val pictureAspect = bmp.intrinsicHeight.toFloat() / bmp.intrinsicWidth
                val stretch = viewAspect / pictureAspect
                // Aspects the field clamps (a very thin strip) are skipped; anything else within 40 %.
                if (viewAspect in 0.25f..2.1f && (stretch < 0.6f || stretch > 1.67f)) {
                    problems += "${label(spec)}: the field picture is stretched (${bmp.intrinsicWidth}x${bmp.intrinsicHeight} px drawn at ${v.width}x${v.height})"
                }
            }
            if (v is ViewGroup) for (i in 0 until v.childCount) fields(v.getChildAt(i))
        }
        fields(host)
    }

    private fun describe(v: View): String {
        val d = v.contentDescription?.let { " \"$it\"" }.orEmpty()
        val t = (v as? TextView)?.text?.let { " text \"${it.toString().take(24)}\"" }.orEmpty()
        return v.javaClass.simpleName + d + t
    }

    /** The widget's corner pixel is the page behind it: the field and the tile are clipped to the rounded corner. */
    private fun checkCorner(spec: Spec, host: AppWidgetHostView, screen: Bitmap) {
        val at = IntArray(2)
        host.getLocationOnScreen(at)
        val x = at[0] + 2
        val y = at[1] + 2
        if (x !in 0 until screen.width || y !in 0 until screen.height) return
        val px = screen.getPixel(x, y)
        val close = abs(Color.red(px) - 255) < 40 && Color.green(px) < 40 && abs(Color.blue(px) - 255) < 40
        if (!close) problems += "${label(spec)}: the corner is not rounded (the pixel there is #%06X, not the page behind)".format(px and 0xFFFFFF)
    }

    // --- Demo data. ---

    private fun writeDemoProfiles() {
        val dir = SnapshotReader.widgetsDir(context.filesDir)
        val now = System.currentTimeMillis()
        val golden = """{"scheme":"dark","base":"3D0000","stops":["CF4630","1B1030","C02D56","986E00"],"text":"FFFFFF"}"""
        val daylight = """{"scheme":"light","base":"EADFC6","stops":["F4B860","9CC5B4","E49BB1","F1DE8A"],"text":"231B10"}"""
        val palettes = listOf(
            golden,
            """{"scheme":"dark","base":"0B1B33","stops":["3B82C4","1D4E89","0B1B33","4FA8C7"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"10302D","stops":["2A9D8F","10302D","1D6B63","3AAE9F"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"1A1236","stops":["8F73E6","5B3FC4","1A1236","B8A6E8"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"2B1612","stops":["E9825A","B04A2E","2B1612","F4A261"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"0F2A38","stops":["6FA853","277DA1","0F2A38","90BE6D"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"1F0E1E","stops":["E07494","7B2D5B","1F0E1E","F28482"],"text":"FFFFFF"}""",
            """{"scheme":"dark","base":"20172A","stops":["B9A0CC","6D597A","20172A","CDB4DB"],"text":"FFFFFF"}""",
        )
        val titles = listOf("Golden Hour", "Night Market", "Salt and Signal", "Tidewater", "Paper Lanterns", "Low Orbit", "Harbor Light", "Cinder Road")
        val mixNames = listOf("Evening Drive", "Rainy Focus", "Drive Home", "Slow Morning", "Late Train", "Sunday Slow")
        fun profile(id: String, privacy: String, written: Long, songPalette: String, mixCount: Int = 5, albumCount: Int = 8) {
            val folder = File(dir, id)
            folder.deleteRecursively()
            File(folder, "art").mkdirs()
            for (i in 0 until 8) writeCover(File(folder, "art/a$i.img"), i)
            writeCover(File(folder, "art/s.img"), 0)
            val albums = (0 until albumCount).joinToString(",") { i ->
                """{"albumId":${i + 1},"title":"${titles[i]}","artist":"Ember Cove","cover":"a$i.img","palette":${palettes[i]}}"""
            }
            fun mix(i: Int, pinned: Boolean) =
                """{"mixId":"mix-$i","title":"${mixNames[i]}","cover":"a${(i + 1) % 8}.img","palette":${palettes[(i + 1) % 8]},"pinned":$pinned}"""
            val body = if (privacy == "normal") {
                """"song":{"songId":412,"title":"Slow Tide","artist":"Ember Cove","albumId":1,"albumTitle":"Golden Hour Static","cover":"s.img","palette":$songPalette,"durationMs":222000,"nowPlaying":true},""" +
                    """"isPlaying":true,"positionMs":84000,"positionAtUtcMs":$now,""" +
                    """"pinnedMixes":[${(1..mixCount).joinToString(",") { mix(it, true) }}],"nextUp":${mix(0, false)},"recentAlbums":[$albums],"""
            } else {
                """"song":null,"isPlaying":false,"positionMs":0,"positionAtUtcMs":0,"pinnedMixes":[],"nextUp":null,"recentAlbums":[],"""
            }
            File(folder, "snapshot.json").writeText(
                """{"schema":1,"profileId":"$id","privacy":"$privacy",$body"reachable":true,"writtenAtUtcMs":$written,"recap":null}""",
            )
        }
        profile("gallery-dark", "normal", now, golden)
        profile("gallery-light", "normal", now, daylight)
        profile("gallery-stale", "normal", now - 2 * 24 * 3600 * 1000L, golden)
        profile("gallery-locked", "locked", now, golden)
        // A new library: one pinned mix and two albums, so a strip has fewer tiles than columns.
        profile("gallery-few", "normal", now, golden, mixCount = 1, albumCount = 2)
    }

    /** A made-up sunset in one of eight color schemes: demo art only. */
    private fun writeCover(file: File, i: Int) {
        val skies = listOf(0xFFF3743B to 0xFFC01B66, 0xFF4FA8C7 to 0xFF1D4E89, 0xFFE9C46A to 0xFF2A9D8F, 0xFFB8A6E8 to 0xFF5B3FC4, 0xFFF4A261 to 0xFFE76F51, 0xFF90BE6D to 0xFF277DA1, 0xFFF28482 to 0xFF7B2D5B, 0xFFCDB4DB to 0xFF6D597A)
        val (top, bottom) = skies[i % skies.size]
        val size = 256
        val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        val c = Canvas(bmp)
        val p = Paint(Paint.ANTI_ALIAS_FLAG)
        p.shader = LinearGradient(0f, 0f, 0f, size * 0.58f, top.toInt(), bottom.toInt(), Shader.TileMode.CLAMP)
        c.drawRect(0f, 0f, size.toFloat(), size * 0.58f, p)
        p.shader = null
        p.color = 0xFF14102A.toInt()
        c.drawRect(0f, size * 0.58f, size.toFloat(), size.toFloat(), p)
        p.color = 0xFFFFD27A.toInt()
        c.drawCircle(size / 2f, size * 0.44f, size * 0.2f, p)
        file.outputStream().use { bmp.compress(Bitmap.CompressFormat.PNG, 100, it) }
        bmp.recycle()
    }

    private fun shot(name: String) {
        shell("screencap -p /data/local/tmp/encore-shot-$name.png")
    }

    private fun shell(command: String): String {
        val pfd = automation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(pfd).use { it.readBytes().toString(Charsets.UTF_8) }
    }

    private companion object {
        const val HOST_ID = 0x5158
        const val MAGENTA = 0xFFFF00FF.toInt()
        const val CAPTION_DP = 16
        const val GAP_DP = 8
        const val TOP_DP = 32
        const val SETTLE_MS = 4_000L
    }
}
