package uk.gcartee.encore.app.ui.settings

import android.content.Context
import android.content.Intent
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ToggleButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.graphics.luminance
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.layout.widthIn
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.text
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import java.io.File
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.theme.ACCENTS
import uk.gcartee.encore.app.ui.theme.Appearance
import uk.gcartee.encore.app.ui.theme.AppearanceChoice
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.app.ui.theme.ThemeMode
import uk.gcartee.encore.app.ui.theme.WALLPAPER
import uk.gcartee.encore.app.ui.theme.accentScheme
import uk.gcartee.encore.core.Adventurousness
import uk.gcartee.encore.core.ListeningMode
import uk.gcartee.encore.core.Network
import uk.gcartee.encore.core.StreamQuality
import uk.gcartee.encore.core.SyncState
import uk.gcartee.encore.core.appName
import uk.gcartee.encore.core.coreVersion
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/** Board 17, Layout: separated rows, 24 dp outer and 6 dp inner corners. */
private const val OUTER = 24
private const val INNER = 6
private const val ROW_ONE_LINE_DP = 56
private const val SWATCHES_PER_ROW = 4
private const val SWATCH_DP = 48
private const val LONG_TITLE = 16
private const val LONG_TITLE_SP = 28
private const val ROW_TWO_LINE_DP = 72
private const val ICON_CIRCLE_DP = 40
private const val RIGHT_MAX_DP = 140

/** Settings type: 16 sp row titles and 14 sp values, so every line reads at arm's length. */
@Composable
private fun rowTitle() = LocalEncoreType.current.listRowTitle.copy(fontSize = 16.sp, lineHeight = 22.sp)

@Composable
private fun rowValue() = LocalEncoreType.current.body.copy(fontSize = 14.sp, lineHeight = 20.sp)

/** A paragraph on a settings page: 15 sp, comfortable line height, page margins. */
@Composable
private fun Body(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        style = LocalEncoreType.current.body.copy(fontSize = 15.sp, lineHeight = 22.sp),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier.padding(horizontal = (Grid.base * 3).dp, vertical = Grid.base.dp),
    )
}

/**
 * A one-of choice with room for words: tonal rows with a line each and a radio button,
 * the chosen row filled. Nothing is cut off at large text sizes, unlike a segmented row.
 */
@Composable
fun <T> RadioRows(options: List<Triple<T, String, String?>>, selected: T, onSelect: (T) -> Unit, modifier: Modifier = Modifier) {
    val haptics = rememberHaptics()
    Column(modifier.padding(horizontal = (Grid.base * 2).dp).selectableGroup(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        options.forEachIndexed { i, (value, label, line) ->
            val on = value == selected
            val top = if (i == 0) OUTER else INNER
            val bottom = if (i == options.lastIndex) OUTER else INNER
            Surface(
                selected = on,
                onClick = {
                    if (!on) {
                        haptics.perform(Haptic.Tick)
                        onSelect(value)
                    }
                },
                shape = RoundedCornerShape(top.dp, top.dp, bottom.dp, bottom.dp),
                color = if (on) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surfaceContainer,
                modifier = Modifier.fillMaxWidth().semantics { role = Role.RadioButton },
            ) {
                Row(Modifier.heightIn(min = ROW_ONE_LINE_DP.dp).padding(horizontal = (Grid.base * 2).dp, vertical = (Grid.base * 1.5f).dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(label, style = rowTitle(), color = if (on) MaterialTheme.colorScheme.onSecondaryContainer else MaterialTheme.colorScheme.onSurface)
                        line?.let { Text(it, style = rowValue(), color = if (on) MaterialTheme.colorScheme.onSecondaryContainer else MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 2.dp)) }
                    }
                    Spacer(Modifier.width(Grid.base.dp))
                    androidx.compose.material3.RadioButton(selected = on, onClick = null)
                }
            }
        }
    }
}

/** A settings row; a trailing lambda is its tap action. */
class Row0(val title: String, val value: String? = null, val icon: (@Composable () -> Painter)? = null, val destructive: Boolean = false, val right: String? = null, val dot: Color? = null, val valueColor: Color? = null, val trailing: (@Composable () -> Unit)? = null, val onClick: (() -> Unit)? = null)

