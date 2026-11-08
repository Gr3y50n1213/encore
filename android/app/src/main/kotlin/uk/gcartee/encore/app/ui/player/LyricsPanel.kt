package uk.gcartee.encore.app.ui.player

import android.os.Build
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.runtime.derivedStateOf
import androidx.compose.ui.draw.clip
import kotlin.math.abs
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import kotlinx.coroutines.flow.first
import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.ui.components.formatDuration
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.LyricSide
import uk.gcartee.encore.core.LyricWord
import uk.gcartee.encore.core.LyricsSync
import uk.gcartee.encore.core.Song
import uk.gcartee.encore.core.SongLyrics
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Motion.Timing

private sealed interface LyricsLoad {
    data object Loading : LyricsLoad
    data object None : LyricsLoad
    data class Ready(val lyrics: SongLyrics) : LyricsLoad
}

/**
 * Board 10, drawn in place of the artwork on Now Playing (item 8). [position] is the
 * frame-time playing position (ScrubBar's rememberSmoothPosition), read where it is drawn
 * so lines change on time and words fill smoothly without recomposing the list each frame.
 */
@Composable
fun LyricsPanel(host: CoreHost, song: Song, position: () -> Long, colour: Color, onSeek: (Long) -> Unit, modifier: Modifier = Modifier) {
    val type = LocalEncoreType.current
    var load by remember(song.id) { mutableStateOf<LyricsLoad>(LyricsLoad.Loading) }
    LaunchedEffect(song.id) {
        load = runCatching { host.core.lyrics(song.id) }.getOrNull()?.let { LyricsLoad.Ready(it) } ?: LyricsLoad.None
    }
    when (val l = load) {
        LyricsLoad.Loading -> Column(modifier.padding((Grid.base * 2).dp)) {
            // Five shimmer lines.
            repeat(5) { i ->
                Box(Modifier.padding(vertical = 10.dp).fillMaxWidth(if (i % 2 == 0) 0.9f else 0.65f).height(22.dp).background(colour.copy(alpha = 0.12f), RoundedCornerShape(8.dp)))
            }
        }
        LyricsLoad.None -> Box(modifier, contentAlignment = Alignment.Center) {
            Text("No lyrics for this song", style = type.sectionHeader, color = colour.copy(alpha = 0.72f))
        }
        is LyricsLoad.Ready -> Lines(host, l.lyrics, position, colour, onSeek, modifier)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Lines(host: CoreHost, lyrics: SongLyrics, position: () -> Long, colour: Color, onSeek: (Long) -> Unit, modifier: Modifier) {
    val type = LocalEncoreType.current
    val still = reduceMotion()
    val synced = lyrics.syncLevel != LyricsSync.UNSYNCED
    // After a tap the highlight moves at once; the player's position catches up a moment
    // later and takes over, so the lines never flick back to where they were.
    var pending by remember { mutableStateOf<Long?>(null) }
    LaunchedEffect(Unit) {
        snapshotFlow { position() }.collect { p ->
            val want = pending
            if (want != null && abs(p - want) < CAUGHT_UP_MS) pending = null
        }
    }
    LaunchedEffect(pending) {
        if (pending != null) {
            delay(PENDING_TIMEOUT_MS)
            pending = null
        }
    }
    // A line lights a moment early, so the change has finished as the singing starts.
    val at: () -> Long = { (pending ?: position()) + LEAD_MS }
    val current by remember(lyrics) {
        derivedStateOf { if (synced) lyrics.lines.indexOfLast { (it.startMs ?: Long.MAX_VALUE) <= at() } else -1 }
    }
    val list = rememberLazyListState()
    var lastTouchEnd by remember { mutableLongStateOf(0L) }
    var back by remember { mutableStateOf<Long?>(null) }

    // Swiping by hand pauses auto-scroll until 3 s after release (item 8). Only a finger
    // counts: the list's own animated scrolls must not pause it, or it falls behind.
    var dragging by remember { mutableStateOf(false) }
    LaunchedEffect(list) {
        list.interactionSource.interactions.collect { i ->
            when (i) {
                is DragInteraction.Start -> dragging = true
                is DragInteraction.Stop, is DragInteraction.Cancel -> {
                    dragging = false
                    lastTouchEnd = System.currentTimeMillis()
                }
            }
        }
    }
    var placed by remember { mutableStateOf(false) }
    LaunchedEffect(current) {
        if (current < 0 || dragging) return@LaunchedEffect
        if (lastTouchEnd != 0L && System.currentTimeMillis() - lastTouchEnd < Timing.lyricAutoScrollResumeMs) return@LaunchedEffect
        // Item 3: the active line sits about 30 % from the top of what is visible. The
        // list's top padding counts, and the first placement waits for the list's size.
        val height = snapshotFlow { list.layoutInfo.viewportSize.height }.first { it > 0 }
        val offset = list.layoutInfo.beforeContentPadding - (height * ACTIVE_LINE_FROM_TOP).toInt()
        val index = current + if (synced) 0 else 1
        val seen = list.layoutInfo.visibleItemsInfo.firstOrNull { it.index == index }
        when {
            still || !placed -> list.scrollToItem(index, offset)
            // One unhurried glide to the next line instead of a spring that overshoots.
            seen != null -> list.animateScrollBy((seen.offset + offset).toFloat(), tween(LINE_SCROLL_MS, easing = FastOutSlowInEasing))
            else -> list.animateScrollToItem(index, offset)
        }
        placed = true
    }
    LaunchedEffect(back) {
        if (back != null) {
            delay(BACK_PILL_MS)
            back = null
        }
    }

    if (lyrics.instrumental) {
        Box(modifier, contentAlignment = Alignment.Center) { BreathingDots(colour) { 1f } }
        return
    }
    Box(modifier) {
        LazyColumn(state = list, contentPadding = PaddingValues(vertical = 120.dp), modifier = Modifier.fillMaxSize()) {
            if (!synced) item {
                Text("Not synced", style = type.caption, color = colour.copy(alpha = 0.6f), modifier = Modifier.padding(horizontal = (Grid.base * 2).dp))
            }
            itemsIndexed(lyrics.lines) { i, line ->
                val past = synced && i < current
                val active = synced && i == current
                val lineAlpha by animateFloatAsState(
                    when {
                        !synced -> 1f
                        active -> 1f
                        past -> PAST_ALPHA
                        else -> UPCOMING_ALPHA
                    },
                    tween(LINE_FADE_MS),
                    label = "line",
                )
                // The sung line grows a touch; the others settle back, all on one curve.
                val scale by animateFloatAsState(if (active || !synced) 1f else IDLE_SCALE, tween(LINE_FADE_MS, easing = FastOutSlowInEasing), label = "scale")
                val blurDp by animateFloatAsState(if (past) PAST_BLUR_DP else 0f, tween(LINE_FADE_MS), label = "blur")
                val gap = remember(i) { lyrics.gaps.firstOrNull { g -> (g.afterLine?.toInt() ?: -1) == i - 1 } }
                val inGap by remember(i) { derivedStateOf { gap != null && position() in gap.startMs until gap.endMs } }
                Column(
                    Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(LINE_RADIUS_DP.dp))
                        .clickable(enabled = synced && line.startMs != null) {
                            val start = line.startMs ?: return@clickable
                            // Item 7: tap plays from the line and offers the way back.
                            back = position()
                            pending = start
                            onSeek(start)
                            runCatching { host.core.lyricLineTapped(i.toUInt(), start) }
                        }
                        .padding(horizontal = (Grid.base * 2).dp, vertical = 10.dp),
                    horizontalAlignment = if (line.side == LyricSide.OPPOSITE) Alignment.End else Alignment.Start,
                ) {
                    // Item 6: the dots open and close smoothly instead of popping in, so the
                    // move from an instrumental gap to the next line does not jump.
                    androidx.compose.animation.AnimatedVisibility(
                        inGap,
                        enter = androidx.compose.animation.expandVertically(tween(LINE_FADE_MS)) + fadeIn(tween(LINE_FADE_MS)),
                        exit = androidx.compose.animation.shrinkVertically(tween(LINE_FADE_MS)) + fadeOut(tween(LINE_FADE_MS / 2)),
                    ) {
                        val g = gap
                        BreathingDots(colour) { if (g == null) 0f else ((position() - g.startMs).toFloat() / (g.endMs - g.startMs)).coerceIn(0f, 1f) }
                    }
                    val opposite = line.side == LyricSide.OPPOSITE
                    val lineModifier = Modifier
                        .graphicsLayer {
                            alpha = lineAlpha
                            scaleX = scale
                            scaleY = scale
                            transformOrigin = TransformOrigin(if (opposite) 1f else 0f, 0.5f)
                        }
                        .let { if (blurDp > 0.05f && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) it.blur(blurDp.dp) else it }
                    if (active && line.words.isNotEmpty()) {
                        FlowRow(lineModifier, horizontalArrangement = if (opposite) Arrangement.End else Arrangement.Start) {
                            line.words.forEach { w -> SungWord(w, position, colour) }
                        }
                    } else {
                        Text(
                            line.text,
                            style = type.lyricLine,
                            color = colour,
                            textAlign = if (line.side == LyricSide.OPPOSITE) TextAlign.End else TextAlign.Start,
                            modifier = lineModifier,
                        )
                    }
                    // Item 5: background vocals, smaller and dimmer, while sung.
                    if (active) line.background.forEach { bg ->
                        Text(bg.text, style = type.lyricLine.copy(fontSize = type.lyricLine.fontSize * 0.65f), color = colour.copy(alpha = 0.6f))
                    }
                    line.translation?.let {
                        Text(it, style = type.lyricLine.copy(fontSize = type.lyricLine.fontSize * 0.6f), color = colour.copy(alpha = 0.6f * lineAlpha))
                    }
                }
            }
        }
        AnimatedVisibility(back != null, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = (Grid.base * 2).dp)) {
            val to = back ?: 0L
            Surface(shape = CircleShape, color = colour.copy(alpha = 0.18f), modifier = Modifier.clickable {
                pending = to
                onSeek(to)
                back = null
            }) {
                Text("Back to ${formatDuration(to)}", style = type.buttonLabelSelected, color = colour, modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp))
            }
        }
    }
}

