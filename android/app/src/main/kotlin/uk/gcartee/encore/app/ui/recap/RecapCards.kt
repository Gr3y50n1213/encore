package uk.gcartee.encore.app.ui.recap

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import kotlin.math.roundToLong
import kotlin.math.sqrt
import uk.gcartee.encore.app.recap.Beat
import uk.gcartee.encore.app.recap.BeatData
import uk.gcartee.encore.app.recap.GroundKind
import uk.gcartee.encore.app.recap.PackId
import uk.gcartee.encore.app.recap.Recap
import uk.gcartee.encore.app.recap.RecapFormat
import uk.gcartee.encore.app.recap.RecapText
import uk.gcartee.encore.tokens.EncoreTokens.Recap.Motion as RecapMotion

/** Room the progress segments and the top row take at the top of every card (design dp). */
const val CHROME_DP = 40

/** Everything one card needs to draw itself. */
@Immutable
class CardEnv(
    val recap: Recap,
    val theme: RecapTheme,
    val ground: Ground,
    val beat: Beat,
    val index: Int,
    val count: Int,
    /** "No. 01": the cards after the intro, counted from one. */
    val number: Int,
    /** Reduce Motion, TalkBack or an export: the still final frame. */
    val still: Boolean,
    /** The rest of a top 5 is showing (board 39, item 4). */
    val reveal: Boolean,
    /** Song, artist and album names hidden, for sharing (board 39, item 8). */
    val hideNames: Boolean,
    val names: Set<String>,
    val forExport: Boolean,
    val countedThrough: String,
    val storySeconds: Long,
    val profileName: String,
) {
    fun n(name: String): String = if (hideNames) RecapText.HIDDEN else name
    fun say(text: String): String = if (hideNames) RecapText.mask(text, names) else text
    fun motif(i: Int): Int = Math.floorMod(beat.shapeSeed + i, 4L).toInt()
}

/** The closer's buttons (board 39, card 11). */
class CloserActions(
    val saving: Boolean,
    val saved: Boolean,
    val onSave: () -> Unit,
    val onShare: () -> Unit,
    val onDetail: () -> Unit,
)

/**
 * One card: its section header, the beat's own body, and the counted-through line (board 41,
 * "Preview": the date on every card). The whole card reads as its text version; only the
 * closer's buttons are separate stops for a screen reader.
 */
@Composable
fun BeatCard(env: CardEnv, insets: PaddingValues, closer: CloserActions?, modifier: Modifier = Modifier) {
    val enterAnim = remember(env.beat) { Animatable(if (env.still) 1f else 0f) }
    LaunchedEffect(env.beat, env.still) {
        if (env.still) enterAnim.snapTo(1f) else enterAnim.animateTo(1f, tween(RecapMotion.easeInMs.lastOrNull() ?: 600, easing = FastOutSlowInEasing))
    }
    val enter = enterAnim.value
    val description = remember(env.beat, env.index, env.count) { RecapText.describe(env.recap, env.beat, env.index, env.count, env.countedThrough) }
    Column(modifier.fillMaxSize().padding(insets).padding(start = 22.dp, end = 22.dp, top = CHROME_DP.dp, bottom = 18.dp)) {
        Column(
            Modifier.weight(1f).fillMaxWidth().clearAndSetSemantics {
                contentDescription = description
                heading()
            },
        ) {
            SectionHeader(env)
            when (val d = env.beat.data) {
                is BeatData.Intro -> IntroBody(env, d, enter)
                is BeatData.Minutes -> MinutesBody(env, d, enter)
                is BeatData.TopSong -> TopSongBody(env, d)
                is BeatData.TopArtist -> TopArtistBody(env, d)
                is BeatData.TopAlbum -> TopAlbumBody(env, d, enter)
                is BeatData.YourSound -> YourSoundBody(env, d, enter)
                is BeatData.Personality -> PersonalityBody(env, d, enter)
                is BeatData.Closer -> CloserBody(env, d)
                is BeatData.ListeningClock -> ClockBody(env, d, enter)
                is BeatData.BiggestDiscovery -> DiscoveryBody(env, d, enter)
                is BeatData.Comeback -> ComebackBody(env, d, enter)
                is BeatData.ObsessionWeek -> ObsessionBody(env, d, enter)
                is BeatData.LongestStreak -> StreakBody(env, d, enter)
                is BeatData.SongOfEachSeason -> SeasonsBody(env, d)
                is BeatData.MostReplayedDay -> ReplayedDayBody(env, d)
                is BeatData.MoodArc -> MoodBody(env, d, enter)
                is BeatData.MostSungLine -> SungLineBody(env, d)
                is BeatData.Unknown -> Body(env, env.say(env.beat.text))
            }
        }
        if (closer != null && env.beat.data is BeatData.Closer && !env.forExport) CloserButtons(env, closer)
        Footer(env)
    }
}

