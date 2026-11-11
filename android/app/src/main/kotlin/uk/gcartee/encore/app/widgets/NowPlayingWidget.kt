package uk.gcartee.encore.app.widgets

import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProviderInfo
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.os.Build
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.ColorFilter
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.LocalContext
import androidx.glance.LocalSize
import androidx.glance.action.Action
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.LinearProgressIndicator
import androidx.glance.appwidget.LocalAppWidgetOptions
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.action.actionStartService
import androidx.glance.appwidget.appWidgetBackground
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.provideContent
import androidx.glance.background
import androidx.glance.color.ColorProviders
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.ContentScale
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxHeight
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.semantics.contentDescription
import androidx.glance.semantics.semantics
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.R
import uk.gcartee.encore.app.notifications.LiveUpdates
import uk.gcartee.encore.app.ui.theme.irisDarkScheme
import uk.gcartee.encore.app.ui.theme.irisLightScheme

/**
 * Board 34, item 2: the Now Playing widget, 2x1 up to 6x2. It reads its profile's
 * snapshot file and nothing else: it never starts the core, opens the database or calls
 * the network (CORE-API, "Widgets"). WidgetSync, in the app's process, asks it to redraw.
 *
 * - Colors follow the wallpaper (GlanceTheme's dynamic colors), not the album; the cover
 *   keeps its own. Before Android 12, the app's Iris scheme.
 * - Buttons are 48 dp or larger, inside the tile. Play is round; while playing, the button
 *   is a rounded square (24 dp corner on 72 dp, scaled).
 * - The cover or the text opens Now Playing.
 */
class NowPlayingWidget : GlanceAppWidget() {
    override val sizeMode: SizeMode = SizeMode.Responsive(setOf(TINY, ROW, FULL, WIDE))

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val appWidgetId = GlanceAppWidgetManager(context).getAppWidgetId(id)
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
                NowPlayingUi(view, if (view is NowPlayingView.Song) loaded.cover else null)
            }
        }
    }

    companion object {
        /** 2x1: the cover and the button. */
        val TINY = DpSize(110.dp, 40.dp)

        /** 4x1: the cover, the title and one button. */
        val ROW = DpSize(180.dp, 40.dp)

        /** 4x2: the cover, title, artist, previous / play / next and the bar. */
        val FULL = DpSize(250.dp, 100.dp)

        /** Wider tablet cells: the album and the times too. */
        val WIDE = DpSize(370.dp, 100.dp)
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
)

object NowPlayingLoader {
    fun load(context: Context, appWidgetId: Int): NowPlayingLoaded {
        val hide = LiveUpdates.hideHistoryWhileLocked(context)
        val state = WidgetConfig.profileState(context)
        val profileId = WidgetConfig.resolve(WidgetConfig.choice(context, appWidgetId), state)
            ?: return NowPlayingLoaded(null, canControl = false, hideHistoryWhileLocked = hide, cover = null)
        val dir = SnapshotReader.widgetsDir(context.filesDir)
        val read = SnapshotReader.read(dir, profileId)
        val cover = (read as? SnapshotRead.Ok)?.snapshot
            ?.takeIf { it.privacy == SnapshotPrivacy.NORMAL }
            ?.song?.cover
            ?.let { SnapshotReader.coverFile(dir, profileId, it) }
            ?.let { SnapshotReader.loadCover(it) }
        return NowPlayingLoaded(read, WidgetConfig.canControl(profileId, state), hide, cover)
    }
}

@Composable
private fun widgetColors(): ColorProviders =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        GlanceTheme.colors
    } else {
        androidx.glance.material3.ColorProviders(light = irisLightScheme(), dark = irisDarkScheme())
    }

// --- Layout. ---

private enum class Layout { TINY, ROW, FULL, WIDE }

@Composable
private fun NowPlayingUi(view: NowPlayingView, cover: Bitmap?) {
    val context = LocalContext.current
    val size = LocalSize.current
    val layout = when {
        size.height >= NowPlayingWidget.FULL.height && size.width >= NowPlayingWidget.WIDE.width -> Layout.WIDE
        size.height >= NowPlayingWidget.FULL.height && size.width >= NowPlayingWidget.FULL.width -> Layout.FULL
        size.width >= NowPlayingWidget.ROW.width -> Layout.ROW
        else -> Layout.TINY
    }
    Box(
        modifier = GlanceModifier
            .fillMaxSize()
            .appWidgetBackground()
            .tileBackground(GlanceTheme.colors.widgetBackground)
            .clickable(openAction(context, view.tap))
            .padding(if (layout == Layout.TINY) 6.dp else 12.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        when (view) {
            is NowPlayingView.Card -> CardUi(view, layout)
            is NowPlayingView.Song -> when (layout) {
                Layout.TINY -> SongTiny(view, cover)
                Layout.ROW -> SongRow(view, cover)
                Layout.FULL -> SongFull(view, cover)
                Layout.WIDE -> SongWide(view, cover)
            }
        }
    }
}

/** The system's corner radius from Android 12; a rounded shape before it. */
private fun GlanceModifier.tileBackground(color: ColorProvider): GlanceModifier =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        background(color).cornerRadius(android.R.dimen.system_app_widget_background_radius)
    } else {
        background(ImageProvider(R.drawable.widget_tile_shape), colorFilter = ColorFilter.tint(color))
    }