fun LazyListScope.group(title: String?, rows: List<Row0>) {
    if (rows.isEmpty()) return
    if (title != null) item { SectionLabel(title) }
    items(rows.size) { i ->
        val r = rows[i]
        val top = if (i == 0) OUTER else INNER
        val bottom = if (i == rows.lastIndex) OUTER else INNER
        val type = LocalEncoreType.current
        val shape = RoundedCornerShape(top.dp, top.dp, bottom.dp, bottom.dp)
        Surface(
            color = MaterialTheme.colorScheme.surfaceContainer,
            shape = shape,
            modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = 1.dp),
        ) {
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = if (r.value != null) ROW_TWO_LINE_DP.dp else ROW_ONE_LINE_DP.dp)
                    .clip(shape)
                    .let { m -> if (r.onClick != null) m.clickable(onClick = r.onClick) else m }
                    .padding(horizontal = (Grid.base * 2).dp, vertical = (Grid.base * 1.5f).dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                r.icon?.let {
                    // The icon in a tonal circle, as the profile sheet draws its rows.
                    Box(
                        Modifier.size(ICON_CIRCLE_DP.dp).background(if (r.destructive) MaterialTheme.colorScheme.errorContainer else MaterialTheme.colorScheme.surfaceContainerHighest, CircleShape),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(it(), contentDescription = null, tint = if (r.destructive) MaterialTheme.colorScheme.onErrorContainer else MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
                    }
                    Spacer(Modifier.width((Grid.base * 2).dp))
                }
                // Board 18, item 2: a colored dot for health, before the name.
                r.dot?.let {
                    Box(Modifier.size(10.dp).background(it, CircleShape))
                    Spacer(Modifier.width((Grid.base * 2).dp))
                }
                Column(Modifier.weight(1f)) {
                    Text(r.title, style = rowTitle(), color = if (r.destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface)
                    r.value?.let { Text(it, style = rowValue(), color = r.valueColor ?: MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 2.dp)) }
                }
                // Board 17: a value on the right, then the chevron for rows that open a page.
                // It keeps to one line and never squeezes the title off the row.
                r.right?.let { Text(it, style = rowValue(), color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis, modifier = Modifier.padding(start = Grid.base.dp, end = Grid.base.dp).widthIn(max = RIGHT_MAX_DP.dp)) }
                when {
                    r.trailing != null -> r.trailing.invoke()
                    r.onClick != null && !r.destructive -> Icon(Icons.chevron(), contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}

/**
 * Boards 17 to 19: back as a tonal rounded square that rounds when pressed, the page's
 * one action as a tonal pill on the right ("Add server"), then the large title.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Page(title: String, onBack: (() -> Unit)?, contentPadding: PaddingValues, action: Pair<String, () -> Unit>? = null, subtitle: String? = null, content: LazyListScope.() -> Unit) {
    val type = LocalEncoreType.current
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 3).dp)) {
        item {
            Column(Modifier.statusBarsPadding().padding(horizontal = (Grid.base * 2).dp)) {
                Row(Modifier.fillMaxWidth().padding(top = (Grid.base * 2).dp, bottom = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
                    if (onBack != null) FilledTonalIconButton(
                        onClick = onBack,
                        shapes = androidx.compose.material3.IconButtonDefaults.shapes(RoundedCornerShape(BACK_RADIUS_DP.dp), RoundedCornerShape(50)),
                        modifier = Modifier.size(48.dp),
                    ) { Icon(Icons.back(), contentDescription = "Back") }
                    Spacer(Modifier.weight(1f))
                    action?.let { (label, run) ->
                        androidx.compose.material3.FilledTonalButton(onClick = run, modifier = Modifier.height(48.dp)) {
                            Text(label, style = type.sectionHeader.copy(fontSize = 16.sp))
                        }
                    }
                }
                // Android: a large top app bar title.
                Text(
                    title,
                    style = if (title.length > LONG_TITLE) type.pageTitle.copy(fontSize = LONG_TITLE_SP.sp, lineHeight = (LONG_TITLE_SP + 6).sp) else type.pageTitle,
                    modifier = Modifier.padding(top = Grid.base.dp, bottom = if (subtitle == null) Grid.base.dp else 0.dp).semantics { heading() },
                )
                subtitle?.let { Text(it, style = rowValue(), color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(bottom = Grid.base.dp)) }
            }
        }
        content()
    }
}

/** Boards 18 and 38: a group's label, small, uppercase and quiet, centered over its rows. */
@Composable
private fun SectionLabel(text: String) {
    Text(
        text,
        style = LocalEncoreType.current.sectionHeader.copy(fontSize = 15.sp, lineHeight = 20.sp),
        color = MaterialTheme.colorScheme.primary,
        // Drawn in capitals, read as written: screen readers get the normal-case words.
        modifier = Modifier.fillMaxWidth().padding(start = (Grid.base * 3).dp, end = (Grid.base * 3).dp, top = (Grid.base * 3).dp, bottom = Grid.base.dp).clearAndSetSemantics {
            heading()
            this.text = androidx.compose.ui.text.AnnotatedString(text)
        },
    )
}

/** Board 19: the Material 3 switch, its thumb carrying a check when on. */
@Composable
fun CheckSwitch(checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Switch(
        checked,
        onCheckedChange = onCheckedChange,
        thumbContent = if (checked) {
            { Icon(Icons.check(), contentDescription = null, modifier = Modifier.size(androidx.compose.material3.SwitchDefaults.IconSize)) }
        } else {
            null
        },
    )
}

private const val BACK_RADIUS_DP = 12

/** The licences page on its own (Welcome's Open-source licenses link). */
@Composable
fun LicencesScreen(onBack: () -> Unit) = LicencesPage(onBack, PaddingValues())

/** Settings and its pages; [page] is the route's page name. */
@Composable
fun SettingsScreen(host: CoreHost, page: String, onBack: () -> Unit, onProfiles: () -> Unit, onAddServer: () -> Unit, contentPadding: PaddingValues) {
    when (page) {
        "playback" -> PlaybackPage(host, onBack, contentPadding)
        "suggestions" -> SuggestionsPage(host, onBack, contentPadding)
        "appearance" -> AppearancePage(host, onBack, contentPadding)
        "servers" -> ServersPage(host, onBack, onAddServer, contentPadding)
        "privacy" -> PrivacyPage(host, onBack, contentPadding)
        "licences" -> LicencesPage(onBack, contentPadding)
        else -> SettingsHome(host, onBack, onProfiles, contentPadding)
    }
}

@Composable
private fun SettingsHome(host: CoreHost, onBack: () -> Unit, onProfiles: () -> Unit, contentPadding: PaddingValues) {
    val actions = LocalAppActions.current
    val profile = remember { runCatching { host.core.profiles().firstOrNull { it.active } }.getOrNull() }
    var query by rememberSaveable { mutableStateOf("") }
    val context = LocalContext.current
    val haptics = rememberHaptics()
    val appearance by Appearance.choice.collectAsState()
    var hapticsOn by remember { mutableStateOf(uk.gcartee.encore.app.ui.components.HapticSetting.enabled) }
    val server = remember { runCatching { host.core.servers().firstOrNull()?.name }.getOrNull() }
    // Board 17's groups, rows without icons, a chevron where a page opens.
    val groups = listOf(
        listOf(
            Row0("Playback and quality") { actions.open(Route.Settings("playback")) },
            Row0("Mixes and suggestions") { actions.open(Route.Settings("suggestions")) },
        ),
        listOf(
            Row0("Servers and sources") { actions.open(Route.Settings("servers")) },
            Row0("Privacy and data") { actions.open(Route.Settings("privacy")) },
        ),
        listOf(
            Row0("Appearance", right = if (appearance.mode == ThemeMode.System) "Auto" else appearance.mode.label) { actions.open(Route.Settings("appearance")) },
            Row0("Haptics", trailing = {
                CheckSwitch(hapticsOn, onCheckedChange = { on ->
                    uk.gcartee.encore.app.ui.components.HapticSetting.set(context, on)
                    hapticsOn = on
                    if (on) haptics.perform(Haptic.ToggleOn)
                })
            }),
            Row0("About and licenses", right = versionName(context)) { actions.open(Route.Settings("licences")) },
        ),
    )
    Page("Settings", onBack, contentPadding) {
        item {
            // Search: filters every setting by name.
            // A filled pill, the same search bar as the Search tab (board 07).
            androidx.compose.material3.TextField(
                query, { query = it },
                placeholder = { Text("Search settings") },
                singleLine = true,
                shape = CircleShape,
                leadingIcon = { Icon(Icons.search(), contentDescription = null) },
                colors = androidx.compose.material3.TextFieldDefaults.colors(
                    focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    focusedIndicatorColor = androidx.compose.ui.graphics.Color.Transparent,
                    unfocusedIndicatorColor = androidx.compose.ui.graphics.Color.Transparent,
                ),
                modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp),
            )
        }
        val q = query.trim()
        if (q.isEmpty()) item {
            // Item 1: the account card, tonal, avatar and server, opening the profiles.
            Surface(
                onClick = onProfiles,
                shape = RoundedCornerShape(OUTER.dp),
                color = MaterialTheme.colorScheme.secondaryContainer,
                modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = (Grid.base * 2).dp),
            ) {
                Row(Modifier.padding((Grid.base * 2).dp), verticalAlignment = Alignment.CenterVertically) {
                    uk.gcartee.encore.app.ui.components.ProfileAvatar(profile?.initials ?: "You", profile?.avatarUrl, 48.dp)
                    Spacer(Modifier.width((Grid.base * 2).dp))
                    Column(Modifier.weight(1f)) {
                        Text(profile?.displayName ?: "You", style = LocalEncoreType.current.sectionHeader)
                        server?.let { Text(it, style = LocalEncoreType.current.body, color = MaterialTheme.colorScheme.onSecondaryContainer) }
                    }
                    Icon(Icons.chevron(), contentDescription = null)
                }
            }
        }
        groups.forEach { rows ->
            val shown = rows.filter { q.isEmpty() || it.title.contains(q, ignoreCase = true) }
            group(null, shown)
            if (shown.isNotEmpty()) item { Spacer(Modifier.height((Grid.base * 2).dp)) }
        }
    }
}

private fun versionName(context: Context): String =
    runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull().orEmpty()

/** A connected group of choices (selected fully round, the others squared toward their neighbours). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun <T> Choices(options: List<Pair<T, String>>, selected: T, onSelect: (T) -> Unit) {
    val haptics = rememberHaptics()
    Row(Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp), horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
        options.forEachIndexed { i, (value, label) ->
            ToggleButton(
                checked = value == selected,
                onCheckedChange = { if (value != selected) { haptics.perform(Haptic.Tick); onSelect(value) } },
                shapes = when (i) {
                    0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                    options.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                    else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                },
                modifier = Modifier.weight(1f).semantics { role = Role.RadioButton },
            ) { Text(label, style = LocalEncoreType.current.buttonLabel, maxLines = 1) }
        }
    }
}

/**
 * How adventurous suggestions are: three steps (board 17, item 4), drawn as board 38's
 * Android page does, a segmented choice whose selected step carries a check, with the
 * line saying what it means. SEGMENT_TICK on each change.
 */
@Composable
fun AdventurousChoice(level: Adventurousness, onChange: (Adventurousness) -> Unit) {
    RadioRows(
        listOf(
            Triple(Adventurousness.FAMILIAR, "Familiar", "1 in 20 picks are new to you"),
            Triple(Adventurousness.BALANCED, "Balanced", "1 in 10 picks are new to you"),
            Triple(Adventurousness.ADVENTUROUS, "Adventurous", "1 in 5 picks are new to you"),
        ),
        level,
        onChange,
    )
}

@Composable
private fun Label(text: String) = SectionLabel(text)

@Composable
private fun PlaybackPage(host: CoreHost, onBack: () -> Unit, contentPadding: PaddingValues) {
    val qualities = listOf(
        Triple(StreamQuality.ORIGINAL, "Original", "The file as it is, gapless included"),
        Triple(StreamQuality.HIGH, "High", "320 kbps"),
        Triple(StreamQuality.NORMAL, "Normal", "160 kbps"),
        Triple(StreamQuality.DATA_SAVER, "Data saver", "96 kbps"),
    )
    var wifi by remember { mutableStateOf(runCatching { host.core.streamQuality(Network.WIFI) }.getOrDefault(StreamQuality.ORIGINAL)) }
    var cell by remember { mutableStateOf(runCatching { host.core.streamQuality(Network.CELLULAR) }.getOrDefault(StreamQuality.HIGH)) }
    Page("Playback and quality", onBack, contentPadding) {
        item { Label("Streaming on Wi-Fi") }
        item { RadioRows(qualities, wifi, { q -> runCatching { host.core.setStreamQuality(Network.WIFI, q) }.onSuccess { wifi = q } }) }
        item { Label("Streaming on cellular") }
        item { RadioRows(qualities, cell, { q -> runCatching { host.core.setStreamQuality(Network.CELLULAR, q) }.onSuccess { cell = q } }) }
        item { Body("A change applies from the next song.") }
    }
}

@Composable
private fun SuggestionsPage(host: CoreHost, onBack: () -> Unit, contentPadding: PaddingValues) {
    val actions = LocalAppActions.current
    val haptics = rememberHaptics()
    var level by remember { mutableStateOf(runCatching { host.core.adventurousness() }.getOrDefault(Adventurousness.BALANCED)) }
    var version by remember { mutableIntStateOf(0) }
    val notInterested = remember(version) { runCatching { host.core.notInterestedList() }.getOrDefault(emptyList()) }
    var confirmReset by remember { mutableStateOf(false) }
    Page("Mixes and suggestions", onBack, contentPadding) {
        item { Label("How adventurous") }
        // The rows bring their own margins, so they line up with the groups below.
        item { AdventurousChoice(level) { l -> runCatching { host.core.setAdventurousness(l) }.onSuccess { level = l } } }
        item { Spacer(Modifier.height(Grid.base.dp)) }
        group("Not interested", notInterested.map { item ->
            Row0(item.label, null, { Icons.block() }, trailing = {
                TextButton(onClick = { runCatching { host.core.removeNotInterested(item) }.onSuccess { version++ } }) { Text("Remove") }
            })
        })
        group(null, listOf(Row0("Reset what the app learned", "Likes, playlists and history stay", { Icons.refresh() }, destructive = true) { confirmReset = true }))
    }
    if (confirmReset) {
        AlertDialog(
            onDismissRequest = { confirmReset = false },
            title = { Text("Reset what the app learned?") },
            text = { Text("Suggestions start over on every device. Your likes, playlists, history and Not interested list stay. This can't be undone.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmReset = false
                    runCatching { host.core.resetSuggestions() }
                        .onSuccess { haptics.perform(Haptic.Confirm); actions.toaster.show("Suggestions start over") }
                        .onFailure { actions.toaster.show(it.userMessage()) }
                }) { Text("Reset", color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { confirmReset = false }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun AppearancePage(host: CoreHost, onBack: () -> Unit, contentPadding: PaddingValues) {
    val context = LocalContext.current
    val choice by Appearance.choice.collectAsState()
    val haptics = rememberHaptics()
    Page("Appearance", onBack, contentPadding) {
        item { Label("Theme") }
        item { uk.gcartee.encore.app.ui.library.ChoiceGroup(ThemeMode.entries.map { it to it.label }, choice.mode, { Appearance.set(context, choice.copy(mode = it)) }, modifier = Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp)) }
        item { Label("Accent color") }
        item {
            // A compact grid, four to a row, in one tonal card: each swatch shows the
            // accent's primary, secondary and tertiary tones; the chosen one turns into a
            // squircle with a check.
            val options = ACCENTS + if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S) listOf(WALLPAPER) else emptyList()
            val dark = MaterialTheme.colorScheme.surface.luminance() < 0.5f
            Surface(shape = RoundedCornerShape(OUTER.dp), color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp)) {
                Column(Modifier.padding(vertical = (Grid.base * 2).dp), verticalArrangement = Arrangement.spacedBy((Grid.base * 2).dp)) {
                    options.chunked(SWATCHES_PER_ROW).forEach { row ->
                        Row(Modifier.fillMaxWidth()) {
                            row.forEach { name ->
                                val scheme = remember(name, dark) { accentScheme(context, name, dark) }
                                val selected = name == choice.accent
                                val corner by androidx.compose.animation.core.animateDpAsState(if (selected) 16.dp else SWATCH_DP.dp / 2, androidx.compose.animation.core.spring(dampingRatio = 0.6f), label = "swatch")
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally,
                                    modifier = Modifier
                                        .weight(1f)
                                        .clip(RoundedCornerShape(16.dp))
                                        .clickable {
                                            haptics.perform(Haptic.Tick)
                                            Appearance.set(context, choice.copy(accent = name))
                                        }
                                        .padding(vertical = 6.dp)
                                        .semantics {
                                            contentDescription = name
                                            stateDescription = if (selected) "Selected" else ""
                                            role = Role.RadioButton
                                        },
                                ) {
                                    Box(Modifier.size(SWATCH_DP.dp).clip(RoundedCornerShape(corner)), contentAlignment = Alignment.Center) {
                                        androidx.compose.foundation.Canvas(Modifier.matchParentSize()) {
                                            drawRect(scheme.primary, size = androidx.compose.ui.geometry.Size(size.width, size.height / 2))
                                            drawRect(scheme.secondaryContainer, topLeft = androidx.compose.ui.geometry.Offset(0f, size.height / 2), size = androidx.compose.ui.geometry.Size(size.width / 2, size.height / 2))
                                            drawRect(scheme.tertiary, topLeft = androidx.compose.ui.geometry.Offset(size.width / 2, size.height / 2), size = androidx.compose.ui.geometry.Size(size.width / 2, size.height / 2))
                                        }
                                        if (selected) Icon(Icons.check(), contentDescription = null, tint = scheme.onPrimary, modifier = Modifier.size(22.dp))
                                    }
                                    Spacer(Modifier.height(6.dp))
                                    Text(name, style = rowValue().copy(fontSize = 13.sp), color = if (selected) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                                }
                            }
                            repeat(SWATCHES_PER_ROW - row.size) { Spacer(Modifier.weight(1f)) }
                        }
                    }
                }
            }
        }
        item { Body("Reduce motion and larger text follow the system settings.") }
    }
}

@Composable
private fun ServersPage(host: CoreHost, onBack: () -> Unit, onAddServer: () -> Unit, contentPadding: PaddingValues) {
    val actions = LocalAppActions.current
    val scope = rememberCoroutineScope()
    var version by remember { mutableIntStateOf(0) }
    val servers = remember(version) { runCatching { host.core.servers() }.getOrDefault(emptyList()) }
    val sync by host.sync.state.collectAsState()
    var status by remember(version) { mutableStateOf(runCatching { host.core.syncStatus() }.getOrNull()) }
    var companion by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf<Pair<String, () -> Unit>?>(null) }
    val fmt = remember { DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT) }
    val error = MaterialTheme.colorScheme.error
    Page("Servers", onBack, contentPadding, action = "Add server" to onAddServer) {
        group("Your sources", servers.map { s ->
            Row0(
                s.name,
                if (s.signedIn) {
                    listOfNotNull(
                        "Jellyfin",
                        "${s.songCount} songs",
                        s.lastSyncUtcMs?.let { "synced " + fmt.format(Date(it)) },
                    ).joinToString(" · ")
                } else {
                    "Session ended. Sign in again"
                },
                dot = if (s.signedIn) HEALTHY else error,
                valueColor = if (s.signedIn) null else error,
            )
        } + listOf(
            Row0("Sync now", if (sync.running) "Syncing…" else null) { host.sync.syncNow() },
        ))
        // Item 5: the companion server.
        val st = status
        if (st == null || st.state == SyncState.NOT_CONNECTED) {
            item { Label("Companion server") }
            item {
                Column {
                    Body("Optional. It syncs history, likes and playlists between your devices and makes handoff possible. Without it everything works on this phone alone.")
                    OutlinedTextField(
                        companion,
                        { companion = it },
                        label = { Text("Companion server address") },
                        placeholder = { Text("encore:8097") },
                        leadingIcon = { Icon(Icons.devices(), contentDescription = null) },
                        shape = RoundedCornerShape(16.dp),
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp),
                    )
                    androidx.compose.material3.FilledTonalButton(modifier = Modifier.align(Alignment.End).padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp), enabled = companion.isNotBlank(), onClick = {
                        scope.launch {
                            runCatching { host.core.connectCompanion(companion.trim()) }
                                .onSuccess { status = it; version++; actions.toaster.show("Connected. Sync runs by itself.") }
                                .onFailure { actions.toaster.show(it.userMessage()) }
                        }
                    }) { Text("Connect") }
                }
            }
        } else {
            group("Companion server", listOf(
                Row0(st.serverUrl ?: "Companion", listOfNotNull(st.state.name.lowercase().replaceFirstChar(Char::uppercase), if (st.pending > 0u) "${st.pending} waiting to sync" else null, st.reason).joinToString(" · "), { Icons.devices() }),
                Row0("Sync now", null, { Icons.sync() }) { scope.launch { runCatching { host.core.syncNow() }.onSuccess { status = it } } },
                Row0("Disconnect", "Everything stays on this phone", { Icons.logout() }, destructive = true) {
                    confirm = "Stop syncing with the companion server? Nothing on this phone is deleted." to {
                        scope.launch { runCatching { host.core.disconnectCompanion() }.onSuccess { version++ } }
                    }
                },
            ))
        }
    }
    confirm?.let { (text, act) ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            text = { Text(text) },
            confirmButton = { TextButton(onClick = { confirm = null; act() }) { Text("Disconnect", color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun PrivacyPage(host: CoreHost, onBack: () -> Unit, contentPadding: PaddingValues) {
    val actions = LocalAppActions.current
    val context = LocalContext.current
    val haptics = rememberHaptics()
    val scope = rememberCoroutineScope()
    val profile = remember { runCatching { host.core.profiles().firstOrNull { it.active } }.getOrNull() }
    val guest = remember { runCatching { host.core.guestSession() }.getOrNull() != null }
    var learning by remember { mutableStateOf(!host.learningOff()) }
    var exporting by remember { mutableStateOf(false) }
    var confirmClear by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf(false) }
    if (guest) {
        Page("Privacy and data", onBack, contentPadding) { item { Text("Nothing is saved in Guest mode", modifier = Modifier.padding((Grid.base * 3).dp)) } }
        return
    }
    val addresses = remember { runCatching { host.core.servers().map { it.baseUrl } + listOfNotNull(host.core.syncStatus().serverUrl) }.getOrDefault(emptyList()) }
    Page("Privacy and data", onBack, contentPadding, subtitle = profile?.displayName?.let { "For $it" }) {
        item {
            // Item 1: the promise, matching what the app does.
            Body("There is no account with the app's makers. This app only talks to the servers you added, and nothing about your listening leaves them.")
        }
        group("Learning", listOf(Row0("Learn from my listening", "Off stops all recommendation learning and hides Mixes", trailing = {
            CheckSwitch(learning, onCheckedChange = { on ->
                haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
                host.setLearningOff(!on)
                learning = on
            })
        })))
        // Item 3: what this app connects to.
        group("What this app connects to", addresses.map { Row0(it, null, { Icons.servers() }) })
        group("Your data", listOf(
            Row0(if (exporting) "Exporting…" else "Export my data", "One file: plays, likes, playlists and settings", { Icons.share() }) {
                if (exporting || profile == null) return@Row0
                exporting = true
                scope.launch {
                    val file = File(File(context.cacheDir, "exports").apply { mkdirs() }, "encore-export.json")
                    runCatching { withContext(Dispatchers.IO) { host.core.exportProfile(profile.profileId, file.absolutePath) } }
                        .onSuccess {
                            val uri = FileProvider.getUriForFile(context, context.packageName + ".exports", file)
                            context.startActivity(Intent.createChooser(Intent(Intent.ACTION_SEND).setType("application/json").putExtra(Intent.EXTRA_STREAM, uri).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION), "Export"))
                        }
                        .onFailure { actions.toaster.show(it.userMessage()) }
                    exporting = false
                }
            },
        ))
        // Item 6: destructive rows apart and red.
        group(null, listOf(
            Row0("Clear listening history", "On this phone, the companion server and every device", { Icons.history() }, destructive = true) { confirmClear = true },
            Row0("Delete this profile everywhere", "Its history, likes, playlists and learned taste", { Icons.delete() }, destructive = true) {
                haptics.perform(Haptic.Reject)
                deleting = true
            },
        ))
    }
    if (confirmClear) {
        AlertDialog(
            onDismissRequest = { confirmClear = false },
            title = { Text("Clear listening history?") },
            text = { Text("Every play so far goes, here, on the companion server and on every device. Likes and playlists stay. There is no undo.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmClear = false
                    runCatching { host.core.clearListeningHistory() }
                        .onSuccess { n -> haptics.perform(Haptic.Confirm); actions.toaster.show("Cleared $n plays") }
                        .onFailure { actions.toaster.show(it.userMessage()) }
                }) { Text("Clear history", color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { confirmClear = false }) { Text("Cancel") } },
        )
    }
    if (deleting && profile != null) {
        var typed by remember { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { deleting = false },
            title = { Text("Delete ${profile.displayName} everywhere?") },
            text = {
                Column {
                    Text("This deletes the profile's history, likes, playlists, recaps, learned taste, tokens and downloads, on this phone, the companion server and every device signed in to it. Your Jellyfin account and music are never touched. Export first if you want a copy.")
                    Spacer(Modifier.height(Grid.base.dp))
                    OutlinedTextField(typed, { typed = it }, placeholder = { Text("Type ${profile.displayName}") }, singleLine = true)
                }
            },
            // GUARD: the destructive button is never under the thumb's resting position.
            dismissButton = {
                TextButton(enabled = typed == profile.displayName, onClick = {
                    scope.launch {
                        runCatching { host.core.deleteProfileEverywhere(profile.profileId, typed) }
                            .onSuccess {
                                profile.credentials.forEach { c -> host.tokens.delete(c.key) }
                                haptics.perform(Haptic.Confirm)
                                deleting = false
                                host.profileClosed()
                            }
                            .onFailure { actions.toaster.show(it.userMessage()) }
                    }
                }) { Text("Delete everywhere", color = MaterialTheme.colorScheme.error) }
            },
            confirmButton = { TextButton(onClick = { deleting = false }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun LicencesPage(onBack: () -> Unit, contentPadding: PaddingValues) {
    val context = LocalContext.current
    val text = remember { runCatching { context.assets.open("licenses/THIRD_PARTY_LICENSES.md").bufferedReader().readText() }.getOrDefault("") }
    val inter = remember { runCatching { context.assets.open("licenses/Inter-OFL.txt").bufferedReader().readText() }.getOrDefault("") }
    Page("About and licenses", onBack, contentPadding) {
        group(null, listOf(
            Row0(appName(), "Version ${versionName(context)} · core ${coreVersion()}", { Icons.info() }),
            Row0("Source code", "github.com/Gr3y50n1213/encore · MPL-2.0", { Icons.share() }),
        ))
        item {
            Text(
                text.ifBlank { "The license list ships with release builds." } + "\n\n" + inter,
                style = LocalEncoreType.current.body.copy(fontSize = 13.sp, lineHeight = 19.sp),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding((Grid.base * 3).dp),
            )
        }
    }
}

/** Board 18: the green health dot. */
private val HEALTHY = Color(0xFF2E7D32)
