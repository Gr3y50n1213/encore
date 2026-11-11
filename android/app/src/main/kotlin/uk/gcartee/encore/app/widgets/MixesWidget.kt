package uk.gcartee.encore.app.widgets

import android.appwidget.AppWidgetManager
import android.content.Context
import android.graphics.Bitmap
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.LocalContext
import androidx.glance.LocalSize
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.provideContent
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.width
import androidx.glance.semantics.contentDescription
import androidx.glance.semantics.semantics
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.R

/**
 * Board 34, item 3: the Mixes widget, 4x2. "Your mixes" and four tiles, each a cover and a
 * name with a small play badge. One tap starts that mix in the background through the
 * playback service (WidgetIntents.playMix, with the install's token), never opening the
 * app. Like Now Playing, it reads its profile's snapshot file and nothing else, never
 * starts the core, and is declared `not_keyguard`: it can't be placed on the lock screen.
 *
 * A widget set to another profile, one with a PIN, or one whose file is over a day old
 * opens the app instead of playing (the plan: a tap never switches profiles silently).
 */
class MixesWidget : GlanceAppWidget() {
    override val sizeMode: SizeMode = SizeMode.Responsive(setOf(BASE, WIDE, TALL, BIG))

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
                MixesUi(view, loaded.covers)
            }
        }
    }

    companion object {
        /** 4x2. */
        val BASE = DpSize(250.dp, 100.dp)

        /** Wider tablet cells. */
        val WIDE = DpSize(370.dp, 100.dp)

        /** Taller: bigger covers. */
        val TALL = DpSize(250.dp, 170.dp)

        val BIG = DpSize(370.dp, 170.dp)
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
)

object MixesLoader {
    fun load(context: Context, appWidgetId: Int): MixesLoaded {
        val choice = WidgetConfig.mixChoice(context, appWidgetId)
        val state = WidgetConfig.profileState(context)
        val profileId = WidgetConfig.resolve(WidgetConfig.choice(context, appWidgetId), state)
            ?: return MixesLoaded(null, canControl = false, choice = choice, covers = emptyMap())
        val read = SnapshotReader.read(SnapshotReader.widgetsDir(context.filesDir), profileId)
        val covers = (read as? SnapshotRead.Ok)?.snapshot
            ?.takeIf { it.privacy == SnapshotPrivacy.NORMAL }
            ?.let { loadCollectionCovers(context, profileId, CollectionStates.mixTiles(it, choice).map { t -> t.cover }) }
            .orEmpty()
        return MixesLoaded(read, WidgetConfig.canControl(profileId, state), choice, covers)
    }
}

@androidx.compose.runtime.Composable
private fun MixesUi(view: MixesView, covers: Map<String, Bitmap>) {
    val context = LocalContext.current
    when (view) {
        is MixesView.Card -> CollectionFrame(view.tap) { CollectionCard(view.kind) }
        is MixesView.Tiles -> CollectionFrame(null) {
            val size = LocalSize.current
            val cell = (size.width - PAD * 2 - GAP * (CollectionStates.MIX_TILES - 1)) / CollectionStates.MIX_TILES
            val room = size.height - PAD * 2 - HEADER_HEIGHT - NAME_HEIGHT
            val cover = minOf(cell, room).coerceAtLeast(32.dp)
            Column(modifier = GlanceModifier.fillMaxSize()) {
                CollectionHeader(context.getString(R.string.mixes_widget_title), view.stale)
                Row(modifier = GlanceModifier.fillMaxWidth()) {
                    for (i in 0 until CollectionStates.MIX_TILES) {
                        if (i > 0) Spacer(GlanceModifier.width(GAP))
                        val tile = view.tiles.getOrNull(i)
                        if (tile == null) {
                            Spacer(GlanceModifier.defaultWeight())
                        } else {
                            val name = tile.title.ifBlank { context.getString(R.string.widget_next_up) }
                            val label = context.getString(R.string.widget_play_mix, name)
                            CollectionTile(
                                name = name,
                                cover = tile.cover?.let { covers[it] },
                                size = cover,
                                dimmed = view.stale,
                                badge = true,
                                // The tile's whole column is the touch target: cover and name.
                                modifier = GlanceModifier
                                    .defaultWeight()
                                    .clickable(buttonAction(context, WidgetIntents.playMix(context, tile.mixId), view.tilesOpen))
                                    .semantics { contentDescription = label },
                            )
                        }
                    }
                }
            }
        }
    }
}