// --- Shared pieces -------------------------------------------------------------------------

@Composable
private fun Label(env: CardEnv, text: String, modifier: Modifier = Modifier, color: Color = env.ground.fg, alpha: Float = 1f) {
    Text(
        if (env.theme.type.labelUpper) text.uppercase() else text,
        style = env.theme.type.label,
        color = color.copy(alpha = color.alpha * alpha),
        modifier = modifier,
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
    )
}

@Composable
private fun Body(env: CardEnv, text: String, modifier: Modifier = Modifier, size: TextUnit = TextUnit.Unspecified, alpha: Float = 1f) {
    if (text.isBlank()) return
    val style = if (size == TextUnit.Unspecified) env.theme.type.body else env.theme.type.body.copy(fontSize = size)
    Text(text, style = style, color = env.ground.fg.copy(alpha = alpha), modifier = modifier)
}

@Composable
private fun Display(env: CardEnv, text: String, size: TextUnit, modifier: Modifier = Modifier, color: Color = env.ground.fg, italic: Boolean = false, maxLines: Int = 3, lineHeight: Float? = null) {
    val base = if (italic) env.theme.type.italic else env.theme.type.display
    val style = if (lineHeight != null) base.copy(fontSize = size, lineHeight = lineHeight.em) else base.copy(fontSize = size)
    Text(text, style = style, color = color, modifier = modifier, maxLines = maxLines, overflow = TextOverflow.Ellipsis)
}

/** A name's size by its length, so a long name still fits its card. */
private fun nameSize(name: String, big: Float): TextUnit {
    val n = name.length
    val f = when {
        n <= 8 -> 1f
        n <= 12 -> 0.8f
        n <= 18 -> 0.62f
        n <= 28 -> 0.48f
        else -> 0.38f
    }
    return (big * f).sp
}

/** Numbers count up over 600 ms (board 39, item 12); Reduce Motion shows the final number. */
@Composable
private fun CountUp(value: Long, still: Boolean, style: TextStyle, color: Color, modifier: Modifier = Modifier, format: (Long) -> String = { RecapFormat.count(it) }) {
    val anim = remember(value) { Animatable(if (still) value.toFloat() else 0f) }
    LaunchedEffect(value, still) {
        if (still) anim.snapTo(value.toFloat()) else anim.animateTo(value.toFloat(), tween(RecapMotion.countUpMs, easing = FastOutSlowInEasing))
    }
    Text(format(anim.value.roundToLong()), style = style, color = color, modifier = modifier, maxLines = 1)
}

private fun Modifier.hairlineTop(on: Boolean, colour: Color): Modifier =
    if (!on) this else drawBehind { drawLine(colour, Offset(0f, 0f), Offset(size.width, 0f), 1.dp.toPx()) }

private fun Modifier.hairlineBottom(colour: Color): Modifier =
    drawBehind { drawLine(colour, Offset(0f, size.height), Offset(size.width, size.height), 1.dp.toPx()) }

private val Tabular = TextStyle(fontFeatureSettings = "tnum")

