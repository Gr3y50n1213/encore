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
import androidx.glance.action.Action
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.provideContent
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.R

/**
 * Board 34, item 4: the Recently played widget, redesigned on 11 November 2026. Up to eight
 * album tiles, each tinted by its own album, on the lead album's field, softened; at 4x3 six
 * bigger covers with title and artist. A tap opens that album in the app
 * (WidgetIntents.openAlbum): the app opens it only when this widget's profile is the one open
 * and the album is in its library, and otherwise opens the profile switcher. A widget for
 * another profile, or one with a PIN, sends the tap to the switcher straight away; a stale
 * file sends it to the app. It reads its snapshot file and nothing else, never starts the
 * core, and is declared `not_keyguard`: listening history never reaches the lock screen.
 */
class RecentWidget : GlanceAppWidget() {
    override val sizeMode: SizeMode = SizeMode.Responsive(RecentSizing.buckets.map { DpSize(it.w.dp, it.h.dp) }.toSet())

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
                RecentUi(view, loaded.covers, loaded.field)
            }
        }
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
    /** The lead album's field behind the tiles; null with no palette. */
    val field: WidgetField.Spec?,
)

object RecentLoader {
    fun load(context: Context, appWidgetId: Int): RecentLoaded {
        val state = WidgetConfig.profileState(context)
        val profileId = WidgetConfig.resolve(WidgetConfig.choice(context, appWidgetId), state)
            ?: return RecentLoaded(null, canControl = false, covers = emptyMap(), field = null)
        val read = SnapshotReader.read(SnapshotReader.widgetsDir(context.filesDir), profileId)
        val snapshot = (read as? SnapshotRead.Ok)?.snapshot?.takeIf { it.privacy == SnapshotPrivacy.NORMAL }
        val albums = snapshot?.recentAlbums?.take(CollectionStates.RECENT_ALBUMS).orEmpty()
        val covers = snapshot?.let { loadCollectionCovers(context, profileId, albums.map { it.cover }) }.orEmpty()
        val stale = snapshot?.let { NowPlayingStates.isStale(it, System.currentTimeMillis()) } ?: false
        return RecentLoaded(read, WidgetConfig.canControl(profileId, state), covers, collectionField(albums.firstOrNull()?.palette, stale))
    }
}

@Composable
private fun RecentUi(view: RecentView, covers: Map<String, Bitmap>, field: WidgetField.Spec?) {
    val context = LocalContext.current
    val size = LocalSize.current
    val layout = CollectionSizing.of(RecentSizing.buckets, size.width.value.toInt(), size.height.value.toInt())
    when (view) {
        is RecentView.Card -> CollectionFrame(null, null, view.tap) { CollectionCard(view.kind, layout, fontScale()) }
        is RecentView.Albums -> {
            val lead = view.albums.firstOrNull()?.palette
            val tiles = view.albums.map { album ->
                val name = album.title.ifBlank { context.getString(R.string.widget_untitled_album) }
                TileData(
                    name = name,
                    artist = album.artist,
                    kicker = null,
                    cover = album.cover?.let { covers[it] },
                    palette = album.palette,
                    label = context.getString(R.string.widget_open_album, name),
                    action = albumAction(context, view, album),
                    badge = false,
                )
            }
            CollectionFrame(lead, field, null) {
                CollectionBody(layout, tiles, context.getString(R.string.recent_widget_title), view.stale, lead)
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
