package uk.gcartee.encore.app.widgets

import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProviderInfo
import android.content.Context
import android.graphics.Bitmap
import androidx.compose.runtime.Composable
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
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.LocalAppWidgetOptions
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.provideContent
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.text.Text
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.R
import uk.gcartee.encore.app.notifications.LiveUpdates

/**
 * Board 34, item 2: the Now Playing widget, 2x1 up to 6x2, redesigned on 11 November 2026 to
 * wear the album's colors like the app's Now Playing. It reads its profile's snapshot file and
 * nothing else: it never starts the core, opens the database or calls the network (CORE-API,
 * "Widgets"). WidgetSync, in the app's process, asks it to redraw.
 *
 * - Every size is a bucket ([NpLayout]) whose content is sized from the bucket and the font
 *   scale ([NpSizing]), so nothing is ever cropped. No progress bar: a widget can't tick, so
 *   the playing state is the play button's shape (round paused, rounded square playing) and
 *   the level bars.
 * - Colors come from the album's palette (a field behind, text on it, a text-colored play
 *   button with a base-colored glyph); the theme's until it is made. Bold system font.
 * - Buttons are 48 dp or larger, inside the tile. The cover or the text opens Now Playing.
 */
class NowPlayingWidget : GlanceAppWidget() {
    override val sizeMode: SizeMode = SizeMode.Responsive(NpLayout.entries.map { DpSize(it.w.dp, it.h.dp) }.toSet())

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val appWidgetId = GlanceAppWidgetManager(context).getAppWidgetId(id)
        android.util.Log.i(WidgetRefresh.TAG, "provideGlance NowPlayingWidget for appWidgetId $appWidgetId")
        // The cover and the field are made before provideContent, so every size shares them.
        val first = withContext(Dispatchers.IO) { NowPlayingLoader.load(context, appWidgetId) }
        provideContent {
            // A running session isn't re-provided on update(): reload when WidgetSync,
            // the receiver or the configuration screen bumps the stamp.
            val stamp by WidgetRedraws.stamp.collectAsState()
            val loaded by produceState(first, stamp) {
                value = withContext(Dispatchers.IO) { NowPlayingLoader.load(context, appWidgetId) }
            }
            val options = LocalAppWidgetOptions.current
            val onLockScreen = options.getInt(AppWidgetManager.OPTION_APPWIDGET_HOST_CATEGORY, -1) ==
                AppWidgetProviderInfo.WIDGET_CATEGORY_KEYGUARD
            val view = NowPlayingStates.decide(
                read = loaded.read,
                canControl = loaded.canControl,
                nowUtcMs = System.currentTimeMillis(),
                onLockScreen = onLockScreen,
                hideHistoryWhileLocked = loaded.hideHistoryWhileLocked,
            )
            GlanceTheme(colors = widgetColors()) {
                NowPlayingUi(view, if (view is NowPlayingView.Song) loaded.cover else null, loaded.field)
            }
        }
    }
}

/** The receiver Android sends widget updates to (exported, as App Widget receivers must be). */
class NowPlayingWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = NowPlayingWidget()

    override fun onUpdate(context: Context, appWidgetManager: AppWidgetManager, appWidgetIds: IntArray) {
        // The 3-hour update: let running sessions read the file again, so the 24 h dim and
        // the "playing" check use the time now.
        WidgetRedraws.bump()
        super.onUpdate(context, appWidgetManager, appWidgetIds)
    }

    override fun onDeleted(context: Context, appWidgetIds: IntArray) {
        WidgetConfig.forget(context, appWidgetIds)
        super.onDeleted(context, appWidgetIds)
    }
}

/** What one widget needs, read from files and preferences only. */
data class NowPlayingLoaded(
    /** Null when no profile resolved (nothing signed in yet). */
    val read: SnapshotRead?,
    val canControl: Boolean,
    val hideHistoryWhileLocked: Boolean,
    val cover: Bitmap?,
    /** What the album's field is made from (drawn per size, by WidgetFrame); null with no palette. */
    val field: WidgetField.Spec?,
)

object NowPlayingLoader {
    fun load(context: Context, appWidgetId: Int): NowPlayingLoaded {
        val hide = LiveUpdates.hideHistoryWhileLocked(context)
        val state = WidgetConfig.profileState(context)
        val profileId = WidgetConfig.resolve(WidgetConfig.choice(context, appWidgetId), state)
            ?: return NowPlayingLoaded(null, canControl = false, hideHistoryWhileLocked = hide, cover = null, field = null)
        val dir = SnapshotReader.widgetsDir(context.filesDir)
        val read = CoverPalette.fill(SnapshotReader.read(dir, profileId), dir)
        val snapshot = (read as? SnapshotRead.Ok)?.snapshot?.takeIf { it.privacy == SnapshotPrivacy.NORMAL }
        val cover = snapshot?.song?.cover
            ?.let { SnapshotReader.coverFile(dir, profileId, it) }
            ?.let { SnapshotReader.loadCover(it) }
        val field = snapshot?.song?.palette
            ?.let {
                val stale = NowPlayingStates.isStale(snapshot, System.currentTimeMillis())
                WidgetField.Spec(it, fade = if (stale) WidgetField.STALE_FADE else 0f)
            }
        return NowPlayingLoaded(read, WidgetConfig.canControl(profileId, state), hide, cover, field)
    }
}