/** A ranked list's rows ("2  Marigold Ave  1,511"), in the rest-of-top-5 fade. */
@Composable
private fun TopRows(env: CardEnv, rows: List<Triple<Int, String, Long>>, modifier: Modifier = Modifier) {
    if (rows.isEmpty()) return
    val alpha by animateFloatAsState(if (env.reveal) 1f else 0f, tween(400), label = "rest")
    val type = env.theme.type
    val g = env.ground
    val ruled = env.theme.pack == PackId.EDITORIAL
    Panel(env.theme, modifier.fillMaxWidth().graphicsLayer { this.alpha = alpha }) {
        Column(Modifier.fillMaxWidth()) {
            rows.forEach { (rank, name, value) ->
                Row(
                    Modifier.fillMaxWidth().hairlineTop(ruled, g.fg.copy(alpha = 0.4f)).padding(vertical = 5.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text("$rank", style = type.row.merge(Tabular).copy(fontSize = 9.sp, fontWeight = FontWeight.Normal), color = g.fg.copy(alpha = 0.6f), modifier = Modifier.width(14.dp))
                    Text(env.n(name), style = type.row, color = g.fg, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                    Text(RecapFormat.count(value), style = type.row.merge(Tabular).copy(fontWeight = FontWeight.Normal), color = g.fg.copy(alpha = 0.8f), modifier = Modifier.padding(start = 8.dp))
                }
            }
        }
    }
}

@Composable
private fun SectionHeader(env: CardEnv) {
    if (env.beat.data is BeatData.Intro) return
    val g = env.ground
    val title = when (env.theme.pack) {
        PackId.TIDE, PackId.BLOOM -> when (env.beat.id) {
            "topSong" -> "Your top song"
            "topArtist" -> "Your top artist"
            "topAlbum" -> "Your top album"
            "listeningClock" -> "Your busiest hour"
            else -> RecapText.title(env.beat.id)
        }
        else -> RecapText.title(env.beat.id)
    }
    if (env.theme.pack == PackId.EDITORIAL) {
        Row(Modifier.fillMaxWidth().hairlineBottom(g.fg).padding(bottom = 6.dp)) {
            Label(env, "No. ${env.number.toString().padStart(2, '0')}")
            Spacer(Modifier.weight(1f))
            Label(env, title)
        }
    } else {
        Label(env, title, alpha = 0.85f)
    }
    Spacer(Modifier.height(16.dp))
}

@Composable
private fun Footer(env: CardEnv) {
    Column(Modifier.fillMaxWidth().padding(top = 10.dp).clearAndSetSemantics { }) {
        if (env.theme.fallback) Label(env, "Shown in a simpler look on this device", alpha = 0.8f, modifier = Modifier.padding(bottom = 4.dp))
        val since = env.recap.since?.let { "Since ${RecapFormat.day(it.day)}" }
        val so = if (env.recap.soFar) "So far" else null
        Label(env, listOfNotNull(so, "Counted through ${env.countedThrough}", since).joinToString(" · "), alpha = 0.7f)
    }
}

@Composable
private fun Centered(content: @Composable () -> Unit) {
    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { content() }
}

// --- The beats -----------------------------------------------------------------------------

@Composable
private fun ColumnScope.IntroBody(env: CardEnv, d: BeatData.Intro, enter: Float) {
    val g = env.ground
    Spacer(Modifier.height(28.dp))
    Label(env, if (env.recap.soFar) "Your year so far" else "Your year in music")
    Display(
        env,
        d.year.toString(),
        if (env.theme.pack == PackId.EDITORIAL) 104.sp else 92.sp,
        modifier = Modifier.padding(top = 8.dp).graphicsLayer {
            alpha = enter
            translationY = (1f - enter) * 24.dp.toPx()
        },
        maxLines = 1,
    )
    if (d.yearLine.isNotBlank()) Display(env, d.yearLine, 27.sp, Modifier.padding(top = 16.dp), italic = true, maxLines = 4, lineHeight = 1.08f)
    Spacer(Modifier.weight(1f))
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        repeat(d.albumKeys.size.coerceIn(1, 4)) { i -> CoverFrame(env.theme, g, env.motif(i), 50.dp, block = false) }
    }
    Spacer(Modifier.height(18.dp))
    Row(Modifier.fillMaxWidth().hairlineTop(true, g.fg).padding(top = 8.dp)) {
        Label(env, env.profileName.takeIf { it.isNotBlank() } ?: "Your ${d.year}", modifier = Modifier.weight(1f))
        Label(env, "${env.count} cards · about ${env.storySeconds} s")
    }
}

@Composable
private fun ColumnScope.MinutesBody(env: CardEnv, d: BeatData.Minutes, enter: Float) {
    val g = env.ground
    CountUp(d.minutes, env.still, env.theme.type.display.copy(fontSize = 62.sp), g.fg)
    Display(env, if (env.recap.soFar) "minutes so far." else "minutes with your music.", 20.sp, Modifier.padding(top = 8.dp), italic = true)
    if (d.days > 0 || d.hours > 0) Label(env, "${d.days} days · ${d.hours} hours", Modifier.padding(top = 8.dp), alpha = 0.7f)
    Spacer(Modifier.weight(1f))
    val months = (1..12).map { m -> d.months.firstOrNull { it.month == m }?.minutes ?: 0L }
    val peak = months.indices.maxByOrNull { months[it] }
    BarsChart(env.theme, g, months, RecapFormat.MONTH_LETTERS, enter, 80.dp, highlight = peak)
    Spacer(Modifier.height(18.dp))
    Body(env, env.say(env.beat.text))
}

@Composable
private fun ColumnScope.TopSongBody(env: CardEnv, d: BeatData.TopSong) {
    val g = env.ground
    val one = d.top.first()
    if (env.theme.pack == PackId.EDITORIAL) {
        Row(verticalAlignment = Alignment.Top) {
            CoverFrame(env.theme, g, env.motif(0), 100.dp)
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Label(env, "${RecapFormat.count(one.plays)} plays", color = g.accentText)
                Display(env, env.n(one.title), 25.sp, Modifier.padding(top = 6.dp), maxLines = 4, lineHeight = 1f)
                Body(env, env.n(one.artist), Modifier.padding(top = 6.dp), size = 11.sp, alpha = 0.8f)
            }
        }
    } else {
        Centered { CoverFrame(env.theme, g, env.motif(0), 124.dp) }
        Spacer(Modifier.height(20.dp))
        Display(env, env.n(one.title), nameSize(one.title, 40f), maxLines = 2)
        Body(env, "${env.n(one.artist)} · ${RecapFormat.count(one.plays)} plays", Modifier.padding(top = 6.dp), alpha = 0.85f)
    }
    Spacer(Modifier.height(22.dp))
    TopRows(env, d.top.drop(1).map { Triple(it.rank, it.title, it.plays) })
    Spacer(Modifier.weight(1f))
    Body(env, env.say(env.beat.text))
}

