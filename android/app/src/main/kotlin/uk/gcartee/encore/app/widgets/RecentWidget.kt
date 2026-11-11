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
import androidx.glance.action.Action
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.provideContent
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.width
import androidx.glance.semantics.contentDescription
import androidx.glance.semantics.semantics
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.R

/**
 * Board 34, item 4: the Recently played widget, 4x3. "Recently played" and up to eight
 * album covers with their names, four to a row. A tap opens that album in the app
 * (WidgetIntents.openAlbum): the app opens it only when this widget's profile is the one
 * open and the album is in its library, and otherwise opens the profile switcher. A widget
 * for another profile, or one with a PIN, sends the tap to the switcher straight away; a
 * stale file sends it to the app. It reads its snapshot file and nothing else, never starts
 * the core, and is declared `not_keyguard`: listening history never reaches the lock screen.
 */
class RecentWidget : GlanceAppWidget() {
    override val sizeMode: SizeMode = SizeMode.Responsive(setOf(BASE, WIDE, TALL, BIG))

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val appWidgetId = GlanceAppWidgetManager(context).getAppWidgetId(id)
        val first = withContext(Dispatchers.IO) { RecentLoader.load(context, appWidgetId) }
        provideContent {
            val stamp by WidgetRedraws.stamp.collectAsState()
            val loaded by produceState(first, stamp) {
                value = withContext(Dispatchers.IO) { RecentLoader.load(context, appWidgetId) }
            }
            val view = CollectionStates.recent(loaded.read, loaded.canControl, System.currentTimeMillis())
            GlanceTheme(colors = widgetColors()) {
                RecentUi(view, loaded.covers)
            }
        }
    }

    companion object {
        /** 4x3. */
        val BASE = DpSize(250.dp, 170.dp)

        /** Wider tablet cells. */
        val WIDE = DpSize(370.dp, 170.dp)

        /** Taller: bigger covers. */
        val TALL = DpSize(250.dp, 250.dp)

        val BIG = DpSize(370.dp, 250.dp)
    }
}

class RecentWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = RecentWidget()

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

data class RecentLoaded(
    /** Null when no profile resolved (nothing signed in yet). */
    val read: SnapshotRead?,
    val canControl: Boolean,
    val covers: Map<String, Bitmap>,
)

object RecentLoader {
    fun load(context: Context, appWidgetId: Int): RecentLoaded {
        val state = WidgetConfig.profileState(context)
        val profileId = WidgetConfig.resolve(WidgetConfig.choice(context, appWidgetId), state)
            ?: return RecentLoaded(null, canControl = false, covers = emptyMap())
        val read = SnapshotReader.read(SnapshotReader.widgetsDir(context.filesDir), profileId)
        val covers = (read as? SnapshotRead.Ok)?.snapshot
            ?.takeIf { it.privacy == SnapshotPrivacy.NORMAL }
            ?.let { s -> loadCollectionCovers(context, profileId, s.recentAlbums.take(CollectionStates.RECENT_ALBUMS).map { it.cover }) }
            .orEmpty()
        return RecentLoaded(read, WidgetConfig.canControl(profileId, state), covers)
    }
}

private const val PER_ROW = 4

@androidx.compose.runtime.Composable
private fun RecentUi(view: RecentView, covers: Map<String, Bitmap>) {
    val context = LocalContext.current
    when (view) {
        is RecentView.Card -> CollectionFrame(view.tap) { CollectionCard(view.kind) }
        is RecentView.Albums -> CollectionFrame(null) {
            val size = LocalSize.current
            val rows = view.albums.chunked(PER_ROW)
            val cell = (size.width - PAD * 2 - GAP * (PER_ROW - 1)) / PER_ROW
            val room = (size.height - PAD * 2 - HEADER_HEIGHT - GAP * (rows.size - 1)) / rows.size - NAME_HEIGHT
            val cover = minOf(cell, room).coerceAtLeast(32.dp)
            Column(modifier = GlanceModifier.fillMaxSize()) {
                CollectionHeader(context.getString(R.string.recent_widget_title), view.stale)
                rows.forEachIndexed { r, row ->
                    if (r > 0) Spacer(GlanceModifier.height(GAP))
                    Row(modifier = GlanceModifier.fillMaxWidth()) {
                        for (i in 0 until PER_ROW) {
                            if (i > 0) Spacer(GlanceModifier.width(GAP))
                            val album = row.getOrNull(i)
                            if (album == null) {
                                Spacer(GlanceModifier.defaultWeight())
                            } else {
                                val name = album.title.ifBlank { context.getString(R.string.widget_untitled_album) }
                                val label = context.getString(R.string.widget_open_album, name)
                                CollectionTile(
                                    name = name,
                                    cover = album.cover?.let { covers[it] },
                                    size = cover,
                                    dimmed = view.stale,
                                    badge = false,
                                    modifier = GlanceModifier
                                        .defaultWeight()
                                        .clickable(albumAction(context, view, album))
                                        .semantics { contentDescription = label },
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

/** The album, or (another profile, a PIN, a stale file) the switcher or the app. */
private fun albumAction(context: Context, view: RecentView.Albums, album: SnapshotAlbum): Action =
    actionStartActivity(
        view.albumsOpen?.let { WidgetIntents.open(context, it) }
            ?: WidgetIntents.openAlbum(context, album.albumId, view.profileId),
    )
