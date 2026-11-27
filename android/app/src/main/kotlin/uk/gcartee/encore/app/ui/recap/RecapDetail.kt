package uk.gcartee.encore.app.ui.recap

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.time.ZoneId
import uk.gcartee.encore.app.recap.BeatData
import uk.gcartee.encore.app.recap.MonthTally
import uk.gcartee.encore.app.recap.Recap
import uk.gcartee.encore.app.recap.RecapFormat
import uk.gcartee.encore.app.recap.RecapText
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.SystemBarIconsFor
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/** One ranked line of the detail page. */
private data class DetailRow(val rank: Int, val title: String, val subtitle: String?, val value: String)

/**
 * Board 39, card 12: after the story, a scrolling page with the top 50 songs, artists and
 * albums, the year month by month, and Show the math: every card's counts and the rule
 * behind them. Drawn in the app's own Material look.
 */
@Composable
fun RecapDetailPage(recap: Recap, onClose: () -> Unit) {
    BackHandler(onBack = onClose)
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    var tab by rememberSaveable { mutableIntStateOf(0) }
    val counted = remember(recap) { RecapFormat.countedThrough(recap, ZoneId.systemDefault()) }
    val beats = remember(recap) { recap.story }
    // The full story has the top 50; a short one has only its cards' top 5.
    val songs = remember(recap) {
        (recap.detail?.topSongs ?: beats.firstNotNullOfOrNull { (it.data as? BeatData.TopSong)?.top }.orEmpty())
            .map { DetailRow(it.rank, it.title, it.artist, "${RecapFormat.count(it.plays)} plays") }
    }
    val artists = remember(recap) {
        (recap.detail?.topArtists ?: beats.firstNotNullOfOrNull { (it.data as? BeatData.TopArtist)?.top }.orEmpty())
            .map { DetailRow(it.rank, it.name, "${RecapFormat.hours(it.listenedMs)} hours", "${RecapFormat.count(it.plays)} plays") }
    }
    val albums = remember(recap) {
        (recap.detail?.topAlbums ?: beats.firstNotNullOfOrNull { (it.data as? BeatData.TopAlbum)?.top }.orEmpty())
            .map { DetailRow(it.rank, it.name, it.artist, "${RecapFormat.count(it.plays)} plays") }
    }
    val months: List<MonthTally> = remember(recap) {
        recap.detail?.months ?: beats.firstNotNullOfOrNull { (it.data as? BeatData.Minutes)?.months }.orEmpty()
    }
    val tabs = listOf("Songs", "Artists", "Albums", "Months")
    SystemBarIconsFor(scheme.surface)
    Surface(Modifier.fillMaxSize().blockTouches(), color = scheme.surface) {
        Column(Modifier.fillMaxSize().statusBarsPadding()) {
            Row(Modifier.fillMaxWidth().padding(start = (Grid.base * 2).dp, end = Grid.base.dp, top = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
                Text("Your ${recap.year}", style = type.pageTitle, color = scheme.onSurface, modifier = Modifier.weight(1f).semantics { heading() })
                IconButton(onClick = onClose, modifier = Modifier.size(48.dp)) { Icon(Icons.close(), contentDescription = "Close details") }
            }
            val since = recap.since?.let { " · Since ${RecapFormat.day(it.day)}" }.orEmpty()
            Text(
                (if (recap.soFar) "So far · " else "") + "Counted through $counted$since",
                style = type.caption,
                color = scheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = (Grid.base * 2).dp),
            )
            Spacer(Modifier.height(Grid.base.dp))
            PrimaryTabRow(selectedTabIndex = tab) {
                tabs.forEachIndexed { i, label ->
                    Tab(selected = tab == i, onClick = { tab = i }, text = { Text(label) })
                }
            }
            val bottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
            LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(bottom = bottom + (Grid.base * 2).dp)) {
                when (tab) {
                    0 -> items(songs) { DetailLine(it) }
                    1 -> items(artists) { DetailLine(it) }
                    2 -> items(albums) { DetailLine(it) }
                    else -> {
                        val most = (months.maxOfOrNull { it.minutes } ?: 0L).coerceAtLeast(1L)
                        items(months) { m -> MonthLine(m, most) }
                    }
                }
                item {
                    Text(
                        "Show the math",
                        style = type.sectionHeader,
                        color = scheme.onSurface,
                        modifier = Modifier.padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = (Grid.base * 3).dp, bottom = Grid.base.dp).semantics { heading() },
                    )
                }
                items(beats) { b ->
                    Surface(
                        color = scheme.surfaceContainer,
                        shape = RoundedCornerShape(20.dp),
                        modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = 4.dp),
                    ) {
                        Column(Modifier.padding((Grid.base * 2).dp)) {
                            Text(RecapText.title(b.id), style = type.listRowTitle, color = scheme.onSurface)
                            if (b.text.isNotBlank()) Text(b.text, style = type.body, color = scheme.onSurface, modifier = Modifier.padding(top = 4.dp))
                            b.math.counts.forEach { c ->
                                Row(Modifier.fillMaxWidth().padding(top = 6.dp)) {
                                    Text(c.label, style = type.caption, color = scheme.onSurfaceVariant, modifier = Modifier.weight(1f))
                                    Text(RecapFormat.mathValue(c), style = type.caption, color = scheme.onSurface)
                                }
                            }
                            if (b.math.rule.isNotBlank()) Text(b.math.rule, style = type.caption, color = scheme.onSurfaceVariant, modifier = Modifier.padding(top = 8.dp))
                        }
                    }
                }
                recap.detail?.personality?.takeIf { it.isNotEmpty() }?.let { metrics ->
                    item {
                        Text(
                            "Behind your personality",
                            style = type.sectionHeader,
                            color = scheme.onSurface,
                            modifier = Modifier.padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = (Grid.base * 3).dp, bottom = Grid.base.dp).semantics { heading() },
                        )
                    }
                    items(metrics) { m ->
                        Row(Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(horizontal = (Grid.base * 2).dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(m.label, style = type.body, color = scheme.onSurface, modifier = Modifier.weight(1f))
                            Text(RecapFormat.metricValue(m), style = type.rowValue, color = scheme.onSurfaceVariant)
                        }
                    }
                }
                item {
                    // Board 41, "Guest or Private": never counted, and the page says so.
                    Text(
                        "Private listening and Guest are never counted. Plays under 2 seconds aren't either, and listened time is counted, not song length.",
                        style = type.caption,
                        color = scheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = (Grid.base * 3).dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun DetailLine(row: DetailRow) {
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    Column {
        Row(Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = (Grid.base * 2).dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("${row.rank}", style = type.rowValue, color = scheme.onSurfaceVariant, modifier = Modifier.width(32.dp))
            Column(Modifier.weight(1f)) {
                Text(row.title, style = type.listRowTitle, color = scheme.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
                row.subtitle?.takeIf { it.isNotBlank() }?.let { Text(it, style = type.caption, color = scheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis) }
            }
            Text(row.value, style = type.rowValue, color = scheme.onSurfaceVariant, modifier = Modifier.padding(start = Grid.base.dp))
        }
        HorizontalDivider(Modifier.padding(start = 48.dp), color = scheme.outlineVariant.copy(alpha = 0.5f))
    }
}

@Composable
private fun MonthLine(m: MonthTally, most: Long) {
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    Row(Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = (Grid.base * 2).dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(RecapFormat.monthLong(m.month), style = type.listRowTitle, color = scheme.onSurface, modifier = Modifier.width(112.dp))
        Box(Modifier.weight(1f).height(8.dp).background(scheme.surfaceContainerHighest, RoundedCornerShape(4.dp))) {
            Box(Modifier.fillMaxWidth((m.minutes.toFloat() / most).coerceIn(0f, 1f)).height(8.dp).background(scheme.primary, RoundedCornerShape(4.dp)))
        }
        Text("${RecapFormat.count(m.minutes)} min · ${RecapFormat.count(m.plays)} plays", style = type.caption, color = scheme.onSurfaceVariant, modifier = Modifier.padding(start = Grid.base.dp))
    }
}