@Composable
private fun ColumnScope.TopArtistBody(env: CardEnv, d: BeatData.TopArtist) {
    val g = env.ground
    val one = d.top.first()
    val stats = "${RecapFormat.count(one.plays)} plays · ${RecapFormat.hours(one.listenedMs)} hours"
    val rest = d.top.drop(1).map { Triple(it.rank, it.name, it.plays) }
    when (env.theme.pack) {
        PackId.EDITORIAL -> {
            Display(env, env.n(one.name), nameSize(one.name, 76f), maxLines = 3, lineHeight = 0.86f)
            Spacer(Modifier.weight(1f))
            Label(env, stats)
            Spacer(Modifier.height(10.dp))
            TopRows(env, rest)
        }
        PackId.GLASS -> {
            Box(Modifier.fillMaxWidth().height(250.dp)) {
                GlassBubble(env, "", 170f, Modifier.offset(6.dp, 6.dp), cover = true)
                val spots = listOf(Offset(146f, 86f), Offset(8f, 178f), Offset(82f, 184f), Offset(154f, 176f))
                d.top.drop(1).take(4).forEachIndexed { i, a ->
                    val s = (70f * sqrt(a.plays.toFloat() / one.plays.coerceAtLeast(1L))).coerceIn(34f, 70f)
                    GlassBubble(env, env.n(a.name.substringBefore(' ')), s, Modifier.offset(spots[i].x.dp, spots[i].y.dp))
                }
            }
            Spacer(Modifier.weight(1f))
            Panel(env.theme, Modifier.fillMaxWidth()) {
                Column {
                    Label(env, "Top artist", alpha = 0.8f)
                    Display(env, env.n(one.name), 26.sp, Modifier.padding(top = 3.dp), maxLines = 2)
                    Body(env, stats, Modifier.padding(top = 2.dp), alpha = 0.85f)
                }
            }
        }
        else -> {
            Centered { CoverFrame(env.theme, g, env.motif(0), if (env.theme.pack == PackId.BLOOM) 150.dp else 124.dp) }
            Spacer(Modifier.weight(1f))
            Display(env, env.n(one.name), nameSize(one.name, 46f), maxLines = 2)
            Body(env, stats, Modifier.padding(top = 6.dp), alpha = 0.85f)
            Spacer(Modifier.height(12.dp))
            TopRows(env, rest.take(3))
        }
    }
}

/** Glass: a bubble, sized by plays, with a name or the top artist's cover inside. */
@Composable
private fun GlassBubble(env: CardEnv, label: String, sizeDp: Float, modifier: Modifier, cover: Boolean = false) {
    Box(
        modifier
            .size(sizeDp.dp)
            .clip(CircleShape)
            .background(Brush.radialGradient(listOf(Color.White.copy(alpha = 0.55f), Color.White.copy(alpha = 0.12f), Color.White.copy(alpha = 0.06f))))
            .border(1.dp, Color.White.copy(alpha = 0.45f), CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        if (cover) CoverArt(env.motif(0), env.theme.coverColours, Modifier.fillMaxSize().graphicsLayer { alpha = 0.85f })
        if (label.isNotEmpty()) Text(label, style = env.theme.type.row.copy(fontSize = 9.sp, fontWeight = FontWeight.Bold), color = Color.White, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 4.dp))
    }
}

