package uk.gcartee.encore.app.ui.player

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.view.WindowManager
import androidx.activity.compose.BackHandler
import androidx.compose.animation.Crossfade
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.BlurEffect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.text.TextStyle
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import kotlinx.coroutines.delay
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.playback.PlayerState
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.app.ui.tv.LocalTv
import uk.gcartee.encore.core.LyricsSync
import uk.gcartee.encore.core.SongLyrics

/*
 * Sleep mode (mock-up 45, boards 3 to 6; owner, 23 November 2026): full screen and nearly
 * black, the line being sung large and centred with the one before and after faint, no
 * controls until a key or a tap, and the screen kept awake only while music plays. The line
 * timing is the lyrics panel's own ([lyricIndex]), so both always agree.
 */

/** Dim warm greys on black, never white, so the room stays dark. */
private val SLEEP_BG = Color(0xFF000000)
private val SLEEP_NOW = Color(0xFF8F8A80)
private val SLEEP_NEAR = Color(0xFF3F3C37)
private val SLEEP_FAR = Color(0xFF24221F)
private val SLEEP_BUTTON = Color(0xFF1A1916)
private val SLEEP_PLAY = Color(0xFF2A2824)
private val SLEEP_ICON = Color(0xFF8A847A)

/** What the screen asks of the display's brightness (0 to 1); a projector may ignore it. */
private const val SLEEP_BRIGHTNESS = 0.08f
private const val CONTROLS_MS = 5_000L
/** How long the controls take to slide in: slow enough to read as calm, not as a cut. */
private const val MOVE_MS = 450
private const val MODE_FADE_MS = 600
private const val DIM_MS = 1_200

private enum class SleepView { NOTHING, TITLE, LYRICS }

