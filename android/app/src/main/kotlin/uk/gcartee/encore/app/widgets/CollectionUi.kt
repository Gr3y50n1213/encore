package uk.gcartee.encore.app.widgets

import android.content.Context
import android.graphics.Bitmap
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.glance.ColorFilter
import androidx.glance.GlanceModifier
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.LocalContext
import androidx.glance.action.Action
import androidx.glance.action.clickable
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.ContentScale
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.semantics.contentDescription
import androidx.glance.semantics.semantics
import androidx.glance.text.Text
import androidx.glance.unit.ColorProvider
import java.io.File
import uk.gcartee.encore.app.R

/**
 * What the Mixes and Recently played widgets share (board 34, items 3 and 4, redesigned on
 * 11 November 2026): album-tinted tiles, a hero card, the words-only cards and the cover
 * loading. Like Now Playing, they read the profile's snapshot file and nothing else, and the
 * frame behind the tiles is the lead item's album field, softened.
 */

/** Tiles are drawn small: eight 128 px covers stay well inside what a launcher accepts. */
const val COLLECTION_COVER_PX = 128

/** The covers named by a snapshot, as small bitmaps keyed by file name. Missing files are left out. */
fun loadCollectionCovers(context: Context, profileId: String, names: List<String?>): Map<String, Bitmap> {
    val dir: File = SnapshotReader.widgetsDir(context.filesDir)
    val out = LinkedHashMap<String, Bitmap>()
    for (name in names.filterNotNull().distinct()) {
        val file = SnapshotReader.coverFile(dir, profileId, name) ?: continue
        SnapshotReader.loadCover(file, COLLECTION_COVER_PX)?.let { out[name] = it }
    }
    return out
}

/**
 * The lead item's field behind the tiles, softened (the mock-up's frame A), and faded more
 * when the file is over a day old; made once for every size.
 */
fun collectionField(lead: SnapshotPalette?, stale: Boolean): Bitmap? =
    lead?.let { WidgetField.bitmap(it, fade = if (stale) WidgetField.STALE_FADE else WidgetField.SOFT_FADE) }

/** One tile of either widget: what it shows and where a tap goes. */
data class TileData(
    val name: String,
    val artist: String?,
    /** "Next up" over the hero's name, when it is the Next up mix. */
    val kicker: String?,
    val cover: Bitmap?,
    val palette: SnapshotPalette?,
    /** TalkBack's name for the tile, e.g. "Play Evening Drive". */
    val label: String,
    val action: Action,
    /** A small play badge on the cover (Mixes). */
    val badge: Boolean,
)

/** The widget's tile: the lead album's field, or the theme's surface; [tap] is where the body goes. */
@Composable
internal fun CollectionFrame(lead: SnapshotPalette?, field: Bitmap?, tap: WidgetTarget?, content: @Composable () -> Unit) {
    val context = LocalContext.current
    WidgetFrame(lead, field, tap?.let { openAction(context, it) }, content)
}

/** Board 34's state cards, in words: a title and one line, never an error icon. */
@Composable
internal fun CollectionCard(kind: CardKind, layout: CollectionLayout, fs: Float) {
    val context = LocalContext.current
    val ink = inkOf(null)
    val app = context.getString(R.string.app_name)
    val (title, subtitle) = when (kind) {
        CardKind.NOTHING_YET -> context.getString(R.string.widget_nothing_yet) to context.getString(R.string.widget_mixes_build)
        CardKind.UNREACHABLE -> context.getString(R.string.widget_saved_mixes) to context.getString(R.string.widget_server_not_reachable)
        CardKind.LOCKED -> context.getString(R.string.widget_open_app, app) to context.getString(R.string.widget_profile_locked)
        CardKind.REMOVED -> context.getString(R.string.widget_profile_removed) to context.getString(R.string.widget_choose_profile)
        else -> context.getString(R.string.widget_open_app, app) to null
    }
    val room = layout.h - 2 * CollectionSizing.PAD
    val showSubtitle = subtitle != null &&
        WidgetSizing.linesThatFit(room, WidgetSizing.lineHeight(18, fs), WidgetSizing.lineHeight(14, fs)) >= 2
    Column(
        modifier = GlanceModifier.fillMaxSize().padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, style = titleStyle(ink, 18), maxLines = 1)
        if (showSubtitle) Text(subtitle!!, style = secondaryStyle(ink, 14), maxLines = 2)
    }
}

/**
 * The widget's content at one bucket: an optional header, then either a hero card with a
 * strip of tiles under it, or rows of tiles. Sizes come from [CollectionSizing] and the
 * user's font scale, so nothing is cropped.
 */
@Composable
internal fun CollectionBody(
    layout: CollectionLayout,
    tiles: List<TileData>,
    title: String,
    stale: Boolean,
    lead: SnapshotPalette?,
) {
    val context = LocalContext.current
    val fs = fontScale()
    val fit = CollectionSizing.fit(layout, fs)
    val ink = inkOf(lead)
    val gap = CollectionSizing.GAP.dp
    Column(modifier = GlanceModifier.fillMaxSize().padding(CollectionSizing.PAD.dp)) {
        if (layout.header) {
            Box(
                modifier = GlanceModifier.height(CollectionSizing.headerHeight(fs).dp).fillMaxWidth(),
                contentAlignment = Alignment.CenterStart,
            ) {
                if (stale) {
                    Text(context.getString(R.string.widget_open_to_refresh, context.getString(R.string.app_name)), style = secondaryStyle(ink, 15), maxLines = 1)
                } else {
                    Text(title, style = titleStyle(ink, 15), maxLines = 1)
                }
            }
            Spacer(GlanceModifier.height(gap))
        }
        if (layout.hero) {
            tiles.firstOrNull()?.let { Hero(it, stale, ink, fs) }
            Spacer(GlanceModifier.height(gap))
            TileRow(tiles.drop(1).take(layout.cols), layout.cols, fit, stale, fs)
        } else {
            tiles.take(layout.cols * layout.rows).chunked(layout.cols).forEachIndexed { r, row ->
                if (r > 0) Spacer(GlanceModifier.height(gap))
                TileRow(row, layout.cols, fit, stale, fs)
            }
        }
    }
}