@Composable
private fun ColumnScope.TopAlbumBody(env: CardEnv, d: BeatData.TopAlbum, enter: Float) {
    val g = env.ground
    val one = d.top.first()
    if (env.theme.pack == PackId.EDITORIAL) CoverFrame(env.theme, g, env.motif(1), 130.dp) else Centered { CoverFrame(env.theme, g, env.motif(1), 140.dp) }
    Spacer(Modifier.height(18.dp))
    Display(env, env.n(one.name), nameSize(one.name, 36f), maxLines = 2)
    Body(env, env.n(one.artist), Modifier.padding(top = 6.dp), size = 11.sp)
    Spacer(Modifier.height(14.dp))
    val through = d.playThroughs ?: 0L
    if (through > 0) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            DotLedger(through, if (g.kind == GroundKind.SPOT) g.fg else g.accentShape, round = env.theme.pack != PackId.EDITORIAL, enter = enter, modifier = Modifier.width(130.dp).height(32.dp), cell = 12.dp, cap = 40)
            Spacer(Modifier.width(12.dp))
            Body(env, "Played start to finish ${RecapFormat.count(through)} ${if (through == 1L) "time" else "times"}.", Modifier.weight(1f), size = 11.sp)
        }
    } else {
        Label(env, "${RecapFormat.count(one.plays)} plays · ${RecapFormat.hours(one.listenedMs)} hours")
    }
    Spacer(Modifier.weight(1f))
    TopRows(env, d.top.drop(1).map { Triple(it.rank, it.name, it.plays) })
}

private fun streamColours(env: CardEnv): List<Color> {
    val g = env.ground
    if (env.theme.pack != PackId.EDITORIAL) return (g.shapes + g.accentShape).ifEmpty { listOf(g.fg) }
    val second = if (g.kind == GroundKind.SPOT) Color(env.theme.colours.paper) else g.accentShape
    return listOf(g.fg, second, lerp(g.top, g.fg, 0.55f), lerp(g.top, g.fg, 0.3f))
}

@Composable
private fun ColumnScope.YourSoundBody(env: CardEnv, d: BeatData.YourSound, enter: Float) {
    val g = env.ground
    Display(env, env.say(env.beat.text), 24.sp, italic = true, maxLines = 5, lineHeight = 1.05f)
    Spacer(Modifier.weight(1f))
    val colours = streamColours(env)
    StreamChart(d.months, colours, enter, Modifier.fillMaxWidth().height(130.dp))
    Row(Modifier.fillMaxWidth().padding(top = 4.dp)) {
        Label(env, "Jan", alpha = 0.7f)
        Spacer(Modifier.weight(1f))
        Label(env, "Dec", alpha = 0.7f)
    }
    Spacer(Modifier.height(10.dp))
    d.top.take(4).forEachIndexed { i, genre ->
        Row(Modifier.fillMaxWidth().hairlineTop(env.theme.pack == PackId.EDITORIAL, g.fg.copy(alpha = 0.3f)).padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(8.dp).background(colours[i % colours.size]))
            Spacer(Modifier.width(8.dp))
            Text(genre.name.replaceFirstChar { it.uppercase() }, style = env.theme.type.row, color = g.fg, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("${RecapFormat.percent(genre.permille)}%", style = env.theme.type.row.merge(Tabular).copy(fontWeight = FontWeight.Normal), color = g.fg)
        }
    }
}

@Composable
private fun ColumnScope.ClockBody(env: CardEnv, d: BeatData.ListeningClock, enter: Float) {
    val g = env.ground
    val hour = RecapFormat.hour(d.peakHour)
    if (env.theme.pack == PackId.EDITORIAL) {
        Display(env, hour, 66.sp, color = g.accentText, maxLines = 1)
        Display(env, env.say(env.beat.text), 19.sp, Modifier.padding(top = 4.dp), italic = true, maxLines = 4, lineHeight = 1.1f)
        Spacer(Modifier.weight(1f))
        HourChart(env.theme, g, d.grid, d.peakWeekday, d.peakHour, enter)
        Spacer(Modifier.height(12.dp))
        Label(env, "↗ ${RecapFormat.weekday(d.peakWeekday)}, $hour", color = g.accentText)
        Spacer(Modifier.weight(1f))
        Label(env, "Rows are Monday to Sunday", alpha = 0.8f)
    } else {
        if (env.theme.pack != PackId.GLASS) Display(env, hour, 56.sp, color = g.accentText, maxLines = 1)
        Body(env, env.say(env.beat.text), Modifier.padding(top = 6.dp))
        Spacer(Modifier.weight(1f))
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            HourChart(env.theme, g, d.grid, d.peakWeekday, d.peakHour, enter, Modifier.size(196.dp))
            if (env.theme.pack == PackId.GLASS) Display(env, hour, 22.sp, maxLines = 1)
        }
        Spacer(Modifier.weight(1f))
        Label(env, "${RecapFormat.weekday(d.peakWeekday)}, $hour", alpha = 0.85f)
    }
}

