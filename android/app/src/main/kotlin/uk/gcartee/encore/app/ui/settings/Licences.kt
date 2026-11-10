package uk.gcartee.encore.app.ui.settings

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.home.PolygonShape
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.appName
import uk.gcartee.encore.core.coreVersion
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/** One open-source component the app ships: a library, a crate. */
internal class Component(val name: String, val group: String?, val version: String, val licence: String)

/** What ships, read from the licence lists bundled at build time (LicenceAssets in Gradle). */
internal class Shipped(val android: List<Component>, val core: List<Component>)

/**
 * Reads the two lists' tables: the Android app's "In the release APK" and the core's
 * "Compiled into the apps". The core list's server and build-tool sections are not in this
 * app, so they are left out.
 */
internal fun parseShipped(text: String): Shipped {
    val android = mutableListOf<Component>()
    val core = mutableListOf<Component>()
    var section = ""
    text.lineSequence().forEach { raw ->
        val line = raw.trim()
        if (line.startsWith("#")) {
            section = line.trimStart('#').trim()
            return@forEach
        }
        if (!line.startsWith("|") || line.startsWith("| ---") || line.startsWith("|---")) return@forEach
        val cells = line.trim('|').split("|").map { it.trim() }
        if (cells.size < 3 || cells[0] == "Library" || cells[0] == "Crate") return@forEach
        val (name, version, licence) = cells
        when {
            section.startsWith("In the release APK") ->
                android += Component(name.substringAfter(':'), name.substringBefore(':', missingDelimiterValue = "").ifBlank { null }, version, licence)
            section.startsWith("Compiled into the apps") -> core += Component(name, null, version, licence)
        }
    }
    return Shipped(android, core)
}

/** The SPDX identifiers in a licence expression, notes in brackets and exceptions left out. */
internal fun licenceIds(expression: String): List<String> =
    expression.replace(Regex("\\(.*?\\)"), " ")
        .split(Regex("[\\s/]+"))
        .map { it.trim(',', ';') }
        .filter { it.isNotEmpty() && it !in OPERATORS && !it.endsWith("-exception") && it.matches(Regex("[A-Za-z0-9][A-Za-z0-9.+-]*")) }
        .distinct()

private val OPERATORS = setOf("OR", "AND", "WITH")

/** A licence expression said one way: "MIT OR Apache-2.0" and "Apache-2.0 OR MIT" alike. */
private fun licenceLabel(expression: String): String {
    val plain = expression.replace(Regex("\\(.*?\\)"), "").trim()
    if (plain.isEmpty()) return "Licence in its source files"
    if (plain.contains(" AND ") || plain.contains(" WITH ") || plain.contains('(')) return plain
    return plain.split(Regex("\\s+OR\\s+")).map { it.trim() }.sorted().joinToString(" or ")
}

/** Android libraries by family, so the list reads as a handful of names. */
private fun family(c: Component): String {
    val g = c.group.orEmpty()
    return when {
        g.startsWith("androidx.compose") -> "Jetpack Compose"
        g.startsWith("androidx.media3") -> "Media3"
        g.startsWith("androidx.lifecycle") -> "Jetpack Lifecycle"
        g.startsWith("androidx.") -> "AndroidX"
        g.startsWith("org.jetbrains.kotlinx") -> "Kotlin libraries"
        g.startsWith("org.jetbrains.kotlin") -> "Kotlin"
        g.startsWith("org.jetbrains") || g.startsWith("org.jspecify") -> "Annotations"
        g.startsWith("com.google.guava") -> "Guava"
        g.startsWith("net.java.dev.jna") -> "JNA"
        g.startsWith("org.rustls") -> "rustls"
        else -> g.ifBlank { c.name }
    }
}

private fun open(context: Context, url: String) {
    runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
}

private fun spdx(id: String) = "https://spdx.org/licenses/$id.html"

