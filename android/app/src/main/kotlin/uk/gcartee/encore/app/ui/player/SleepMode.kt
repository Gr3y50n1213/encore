package uk.gcartee.encore.app.ui.player

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.view.WindowManager
import androidx.activity.compose.BackHandler
import androidx.compose.animation.Crossfade
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
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
private const val LINE_FADE_MS = 400

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
            w.attributes = w.attributes.apply { screenBrightness = SLEEP_BRIGHTNESS }
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

    val smooth = rememberSmoothPosition(state.positionMs, state.isPlaying, state.durationMs)
    var lyrics by remember { mutableStateOf<SongLyrics?>(null) }
    val songId = entry?.song?.id
    LaunchedEffect(songId) {
        lyrics = null
        if (songId != null) lyrics = runCatching { host.core.lyrics(songId) }.getOrNull()
    }
    val l = lyrics
    val synced = l != null && !l.instrumental && l.syncLevel != LyricsSync.UNSYNCED && l.lines.isNotEmpty()
    val current by remember(l) { derivedStateOf { if (l != null && synced) lyricIndex(l, smooth.value) else -1 } }
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
        Column(
            Modifier.fillMaxSize().padding(horizontal = maxWidth * 0.1f).padding(bottom = if (shown) 96.dp else 0.dp),
            verticalArrangement = Arrangement.spacedBy(gap, Alignment.CenterVertically),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            if (song == null) {
                Text("Nothing is playing", style = type.body.copy(fontSize = near), color = SLEEP_NEAR, textAlign = TextAlign.Center)
            } else if (!synced || l == null) {
                // A song without synced lyrics: its title and artist, big and quiet.
                Text(song.title, style = type.body.copy(fontSize = now * 0.9f, lineHeight = now * 1.1f, fontWeight = FontWeight.Bold), color = SLEEP_NOW, textAlign = TextAlign.Center)
                Text(song.artist, style = type.body.copy(fontSize = near), color = SLEEP_NEAR, textAlign = TextAlign.Center)
            } else {
                Crossfade(targetState = current, animationSpec = tween(LINE_FADE_MS), label = "line") { i ->
                    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(gap), horizontalAlignment = Alignment.CenterHorizontally) {
                        val before = l.lines.getOrNull(i - 1)?.text.orEmpty()
                        // Before the first line: the song's title, then the first line to come.
                        val sung = if (i < 0) song.title else l.lines[i].text
                        val after = l.lines.getOrNull(i + 1)?.text.orEmpty()
                        Text(before, style = type.body.copy(fontSize = far, lineHeight = far * 1.25f, fontWeight = FontWeight.SemiBold), color = SLEEP_FAR, textAlign = TextAlign.Center, maxLines = 2, overflow = TextOverflow.Ellipsis)
                        Text(sung, style = type.body.copy(fontSize = now, lineHeight = now * 1.2f, fontWeight = FontWeight.Bold), color = SLEEP_NOW, textAlign = TextAlign.Center, maxLines = 3, overflow = TextOverflow.Ellipsis)
                        Text(after, style = type.body.copy(fontSize = near, lineHeight = near * 1.25f, fontWeight = FontWeight.SemiBold), color = SLEEP_NEAR, textAlign = TextAlign.Center, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
        if (song != null && !shown) {
            Text(
                "${song.title} · ${song.artist}",
                style = type.caption,
                color = SLEEP_FAR,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 28.dp, start = 32.dp, end = 32.dp),
            )
        }
        AnimatedVisibility(
            shown,
            enter = fadeIn(tween(200)),
            exit = fadeOut(tween(400)),
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
