package uk.gcartee.encore.app.widgets

import kotlin.math.ceil
import kotlin.math.max
import kotlin.math.min

/**
 * What each widget layout needs, in dp, as plain numbers (widgets redesign, 11 November 2026).
 *
 * The first widgets chose a layout by width alone and drew fixed-size content into buckets
 * that were shorter than it, so Android cropped the play button and the text. Here every
 * layout is a bucket of a known size, the content it draws is computed from that size and the
 * user's font scale by the functions below, and [WidgetLayoutTest] checks, for every layout
 * and every font scale, that what it needs fits the bucket. The widgets draw with these same
 * functions, so a test that passes is the same arithmetic the launcher sees. No Android classes.
 */
object WidgetSizing {
    /** Text is Roboto with its font padding, about 1.35 times its size, scaled by the user's font size. */
    fun lineHeight(sp: Int, fontScale: Float): Int = ceil(1.35f * sp * fontScale).toInt()

    /** The font scales the tests walk: Samsung's slider reaches 2.0 at its largest. */
    val FONT_SCALES = listOf(1.0f, 1.15f, 1.3f, 1.5f, 1.8f, 2.0f)

    const val MIN_TOUCH = 48
    const val MIN_COVER = 44

    /** How many of the leading [heights] fit in [available] dp when stacked. */
    fun linesThatFit(available: Int, vararg heights: Int): Int {
        var used = 0
        var n = 0
        for (h in heights) {
            if (used + h > available) break
            used += h
            n++
        }
        return n
    }
}

/** The controls a Now Playing layout draws. */
enum class NpControls(val width: Int, val height: Int) {
    /** One round or square play button. */
    PLAY(48, 48),

    /** Play and next: 48 + 4 + 52. */
    PLAY_NEXT(104, 48),

    /** Previous, play, next at 48 dp high: 52 + 4 + 64 + 4 + 52. */
    TRIO_48(176, 48),

    /** The same at 56 dp high: 60 + 6 + 76 + 6 + 60. */
    TRIO_56(208, 56),

    /** The play button sits over the cover's corner. */
    OVERLAY(0, 48),
}

enum class NpKind { COMPACT, ROW, TILE, STACK, SIDE }

/**
 * The Now Playing widget's layouts, 2x1 up to 6x2. Each is also the [DpSize] bucket the widget
 * declares: Android picks the best fit for the real size, so the layout is never shorter than
 * the content it was sized for. Widths are the content's needs rounded up to a tidy number.
 */
enum class NpLayout(val w: Int, val h: Int, val kind: NpKind, val controls: NpControls, val tall: Boolean = false) {
    COMPACT(116, 48, NpKind.COMPACT, NpControls.PLAY),
    COMPACT_TALL(128, 72, NpKind.COMPACT, NpControls.PLAY, tall = true),
    ROW(208, 48, NpKind.ROW, NpControls.PLAY),
    ROW_TALL(220, 72, NpKind.ROW, NpControls.PLAY, tall = true),
    NEXT(264, 48, NpKind.ROW, NpControls.PLAY_NEXT),
    NEXT_TALL(276, 72, NpKind.ROW, NpControls.PLAY_NEXT, tall = true),
    TRANSPORT(336, 48, NpKind.ROW, NpControls.TRIO_48),
    TRANSPORT_TALL(348, 72, NpKind.ROW, NpControls.TRIO_48, tall = true),
    TILE(120, 150, NpKind.TILE, NpControls.OVERLAY),
    TILE_LARGE(160, 196, NpKind.TILE, NpControls.OVERLAY),
    FULL(200, 146, NpKind.STACK, NpControls.TRIO_48),
    FULL_TALL(240, 196, NpKind.STACK, NpControls.TRIO_56),
    SIDE(350, 160, NpKind.SIDE, NpControls.TRIO_48),
    WIDE(440, 160, NpKind.SIDE, NpControls.TRIO_56);

