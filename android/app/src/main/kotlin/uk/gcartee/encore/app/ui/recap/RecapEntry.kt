package uk.gcartee.encore.app.ui.recap

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.time.LocalDate
import java.time.ZoneId
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.recap.PackColours
import uk.gcartee.encore.app.recap.PackId
import uk.gcartee.encore.app.recap.Recap
import uk.gcartee.encore.app.recap.RecapColour
import uk.gcartee.encore.app.recap.RecapFormat
import uk.gcartee.encore.app.recap.RecapKind
import uk.gcartee.encore.app.recap.RecapProgress
import uk.gcartee.encore.app.recap.RecapRules
import uk.gcartee.encore.app.recap.RecapStore
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/** The card's colours: the year's lead colour, deep enough for white words. */
private fun entryColours(recap: Recap): Pair<Color, Color> {
    val c = PackColours(PackId.EDITORIAL, recap.palette)
    val lead = c.year.firstOrNull()?.let { RecapColour.darkForWhite(it) } ?: RecapColour.darkForWhite(c.spot) ?: c.ink
    val second = c.year.getOrNull(2)?.let { RecapColour.darkForWhite(it) } ?: RecapColour.withLightness(lead, RecapColour.lightness(lead) * 0.6f)
    return Color(lead) to Color(second)
}

/**
 * Board 41, "When it is ready": on Home from 1 December ("so far") to 8 January (the final
 * run), the year's card, which opens the story. A "new" dot until the story is finished;
 * in the final run, how many plays arrived since it was last opened. Nothing for Guest, for
 * Recap off or with too few plays (kind none).
 */
@Composable
fun RecapHomeCard(host: CoreHost, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val open = LocalOpenRecap.current
    val today = remember { LocalDate.now(ZoneId.systemDefault()) }
    val year = remember(today) { RecapRules.seasonYear(today) } ?: return
    var recap by remember { mutableStateOf<Recap?>(null) }
    LaunchedEffect(year) {
        if (RecapStore.isGuest(host)) return@LaunchedEffect
        recap = RecapStore.load(host, year).getOrNull()
    }
    val r = recap ?: return
    if (r.kind == RecapKind.NONE || r.story.isEmpty()) return
    val profile = remember { runCatching { host.core.currentProfile() }.getOrNull().orEmpty() }
    val progress = remember(profile) { RecapProgress(context, profile) }
    val fresh = !progress.finished(r.year)
    val seen = progress.seenPlays(r.year)
    val more = if (RecapRules.inFinalRun(today) && seen >= 0 && r.totals.plays > seen) r.totals.plays - seen else 0L
    val type = LocalEncoreType.current
    val (lead, second) = remember(r) { entryColours(r) }
    val counted = remember(r) { RecapFormat.countedThrough(r, ZoneId.systemDefault()) }
    val line = when {
        more > 0 -> "$more more ${if (more == 1L) "play" else "plays"} since you last looked"
        r.soFar -> "So far · counted through $counted"
        else -> "Your year in music, ready to play"
    }
    Surface(
        shape = RoundedCornerShape(28.dp),
        color = Color.Transparent,
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = (Grid.base * 2).dp)
            .clip(RoundedCornerShape(28.dp))
            .clickable(role = Role.Button, onClickLabel = "Open your ${r.year}") { open(r.year) }
            .semantics { contentDescription = "Your ${r.year} in music. $line." + if (fresh) " New." else "" },
    ) {
        Box(Modifier.background(Brush.linearGradient(listOf(lead, second))).padding((Grid.base * 2.5f).dp)) {
            Column(Modifier.fillMaxWidth()) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        if (r.soFar) "Your ${r.year} so far" else "Your ${r.year}",
                        style = type.sectionHeader.copy(fontSize = 13.sp),
                        color = Color.White.copy(alpha = 0.9f),
                        modifier = Modifier.weight(1f),
                    )
                    if (fresh) {
                        Box(Modifier.size(10.dp).background(Color.White, CircleShape))
                        Spacer(Modifier.width(6.dp))
                        Text("New", style = type.caption, color = Color.White)
                    }
                }
                Spacer(Modifier.height(Grid.base.dp))
                Text(
                    r.yearLine?.takeIf { it.isNotBlank() } ?: "${RecapFormat.count(r.totals.minutes)} minutes with your music",
                    style = type.collectionTitle,
                    color = Color.White,
                    maxLines = 3,
                    modifier = Modifier.semantics { heading() },
                )
                Spacer(Modifier.height(Grid.base.dp))
                Text(line, style = type.body.copy(fontSize = 14.sp), color = Color.White.copy(alpha = 0.9f))
                Spacer(Modifier.height((Grid.base * 2).dp))
                Row(
                    Modifier.clip(RoundedCornerShape(24.dp)).background(Color.White).heightIn(min = 48.dp).padding(horizontal = 20.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(Icons.play(), contentDescription = null, tint = lead, modifier = Modifier.size(20.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(if (fresh) "Play your year" else "Play it again", style = type.buttonLabel, color = lead)
                }
            }
        }
    }
}

/**
 * Board 23: below the mixes, "Your recap" with every year this device keeps (locked past
 * years) and, in the season, this year's. Each opens its story.
 */
@Composable
fun RecapYearsSection(host: CoreHost, modifier: Modifier = Modifier) {
    val open = LocalOpenRecap.current
    val today = remember { LocalDate.now(ZoneId.systemDefault()) }
    var years by remember { mutableStateOf<List<Int>>(emptyList()) }
    LaunchedEffect(Unit) {
        if (RecapStore.isGuest(host)) return@LaunchedEffect
        val kept = RecapStore.pastYears(host)
        val season = RecapRules.seasonYear(today)?.takeIf { y ->
            RecapStore.load(host, y).getOrNull()?.let { it.kind != RecapKind.NONE && it.story.isNotEmpty() } == true
        }
        years = (listOfNotNull(season) + kept).distinct().sortedDescending()
    }
    if (years.isEmpty()) return
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    Column(modifier.fillMaxWidth()) {
        Text("Your recap", style = type.sectionHeader, color = scheme.onSurface, modifier = Modifier.padding(vertical = Grid.base.dp).semantics { heading() })
        years.forEach { y ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 64.dp)
                    .clip(RoundedCornerShape(20.dp))
                    .clickable(role = Role.Button, onClickLabel = "Open your $y") { open(y) }
                    .padding(vertical = Grid.base.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(Modifier.size(48.dp).background(scheme.primaryContainer, RoundedCornerShape(14.dp)), contentAlignment = Alignment.Center) {
                    Text("${y % 100}".padStart(2, '0'), style = type.sectionHeader, color = scheme.onPrimaryContainer)
                }
                Spacer(Modifier.width((Grid.base * 2).dp))
                Column(Modifier.weight(1f)) {
                    Text("Your $y", style = type.listRowTitle, color = scheme.onSurface)
                    Text(if (y == RecapRules.seasonYear(today) && today.year == y) "So far" else "Your year in music", style = type.caption, color = scheme.onSurfaceVariant)
                }
                Icon(Icons.chevron(), contentDescription = null, tint = scheme.onSurfaceVariant)
            }
        }
    }
}
