package uk.gcartee.encore.wear

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.pager.VerticalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.rotary.onRotaryScrollEvent
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.ButtonDefaults
import androidx.wear.compose.material3.ColorScheme
import androidx.wear.compose.material3.FilledIconButton
import androidx.wear.compose.material3.FilledTonalIconButton
import androidx.wear.compose.material3.Icon
import androidx.wear.compose.material3.IconButtonDefaults
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.Text
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.StateFlow
import uk.gcartee.encore.wearlink.NowPlaying
import uk.gcartee.encore.wearlink.WatchPrivacy
import uk.gcartee.encore.wearlink.WearCommand

// Mock-up 50's watch colours (screen 4), on the phone's Iris accent.
private val Lilac = Color(0xFFE8DDFF)
private val Ink = Color(0xFF2A1A73)
private val Words = Color(0xFFF1ECFF)
private val Muted = Color(0xFFB9B2CF)
private val Button2 = Color(0xFF2A2735)

/** Material 3 for Wear (expressive), in Encore's colours. */
@Composable
fun EncoreWatchTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = ColorScheme(
            primary = Lilac,
            onPrimary = Ink,
            primaryContainer = Lilac,
            onPrimaryContainer = Ink,
            surfaceContainer = Button2,
            onSurface = Words,
            onSurfaceVariant = Muted,
        ),
        content = content,
    )
}

/** One step of the crown or bezel: this many scroll pixels turn the volume one step. */
private const val VOLUME_STEP_PX = 48f
private const val VOLUME_MIN_GAP_MS = 120L

@Composable
fun NowPlayingApp(locked: StateFlow<Boolean>, checkLocked: () -> Unit, send: (WearCommand) -> Unit) {
    val context = LocalContext.current
    val now by WatchStore.now.collectAsState()
    val isLocked by locked.collectAsState()
    val coverVersion by WatchStore.coverVersion.collectAsState()
    val cover: ImageBitmap? = remember(coverVersion) { WatchStore.cover(context)?.asImageBitmap() }
    // The lock can come on while the screen shows: look again every few seconds.
    LaunchedEffect(Unit) {
        while (true) {
            delay(LOCK_CHECK_MS)
            checkLocked()
        }
    }
    val n = now
    val showSong = WatchPrivacy.showSong(isLocked, n)
    val pages = if (n == null || n.isEmpty) 1 else 2
    val pager = rememberPagerState(pageCount = { pages })
    VerticalPager(state = pager, modifier = Modifier.fillMaxSize().background(Color.Black)) { page ->
        if (n == null || n.isEmpty) {
            Empty()
        } else if (page == 0) {
            NowPlayingPage(n, showSong, send)
        } else {
            MorePage(n, showSong, if (showSong) cover else null, send)
        }
    }
}

private const val LOCK_CHECK_MS = 3_000L

@Composable
private fun Empty() {
    Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) {
        Text(
            stringResource(R.string.open_on_phone, stringResource(R.string.app_name)),
            style = MaterialTheme.typography.bodyMedium,
            color = Words,
            textAlign = TextAlign.Center,
        )
    }
}

@Composable
private fun NowPlayingPage(n: NowPlaying, showSong: Boolean, send: (WearCommand) -> Unit) {
    val focus = remember { FocusRequester() }
    val turned = remember { mutableFloatStateOf(0f) }
    val lastStep = remember { mutableLongStateOf(0L) }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    val volumeHint = stringResource(R.string.volume_hint)
    Box(
        Modifier
            .fillMaxSize()
            // Mock-up 50, screen 4: the crown or bezel sets the phone's volume.
            .onRotaryScrollEvent { event ->
                turned.floatValue += event.verticalScrollPixels
                val at = System.currentTimeMillis()
                if (kotlin.math.abs(turned.floatValue) >= VOLUME_STEP_PX && at - lastStep.longValue >= VOLUME_MIN_GAP_MS) {
                    send(WearCommand.Volume(up = turned.floatValue > 0))
                    turned.floatValue = 0f
                    lastStep.longValue = at
                }
                true
            }
            .focusRequester(focus)
            .focusable()
            .semantics { contentDescription = volumeHint },
        contentAlignment = Alignment.Center,
    ) {
        ProgressRing(n)
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 30.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            if (showSong) {
                Text(n.artist, style = MaterialTheme.typography.labelSmall, color = Muted, maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center)
                Text(
                    n.title,
                    style = MaterialTheme.typography.titleMedium,
                    color = Words,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.semantics { heading() },
                )
                if (n.line.isNotBlank()) {
                    Text(
                        "“${n.line}”",
                        style = MaterialTheme.typography.bodySmall,
                        color = Lilac,
                        fontStyle = FontStyle.Italic,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }
            } else {
                // Locked: the name and the controls only.
                Text(stringResource(R.string.app_name), style = MaterialTheme.typography.titleMedium, color = Words, textAlign = TextAlign.Center)
            }
            Spacer(Modifier.height(8.dp))
            Controls(n, send)
        }
    }
}