/**
 * Board 17, About and licenses. The owner (8 November 2026) asked for it to be polished:
 * it printed the raw lists. Now the app and its version head it with the source and its
 * licence a tap away; then what it is built on, counted by licence (the chips filter),
 * searchable, in families that open; and each licence's full text one tap from its name.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun LicencesPage(onBack: () -> Unit, contentPadding: PaddingValues) {
    val context = LocalContext.current
    val shipped = remember {
        parseShipped(runCatching { context.assets.open("licenses/THIRD_PARTY_LICENSES.md").bufferedReader().readText() }.getOrDefault(""))
    }
    val ofl = remember { runCatching { context.assets.open("licenses/Inter-OFL.txt").bufferedReader().readText() }.getOrDefault("") }
    val all = remember(shipped) { shipped.android + shipped.core }
    // The licences by how many components use them, for the chips and the texts.
    val counts = remember(all) {
        (all.flatMap { licenceIds(it.licence) } + listOf("OFL-1.1", "Apache-2.0"))
            .groupingBy { it }.eachCount().entries.sortedByDescending { it.value }.map { it.key to it.value }
    }
    var query by rememberSaveable { mutableStateOf("") }
    var only by rememberSaveable { mutableStateOf<String?>(null) }
    val opened = remember { mutableStateMapOf<String, Boolean>() }
    var fontText by remember { mutableStateOf(false) }
    val filtering = query.isNotBlank() || only != null
    val results = remember(all, query, only) {
        all.filter { c ->
            (only == null || only in licenceIds(c.licence)) &&
                (query.isBlank() || c.name.contains(query.trim(), ignoreCase = true) || c.group.orEmpty().contains(query.trim(), ignoreCase = true) || c.licence.contains(query.trim(), ignoreCase = true))
        }
    }

    Page("About and licenses", onBack, contentPadding) {
        item { AboutCard(context) }
        item {
            SectionLabel("Built on open source")
            Body("Encore stands on ${all.size + 2} open-source projects. Thank you to everyone who makes them.")
        }
        item {
            TextField(
                value = query,
                onValueChange = { query = it },
                placeholder = { Text("Find a library or licence", maxLines = 1, overflow = TextOverflow.Ellipsis) },
                leadingIcon = { Icon(Icons.search(), contentDescription = null) },
                trailingIcon = { if (query.isNotEmpty()) IconButton(onClick = { query = "" }, shapes = androidx.compose.material3.IconButtonDefaults.shapes()) { Icon(Icons.close(), contentDescription = "Clear") } },
                singleLine = true,
                shape = CircleShape,
                colors = TextFieldDefaults.colors(
                    focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent,
                ),
                modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
            )
        }
        item {
            // The licences, most used first; one chosen narrows the list to it.
            FlowRow(
                Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp),
                horizontalArrangement = Arrangement.spacedBy(Grid.base.dp),
            ) {
                counts.take(CHIPS).forEach { (id, n) ->
                    val on = only == id
                    FilterChip(
                        selected = on,
                        onClick = { only = if (on) null else id },
                        label = { Text("$id  $n") },
                        leadingIcon = if (on) {
                            { Icon(Icons.check(), contentDescription = null, modifier = Modifier.size(FilterChipDefaults.IconSize)) }
                        } else {
                            null
                        },
                        shape = RoundedCornerShape(CHIP_RADIUS_DP.dp),
                    )
                }
            }
        }
        if (filtering) {
            item { SectionLabel(if (results.isEmpty()) "Nothing matches" else "${results.size} found") }
            items(results.size) { i ->
                val c = results[i]
                Card(i == 0, i == results.lastIndex) { ComponentLine(c, showLicence = true) }
            }
        } else {
            item { SectionLabel("Fonts and icons") }
            item {
                Card(top = true, bottom = false) {
                    Line("Inter", "Rasmus Andersson · SIL Open Font License 1.1", onClick = { fontText = true })
                }
            }
            item {
                Card(top = false, bottom = true) {
                    Line("Material Symbols", "Google · Apache-2.0", onClick = { open(context, spdx("Apache-2.0")) })
                }
            }
            families("Android libraries", "${shipped.android.size} libraries", shipped.android.groupBy(::family).entries.sortedByDescending { it.value.size }.map { it.key to it.value }, opened)
            families("The Rust core", "${shipped.core.size} crates", shipped.core.groupBy { licenceLabel(it.licence) }.entries.sortedByDescending { it.value.size }.map { it.key to it.value }, opened)
            item {
                SectionLabel("Licence texts")
                Body("Each licence in full, on spdx.org.")
            }
            items(counts.size) { i ->
                val (id, n) = counts[i]
                Card(i == 0, i == counts.lastIndex) {
                    Line(id, if (n == 1) "Used once" else "Used by $n", onClick = { if (id == "OFL-1.1") fontText = true else open(context, spdx(id)) })
                }
            }
        }
    }
    if (fontText) {
        ModalBottomSheet(onDismissRequest = { fontText = false }, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
            Column(Modifier.navigationBarsPadding().padding(horizontal = (Grid.base * 3).dp)) {
                Text("SIL Open Font License 1.1", style = LocalEncoreType.current.sheetTitle)
                Text("For Inter, the app's typeface", style = rowValue(), color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height((Grid.base * 2).dp))
                Text(
                    ofl,
                    style = LocalEncoreType.current.body.copy(fontSize = 13.sp, lineHeight = 19.sp),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.heightIn(max = OFL_MAX_DP.dp).verticalScroll(rememberScrollState()).padding(bottom = (Grid.base * 3).dp),
                )
            }
        }
    }
}

/** The app itself: its mark, name and version, and the source and its licence a tap away. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun AboutCard(context: Context) {
    val type = LocalEncoreType.current
    Surface(
        shape = RoundedCornerShape(HERO_RADIUS_DP.dp),
        color = MaterialTheme.colorScheme.primaryContainer,
        contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
        modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
    ) {
        Column(Modifier.padding((Grid.base * 3).dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(
                    Modifier.size(MARK_DP.dp).clip(PolygonShape(MaterialShapes.Cookie9Sided)).background(MaterialTheme.colorScheme.primary),
                    contentAlignment = Alignment.Center,
                ) { Icon(Icons.song(), contentDescription = null, tint = MaterialTheme.colorScheme.onPrimary, modifier = Modifier.size(32.dp)) }
                Spacer(Modifier.width((Grid.base * 2).dp))
                Column {
                    Text(appName(), style = type.pageTitle.copy(fontSize = 28.sp, lineHeight = 34.sp))
                    Text("Version ${versionName(context)} · core ${coreVersion()}", style = rowValue())
                }
            }
            Spacer(Modifier.height((Grid.base * 2).dp))
            Text("Free and open source, under the Mozilla Public License 2.0.", style = type.body.copy(fontSize = 15.sp, lineHeight = 22.sp))
            Spacer(Modifier.height((Grid.base * 2).dp))
            Row(horizontalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
                Button(onClick = { open(context, SOURCE_URL) }, shapes = ButtonDefaults.shapes()) { Text("Source code") }
                OutlinedButton(onClick = { open(context, spdx("MPL-2.0")) }) { Text("MPL-2.0") }
            }
        }
    }
}

/** A group of families, each a row that opens to its members. */
private fun LazyListScope.families(title: String, count: String, families: List<Pair<String, List<Component>>>, opened: MutableMap<String, Boolean>) {
    if (families.isEmpty()) return
    item {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
            Box(Modifier.weight(1f)) { SectionLabel(title) }
            Text(count, style = rowValue(), color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(end = (Grid.base * 3).dp, bottom = Grid.base.dp))
        }
    }
    items(families.size, key = { "$title-${families[it].first}" }) { i ->
        val (name, members) = families[i]
        val key = "$title-$name"
        val open = opened[key] == true
        val still = reduceMotion()
        val turn by animateFloatAsState(if (open) 180f else 0f, label = "chevron")
        val licences = members.map { licenceLabel(it.licence) }.distinct()
        Card(i == 0, i == families.lastIndex) {
            Column {
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = ROW_DP.dp)
                        .clickable(onClickLabel = if (open) "Close" else "Open") { opened[key] = !open }
                        .semantics {
                            role = Role.Button
                            stateDescription = if (open) "Open" else "Closed"
                        }
                        .padding(horizontal = (Grid.base * 2).dp, vertical = (Grid.base * 1.5f).dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(name, style = rowTitle())
                        Text(
                            "${members.size} · " + if (licences.size == 1) licences[0] else "${licences.size} licences",
                            style = rowValue(),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.padding(top = 2.dp),
                        )
                    }
                    Icon(Icons.expandMore(), contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.graphicsLayer { rotationZ = turn })
                }
                AnimatedVisibility(
                    open,
                    enter = if (still) fadeIn() else expandVertically() + fadeIn(),
                    exit = if (still) fadeOut() else shrinkVertically() + fadeOut(),
                ) {
                    Column(Modifier.padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, bottom = (Grid.base * 1.5f).dp)) {
                        members.sortedBy { it.name.lowercase() }.forEach { c -> ComponentLine(c, showLicence = licences.size > 1, inset = false) }
                    }
                }
            }
        }
    }
}