@Composable
fun SleepModeScreen(host: CoreHost, onExit: () -> Unit, onSleepTimer: () -> Unit) {
    val context = LocalContext.current
    val view = LocalView.current
    val tv = LocalTv.current
    val state by PlaybackHub.state.collectAsState(PlayerState())
    val entry = state.entry
    BackHandler(onBack = onExit)

    // The window: dimmed, the system bars hidden, and awake only while music plays.
    DisposableEffect(Unit) {
        val activity = context.findActivity()
        val window = activity?.window
        val before = window?.attributes?.screenBrightness ?: WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE
        window?.let { w ->
            WindowCompat.getInsetsController(w, view).apply {
                systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                hide(WindowInsetsCompat.Type.systemBars())
            }
        }
        onDispose {
            view.keepScreenOn = false
            window?.let { w ->
                w.attributes = w.attributes.apply { screenBrightness = before }
                WindowCompat.getInsetsController(w, view).show(WindowInsetsCompat.Type.systemBars())
            }
        }
    }
    LaunchedEffect(state.isPlaying) { view.keepScreenOn = state.isPlaying }
    // The room dims over a second instead of all at once.
    LaunchedEffect(Unit) {
        val window = context.findActivity()?.window ?: return@LaunchedEffect
        val from = window.attributes.screenBrightness.takeIf { it in 0f..1f } ?: 0.5f
        // A dozen steps, not one a frame: each is a window update, and sixty of them a second
        // made the first moment stutter.
        for (step in 1..DIM_STEPS) {
            val t = LinearOutSlowInEasing.transform(step / DIM_STEPS.toFloat())
            window.attributes = window.attributes.apply { screenBrightness = from + (SLEEP_BRIGHTNESS - from) * t }
            delay(DIM_MS.toLong() / DIM_STEPS)
        }
    }

    // The controls: shown by a key or a tap, hidden again five seconds after the last one.
    var shown by remember { mutableStateOf(false) }
    var poke by remember { mutableIntStateOf(0) }
    LaunchedEffect(shown, poke) {
        if (shown) {
            delay(CONTROLS_MS)
            shown = false
        }
    }
    val root = remember { FocusRequester() }
    val play = remember { FocusRequester() }
    LaunchedEffect(shown) {
        runCatching { if (shown) play.requestFocus() else root.requestFocus() }
    }

    val smooth = rememberFramePosition(state.positionMs, state.isPlaying, state.durationMs)
    var lyrics by remember { mutableStateOf<SongLyrics?>(null) }
    val songId = entry?.song?.id
    LaunchedEffect(songId) {
        lyrics = null
        if (songId != null) lyrics = runCatching { host.core.lyrics(songId) }.getOrNull()
    }
    val l = lyrics
    val synced = l != null && !l.instrumental && l.syncLevel != LyricsSync.UNSYNCED && l.lines.isNotEmpty()
    val current by remember(l) { derivedStateOf { if (l != null && synced) lyricIndex(l, smooth.value) else -1 } }
    // The same list each time, so the strip isn't recomposed by every position report.
    val lineTexts = remember(l) { l?.lines?.map { it.text }.orEmpty() }
    val density = LocalDensity.current

    BoxWithConstraints(
        Modifier
            .fillMaxSize()
            .background(SLEEP_BG)
            .focusRequester(root)
            .focusable()
            .onPreviewKeyEvent { e ->
                if (e.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                // Back goes to the BackHandler; the remote's media keys to the player.
                if (e.key == Key.Back || e.key == Key.MediaPlayPause || e.key == Key.MediaPlay || e.key == Key.MediaPause ||
                    e.key == Key.MediaNext || e.key == Key.MediaPrevious
                ) {
                    return@onPreviewKeyEvent false
                }
                poke++
                // The first press only shows the controls; later ones move between them.
                if (!shown) {
                    shown = true
                    true
                } else {
                    false
                }
            }
            .pointerInput(Unit) { detectTapGestures { shown = !shown; poke++ } }
            .pointerInput(Unit) {
                // A swipe down leaves (phones).
                var dragged = 0f
                detectVerticalDragGestures(
                    onDragStart = { dragged = 0f },
                    onVerticalDrag = { _, dy -> dragged += dy },
                    onDragEnd = { if (dragged > with(density) { EXIT_DRAG_DP.dp.toPx() }) onExit() },
                )
            }
            .semantics { contentDescription = "Sleep mode. Press a key or tap for controls." },
    ) {
        // Sized from the screen: a projector's 960 dp and a phone's 400 dp both read well.
        val now = (maxWidth.value * if (tv) 0.05f else 0.085f).coerceIn(28f, 64f).sp
        val near = now * 0.58f
        val far = now * 0.5f
        val gap: Dp = (maxWidth.value * if (tv) 0.024f else 0.05f).coerceIn(16f, 40f).dp
        val type = LocalEncoreType.current
        val song = entry?.song
        // The controls make room by sliding the words up, never by a jump.
        val room by animateDpAsState(if (shown) 96.dp else 0.dp, tween(MOVE_MS, easing = FastOutSlowInEasing), label = "room")
        val mode = when {
            song == null -> SleepView.NOTHING
            !synced || l == null -> SleepView.TITLE
            else -> SleepView.LYRICS
        }
        Crossfade(
            targetState = mode,
            animationSpec = tween(MODE_FADE_MS),
            label = "view",
            // Clear of the song's name at the bottom, and as much room at the top so the sung
            // line stays in the middle.
            modifier = Modifier.fillMaxSize().padding(horizontal = maxWidth * 0.1f).padding(vertical = CAPTION_ROOM.dp).padding(bottom = room),
        ) { m ->
            when (m) {
                SleepView.NOTHING -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("Nothing is playing", style = type.body.copy(fontSize = near), color = SLEEP_NEAR, textAlign = TextAlign.Center)
                }
                // A song without synced lyrics: its title and artist, big and quiet.
                SleepView.TITLE -> Column(
                    Modifier.fillMaxSize(),
                    verticalArrangement = Arrangement.spacedBy(gap, Alignment.CenterVertically),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(song?.title.orEmpty(), style = type.body.copy(fontSize = now * 0.9f, lineHeight = now * 1.1f, fontWeight = FontWeight.Bold), color = SLEEP_NOW, textAlign = TextAlign.Center)
                    Text(song?.artist.orEmpty(), style = type.body.copy(fontSize = near), color = SLEEP_NEAR, textAlign = TextAlign.Center)
                }
                SleepView.LYRICS -> SleepLyrics(
                    lines = lineTexts,
                    current = current,
                    title = song?.title.orEmpty(),
                    style = type.body.copy(fontSize = now, lineHeight = now * 1.2f, fontWeight = FontWeight.Bold),
                    gap = gap,
                )
            }
        }
        AnimatedVisibility(
            song != null && !shown,
            enter = fadeIn(tween(MODE_FADE_MS)),
            exit = fadeOut(tween(200)),
            modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 28.dp, start = 32.dp, end = 32.dp),
        ) {
            Text(
                song?.let { "${it.title} · ${it.artist}" }.orEmpty(),
                style = type.caption,
                color = SLEEP_FAR,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        AnimatedVisibility(
            shown,
            enter = fadeIn(tween(MOVE_MS)) + slideInVertically(tween(MOVE_MS, easing = FastOutSlowInEasing)) { it / 3 },
            exit = fadeOut(tween(MOVE_MS)) + slideOutVertically(tween(MOVE_MS, easing = FastOutSlowInEasing)) { it / 3 },
            modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 32.dp),
        ) {
            val timer by rememberSleepTimer(host)
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
                SleepButton(Icons.previous(), "Previous", SLEEP_BUTTON) { poke++; PlaybackHub.previous(context) }
                SleepButton(
                    if (state.isPlaying) Icons.pause() else Icons.play(),
                    if (state.isPlaying) "Pause" else "Play",
                    SLEEP_PLAY,
                    size = 64.dp,
                    shape = RoundedCornerShape(20.dp),
                    modifier = Modifier.focusRequester(play),
                ) { poke++; PlaybackHub.togglePlay(context) }
                SleepButton(Icons.next(), "Next", SLEEP_BUTTON) { poke++; PlaybackHub.next(context) }
                // The projector also gets the sleep timer and a way out; a phone swipes down.
                if (tv) {
                    Surface(onClick = { poke++; onSleepTimer() }, shape = CircleShape, color = SLEEP_BUTTON, contentColor = SLEEP_ICON, modifier = Modifier.height(52.dp)) {
                        Row(Modifier.padding(horizontal = 20.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Icon(Icons.sleepTimer(), contentDescription = null, modifier = Modifier.size(20.dp))
                            Text(if (timer == null) "Sleep timer" else sleepSummary(context, timer), style = type.buttonLabel)
                        }
                    }
                    SleepButton(Icons.close(), "Leave Sleep mode", SLEEP_BUTTON, onClick = onExit)
                }
            }
        }
    }
}

