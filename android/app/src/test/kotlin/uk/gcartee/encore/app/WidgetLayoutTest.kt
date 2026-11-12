package uk.gcartee.encore.app

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.widgets.CollectionLayout
import uk.gcartee.encore.app.widgets.CollectionSizing
import uk.gcartee.encore.app.widgets.MixesSizing
import uk.gcartee.encore.app.widgets.NpLayout
import uk.gcartee.encore.app.widgets.NpSizing
import uk.gcartee.encore.app.widgets.RecentSizing
import uk.gcartee.encore.app.widgets.WidgetSizing

/**
 * Widgets redesign (11 November 2026): no widget layout is ever shorter than the content it
 * draws. The first widgets chose a layout by width alone and drew fixed-size content (a 48 to
 * 56 dp button, a 108 dp text stack) into buckets of 40 to 100 dp, and Android cropped it.
 * These tests walk every layout, at every font scale and every real size the provider lets
 * a person reach, against the same arithmetic the widgets draw with. Read from the sources, so a
 * change that brings the clipping back fails here, not on a phone.
 */
class WidgetLayoutTest {
    private val xml = File("src/main/res/xml")
    private val android = "http://schemas.android.com/apk/res/android"

    private data class Limits(val minW: Int, val minH: Int, val maxW: Int, val maxH: Int)

    private fun limits(name: String): Limits {
        val file = File(xml, name)
        assertTrue("no ${file.absolutePath}", file.isFile)
        val e = DocumentBuilderFactory.newInstance().apply { isNamespaceAware = true }.newDocumentBuilder().parse(file).documentElement
        fun dp(attr: String): Int {
            val v = e.getAttributeNS(android, attr)
            assertTrue("$name has no $attr", v.endsWith("dp"))
            return v.removeSuffix("dp").toInt()
        }
        return Limits(dp("minResizeWidth"), dp("minResizeHeight"), dp("maxResizeWidth"), dp("maxResizeHeight"))
    }

    // --- Now Playing. ---

    @Test
    fun everyNowPlayingLayoutFitsItsBucketAtEveryFontScale() {
        for (l in NpLayout.entries) {
            assertTrue("$l: needs width ${NpSizing.minWidth(l)}, bucket ${l.w}", NpSizing.minWidth(l) <= l.w)
            for (fs in WidgetSizing.FONT_SCALES) {
                assertTrue("$l at $fs: needs ${NpSizing.minHeight(l, fs)} high, bucket ${l.h}", NpSizing.minHeight(l, fs) <= l.h)
                assertTrue("$l at $fs: cover ${NpSizing.cover(l, fs)} is under ${WidgetSizing.MIN_COVER}", NpSizing.cover(l, fs) >= WidgetSizing.MIN_COVER)
            }
        }
    }

    @Test
    fun theOptionalTextLinesAPlanKeepsAlwaysFit() {
        // Whatever optional lines the plan keeps fit beside the controls; the title alone always does.
        for (l in NpLayout.entries) for (fs in WidgetSizing.FONT_SCALES) for (playing in listOf(true, false)) {
            val plan = NpSizing.text(l, fs, playing)
            val title = WidgetSizing.lineHeight(NpSizing.titleSp(l), fs)
            val artist = WidgetSizing.lineHeight(NpSizing.artistSp(l), fs)
            val used = title + (if (plan.artist) artist else 0) + (if (plan.album) artist else 0) +
                (if (plan.bars) 4 + NpSizing.BARS_H else 0)
            val room = when (l.kind) {
                uk.gcartee.encore.app.widgets.NpKind.COMPACT -> 0
                uk.gcartee.encore.app.widgets.NpKind.ROW -> l.h
                uk.gcartee.encore.app.widgets.NpKind.TILE -> Int.MAX_VALUE
                uk.gcartee.encore.app.widgets.NpKind.STACK -> NpSizing.cover(l, fs)
                uk.gcartee.encore.app.widgets.NpKind.SIDE -> NpSizing.cover(l, fs) - 8 - l.controls.height
            }
            val optional = plan.artist || plan.album || plan.bars
            assertTrue("$l at $fs playing=$playing: text needs $used of $room", !optional || used <= room)
        }
    }

    @Test
    fun everyNowPlayingSizeInTheProviderRangeHasABucketThatFitsIt() {
        val lim = limits("now_playing_widget_info.xml")
        val smallest = NpLayout.entries.minByOrNull { it.w * it.h }!!
        assertTrue("the smallest bucket $smallest is within minResize ${lim.minW}x${lim.minH}", smallest.w <= lim.minW && smallest.h <= lim.minH)
        // Every real size from the minimum to the maximum, in 5 dp steps, has a bucket no larger
        // than it, and the one Android would pick for it needs no more than the size it is drawn in.
        for (w in lim.minW..lim.maxW step 5) for (h in lim.minH..lim.maxH step 5) {
            val fitting = NpLayout.entries.filter { it.w <= w && it.h <= h }
            assertTrue("no bucket fits ${w}x$h", fitting.isNotEmpty())
            for (b in fitting) for (fs in WidgetSizing.FONT_SCALES) {
                assertTrue("$b in ${w}x$h at $fs needs ${NpSizing.minHeight(b, fs)} high", NpSizing.minHeight(b, fs) <= h)
                assertTrue("$b in ${w}x$h needs ${NpSizing.minWidth(b)} wide", NpSizing.minWidth(b) <= w)
            }
        }
    }