    companion object {
        /** The layout whose bucket is [width] x [height] dp (Glance's `LocalSize` is the bucket), else the largest that fits. */
        fun of(width: Int, height: Int): NpLayout =
            entries.firstOrNull { it.w == width && it.h == height }
                ?: entries.filter { it.w <= width && it.h <= height }.maxByOrNull { it.w * it.h }
                ?: COMPACT
    }
}

/** Which of the text lines a layout has room for. */
data class NpText(val artist: Boolean, val album: Boolean, val bars: Boolean)

object NpSizing {
    private const val PAD_ROW = 8
    private const val PAD_BLOCK = 12
    private const val PAD_TILE = 10
    private const val GAP_COVER = 10
    private const val GAP_TEXT = 4

    /**
     * The narrowest the title may be given beside a cover and controls: "Slow Tide" in bold 16 sp
     * is about 70 dp, and at 60 the first CI gallery showed titles cut to "Slow Ti...".
     */
    const val TEXT_MIN_W = 80

    /** The level bars: 14 dp high. */
    const val BARS_H = 14

    fun titleSp(l: NpLayout) = when (l) {
        NpLayout.FULL, NpLayout.FULL_TALL, NpLayout.SIDE -> 20
        NpLayout.WIDE -> 22
        NpLayout.TILE, NpLayout.TILE_LARGE -> 15
        else -> 16
    }

    fun artistSp(l: NpLayout) = when (l) {
        NpLayout.FULL, NpLayout.FULL_TALL, NpLayout.SIDE, NpLayout.WIDE -> 14
        else -> 13
    }

    fun cover(l: NpLayout, fs: Float): Int = when (l.kind) {
        NpKind.COMPACT, NpKind.ROW -> if (l.tall) 56 else WidgetSizing.MIN_COVER
        NpKind.TILE -> {
            // A tile has its title below the cover (and, large, the artist): the cover takes the rest.
            val lines = WidgetSizing.lineHeight(titleSp(l), fs) +
                if (l == NpLayout.TILE_LARGE) WidgetSizing.lineHeight(artistSp(l), fs) else 0
            min(l.w - 2 * PAD_TILE, l.h - 2 * PAD_TILE - 6 - lines)
        }
        NpKind.STACK -> if (l == NpLayout.FULL) 64 else 104
        NpKind.SIDE -> min(l.h - 2 * PAD_BLOCK, l.w - 212)
    }

    fun text(l: NpLayout, fs: Float, playing: Boolean): NpText {
        val title = WidgetSizing.lineHeight(titleSp(l), fs)
        val artist = WidgetSizing.lineHeight(artistSp(l), fs)
        return when (l.kind) {
            NpKind.COMPACT -> NpText(artist = false, album = false, bars = false)
            NpKind.ROW -> NpText(artist = title + artist <= l.h - 6, album = false, bars = false)
            NpKind.TILE -> NpText(artist = l == NpLayout.TILE_LARGE, album = false, bars = false)
            NpKind.STACK -> {
                val room = cover(l, fs)
                val a = title + artist <= room
                NpText(a, album = false, bars = playing && a && title + artist + GAP_TEXT + BARS_H <= room)
            }
            NpKind.SIDE -> {
                val room = cover(l, fs) - 8 - l.controls.height
                val a = title + artist <= room
                val album = l == NpLayout.WIDE && a && title + 2 * artist <= room
                val used = title + artist + (if (album) artist else 0)
                NpText(a, album, bars = playing && a && used + GAP_TEXT + BARS_H <= room)
            }
        }
    }

    /** The least width the layout can be drawn in. */
    fun minWidth(l: NpLayout): Int = when (l.kind) {
        NpKind.COMPACT -> PAD_ROW + cover(l, 1f) + PAD_ROW + l.controls.width + PAD_ROW
        NpKind.ROW -> PAD_ROW + cover(l, 1f) + GAP_COVER + TEXT_MIN_W + PAD_ROW + l.controls.width + PAD_ROW
        NpKind.TILE -> 2 * PAD_TILE + WidgetSizing.MIN_COVER + 28
        NpKind.STACK -> max(2 * PAD_BLOCK + l.controls.width, 2 * PAD_BLOCK + cover(l, 1f) + PAD_BLOCK + TEXT_MIN_W)
        NpKind.SIDE -> 2 * PAD_BLOCK + cover(l, 1f) + PAD_BLOCK + l.controls.width
    }