/**
 * The lines as one strip that glides: the sung line sharp in the middle, and two lines each
 * side that grow smaller, fainter and more blurred the further they are from it (owner, 24
 * November 2026: blur, not only darkness). When the line changes each one moves to its new
 * place, size, fade and blur on a soft spring with no bounce. Every line is whole, however
 * long; none ends in "...".
 *
 * Smoothness: the text is drawn once into its own layer (offscreen) in one fixed colour, and
 * everything that moves is a property of that layer: position, size, fade and blur. Nothing
 * asks the text to be measured or its letters drawn again while it glides. Blur needs Android
 * 12; before that the lines only fade.
 */
@Composable
private fun SleepLyrics(lines: List<String>, current: Int, title: String, style: TextStyle, gap: Dp) {
    val density = LocalDensity.current
    val gapPx = with(density) { gap.toPx() }
    val fontPx = with(density) { style.fontSize.toPx() }
    val fallback = fontPx * 1.2f
    // Room around each line for its blur, so the soft edge isn't cut off by the layer.
    val pad = with(density) { (fontPx * BLUR_ROOM).toDp() }
    val padPx = fontPx * BLUR_ROOM
    val centred = remember(style) { style.copy(textAlign = TextAlign.Center, color = SLEEP_NOW) }
    // Each line's height at full size without its blur room, once it has been laid out.
    val heights = remember(lines) { mutableStateMapOf<Int, Int>() }
    fun h(i: Int): Float = heights[i]?.toFloat() ?: fallback
    fun offset(i: Int): Float {
        val role = i - current
        if (role == 0) return 0f
        val dir = if (role > 0) 1 else -1
        var y = h(current) / 2f
        for (k in 1 until kotlin.math.abs(role)) y += gapPx + h(current + dir * k) * scaleOf(dir * k)
        y += gapPx + h(i) * scaleOf(role) / 2f
        return dir * y
    }
    Box(
        Modifier
            .fillMaxSize()
            .clipToBounds()
            // The outer lines fade into the dark at the top and bottom instead of being cut.
            .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
            .drawWithContent {
                drawContent()
                drawRect(EDGE_FADE, blendMode = BlendMode.DstIn)
            },
        contentAlignment = Alignment.Center,
    ) {
        // -1 is the song's title, shown before the first line. The third line each side is
        // there only to fade in and out, so no line appears or vanishes at once.
        for (i in (current - REACH).coerceAtLeast(-1)..(current + REACH).coerceAtMost(lines.lastIndex)) {
            key(title, i) {
                val role = (i - current).coerceIn(-REACH, REACH)
                val move = spring<Float>(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = LINE_STIFFNESS)
                val y = animateFloatAsState(offset(i), move, label = "y")
                val scale = animateFloatAsState(scaleOf(role), move, label = "scale")
                val fade = animateFloatAsState(fadeOf(role), move, label = "fade")
                val blur = animateFloatAsState(fontPx * blurOf(role), move, label = "blur")
                BasicText(
                    if (i < 0) title else lines[i],
                    style = centred,
                    modifier = Modifier
                        .fillMaxWidth()
                        .onSizeChanged { heights[i] = (it.height - 2 * padPx).toInt().coerceAtLeast(1) }
                        .graphicsLayer {
                            compositingStrategy = CompositingStrategy.Offscreen
                            translationY = y.value
                            scaleX = scale.value
                            scaleY = scale.value
                            alpha = fade.value
                            val r = blur.value
                            renderEffect = if (r < 0.5f) null else BlurEffect(r, r, TileMode.Decal)
                        }
                        .padding(horizontal = pad, vertical = pad),
                )
            }
        }
    }
}