/** Each word fills left to right across its own sung time, with a soft edge. */
@Composable
private fun SungWord(w: LyricWord, position: () -> Long, colour: Color) {
    val type = LocalEncoreType.current
    val span = (w.endMs - w.startMs).coerceAtLeast(1)
    Box {
        Text(w.text + if (w.spaceAfter) " " else "", style = type.lyricLine, color = colour.copy(alpha = UPCOMING_ALPHA))
        Text(
            w.text + if (w.spaceAfter) " " else "",
            style = type.lyricLine,
            color = colour,
            modifier = Modifier.drawWithContent {
                val word = this
                // Read while drawing: the fill moves every frame without recomposing.
                val fill = ((position() + LEAD_MS - w.startMs).toFloat() / span).coerceIn(0f, 1f)
                clipRect(right = size.width * fill) { word.drawContent() }
            },
        )
    }
}

/** Item 6: three dots breathe in time and fill one by one before the next line. */
@Composable
private fun BreathingDots(colour: Color, progress: () -> Float) {
    val breathe by rememberInfiniteTransition(label = "dots").animateFloat(0.85f, 1.15f, infiniteRepeatable(tween(900), RepeatMode.Reverse), label = "b")
    // Drawn, not composed: each dot brightens smoothly as its third of the gap passes.
    androidx.compose.foundation.Canvas(Modifier.padding(vertical = 8.dp).size(width = DOTS_W_DP.dp, height = DOTS_H_DP.dp).clearAndSetSemantics { }) {
        val p = progress()
        val r = 5.dp.toPx() * breathe
        val step = 20.dp.toPx()
        repeat(3) { i ->
            val lit = ((p - i / 3f) * 3f).coerceIn(0f, 1f)
            drawCircle(colour.copy(alpha = 0.35f + 0.65f * lit), r, androidx.compose.ui.geometry.Offset(r + 2.dp.toPx() + i * step, size.height / 2))
        }
    }
}

private const val PAST_ALPHA = 0.4f
private const val UPCOMING_ALPHA = 0.55f
private const val PAST_BLUR_DP = 1.2f
private const val LEAD_MS = 150L
private const val LINE_SCROLL_MS = 650
private const val LINE_FADE_MS = 450
private const val IDLE_SCALE = 0.96f
private const val DOTS_W_DP = 56
private const val DOTS_H_DP = 16
private const val BACK_PILL_MS = 4_000L

private const val ACTIVE_LINE_FROM_TOP = 0.3f
private const val CAUGHT_UP_MS = 1_000L
private const val PENDING_TIMEOUT_MS = 2_000L
private const val LINE_RADIUS_DP = 16