/** A tonal card in a run of them: 24 dp at the run's ends, 6 dp between (board 17). */
@Composable
private fun Card(top: Boolean, bottom: Boolean, content: @Composable () -> Unit) {
    val t = if (top) OUTER else INNER
    val b = if (bottom) OUTER else INNER
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainer,
        shape = RoundedCornerShape(t.dp, t.dp, b.dp, b.dp),
        modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = 1.dp),
    ) { content() }
}

/** A row in a card: a title, a line under it, and a tap. */
@Composable
private fun Line(title: String, value: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = ROW_DP.dp).clickable(onClick = onClick).padding(horizontal = (Grid.base * 2).dp, vertical = (Grid.base * 1.5f).dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = rowTitle())
            Text(value, style = rowValue(), color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 2.dp))
        }
        Icon(Icons.chevron(), contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** One library or crate: its name, version and, where it differs from its family's, licence. */
@Composable
private fun ComponentLine(c: Component, showLicence: Boolean, inset: Boolean = true) {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = if (inset) (Grid.base * 2).dp else 0.dp, vertical = if (inset) (Grid.base * 1.5f).dp else 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(c.name, style = rowValue().copy(color = MaterialTheme.colorScheme.onSurface), maxLines = 1, overflow = TextOverflow.Ellipsis)
            val under = listOfNotNull(c.group?.takeIf { inset }, if (showLicence) licenceLabel(c.licence) else null).joinToString(" · ")
            if (under.isNotEmpty()) Text(under, style = rowValue().copy(fontSize = 13.sp), color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        Spacer(Modifier.width(Grid.base.dp))
        Text(c.version, style = rowValue().copy(fontSize = 13.sp), color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(start = Grid.base.dp))
    }
}

private const val SOURCE_URL = "https://github.com/Gr3y50n1213/encore"
private const val CHIPS = 6
private const val CHIP_RADIUS_DP = 12
private const val HERO_RADIUS_DP = 28
private const val MARK_DP = 64
private const val ROW_DP = 64
private const val OFL_MAX_DP = 520