/** How far each side of the sung line the strip reaches; the last one is fully faded. */
private const val REACH = 3
private const val NEXT_SCALE = 0.62f
private const val PREV_SCALE = 0.55f
private const val EDGE_SCALE = 0.46f
private const val OUT_SCALE = 0.4f

private fun scaleOf(role: Int): Float = when {
    role == 0 -> 1f
    role == 1 -> NEXT_SCALE
    role == -1 -> PREV_SCALE
    kotlin.math.abs(role) == 2 -> EDGE_SCALE
    else -> OUT_SCALE
}

/** The sung line full, the next one clearer than the one just sung, the outer ones faint. */
private fun fadeOf(role: Int): Float = when {
    role == 0 -> 1f
    role == 1 -> 0.6f
    role == -1 -> 0.45f
    kotlin.math.abs(role) == 2 -> 0.28f
    else -> 0f
}

/** Blur as a share of the font size, more the further a line is from the sung one. */
private fun blurOf(role: Int): Float = when (kotlin.math.abs(role)) {
    0 -> 0f
    1 -> 0.05f
    2 -> 0.12f
    else -> 0.2f
}

/** Room around a line for its blur, as a share of the font size. */
private const val BLUR_ROOM = 0.25f
/** Between Compose's low and very low stiffness: about half a second to settle, no overshoot. */
private const val LINE_STIFFNESS = 120f
private const val DIM_STEPS = 12
private const val CAPTION_ROOM = 56
private val EDGE_FADE = Brush.verticalGradient(
    0f to Color.Transparent,
    0.14f to Color.Black,
    0.86f to Color.Black,
    1f to Color.Transparent,
)

@Composable
private fun SleepButton(icon: Painter, label: String, colour: Color, size: Dp = 52.dp, shape: androidx.compose.ui.graphics.Shape = CircleShape, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Surface(onClick = onClick, shape = shape, color = colour, contentColor = SLEEP_ICON, modifier = modifier.size(size)) {
        Box(contentAlignment = Alignment.Center) { Icon(icon, contentDescription = label, modifier = Modifier.size(size * 0.45f)) }
    }
}

private fun Context.findActivity(): Activity? {
    var c: Context? = this
    while (c is ContextWrapper) {
        if (c is Activity) return c
        c = c.baseContext
    }
    return null
}

private const val EXIT_DRAG_DP = 120