    @Test
    fun theLargestNowPlayingBucketsAreInsideTheMaxResize() {
        val lim = limits("now_playing_widget_info.xml")
        for (l in NpLayout.entries) assertTrue("$l is inside ${lim.maxW}x${lim.maxH}", l.w <= lim.maxW && l.h <= lim.maxH)
    }

    @Test
    fun everyNowPlayingControlIsAtLeast48dpHigh() {
        for (l in NpLayout.entries) assertTrue("$l: controls ${l.controls.height} dp high", l.controls.height >= WidgetSizing.MIN_TOUCH)
    }

    @Test
    fun theBucketsAreFewEnoughForTheLauncher() {
        // Each bucket is one more RemoteViews in what is sent to the launcher.
        assertTrue("${NpLayout.entries.size} Now Playing buckets", NpLayout.entries.size <= 14)
        assertTrue("${MixesSizing.buckets.size} Mixes buckets", MixesSizing.buckets.size <= 14)
        assertTrue("${RecentSizing.buckets.size} Recently played buckets", RecentSizing.buckets.size <= 14)
    }

    // --- Mixes and Recently played. ---

    private fun checkCollection(name: String, buckets: List<CollectionLayout>, file: String) {
        val lim = limits(file)
        val smallest = buckets.minByOrNull { it.w * it.h }!!
        assertTrue("$name: the smallest bucket $smallest is within minResize ${lim.minW}x${lim.minH}", smallest.w <= lim.minW && smallest.h <= lim.minH)
        for (b in buckets) {
            assertTrue("$name $b is inside ${lim.maxW}x${lim.maxH}", b.w <= lim.maxW && b.h <= lim.maxH)
            assertTrue("$name $b: needs ${CollectionSizing.minWidth(b)} wide", CollectionSizing.minWidth(b) <= b.w)
            for (fs in WidgetSizing.FONT_SCALES) {
                assertTrue("$name $b at $fs: needs ${CollectionSizing.minHeight(b, fs)} high", CollectionSizing.minHeight(b, fs) <= b.h)
                val fit = CollectionSizing.fit(b, fs)
                assertTrue("$name $b at $fs: cover ${fit.cover} is under ${WidgetSizing.MIN_COVER}", fit.cover >= WidgetSizing.MIN_COVER)
                // The covers and the names the fit keeps really fit the bucket's height.
                val head = if (b.header) CollectionSizing.headerHeight(fs) + CollectionSizing.GAP else 0
                val hero = if (b.hero) CollectionSizing.HERO_H + CollectionSizing.GAP else 0
                val rows = if (b.hero) 1 else b.rows
                val text = (if (fit.names) CollectionSizing.nameHeight(fs) else 0) + (if (fit.artist) CollectionSizing.artistHeight(fs) else 0)
                val total = 2 * CollectionSizing.PAD + head + hero + rows * (fit.cover + 2 * CollectionSizing.CARD_PAD + text) + (rows - 1) * CollectionSizing.GAP
                assertTrue("$name $b at $fs: draws $total dp in a ${b.h} dp bucket", total <= b.h)
            }
        }
        for (w in lim.minW..lim.maxW step 5) for (h in lim.minH..lim.maxH step 5) {
            val fitting = buckets.filter { it.w <= w && it.h <= h }
            assertTrue("$name: no bucket fits ${w}x$h", fitting.isNotEmpty())
        }
    }

    @Test
    fun everyMixesLayoutFitsItsBucketAtEveryFontScale() =
        checkCollection("Mixes", MixesSizing.buckets, "mixes_widget_info.xml")

    @Test
    fun everyRecentlyPlayedLayoutFitsItsBucketAtEveryFontScale() =
        checkCollection("Recently played", RecentSizing.buckets, "recent_widget_info.xml")

    @Test
    fun theFourByThreeRecentlyPlayedShowsSixBiggerCoversWithAnArtistLine() {
        // The owner's choice (11 November 2026): 3 columns of 2 rows, with the artist under the title.
        val big = RecentSizing.buckets.first { it.cols == 3 && it.rows == 2 }
        assertTrue(big.artist && big.header)
        assertTrue("covers at 1.0 are bigger than the 4-across row's", CollectionSizing.fit(big, 1f).cover > CollectionSizing.fit(RecentSizing.buckets.first(), 1f).cover)
        assertEquals(8, RecentSizing.MAX_ALBUMS)
    }

    @Test
    fun theTextHeightModelGrowsWithTheFontScale() {
        // The arithmetic the whole file leans on.
        assertEquals(22, WidgetSizing.lineHeight(16, 1.0f))
        assertEquals(44, WidgetSizing.lineHeight(16, 2.0f))
        assertEquals(2, WidgetSizing.linesThatFit(40, 20, 20, 20))
        assertEquals(0, WidgetSizing.linesThatFit(10, 20))
    }
}
