package uk.gcartee.encore.app.widgets

import android.appwidget.AppWidgetManager
import android.content.Context
import android.graphics.Bitmap
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.glance.GlanceId
import androidx.glance.GlanceTheme
import androidx.glance.LocalContext
import androidx.glance.LocalSize
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.provideContent
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.R

/**
 * Board 34, item 3: the Mixes widget, redesigned on 11 November 2026. Each mix is a tile tinted
 * by its own album; the frame behind them is the lead mix's field, softened. Next up leads
 * as a hero card where there is room, then the pinned mixes. One tap on a tile starts that
 * mix in the background through the playback service (WidgetIntents.playMix, with the
 * install's token), never opening the app. It reads its profile's snapshot file and nothing
 * else, never starts the core, and is declared `not_keyguard`: it can't be placed on the lock
 * screen.
 *
 * A widget set to another profile, one with a PIN, or one whose file is over a day old opens
 * the app instead of playing (a tap never switches profiles silently).
 */
class MixesWidget : GlanceAppWidget() {
    override val sizeMode: SizeMode = SizeMode.Responsive(MixesSizing.buckets.map { DpSize(it.w.dp, it.h.dp) }.toSet())

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val appWidgetId = GlanceAppWidgetManager(context).getAppWidgetId(id)
        val first = withContext(Dispatchers.IO) { MixesLoader.load(context, appWidgetId) }
        provideContent {
            val stamp by WidgetRedraws.stamp.collectAsState()
            val loaded by produceState(first, stamp) {
                value = withContext(Dispatchers.IO) { MixesLoader.load(context, appWidgetId) }
            }
            val view = CollectionStates.mixes(loaded.read, loaded.choice, loaded.canControl, System.currentTimeMillis())
            GlanceTheme(colors = widgetColors()) {
                MixesUi(view, loaded.covers, loaded.field)
            }
        }
    }
}

class MixesWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = MixesWidget()

    override fun onUpdate(context: Context, appWidgetManager: AppWidgetManager, appWidgetIds: IntArray) {
        // The 3-hour update: read the file again so the 24 h dim uses the time now.
        WidgetRedraws.bump()
        super.onUpdate(context, appWidgetManager, appWidgetIds)
    }

    override fun onDeleted(context: Context, appWidgetIds: IntArray) {
        WidgetConfig.forget(context, appWidgetIds)
        super.onDeleted(context, appWidgetIds)
    }
}

data class MixesLoaded(
    /** Null when no profile resolved (nothing signed in yet). */
    val read: SnapshotRead?,
    val canControl: Boolean,
    val choice: WidgetMixChoice,
    val covers: Map<String, Bitmap>,
    /** The lead mix's field behind the tiles; null with no palette. */
    val field: WidgetField.Spec?,
)

object MixesLoader {
    fun load(context: Context, appWidgetId: Int): MixesLoaded {
        val choice = WidgetConfig.mixChoice(context, appWidgetId)
        val state = WidgetConfig.profileState(context)
        val profileId = WidgetConfig.resolve(WidgetConfig.choice(context, appWidgetId), state)
            ?: return MixesLoaded(null, canControl = false, choice = choice, covers = emptyMap(), field = null)
        val read = SnapshotReader.read(SnapshotReader.widgetsDir(context.filesDir), profileId)
        val snapshot = (read as? SnapshotRead.Ok)?.snapshot?.takeIf { it.privacy == SnapshotPrivacy.NORMAL }
        val tiles = snapshot?.let { CollectionStates.mixTiles(it, choice) }.orEmpty()
        val covers = snapshot?.let { loadCollectionCovers(context, profileId, tiles.map { t -> t.cover }) }.orEmpty()
        val stale = snapshot?.let { NowPlayingStates.isStale(it, System.currentTimeMillis()) } ?: false
        return MixesLoaded(read, WidgetConfig.canControl(profileId, state), choice, covers, collectionField(tiles.firstOrNull()?.palette, stale))
    }
}

@Composable
private fun MixesUi(view: MixesView, covers: Map<String, Bitmap>, field: WidgetField.Spec?) {
    val context = LocalContext.current
    val size = LocalSize.current
    val layout = CollectionSizing.of(MixesSizing.buckets, size.width.value.toInt(), size.height.value.toInt())
    when (view) {
        is MixesView.Card -> CollectionFrame(null, null, view.tap) { CollectionCard(view.kind, layout, fontScale()) }
        is MixesView.Tiles -> {
            val lead = view.tiles.firstOrNull()?.palette
            val tiles = view.tiles.map { tile ->
                val name = tile.title.ifBlank { context.getString(R.string.widget_next_up) }
                TileData(
                    name = name,
                    artist = null,
                    kicker = if (tile.mixId == null) context.getString(R.string.widget_next_up) else null,
                    cover = tile.cover?.let { covers[it] },
                    palette = tile.palette,
                    label = context.getString(R.string.widget_play_mix, name),
                    // The tile's whole card is the touch target.
                    action = buttonAction(context, WidgetIntents.playMix(context, tile.mixId), view.tilesOpen),
                    badge = true,
                )
            }
            CollectionFrame(lead, field, null) {
                CollectionBody(layout, tiles, context.getString(R.string.mixes_widget_title), view.stale, lead)
            }
        }
    }
}