@Composable
private fun CardUi(card: NowPlayingView.Card, layout: Layout) {
    val context = LocalContext.current
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
    Row(modifier = GlanceModifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
        if (layout != Layout.TINY || card.button == null) {
            Column(modifier = GlanceModifier.defaultWeight(), verticalAlignment = Alignment.CenterVertically) {
                Text(title, style = titleStyle(if (layout == Layout.TINY) 13.sp else 15.sp), maxLines = 1)
                if (subtitle != null) Text(subtitle, style = secondaryStyle(13.sp), maxLines = if (layout == Layout.FULL || layout == Layout.WIDE) 2 else 1)
            }
        }
        card.button?.let {
            if (layout != Layout.TINY) Spacer(GlanceModifier.width(8.dp))
            MainButtonUi(it, card.buttonsOpen, if (layout == Layout.FULL || layout == Layout.WIDE) 56.dp else 48.dp)
        }
    }
}

@Composable
private fun SongTiny(song: NowPlayingView.Song, cover: Bitmap?) {
    Row(modifier = GlanceModifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
        Cover(cover, 48.dp, song.stale)
        Spacer(GlanceModifier.defaultWeight())
        MainButtonUi(song.button, song.buttonsOpen, 48.dp)
    }
}

@Composable
private fun SongRow(song: NowPlayingView.Song, cover: Bitmap?) {
    Row(modifier = GlanceModifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
        Cover(cover, 48.dp, song.stale)
        Spacer(GlanceModifier.width(10.dp))
        Column(modifier = GlanceModifier.defaultWeight()) {
            SongText(song, 14.sp, withAlbum = false)
        }
        Spacer(GlanceModifier.width(8.dp))
        MainButtonUi(song.button, song.buttonsOpen, 48.dp)
    }
}

@Composable
private fun SongFull(song: NowPlayingView.Song, cover: Bitmap?) {
    val size = LocalSize.current
    val coverSize = minOf(size.height - 24.dp, 120.dp).coerceAtLeast(56.dp)
    Row(modifier = GlanceModifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
        Cover(cover, coverSize, song.stale)
        Spacer(GlanceModifier.width(12.dp))
        Column(modifier = GlanceModifier.defaultWeight().fillMaxHeight(), verticalAlignment = Alignment.CenterVertically) {
            SongText(song, 15.sp, withAlbum = false)
            Spacer(GlanceModifier.height(4.dp))
            Row(modifier = GlanceModifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalAlignment = Alignment.CenterVertically) {
                SkipButton(previous = true, buttonsOpen = song.buttonsOpen)
                Spacer(GlanceModifier.width(4.dp))
                MainButtonUi(song.button, song.buttonsOpen, 56.dp)
                Spacer(GlanceModifier.width(4.dp))
                SkipButton(previous = false, buttonsOpen = song.buttonsOpen)
            }
            if (song.current && !song.stale) {
                Spacer(GlanceModifier.height(6.dp))
                Bar(song)
            }
        }
    }
}

@Composable
private fun SongWide(song: NowPlayingView.Song, cover: Bitmap?) {
    val size = LocalSize.current
    val coverSize = minOf(size.height - 24.dp, 140.dp).coerceAtLeast(64.dp)
    Row(modifier = GlanceModifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically) {
        Cover(cover, coverSize, song.stale)
        Spacer(GlanceModifier.width(16.dp))
        Column(modifier = GlanceModifier.defaultWeight().fillMaxHeight(), verticalAlignment = Alignment.CenterVertically) {
            SongText(song, 17.sp, withAlbum = true)
            if (song.current && !song.stale) {
                Spacer(GlanceModifier.height(6.dp))
                Bar(song)
                Row(modifier = GlanceModifier.fillMaxWidth()) {
                    Text(clock(song.positionMs), style = secondaryStyle(11.sp), modifier = GlanceModifier.defaultWeight())
                    Text(clock(song.durationMs), style = secondaryStyle(11.sp))
                }
            }
            Spacer(GlanceModifier.height(2.dp))
            Row(modifier = GlanceModifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalAlignment = Alignment.CenterVertically) {
                SkipButton(previous = true, buttonsOpen = song.buttonsOpen)
                Spacer(GlanceModifier.width(12.dp))
                MainButtonUi(song.button, song.buttonsOpen, 56.dp)
                Spacer(GlanceModifier.width(12.dp))
                SkipButton(previous = false, buttonsOpen = song.buttonsOpen)
            }
        }
    }
}

