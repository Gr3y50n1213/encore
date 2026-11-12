package uk.gcartee.encore.app.widgets

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.os.Build
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.ColorFilter
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.LocalContext
import androidx.glance.action.Action
import androidx.glance.action.clickable
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.action.actionStartService
import androidx.glance.appwidget.appWidgetBackground
import androidx.glance.appwidget.cornerRadius
import androidx.glance.background
import androidx.glance.color.ColorProviders
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.ContentScale
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.height
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.semantics.contentDescription
import androidx.glance.semantics.semantics
import androidx.glance.text.FontWeight
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import uk.gcartee.encore.app.R
import uk.gcartee.encore.app.ui.theme.irisDarkScheme
import uk.gcartee.encore.app.ui.theme.irisLightScheme

/**
 * What the three widgets are built from (the redesign of 11 November 2026, matching the
 * app's Now Playing): the album's colors for the ink, a field background drawn from its
 * palette, round and rounded-square play buttons, connected previous and next buttons, covers
 * with real radii and the level bars. System font only (Glance has no custom fonts), in bold.
 * Everything is a drawable tinted at draw time, or a bitmap made once, so a widget still
 * reads nothing but its snapshot file.
 */

/** The colors a widget's text and buttons wear: the album's when it has a palette, else the theme's. */
class WidgetInk(
    val text: ColorProvider,
    val secondary: ColorProvider,
    /** Behind a play button or badge. */
    val container: ColorProvider,
    /** On a play button or badge. */
    val glyph: ColorProvider,
    /** Behind previous and next, and an empty cover. */
    val tonal: ColorProvider,
    /** On previous and next, and an empty cover. */
    val tonalGlyph: ColorProvider,
)

/** The album's ink (the app's Now Playing: text on the field, a text-colored button with a base-colored glyph), or the theme's. */
@Composable
internal fun inkOf(p: SnapshotPalette?): WidgetInk =
    if (p != null) {
        val text = Color(p.text)
        WidgetInk(
            text = ColorProvider(text),
            secondary = ColorProvider(text.copy(alpha = SECONDARY_ALPHA)),
            container = ColorProvider(text),
            glyph = ColorProvider(Color(p.base)),
            // Opaque: a tint with alpha loses it as a background, which drew previous and next as
            // white pills with white arrows (first CI gallery). The app's tonal buttons are the
            // surface with a share of the text color laid over it; so is this, over the base.
            tonal = ColorProvider(Color(WidgetField.blend(p.base, p.text, TONAL_SHARE))),
            tonalGlyph = ColorProvider(text),
        )
    } else {
        WidgetInk(
            text = GlanceTheme.colors.onSurface,
            secondary = GlanceTheme.colors.onSurfaceVariant,
            container = GlanceTheme.colors.primary,
            glyph = GlanceTheme.colors.onPrimary,
            tonal = GlanceTheme.colors.secondaryContainer,
            tonalGlyph = GlanceTheme.colors.onSecondaryContainer,
        )
    }

internal const val SECONDARY_ALPHA = 0.78f
/** How much of the text color is laid over the base for a tonal button or an empty cover. */
internal const val TONAL_SHARE = 0.25f
internal const val DIM_ALPHA = 0.5f

@Composable
internal fun widgetColors(): ColorProviders =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        GlanceTheme.colors
    } else {
        androidx.glance.material3.ColorProviders(light = irisLightScheme(), dark = irisDarkScheme())
    }

/** The user's font scale: text grows with it, boxes in dp don't, so layouts are sized for it (WidgetSizing). */
@Composable
internal fun fontScale(): Float = LocalContext.current.resources.configuration.fontScale

private val SYSTEM_RADIUS = android.R.dimen.system_app_widget_background_radius

/** The system's corner radius from Android 12; a rounded shape before it. */
internal fun GlanceModifier.tileBackground(color: ColorProvider): GlanceModifier =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        background(color).cornerRadius(SYSTEM_RADIUS)
    } else {
        background(ImageProvider(R.drawable.widget_bg_shape), colorFilter = ColorFilter.tint(color))
    }

/**
 * The widget's tile. With an album [palette], its field (the app's Now Playing gradient, a
 * bitmap made once for all sizes) fills it under the system corner radius; before Android 12,
 * or before the colors are made, it is the theme's surface, or a tinted shape and fade. [tap]
 * is where the body goes.
 */
@Composable
internal fun WidgetFrame(palette: SnapshotPalette?, field: Bitmap?, tap: Action?, content: @Composable () -> Unit) {
    val modern = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
    var root = GlanceModifier.fillMaxSize().appWidgetBackground()
    if (modern) root = root.cornerRadius(SYSTEM_RADIUS)
    if (palette == null) root = if (modern) root.background(GlanceTheme.colors.widgetBackground) else root.tileBackground(GlanceTheme.colors.widgetBackground)
    if (tap != null) root = root.clickable(tap)
    Box(modifier = root) {
        if (palette != null) {
            if (modern && field != null) {
                // Rounded on the picture too: the container's clip is not relied on alone.
                Image(
                    provider = ImageProvider(field),
                    contentDescription = null,
                    modifier = GlanceModifier.fillMaxSize().cornerRadius(SYSTEM_RADIUS),
                    contentScale = ContentScale.FillBounds,
                )
            } else {
                Image(
                    provider = ImageProvider(R.drawable.widget_bg_shape),
                    contentDescription = null,
                    modifier = GlanceModifier.fillMaxSize(),
                    colorFilter = ColorFilter.tint(ColorProvider(Color(palette.base))),
                    contentScale = ContentScale.FillBounds,
                )
                Image(
                    provider = ImageProvider(R.drawable.widget_bg_fade),
                    contentDescription = null,
                    modifier = GlanceModifier.fillMaxSize(),
                    colorFilter = ColorFilter.tint(ColorProvider(Color(palette.stops[0]))),
                    contentScale = ContentScale.FillBounds,
                )
            }
        }
        content()
    }
}