@Composable
private fun ColumnScope.DiscoveryBody(env: CardEnv, d: BeatData.BiggestDiscovery, enter: Float) {
    val g = env.ground
    Row(verticalAlignment = Alignment.Top) {
        Display(env, env.n(d.artist.name), nameSize(d.artist.name, 50f), Modifier.weight(1f), maxLines = 3, lineHeight = 0.9f)
        Spacer(Modifier.width(10.dp))
        CoverFrame(env.theme, g, env.motif(2), 70.dp, block = false)
    }
    Spacer(Modifier.height(18.dp))
    Body(env, env.say(env.beat.text))
    Spacer(Modifier.weight(1f))
    val colour = if (g.kind == GroundKind.SPOT) g.fg else g.accentShape
    BarsChart(env.theme, g, (0 until 12).map { d.months.getOrElse(it) { 0L } }, RecapFormat.MONTH_LETTERS, enter, 92.dp, colour = colour, accent = colour)
}

@Composable
private fun ColumnScope.ComebackBody(env: CardEnv, d: BeatData.Comeback, enter: Float) {
    val g = env.ground
    Label(env, "Welcome back", alpha = 0.8f)
    Display(env, env.n(d.artist.name), nameSize(d.artist.name, 50f), Modifier.padding(top = 6.dp), maxLines = 3, lineHeight = 0.9f)
    Spacer(Modifier.height(18.dp))
    Body(env, env.say(env.beat.text))
    Spacer(Modifier.weight(1f))
    // Before, the time away, and this year: two solid runs with a dashed gap between.
    val line = g.fg
    val accent = if (g.kind == GroundKind.SPOT) g.fg else g.accentShape
    androidx.compose.foundation.Canvas(Modifier.fillMaxWidth().height(28.dp)) {
        val y = size.height / 2f
        val third = size.width / 3f
        val stroke = 6.dp.toPx()
        drawLine(line, Offset(0f, y), Offset(third * 0.9f * enter, y), stroke)
        val dash = 4.dp.toPx()
        var x = third
        while (x < third * 2f) {
            drawLine(line.copy(alpha = 0.5f), Offset(x, y), Offset((x + dash).coerceAtMost(third * 2f), y), 2.dp.toPx())
            x += dash * 2f
        }
        drawLine(accent, Offset(third * 2.1f, y), Offset(third * 2.1f + third * 0.9f * enter, y), stroke)
    }
    Row(Modifier.fillMaxWidth().padding(top = 6.dp)) {
        Label(env, "${RecapFormat.count(d.playsBefore)} plays before", Modifier.weight(1f))
        Label(env, "${RecapFormat.count(d.gapDays)} days away", Modifier.weight(1f))
        Label(env, "${RecapFormat.count(d.artist.plays)} this year", Modifier.weight(1f))
    }
}

@Composable
private fun ColumnScope.ObsessionBody(env: CardEnv, d: BeatData.ObsessionWeek, enter: Float) {
    val g = env.ground
    Display(env, env.say(env.beat.text), 33.sp, maxLines = 5, lineHeight = 1f)
    Spacer(Modifier.weight(1f))
    val days = (0 until 7).map { d.days.getOrElse(it) { 0L } }
    BarsChart(env.theme, g, days, RecapFormat.WEEKDAY_LETTERS, enter, 140.dp, highlight = days.indices.maxByOrNull { days[it] }, showValues = true)
    Spacer(Modifier.height(12.dp))
    Label(env, "${RecapFormat.count(d.weekPlays)} plays in seven days")
}

@Composable
private fun ColumnScope.StreakBody(env: CardEnv, d: BeatData.LongestStreak, enter: Float) {
    val g = env.ground
    CountUp(d.days, env.still, env.theme.type.display.copy(fontSize = 96.sp), g.accentText)
    Display(env, "days in a row.", 22.sp, italic = true, maxLines = 1)
    Label(env, "${RecapFormat.day(d.firstDay)} to ${RecapFormat.day(d.lastDay)}", Modifier.padding(top = 8.dp), alpha = 0.8f)
    Spacer(Modifier.weight(1f))
    val colour = if (g.kind == GroundKind.SPOT || env.theme.pack == PackId.EDITORIAL) g.fg else g.accentShape
    DotLedger(d.days, colour, round = env.theme.pack != PackId.EDITORIAL, enter = enter, modifier = Modifier.fillMaxWidth().height(110.dp), cell = 9.dp)
    Spacer(Modifier.height(12.dp))
    Body(env, env.say(env.beat.text))
}

