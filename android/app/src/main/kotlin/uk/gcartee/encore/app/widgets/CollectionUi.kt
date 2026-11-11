package uk.gcartee.encore.app.widgets

import android.content.Context
import android.graphics.Bitmap
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.ColorFilter
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.LocalContext
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.appWidgetBackground
import androidx.glance.action.clickable
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.text.Text
import androidx.glance.text.TextAlign
import androidx.glance.text.TextStyle
import java.io.File
import uk.gcartee.encore.app.R

/**
 * What the Mixes and Recently played widgets share (board 34, items 3 and 4): the tile,
 * the words-only cards, the header and the cover loading. Like Now Playing, they read the
 * profile's snapshot file and nothing else.
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

/** The tile behind both widgets: wallpaper colors, the system's corner radius. */
@Composable
internal fun CollectionFrame(tap: WidgetTarget?, content: @Composable () -> Unit) {
    val context = LocalContext.current
    val base = GlanceModifier
        .fillMaxSize()
        .appWidgetBackground()
        .tileBackground(GlanceTheme.colors.widgetBackground)
    val modifier = if (tap != null) base.clickable(actionStartActivity(WidgetIntents.open(context, tap))) else base
    Box(modifier = modifier.padding(PAD), contentAlignment = Alignment.TopStart) { content() }
}

/** Board 34's state cards, in words: a title and one line, never an error icon. */
@Composable
internal fun CollectionCard(kind: CardKind) {
    val context = LocalContext.current
    val app = context.getString(R.string.app_name)
    val (title, subtitle) = when (kind) {
        CardKind.NOTHING_YET -> context.getString(R.string.widget_nothing_yet) to context.getString(R.string.widget_mixes_build)
        CardKind.UNREACHABLE -> context.getString(R.string.widget_saved_mixes) to context.getString(R.string.widget_server_not_reachable)
        CardKind.LOCKED -> context.getString(R.string.widget_open_app, app) to context.getString(R.string.widget_profile_locked)
        CardKind.REMOVED -> context.getString(R.string.widget_profile_removed) to context.getString(R.string.widget_choose_profile)
        else -> context.getString(R.string.widget_open_app, app) to null
    }
    Column(modifier = GlanceModifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = titleStyle(15.sp), maxLines = 1)
        if (subtitle != null) Text(subtitle, style = secondaryStyle(13.sp), maxLines = 2)
    }
}

/** The widget's name, or "Open Encore to refresh" when the file is over 24 hours old. */
@Composable
internal fun CollectionHeader(title: String, stale: Boolean) {
    val context = LocalContext.current
    Box(modifier = GlanceModifier.height(HEADER_HEIGHT), contentAlignment = Alignment.CenterStart) {
        if (stale) {
            Text(context.getString(R.string.widget_open_to_refresh, context.getString(R.string.app_name)), style = secondaryStyle(13.sp), maxLines = 1)
        } else {
            Text(title, style = titleStyle(13.sp), maxLines = 1)
        }
    }
}

/** A cover over a name, centered; [badge] adds the small play badge (Mixes). */
@Composable
internal fun CollectionTile(
    name: String,
    cover: Bitmap?,
    size: Dp,
    dimmed: Boolean,
    badge: Boolean,
    modifier: GlanceModifier,
) {
    Column(modifier = modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        Box(modifier = GlanceModifier.size(size), contentAlignment = Alignment.BottomEnd) {
            Cover(cover, size, dimmed)
            if (badge) PlayBadge(if (size < 44.dp) 18.dp else 22.dp)
        }
        Box(modifier = GlanceModifier.height(NAME_HEIGHT).padding(top = 2.dp), contentAlignment = Alignment.TopCenter) {
            Text(
                name,
                style = TextStyle(
                    color = if (dimmed) GlanceTheme.colors.onSurfaceVariant else GlanceTheme.colors.onSurface,
                    fontSize = 11.sp,
                    textAlign = TextAlign.Center,
                ),
                maxLines = 1,
            )
        }
    }
}

@Composable
private fun PlayBadge(size: Dp) {
    Box(modifier = GlanceModifier.padding(3.dp)) {
        Box(
            modifier = GlanceModifier
                .size(size)
                .background(ImageProvider(R.drawable.widget_button_round), colorFilter = ColorFilter.tint(GlanceTheme.colors.primary)),
            contentAlignment = Alignment.Center,
        ) {
            Image(
                provider = ImageProvider(R.drawable.sym_play_arrow_fill),
                contentDescription = null,
                modifier = GlanceModifier.size(size * 0.6f),
                colorFilter = ColorFilter.tint(GlanceTheme.colors.onPrimary),
            )
        }
    }
}

internal val PAD = 10.dp
internal val GAP = 6.dp
internal val HEADER_HEIGHT = 20.dp
internal val NAME_HEIGHT = 16.dp