/** A cover with a real radius (Android 12 and later; square before it) or, with none, the music-note tile. */
@Composable
internal fun Cover(cover: Bitmap?, size: Dp, radius: Dp, dimmed: Boolean, ink: WidgetInk) {
    val shaped = GlanceModifier.size(size).let {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) it.cornerRadius(radius) else it
    }
    if (cover != null) {
        // Decorative: the title beside it says what it is.
        Image(
            provider = ImageProvider(cover),
            contentDescription = null,
            alpha = if (dimmed) DIM_ALPHA else 1f,
            modifier = shaped,
            contentScale = ContentScale.Crop,
        )
    } else {
        Box(modifier = shaped.background(ink.tonal), contentAlignment = Alignment.Center) {
            Image(
                provider = ImageProvider(R.drawable.sym_music_note),
                contentDescription = null,
                modifier = GlanceModifier.size(size * 0.4f),
                colorFilter = ColorFilter.tint(ink.tonalGlyph),
            )
        }
    }
}

/**
 * Play or Pause, [w] by [h] dp: a pill (a circle at 48 by 48) while paused, a rounded square
 * while playing, as the app's transport. Its container is the ink's, its glyph the field's
 * base, as in the app.
 */
@Composable
internal fun PlayButton(button: MainButton, buttonsOpen: WidgetTarget?, w: Int, h: Int, ink: WidgetInk) {
    val context = LocalContext.current
    val playing = button == MainButton.PAUSE
    val label = context.getString(if (playing) R.string.widget_pause else R.string.widget_play)
    val intent = when (button) {
        MainButton.PLAY, MainButton.PAUSE -> WidgetIntents.mediaButton(context, WidgetIntents.KEY_PLAY_PAUSE)
        MainButton.START_NEXT_UP -> WidgetIntents.playMix(context, null)
    }
    val shape = when {
        !playing -> R.drawable.widget_pill
        h > 48 -> R.drawable.widget_sq_18
        else -> R.drawable.widget_sq_16
    }
    Box(
        modifier = GlanceModifier
            .width(w.dp)
            .height(h.dp)
            .background(ImageProvider(shape), colorFilter = ColorFilter.tint(ink.container))
            .clickable(buttonAction(context, intent, buttonsOpen))
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Image(
            provider = ImageProvider(if (playing) R.drawable.sym_pause_fill else R.drawable.sym_play_arrow_fill),
            contentDescription = null,
            modifier = GlanceModifier.size((h / 2).dp),
            colorFilter = ColorFilter.tint(ink.glyph),
        )
    }
}

/** Previous or next, [w] by [h] dp, tonal, its outer end fully round and its inner end 10 dp, as the app's. */
@Composable
internal fun SkipButton(previous: Boolean, buttonsOpen: WidgetTarget?, w: Int, h: Int, ink: WidgetInk) {
    val context = LocalContext.current
    val label = context.getString(if (previous) R.string.widget_previous else R.string.widget_next)
    val key = if (previous) WidgetIntents.KEY_PREVIOUS else WidgetIntents.KEY_NEXT
    val big = h > 48
    val shape = when {
        previous && big -> R.drawable.widget_skip_prev_56
        previous -> R.drawable.widget_skip_prev_48
        big -> R.drawable.widget_skip_next_56
        else -> R.drawable.widget_skip_next_48
    }
    Box(
        modifier = GlanceModifier
            .width(w.dp)
            .height(h.dp)
            .background(ImageProvider(shape), colorFilter = ColorFilter.tint(ink.tonal))
            .clickable(buttonAction(context, WidgetIntents.mediaButton(context, key), buttonsOpen))
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Image(
            provider = ImageProvider(if (previous) R.drawable.sym_skip_previous_fill else R.drawable.sym_skip_next_fill),
            contentDescription = null,
            modifier = GlanceModifier.size(if (big) 26.dp else 24.dp),
            colorFilter = ColorFilter.tint(ink.tonalGlyph),
        )
    }
}

/** The three level bars, shown only while it plays; the drawing changes with every state change. */
@Composable
internal fun LevelBars(variant: Int, ink: WidgetInk) {
    val bars = when (variant) {
        0 -> R.drawable.widget_bars_a
        1 -> R.drawable.widget_bars_b
        else -> R.drawable.widget_bars_c
    }
    Image(
        provider = ImageProvider(bars),
        contentDescription = LocalContext.current.getString(R.string.widget_playing),
        modifier = GlanceModifier.width(15.dp).height(NpSizing.BARS_H.dp),
        colorFilter = ColorFilter.tint(ink.text),
    )
}

/** Straight to the playback service (a foreground start, allowed for a widget tap), or the app. */
internal fun buttonAction(context: Context, intent: Intent, buttonsOpen: WidgetTarget?): Action =
    if (buttonsOpen != null) {
        actionStartActivity(WidgetIntents.open(context, buttonsOpen))
    } else {
        actionStartService(intent, isForegroundService = true)
    }

internal fun openAction(context: Context, target: WidgetTarget): Action =
    actionStartActivity(WidgetIntents.open(context, target))

@Composable
internal fun titleStyle(ink: WidgetInk, size: Int) =
    TextStyle(color = ink.text, fontSize = size.sp, fontWeight = FontWeight.Bold)

@Composable
internal fun secondaryStyle(ink: WidgetInk, size: Int) =
    TextStyle(color = ink.secondary, fontSize = size.sp, fontWeight = FontWeight.Medium)