@Composable
private fun ColumnScope.SeasonsBody(env: CardEnv, d: BeatData.SongOfEachSeason) {
    val g = env.ground
    Display(env, env.say(env.beat.text), 22.sp, italic = true, maxLines = 4, lineHeight = 1.05f)
    Spacer(Modifier.weight(1f))
    Panel(env.theme, Modifier.fillMaxWidth()) {
        Column {
            d.seasons.forEachIndexed { i, s ->
                Row(Modifier.fillMaxWidth().hairlineTop(env.theme.pack == PackId.EDITORIAL, g.fg.copy(alpha = 0.4f)).padding(vertical = 7.dp), verticalAlignment = Alignment.CenterVertically) {
                    CoverFrame(env.theme, g, env.motif(i), 40.dp, block = false)
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Label(env, RecapFormat.seasonName(s.id), alpha = 0.75f)
                        Display(env, env.n(s.song.title), 16.sp, maxLines = 1)
                        Body(env, env.n(s.song.artist), size = 10.5.sp, alpha = 0.8f)
                    }
                }
            }
        }
    }
}

@Composable
private fun ColumnScope.ReplayedDayBody(env: CardEnv, d: BeatData.MostReplayedDay) {
    val g = env.ground
    CountUp(d.plays, env.still, env.theme.type.display.copy(fontSize = 96.sp), g.accentText)
    Display(env, "times in one day.", 22.sp, italic = true, maxLines = 1)
    Label(env, RecapFormat.day(d.day), Modifier.padding(top = 8.dp), alpha = 0.8f)
    Spacer(Modifier.weight(1f))
    Row(verticalAlignment = Alignment.CenterVertically) {
        CoverFrame(env.theme, g, env.motif(0), 80.dp)
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Display(env, env.n(d.song.title), nameSize(d.song.title, 24f), maxLines = 3)
            Body(env, env.n(d.song.artist), Modifier.padding(top = 4.dp), alpha = 0.8f)
        }
    }
    Spacer(Modifier.height(16.dp))
    Body(env, env.say(env.beat.text))
}

@Composable
private fun ColumnScope.MoodBody(env: CardEnv, d: BeatData.MoodArc, enter: Float) {
    val g = env.ground
    Display(env, env.say(env.beat.text), 24.sp, italic = true, maxLines = 4, lineHeight = 1.05f)
    Spacer(Modifier.weight(1f))
    val energy = (1..12).map { m -> d.months.firstOrNull { it.month == m }?.energy }
    val valence = (1..12).map { m -> d.months.firstOrNull { it.month == m }?.valence }
    val accent = if (g.kind == GroundKind.SPOT) g.fg else g.accentShape
    val second = g.fg.copy(alpha = 0.45f)
    LineChart(energy, valence, accent, second, enter, Modifier.fillMaxWidth().height(130.dp))
    Row(Modifier.fillMaxWidth().padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        RecapFormat.MONTH_LETTERS.forEach { l -> Text(l, style = env.theme.type.row.copy(fontSize = 7.5.sp, textAlign = TextAlign.Center), color = g.fg.copy(alpha = 0.7f), modifier = Modifier.weight(1f)) }
    }
    Spacer(Modifier.height(10.dp))
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(width = 14.dp, height = 3.dp).background(accent))
        Spacer(Modifier.width(6.dp))
        Label(env, "Energy")
        Spacer(Modifier.width(16.dp))
        Box(Modifier.size(width = 14.dp, height = 2.dp).background(second))
        Spacer(Modifier.width(6.dp))
        Label(env, "Brightness")
    }
    Spacer(Modifier.height(8.dp))
    Body(env, "It describes the music, not you.", alpha = 0.8f)
}

@Composable
private fun ColumnScope.SungLineBody(env: CardEnv, d: BeatData.MostSungLine) {
    val g = env.ground
    // A big opening quote mark, drawn so it needs no font.
    androidx.compose.foundation.Canvas(Modifier.size(56.dp, 44.dp)) {
        val r = size.height * 0.28f
        for (k in 0..1) {
            val cx = r + k * size.width * 0.5f
            drawCircle(g.accentText, r, Offset(cx, size.height - r))
            val tail = Path().apply {
                moveTo(cx - r, size.height - r)
                quadraticTo(cx - r, r * 0.6f, cx + r * 0.9f, 0f)
                lineTo(cx + r * 0.9f, r * 0.5f)
                quadraticTo(cx, r, cx + r * 0.2f, size.height - r * 1.6f)
                close()
            }
            drawPath(tail, g.accentText)
        }
    }
    Spacer(Modifier.height(10.dp))
    CountUp(d.jumps, env.still, env.theme.type.display.copy(fontSize = 60.sp), g.fg, format = { "${RecapFormat.count(it)} times" })
    Display(env, "back to line ${d.lineIndex + 1} of ${env.n(d.song.title)}.", 22.sp, Modifier.padding(top = 6.dp), italic = true, maxLines = 4, lineHeight = 1.08f)
    Spacer(Modifier.weight(1f))
    Label(env, env.n(d.song.artist), alpha = 0.8f)
    Spacer(Modifier.height(8.dp))
    Body(env, env.say(env.beat.text))
}