@Composable
private fun TileRow(row: List<TileData>, cols: Int, fit: CollectionFit, stale: Boolean, fs: Float) {
    Row(modifier = GlanceModifier.fillMaxWidth()) {
        for (i in 0 until cols) {
            if (i > 0) Spacer(GlanceModifier.width(CollectionSizing.GAP.dp))
            val tile = row.getOrNull(i)
            if (tile == null) Spacer(GlanceModifier.defaultWeight()) else Tile(tile, fit, stale, fs, GlanceModifier.defaultWeight())
        }
    }
}

/** A tile: a card tinted by its album (its base, a fade of its leading color), the cover inset, then the name and artist. */
@Composable
private fun Tile(t: TileData, fit: CollectionFit, stale: Boolean, fs: Float, modifier: GlanceModifier) {
    val ink = inkOf(t.palette)
    val s = fit.cover
    val p = t.palette
    val cardTint = if (p != null) ColorProvider(Color(p.base)) else ink.tonal
    // An explicit height: the fade below fills its parent, and in a parent that wraps its
    // content that is the whole widget, which stretched every tile to the widget's height and
    // pushed the second row off it (first CI gallery). The sizing model knows the height.
    val height = s + 2 * CollectionSizing.CARD_PAD +
        (if (fit.names) CollectionSizing.nameHeight(fs) + 2 else 0) +
        (if (fit.artist) CollectionSizing.artistHeight(fs) else 0)
    Box(
        modifier = modifier
            .height(height.dp)
            .background(ImageProvider(R.drawable.widget_card_18), colorFilter = ColorFilter.tint(cardTint))
            .clickable(t.action)
            .semantics { contentDescription = t.label },
    ) {
        if (p != null) {
            Image(
                provider = ImageProvider(R.drawable.widget_card_fade),
                contentDescription = null,
                modifier = GlanceModifier.fillMaxSize(),
                colorFilter = ColorFilter.tint(ColorProvider(Color(p.stops[0]))),
                contentScale = ContentScale.FillBounds,
            )
        }
        Column(
            modifier = GlanceModifier.fillMaxWidth().padding(CollectionSizing.CARD_PAD.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Box(modifier = GlanceModifier.size(s.dp), contentAlignment = Alignment.BottomEnd) {
                Cover(t.cover, s.dp, if (s >= 56) 15.dp else 12.dp, stale, ink)
                if (t.badge) Badge(if (s >= 56) 24 else 20, ink)
            }
            if (fit.names) {
                Column(modifier = GlanceModifier.fillMaxWidth().padding(horizontal = 2.dp)) {
                    Text(t.name, style = titleStyle(ink, 13), maxLines = 1, modifier = GlanceModifier.padding(top = 2.dp))
                    if (fit.artist && !t.artist.isNullOrBlank()) Text(t.artist, style = secondaryStyle(ink, 12), maxLines = 1)
                }
            }
        }
    }
}

/** A small play badge on a cover's corner. */
@Composable
private fun Badge(size: Int, ink: WidgetInk) {
    Box(modifier = GlanceModifier.padding(4.dp)) {
        Box(
            modifier = GlanceModifier
                .size(size.dp)
                .background(ImageProvider(R.drawable.widget_pill), colorFilter = ColorFilter.tint(ink.container)),
            contentAlignment = Alignment.Center,
        ) {
            Image(
                provider = ImageProvider(R.drawable.sym_play_arrow_fill),
                contentDescription = null,
                modifier = GlanceModifier.size((size / 2).dp),
                colorFilter = ColorFilter.tint(ink.glyph),
            )
        }
    }
}

/** The lead mix as a wide card on the field: its cover, "Next up" over its name, and a play button. */
@Composable
private fun Hero(t: TileData, stale: Boolean, ink: WidgetInk, fs: Float) {
    val room = CollectionSizing.HERO_H - 8
    val showKicker = t.kicker != null &&
        WidgetSizing.linesThatFit(room, WidgetSizing.lineHeight(12, fs), WidgetSizing.lineHeight(16, fs)) >= 2
    Row(
        modifier = GlanceModifier
            .fillMaxWidth()
            .height(CollectionSizing.HERO_H.dp)
            .clickable(t.action)
            .semantics { contentDescription = t.label },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Spacer(GlanceModifier.width(4.dp))
        Cover(t.cover, 64.dp, 16.dp, stale, ink)
        Spacer(GlanceModifier.width(10.dp))
        Column(modifier = GlanceModifier.defaultWeight()) {
            if (showKicker) Text(t.kicker!!, style = secondaryStyle(ink, 12), maxLines = 1)
            Text(t.name, style = titleStyle(ink, 16), maxLines = 1)
        }
        Box(
            modifier = GlanceModifier
                .size(48.dp)
                .background(ImageProvider(R.drawable.widget_pill), colorFilter = ColorFilter.tint(ink.container)),
            contentAlignment = Alignment.Center,
        ) {
            Image(
                provider = ImageProvider(R.drawable.sym_play_arrow_fill),
                contentDescription = null,
                modifier = GlanceModifier.size(24.dp),
                colorFilter = ColorFilter.tint(ink.glyph),
            )
        }
        Spacer(GlanceModifier.width(4.dp))
    }
}