@Composable
private fun SongText(song: NowPlayingView.Song, titleSize: TextUnit, withAlbum: Boolean) {
    val context = LocalContext.current
    Text(song.title, style = if (song.stale) secondaryStyle(titleSize) else titleStyle(titleSize), maxLines = 1)
    val second = when {
        song.stale -> context.getString(R.string.widget_open_to_refresh, context.getString(R.string.app_name))
        withAlbum && !song.album.isNullOrBlank() -> context.getString(R.string.widget_artist_and_album, song.artist, song.album)
        else -> song.artist
    }
    if (second.isNotBlank()) Text(second, style = secondaryStyle(13.sp), maxLines = 1)
}

@Composable
private fun Bar(song: NowPlayingView.Song) {
    LinearProgressIndicator(
        progress = NowPlayingStates.fraction(song.positionMs, song.durationMs),
        modifier = GlanceModifier.fillMaxWidth().height(4.dp),
        color = GlanceTheme.colors.primary,
        backgroundColor = GlanceTheme.colors.surfaceVariant,
    )
}

@Composable
private fun Cover(cover: Bitmap?, size: Dp, dimmed: Boolean) {
    val modifier = GlanceModifier.size(size).let {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) it.cornerRadius(android.R.dimen.system_app_widget_inner_radius) else it
    }
    if (cover != null) {
        // Decorative: the title beside it says what it is.
        Image(
            provider = ImageProvider(cover),
            contentDescription = null,
            alpha = if (dimmed) DIM_ALPHA else 1f,
            modifier = modifier,
            contentScale = ContentScale.Crop,
        )
    } else {
        Box(modifier = modifier.background(GlanceTheme.colors.secondaryContainer), contentAlignment = Alignment.Center) {
            Image(
                provider = ImageProvider(R.drawable.sym_music_note),
                contentDescription = null,
                modifier = GlanceModifier.size(size * 0.4f),
                colorFilter = ColorFilter.tint(GlanceTheme.colors.onSecondaryContainer),
            )
        }
    }
}

/** Round when it plays; a rounded square (24 dp on 72 dp, scaled by the drawable) while playing. */
@Composable
private fun MainButtonUi(button: MainButton, buttonsOpen: WidgetTarget?, size: Dp) {
    val context = LocalContext.current
    val playing = button == MainButton.PAUSE
    val label = context.getString(if (playing) R.string.widget_pause else R.string.widget_play)
    val intent = when (button) {
        MainButton.PLAY, MainButton.PAUSE -> WidgetIntents.mediaButton(context, WidgetIntents.KEY_PLAY_PAUSE)
        MainButton.START_NEXT_UP -> WidgetIntents.playMix(context, null)
    }
    Box(
        modifier = GlanceModifier
            .size(size)
            .background(
                ImageProvider(if (playing) R.drawable.widget_button_playing else R.drawable.widget_button_round),
                colorFilter = ColorFilter.tint(GlanceTheme.colors.primary),
            )
            .clickable(buttonAction(context, intent, buttonsOpen))
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Image(
            provider = ImageProvider(if (playing) R.drawable.sym_pause_fill else R.drawable.sym_play_arrow_fill),
            contentDescription = null,
            modifier = GlanceModifier.size(size * 0.5f),
            colorFilter = ColorFilter.tint(GlanceTheme.colors.onPrimary),
        )
    }
}

@Composable
private fun SkipButton(previous: Boolean, buttonsOpen: WidgetTarget?) {
    val context = LocalContext.current
    val label = context.getString(if (previous) R.string.widget_previous else R.string.widget_next)
    val key = if (previous) WidgetIntents.KEY_PREVIOUS else WidgetIntents.KEY_NEXT
    Box(
        modifier = GlanceModifier
            .size(48.dp)
            .clickable(buttonAction(context, WidgetIntents.mediaButton(context, key), buttonsOpen))
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Image(
            provider = ImageProvider(if (previous) R.drawable.sym_skip_previous_fill else R.drawable.sym_skip_next_fill),
            contentDescription = null,
            modifier = GlanceModifier.size(24.dp),
            colorFilter = ColorFilter.tint(GlanceTheme.colors.onSurfaceVariant),
        )
    }
}

/** Straight to the playback service (a foreground start, allowed for a widget tap), or the app. */
private fun buttonAction(context: Context, intent: Intent, buttonsOpen: WidgetTarget?): Action =
    if (buttonsOpen != null) {
        actionStartActivity(WidgetIntents.open(context, buttonsOpen))
    } else {
        actionStartService(intent, isForegroundService = true)
    }

private fun openAction(context: Context, target: WidgetTarget): Action =
    actionStartActivity(WidgetIntents.open(context, target))

@Composable
private fun titleStyle(size: TextUnit) = TextStyle(color = GlanceTheme.colors.onSurface, fontSize = size, fontWeight = FontWeight.Medium)

@Composable
private fun secondaryStyle(size: TextUnit) = TextStyle(color = GlanceTheme.colors.onSurfaceVariant, fontSize = size)

/** m:ss, as the app's scrub bar shows it. */
private fun clock(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    return "%d:%02d".format(s / 60, s % 60)
}

private const val DIM_ALPHA = 0.5f