@Composable
private fun ColumnScope.PersonalityBody(env: CardEnv, d: BeatData.Personality, enter: Float) {
    val g = env.ground
    Label(env, "You are")
    Display(env, d.name, if (env.theme.pack == PackId.EDITORIAL) 46.sp else 40.sp, Modifier.padding(top = 4.dp), italic = env.theme.pack == PackId.EDITORIAL, maxLines = 3, lineHeight = 0.95f)
    Spacer(Modifier.weight(1f))
    val permille = d.unit == "permille"
    val fraction = if (permille) d.value / 1000f else (d.value.toFloat() / (d.threshold * 2).coerceAtLeast(1L)).coerceIn(0f, 1f)
    val big = if (permille) "${RecapFormat.percent(d.value)}%" else RecapFormat.count(d.value)
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(if (env.theme.pack == PackId.BLOOM) 120.dp else 96.dp), contentAlignment = Alignment.Center) {
            ValueRing(env.theme, g, fraction, enter, Modifier.fillMaxSize())
            Display(env, big, if (env.theme.pack == PackId.BLOOM) 28.sp else 20.sp, maxLines = 1)
        }
        Spacer(Modifier.width(14.dp))
        Body(env, env.say(env.beat.text), Modifier.weight(1f), size = 11.5.sp)
    }
    Spacer(Modifier.weight(1f))
    Panel(env.theme, Modifier.fillMaxWidth()) {
        Column {
            d.metrics.take(3).forEach { m ->
                Row(Modifier.fillMaxWidth().hairlineTop(env.theme.pack == PackId.EDITORIAL, g.fg.copy(alpha = 0.27f)).padding(vertical = 5.dp)) {
                    Text(m.label, style = env.theme.type.row, color = g.fg, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(RecapFormat.metricValue(m), style = env.theme.type.row.merge(Tabular).copy(fontWeight = FontWeight.Normal), color = g.fg)
                }
            }
        }
    }
}

@Composable
private fun ColumnScope.CloserBody(env: CardEnv, d: BeatData.Closer) {
    val g = env.ground
    Display(env, d.title.ifBlank { "Your Top Songs ${env.recap.year}" }, 30.sp, maxLines = 3, lineHeight = 1f)
    Spacer(Modifier.height(16.dp))
    Column(Modifier.fillMaxWidth().weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        for (r in 0 until 2) {
            Row(Modifier.fillMaxWidth().weight(1f), horizontalArrangement = Arrangement.SpaceBetween) {
                for (c in 0 until 2) {
                    Box(Modifier.weight(1f).fillMaxSize(), contentAlignment = if (c == 0) Alignment.CenterStart else Alignment.CenterEnd) {
                        CoverFrame(env.theme, g, env.motif(r * 2 + c), 92.dp, block = false)
                    }
                }
            }
        }
    }
    Spacer(Modifier.height(8.dp))
    Body(env, "${RecapFormat.count(d.count)} songs · ${RecapFormat.hoursMinutes(d.durationMs)}", size = 11.sp, alpha = 0.8f)
}

/** Save to library and Share, then the way into the detail page (board 39, card 11). */
@Composable
private fun CloserButtons(env: CardEnv, a: CloserActions) {
    val g = env.ground
    val type = env.theme.type
    val labelStyle = type.body.copy(fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
    Row(Modifier.fillMaxWidth().padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Box(
            Modifier
                .weight(1f)
                .heightIn(min = 40.dp)
                .clip(RoundedCornerShape(20.dp))
                .background(g.fg)
                .clickable(enabled = !a.saving && !a.saved, role = Role.Button, onClick = a.onSave),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                when {
                    a.saved -> "Saved to your library"
                    a.saving -> "Saving…"
                    else -> "Save to library"
                },
                style = labelStyle,
                color = g.top,
            )
        }
        Box(
            Modifier
                .width(84.dp)
                .heightIn(min = 40.dp)
                .clip(RoundedCornerShape(20.dp))
                .border(1.dp, g.fg, RoundedCornerShape(20.dp))
                .clickable(role = Role.Button, onClick = a.onShare),
            contentAlignment = Alignment.Center,
        ) { Text("Share", style = labelStyle, color = g.fg) }
    }
    Box(
        Modifier.fillMaxWidth().heightIn(min = 40.dp).clickable(role = Role.Button, onClick = a.onDetail),
        contentAlignment = Alignment.Center,
    ) {
        Text(if (type.labelUpper) "SEE YOUR YEAR IN DETAIL" else "See your year in detail", style = type.label, color = g.fg)
    }
}
