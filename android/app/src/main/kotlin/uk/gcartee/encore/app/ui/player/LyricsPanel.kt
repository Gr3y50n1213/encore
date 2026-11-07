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

/** Board 10, drawn in place of the artwork on Now Playing (item 8). */
@Composable
fun LyricsPanel(host: CoreHost, song: Song, positionMs: Long, colour: Color, onSeek: (Long) -> Unit, modifier: Modifier = Modifier) {
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
        is LyricsLoad.Ready -> Lines(host, l.lyrics, positionMs, colour, onSeek, modifier)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Lines(host: CoreHost, lyrics: SongLyrics, positionMs: Long, colour: Color, onSeek: (Long) -> Unit, modifier: Modifier) {
    val type = LocalEncoreType.current
    val still = reduceMotion()
    val synced = lyrics.syncLevel != LyricsSync.UNSYNCED
    val current = if (synced) lyrics.lines.indexOfLast { (it.startMs ?: Long.MAX_VALUE) <= positionMs } else -1
    val list = rememberLazyListState()
    var lastTouchEnd by remember { mutableLongStateOf(0L) }
    var back by remember { mutableStateOf<Long?>(null) }

    // Swiping by hand pauses auto-scroll until 3 s after release (item 8).
    LaunchedEffect(list) {
        snapshotFlow { list.isScrollInProgress }.collect { scrolling -> if (!scrolling) lastTouchEnd = System.currentTimeMillis() }
    }
    LaunchedEffect(current) {
        if (current < 0 || list.isScrollInProgress) return@LaunchedEffect
        if (System.currentTimeMillis() - lastTouchEnd < Timing.lyricAutoScrollResumeMs && lastTouchEnd != 0L) return@LaunchedEffect
        // The active line sits about 30 % from the top.
        val offset = -(list.layoutInfo.viewportSize.height * 0.3f).toInt()
        if (still) list.scrollToItem(current, offset) else list.animateScrollToItem(current, offset)
    }
    LaunchedEffect(back) {
        if (back != null) {
            delay(BACK_PILL_MS)
            back = null
        }
    }

    if (lyrics.instrumental) {
        Box(modifier, contentAlignment = Alignment.Center) { BreathingDots(colour, 1f) }
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
                val alpha by animateFloatAsState(
                    when {
                        !synced -> 1f
                        active -> 1f
                        past -> PAST_ALPHA
                        else -> UPCOMING_ALPHA
                    },
                    label = "line",
                )
                val gapBefore = lyrics.gaps.firstOrNull { g ->
                    ((g.afterLine?.toInt() ?: -1) == i - 1) && positionMs in g.startMs until g.endMs
                }
                Column(
                    Modifier
                        .fillMaxWidth()
                        .clickable(enabled = synced && line.startMs != null) {
                            val start = line.startMs ?: return@clickable
                            // Item 7: tap seeks to the line and offers the way back.
                            back = positionMs
                            onSeek(start)
                            runCatching { host.core.lyricLineTapped(i.toUInt(), start) }
                        }
                        .padding(horizontal = (Grid.base * 2).dp, vertical = 10.dp),
                    horizontalAlignment = if (line.side == LyricSide.OPPOSITE) Alignment.End else Alignment.Start,
                ) {
                    gapBefore?.let { g ->
                        BreathingDots(colour, ((positionMs - g.startMs).toFloat() / (g.endMs - g.startMs)).coerceIn(0f, 1f))
                    }
                    val lineModifier = Modifier
                        .alpha(alpha)
                        .let { if (past && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) it.blur(PAST_BLUR_DP.dp) else it }
                    if (active && line.words.isNotEmpty()) {
                        FlowRow(lineModifier, horizontalArrangement = if (line.side == LyricSide.OPPOSITE) Arrangement.End else Arrangement.Start) {
                            line.words.forEach { w -> SungWord(w, positionMs, colour) }
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
                        Text(it, style = type.lyricLine.copy(fontSize = type.lyricLine.fontSize * 0.6f), color = colour.copy(alpha = 0.6f * alpha))
                    }
                }
            }
        }
        AnimatedVisibility(back != null, enter = fadeIn(), exit = fadeOut(), modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = (Grid.base * 2).dp)) {
            val to = back ?: 0L
            Surface(shape = CircleShape, color = colour.copy(alpha = 0.18f), modifier = Modifier.clickable {
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
private fun SungWord(w: LyricWord, positionMs: Long, colour: Color) {
    val type = LocalEncoreType.current
    val span = (w.endMs - w.startMs).coerceAtLeast(1)
    val fill = ((positionMs - w.startMs).toFloat() / span).coerceIn(0f, 1f)
    Box {
        Text(w.text + if (w.spaceAfter) " " else "", style = type.lyricLine, color = colour.copy(alpha = UPCOMING_ALPHA))
        Text(
            w.text + if (w.spaceAfter) " " else "",
            style = type.lyricLine,
            color = colour,
            modifier = Modifier.drawWithContent {
                val word = this
                clipRect(right = size.width * fill) { word.drawContent() }
            },
        )
    }
}

/** Item 6: three dots breathe in time and fill one by one before the next line. */
@Composable
private fun BreathingDots(colour: Color, progress: Float) {
    val breathe by rememberInfiniteTransition(label = "dots").animateFloat(0.85f, 1.15f, infiniteRepeatable(tween(900), RepeatMode.Reverse), label = "b")
    Row(Modifier.padding(vertical = 8.dp).clearAndSetSemantics { }, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        repeat(3) { i ->
            val lit = progress >= (i + 1) / 3f
            Box(Modifier.size((10 * breathe).dp).background(colour.copy(alpha = if (lit) 1f else 0.35f), CircleShape))
        }
    }
}

private const val PAST_ALPHA = 0.4f
private const val UPCOMING_ALPHA = 0.55f
private const val PAST_BLUR_DP = 1.2f
private const val BACK_PILL_MS = 4_000L