// --- Layout. ---

@Composable
private fun NowPlayingUi(view: NowPlayingView, cover: Bitmap?, field: WidgetField.Spec?) {
    val context = LocalContext.current
    val size = LocalSize.current
    val layout = NpLayout.of(size.width.value.toInt(), size.height.value.toInt())
    val fs = fontScale()
    when (view) {
        is NowPlayingView.Card ->
            WidgetFrame(null, null, openAction(context, view.tap)) { CardUi(view, layout, fs) }
        is NowPlayingView.Song ->
            WidgetFrame(view.palette, field, openAction(context, view.tap)) { SongUi(view, cover, layout, fs) }
    }
}

/** A cover's corner: a quarter of its side, between 10 and 22 dp, so small ones stay soft and big ones don't balloon. */
private fun coverRadius(size: Int) = (size / 4).coerceIn(10, 22).dp

@Composable
private fun CardUi(card: NowPlayingView.Card, layout: NpLayout, fs: Float) {
    val context = LocalContext.current
    val ink = inkOf(null)
    val app = context.getString(R.string.app_name)
    val (title, subtitle) = when (card.kind) {
        CardKind.NOTHING_YET -> context.getString(R.string.widget_nothing_yet) to context.getString(R.string.widget_mixes_build)
        CardKind.MIX_READY -> (card.mixName ?: context.getString(R.string.widget_play)) to context.getString(R.string.widget_next_up)
        CardKind.UNREACHABLE -> context.getString(R.string.widget_saved_mixes) to context.getString(R.string.widget_server_not_reachable)
        CardKind.LOCKED -> context.getString(R.string.widget_open_app, app) to context.getString(R.string.widget_profile_locked)
        CardKind.OPEN_APP -> context.getString(R.string.widget_open_app, app) to null
        CardKind.REMOVED -> context.getString(R.string.widget_profile_removed) to context.getString(R.string.widget_choose_profile)
        CardKind.LOCK_SCREEN_IDLE -> (card.mixName ?: app) to card.mixName?.let { context.getString(R.string.widget_next_up) }
    }
    val oneRow = layout.kind == NpKind.COMPACT || layout.kind == NpKind.ROW
    val titleSp = if (oneRow) 16 else 18
    val subSp = if (oneRow) 13 else 14
    val room = layout.h - 12
    val showSubtitle = subtitle != null &&
        WidgetSizing.linesThatFit(room, WidgetSizing.lineHeight(titleSp, fs), WidgetSizing.lineHeight(subSp, fs)) >= 2
    val button = card.button
    if (layout.kind == NpKind.COMPACT) {
        // 2x1: the button alone when there is one, else the title.
        Box(modifier = GlanceModifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            if (button != null) {
                PlayButton(button, card.buttonsOpen, 48, 48, ink)
            } else {
                Text(title, style = titleStyle(ink, 14), maxLines = 1, modifier = GlanceModifier.padding(horizontal = 8.dp))
            }
        }
        return
    }
    if (oneRow) {
        Row(modifier = GlanceModifier.fillMaxSize().padding(horizontal = 14.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(modifier = GlanceModifier.defaultWeight()) {
                Text(title, style = titleStyle(ink, titleSp), maxLines = 1)
                if (showSubtitle) Text(subtitle!!, style = secondaryStyle(ink, subSp), maxLines = 1)
            }
            if (button != null) {
                Spacer(GlanceModifier.width(8.dp))
                PlayButton(button, card.buttonsOpen, 48, 48, ink)
            }
        }
    } else {
        Column(
            modifier = GlanceModifier.fillMaxSize().padding(14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(title, style = titleStyle(ink, titleSp), maxLines = 1)
            if (subtitle != null) Text(subtitle, style = secondaryStyle(ink, subSp), maxLines = 2)
            if (button != null) {
                Spacer(GlanceModifier.height(10.dp))
                PlayButton(button, card.buttonsOpen, 48, 48, ink)
            }
        }
    }
}

@Composable
private fun SongUi(song: NowPlayingView.Song, cover: Bitmap?, layout: NpLayout, fs: Float) {
    val ink = inkOf(song.palette)
    val c = NpSizing.cover(layout, fs)
    val text = NpSizing.text(layout, fs, song.playing && !song.stale)
    when (layout.kind) {
        NpKind.COMPACT -> Row(
            modifier = GlanceModifier.fillMaxSize().padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Cover(cover, c.dp, coverRadius(c), song.stale, ink)
            Spacer(GlanceModifier.defaultWeight())
            Controls(layout.controls, song, ink)
        }
        NpKind.ROW -> Row(
            modifier = GlanceModifier.fillMaxSize().padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Cover(cover, c.dp, coverRadius(c), song.stale, ink)
            Spacer(GlanceModifier.width(10.dp))
            Column(modifier = GlanceModifier.defaultWeight()) { SongText(song, ink, layout, text) }
            Spacer(GlanceModifier.width(8.dp))
            Controls(layout.controls, song, ink)
        }
        NpKind.TILE -> Column(
            modifier = GlanceModifier.fillMaxSize().padding(10.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Box(modifier = GlanceModifier.size(c.dp), contentAlignment = Alignment.BottomEnd) {
                Cover(cover, c.dp, coverRadius(c), song.stale, ink)
                Box(modifier = GlanceModifier.padding(6.dp)) { PlayButton(song.button, song.buttonsOpen, 48, 48, ink) }
            }
            Spacer(GlanceModifier.height(6.dp))
            Column(modifier = GlanceModifier.fillMaxWidth()) { SongText(song, ink, layout, text) }
        }
        // Centered with a fixed gap (the model's 10 dp): in a cell taller than the bucket, as a
        // One UI 4x3 is, the block sits in the middle instead of leaving a hole between the
        // title and the buttons (device test, 12 Nov 2026).
        NpKind.STACK -> Column(modifier = GlanceModifier.fillMaxSize().padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Row(modifier = GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.Top) {
                Cover(cover, c.dp, coverRadius(c), song.stale, ink)
                Spacer(GlanceModifier.width(12.dp))
                Column(modifier = GlanceModifier.defaultWeight()) { SongText(song, ink, layout, text) }
            }
            Spacer(GlanceModifier.height(10.dp))
            Row(modifier = GlanceModifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                Controls(layout.controls, song, ink)
            }
        }
        NpKind.SIDE -> Row(
            modifier = GlanceModifier.fillMaxSize().padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Cover(cover, c.dp, coverRadius(c), song.stale, ink)
            Spacer(GlanceModifier.width(12.dp))
            Column(modifier = GlanceModifier.defaultWeight()) {
                SongText(song, ink, layout, text)
                Spacer(GlanceModifier.height(8.dp))
                Controls(layout.controls, song, ink)
            }
        }
    }
}

/** The title, then the lines this layout has room for: the artist, the album, the level bars. */
@Composable
private fun SongText(song: NowPlayingView.Song, ink: WidgetInk, layout: NpLayout, plan: NpText) {
    val context = LocalContext.current
    Text(song.title, style = if (song.stale) secondaryStyle(ink, NpSizing.titleSp(layout)) else titleStyle(ink, NpSizing.titleSp(layout)), maxLines = 1)
    val second = when {
        song.stale -> context.getString(R.string.widget_open_to_refresh, context.getString(R.string.app_name))
        plan.album && !song.album.isNullOrBlank() -> context.getString(R.string.widget_artist_and_album, song.artist, song.album)
        else -> song.artist
    }
    if ((plan.artist || song.stale) && second.isNotBlank()) {
        Text(second, style = secondaryStyle(ink, NpSizing.artistSp(layout)), maxLines = 1)
    }
    if (plan.bars) {
        Spacer(GlanceModifier.height(4.dp))
        LevelBars(song.barsVariant, ink)
    }
}

/** The layout's controls, joined as the app's transport: previous and next hug a wider play button. */
@Composable
private fun Controls(controls: NpControls, song: NowPlayingView.Song, ink: WidgetInk) {
    val open = song.buttonsOpen
    when (controls) {
        NpControls.PLAY, NpControls.OVERLAY -> PlayButton(song.button, open, 48, 48, ink)
        NpControls.PLAY_NEXT -> Row(verticalAlignment = Alignment.CenterVertically) {
            PlayButton(song.button, open, 48, 48, ink)
            Spacer(GlanceModifier.width(4.dp))
            SkipButton(previous = false, buttonsOpen = open, w = 52, h = 48, ink = ink)
        }
        NpControls.TRIO_48 -> Row(verticalAlignment = Alignment.CenterVertically) {
            SkipButton(previous = true, buttonsOpen = open, w = 52, h = 48, ink = ink)
            Spacer(GlanceModifier.width(4.dp))
            PlayButton(song.button, open, 64, 48, ink)
            Spacer(GlanceModifier.width(4.dp))
            SkipButton(previous = false, buttonsOpen = open, w = 52, h = 48, ink = ink)
        }
        NpControls.TRIO_56 -> Row(verticalAlignment = Alignment.CenterVertically) {
            SkipButton(previous = true, buttonsOpen = open, w = 60, h = 56, ink = ink)
            Spacer(GlanceModifier.width(6.dp))
            PlayButton(song.button, open, 76, 56, ink)
            Spacer(GlanceModifier.width(6.dp))
            SkipButton(previous = false, buttonsOpen = open, w = 60, h = 56, ink = ink)
        }
    }
}