    /** The least height the layout can be drawn in at font scale [fs], with its title and nothing optional. */
    fun minHeight(l: NpLayout, fs: Float): Int {
        val title = WidgetSizing.lineHeight(titleSp(l), fs)
        return when (l.kind) {
            NpKind.COMPACT -> if (l.tall) 2 * PAD_ROW + cover(l, fs) else l.controls.height
            NpKind.ROW -> max(if (l.tall) 2 * PAD_ROW + cover(l, fs) else l.controls.height, title)
            NpKind.TILE -> {
                val extra = if (l == NpLayout.TILE_LARGE) WidgetSizing.lineHeight(artistSp(l), fs) else 0
                2 * PAD_TILE + 6 + cover(l, fs) + title + extra
            }
            NpKind.STACK -> 2 * PAD_BLOCK + max(cover(l, fs), title) + 10 + l.controls.height
            NpKind.SIDE -> 2 * PAD_BLOCK + max(cover(l, fs), title + 8 + l.controls.height)
        }
    }

    /** The smallest cover a layout may be drawn with at font scale [fs]. */
    fun minCover(l: NpLayout, fs: Float): Int = cover(l, fs)
}

// --- Mixes and Recently played. ---

/**
 * One bucket of a collection widget: [cols] tiles across, [rows] down, an optional header line
 * and an artist line under each name. [hero] gives the first tile a wide card of its own (Mixes)
 * with a strip of [cols] tiles under it. Sizes are the bucket the widget declares.
 */
data class CollectionLayout(
    val w: Int,
    val h: Int,
    val cols: Int,
    val rows: Int,
    val header: Boolean = false,
    val artist: Boolean = false,
    val hero: Boolean = false,
)

/** How big a layout's covers are, and whether its names fit. */
data class CollectionFit(val cover: Int, val names: Boolean, val artist: Boolean)

object CollectionSizing {
    const val PAD = 10
    const val GAP = 6
    const val CARD_PAD = 4
    const val HERO_H = 72
    const val NAME_TOP = 2
    private const val NAME_SP = 13
    private const val ARTIST_SP = 12
    private const val HEADER_SP = 15

    fun headerHeight(fs: Float) = WidgetSizing.lineHeight(HEADER_SP, fs)

    /** A tile's name line: the text and the 2 dp the tile leaves above it. */
    fun nameHeight(fs: Float) = WidgetSizing.lineHeight(NAME_SP, fs) + NAME_TOP

    fun artistHeight(fs: Float) = WidgetSizing.lineHeight(ARTIST_SP, fs)

    fun tileWidth(l: CollectionLayout): Int = (l.w - 2 * PAD - (l.cols - 1) * GAP) / l.cols

    /**
     * The covers' size: the tile's width less its padding, or what the height leaves for [CollectionLayout.rows]
     * rows, whichever is smaller. A layout whose covers would fall under [WidgetSizing.MIN_COVER] with
     * names drops them (the tile still says what it is to TalkBack), then the artist first.
     */
    fun fit(l: CollectionLayout, fs: Float): CollectionFit {
        val head = if (l.header) headerHeight(fs) + GAP else 0
        val heroSpace = if (l.hero) HERO_H + GAP else 0
        val rows = if (l.hero) 1 else l.rows
        val widthLimit = tileWidth(l) - 2 * CARD_PAD
        fun cover(names: Boolean, artist: Boolean): Int {
            val text = (if (names) nameHeight(fs) else 0) + (if (artist) artistHeight(fs) else 0)
            val rowBlock = (l.h - 2 * PAD - head - heroSpace - (rows - 1) * GAP) / rows
            val heightLimit = rowBlock - 2 * CARD_PAD - text
            return min(widthLimit, heightLimit)
        }
        for ((names, artist) in listOf(true to l.artist, true to false, false to false)) {
            if (!names && artist) continue
            val c = cover(names, artist)
            if (c >= WidgetSizing.MIN_COVER) return CollectionFit(c, names, artist)
        }
        return CollectionFit(cover(false, false), names = false, artist = false)
    }