@Composable
private fun Controls(n: NowPlaying, send: (WearCommand) -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
        FilledTonalIconButton(
            onClick = { send(WearCommand.Previous) },
            modifier = Modifier.size(48.dp),
            colors = IconButtonDefaults.filledTonalIconButtonColors(containerColor = Button2, contentColor = Words),
        ) {
            Icon(painterResource(R.drawable.sym_skip_previous_fill), contentDescription = stringResource(R.string.previous))
        }
        FilledIconButton(
            onClick = { send(if (n.isPlaying) WearCommand.Pause else WearCommand.Play) },
            modifier = Modifier.size(56.dp),
            // Mock-up 50: the play button is a rounded square between two round skips.
            shapes = IconButtonDefaults.shapes(RoundedCornerShape(18.dp)),
            colors = IconButtonDefaults.filledIconButtonColors(containerColor = Lilac, contentColor = Ink),
        ) {
            Icon(
                painterResource(if (n.isPlaying) R.drawable.sym_pause_fill else R.drawable.sym_play_arrow_fill),
                contentDescription = stringResource(if (n.isPlaying) R.string.pause else R.string.play),
                modifier = Modifier.size(28.dp),
            )
        }
        FilledTonalIconButton(
            onClick = { send(WearCommand.Next) },
            modifier = Modifier.size(48.dp),
            colors = IconButtonDefaults.filledTonalIconButtonColors(containerColor = Button2, contentColor = Words),
        ) {
            Icon(painterResource(R.drawable.sym_skip_next_fill), contentDescription = stringResource(R.string.next))
        }
    }
}

/** Mock-up 50, screen 4: the ring round the edge shows how far into the song it is. */
@Composable
private fun ProgressRing(n: NowPlaying) {
    val nowMs by produceState(System.currentTimeMillis(), n) {
        while (true) {
            value = System.currentTimeMillis()
            delay(if (n.isPlaying) RING_TICK_MS else RING_IDLE_MS)
        }
    }
    val progress = n.progressAt(nowMs)
    val label = stringResource(R.string.progress, (progress * 100).toInt())
    Canvas(Modifier.fillMaxSize().padding(4.dp).semantics { contentDescription = label }) {
        val stroke = 4.dp.toPx()
        val inset = stroke / 2
        val topLeft = androidx.compose.ui.geometry.Offset(inset, inset)
        val size = androidx.compose.ui.geometry.Size(this.size.width - stroke, this.size.height - stroke)
        drawArc(Lilac.copy(alpha = 0.18f), 0f, 360f, useCenter = false, topLeft = topLeft, size = size, style = Stroke(stroke))
        if (progress > 0f) {
            drawArc(Lilac, -90f, 360f * progress, useCenter = false, topLeft = topLeft, size = size, style = Stroke(stroke, cap = StrokeCap.Round))
        }
    }
}

private const val RING_TICK_MS = 1_000L
private const val RING_IDLE_MS = 10_000L

/** Swipe up from Now Playing: Like and Up Next (mock-up 50, screen 4). */
@Composable
private fun MorePage(n: NowPlaying, showSong: Boolean, cover: ImageBitmap?, send: (WearCommand) -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(horizontal = 24.dp, vertical = 20.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        if (cover != null) {
            Image(
                cover,
                contentDescription = stringResource(R.string.cover),
                contentScale = ContentScale.Crop,
                modifier = Modifier.size(44.dp).clip(CircleShape),
            )
            Spacer(Modifier.height(8.dp))
        }
        val likeLabel = stringResource(if (n.liked) R.string.unlike else R.string.like)
        Button(
            onClick = { send(if (n.liked) WearCommand.Unlike else WearCommand.Like) },
            modifier = Modifier.fillMaxWidth().semantics { contentDescription = likeLabel },
            colors = ButtonDefaults.filledTonalButtonColors(),
            icon = {
                Icon(
                    painterResource(if (n.liked) R.drawable.sym_favorite_fill else R.drawable.sym_favorite),
                    contentDescription = null,
                )
            },
            label = {
                Text(stringResource(if (n.liked) R.string.liked else R.string.like))
            },
        )
        if (showSong && n.upNext.isNotBlank()) {
            Spacer(Modifier.height(10.dp))
            Text(stringResource(R.string.up_next), style = MaterialTheme.typography.labelSmall, color = Muted)
            Text(n.upNext, style = MaterialTheme.typography.bodyMedium, color = Words, maxLines = 2, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center, fontSize = 14.sp)
        }
    }
}