    /** The least height the layout can be drawn in at font scale [fs], with names and the artist dropped if they don't fit. */
    fun minHeight(l: CollectionLayout, fs: Float): Int {
        val head = if (l.header) headerHeight(fs) + GAP else 0
        val heroSpace = if (l.hero) HERO_H + GAP else 0
        val rows = if (l.hero) 1 else l.rows
        return 2 * PAD + head + heroSpace + rows * (WidgetSizing.MIN_COVER + 2 * CARD_PAD) + (rows - 1) * GAP
    }

    /** The least width: [CollectionLayout.cols] tiles with covers of the minimum size. */
    fun minWidth(l: CollectionLayout): Int =
        2 * PAD + l.cols * (WidgetSizing.MIN_COVER + 2 * CARD_PAD) + (l.cols - 1) * GAP

    fun of(buckets: List<CollectionLayout>, width: Int, height: Int): CollectionLayout =
        buckets.firstOrNull { it.w == width && it.h == height }
            ?: buckets.filter { it.w <= width && it.h <= height }.maxByOrNull { it.w * it.h }
            ?: buckets.first()
}

/** Mixes (board 34, item 3): a row of tiles, a hero card with a strip, or a grid. */
object MixesSizing {
    val buckets = listOf(
        // Widths 250, 310, 410: One UI's cells are 311, 326 and 418 dp wide (S20 FE, A13), so a bucket
        // sits just under each and a widget never draws much narrower than it is.
        CollectionLayout(250, 110, cols = 4, rows = 1),
        CollectionLayout(310, 110, cols = 4, rows = 1),
        CollectionLayout(410, 110, cols = 6, rows = 1),
        CollectionLayout(250, 150, cols = 4, rows = 1, header = true),
        CollectionLayout(310, 150, cols = 4, rows = 1, header = true),
        CollectionLayout(410, 150, cols = 6, rows = 1, header = true),
        CollectionLayout(250, 200, cols = 3, rows = 1, hero = true),
        CollectionLayout(310, 200, cols = 4, rows = 1, hero = true),
        CollectionLayout(410, 200, cols = 5, rows = 1, hero = true),
        CollectionLayout(250, 300, cols = 2, rows = 2, header = true),
        CollectionLayout(310, 300, cols = 3, rows = 2, header = true),
        CollectionLayout(410, 300, cols = 3, rows = 2, header = true),
    )

    /** At most this many mixes are drawn: three across and two down. */
    const val MAX_TILES = 6
}

/** Recently played (board 34, item 4): rows of albums, up to three rows and eight albums. */
object RecentSizing {
    val buckets = listOf(
        // Widths 250, 310, 410: see MixesSizing. A 4x3 widget (One UI: 326x354) uses the 310x250 bucket and
        // its spare height is shared above and below (CollectionBody centers), as covers can't outgrow their column.
        CollectionLayout(250, 110, cols = 4, rows = 1),
        CollectionLayout(310, 110, cols = 5, rows = 1),
        CollectionLayout(410, 110, cols = 6, rows = 1),
        CollectionLayout(250, 150, cols = 4, rows = 1, header = true),
        CollectionLayout(310, 150, cols = 5, rows = 1, header = true),
        CollectionLayout(410, 150, cols = 6, rows = 1, header = true),
        CollectionLayout(250, 190, cols = 4, rows = 2),
        CollectionLayout(310, 190, cols = 4, rows = 2),
        CollectionLayout(250, 250, cols = 3, rows = 2, header = true, artist = true),
        CollectionLayout(310, 250, cols = 3, rows = 2, header = true, artist = true),
        CollectionLayout(410, 250, cols = 4, rows = 2, header = true),
    )

    const val MAX_ALBUMS = 8
}
