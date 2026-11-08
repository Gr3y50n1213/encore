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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
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

/** A settings row; a trailing lambda is its tap action. */
class Row0(val title: String, val value: String? = null, val icon: (@Composable () -> Painter)? = null, val destructive: Boolean = false, val right: String? = null, val trailing: (@Composable () -> Unit)? = null, val onClick: (() -> Unit)? = null)

fun LazyListScope.group(title: String?, rows: List<Row0>) {
    if (rows.isEmpty()) return
    if (title != null) item {
        Text(title, style = LocalEncoreType.current.groupLabel, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = (Grid.base * 3).dp, top = (Grid.base * 3).dp, bottom = Grid.base.dp).semantics { heading() })
    }
    items(rows.size) { i ->
        val r = rows[i]
        val top = if (i == 0) OUTER else INNER
        val bottom = if (i == rows.lastIndex) OUTER else INNER
        val type = LocalEncoreType.current
        Surface(
            color = MaterialTheme.colorScheme.surfaceContainer,
            shape = RoundedCornerShape(top.dp, top.dp, bottom.dp, bottom.dp),
            modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = 1.dp),
        ) {
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 56.dp)
                    .let { m -> if (r.onClick != null) m.clickable(onClick = r.onClick) else m }
                    .padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                r.icon?.let {
                    Icon(it(), contentDescription = null, tint = if (r.destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
                    Spacer(Modifier.width((Grid.base * 2).dp))
                }
                Column(Modifier.weight(1f)) {
                    Text(r.title, style = type.listRowTitle, color = if (r.destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface)
                    r.value?.let { Text(it, style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                }
                // Board 17: a value on the right, then the chevron for rows that open a page.
                r.right?.let { Text(it, style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(end = Grid.base.dp)) }
                when {
                    r.trailing != null -> r.trailing.invoke()
                    r.onClick != null && !r.destructive -> Icon(Icons.chevron(), contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}

@Composable
private fun Page(title: String, onBack: (() -> Unit)?, contentPadding: PaddingValues, content: LazyListScope.() -> Unit) {
    val type = LocalEncoreType.current
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 3).dp)) {
        item {
            Column(Modifier.statusBarsPadding().padding(horizontal = (Grid.base * 2).dp)) {
                if (onBack != null) FilledTonalIconButton(onClick = onBack, modifier = Modifier.padding(vertical = Grid.base.dp)) { Icon(Icons.back(), contentDescription = "Back") }
                // Android: a large top app bar title.
                Text(title, style = type.pageTitle, modifier = Modifier.padding(vertical = Grid.base.dp).semantics { heading() })
            }
        }
        content()
    }
}

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
                Switch(hapticsOn, onCheckedChange = { on ->
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
                    Box(Modifier.size(48.dp).background(MaterialTheme.colorScheme.surfaceContainerHighest, CircleShape), contentAlignment = Alignment.Center) {
                        Text(profile?.initials ?: "You", style = LocalEncoreType.current.buttonLabelSelected)
                    }
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
 * Board 17, item 4: how adventurous suggestions are, a 3-step slider with a detent tick
 * (SEGMENT_FREQUENT_TICK) at each step.
 */
@Composable
fun AdventurousSlider(level: Adventurousness, onChange: (Adventurousness) -> Unit) {
    val haptics = rememberHaptics()
    val steps = listOf(Adventurousness.FAMILIAR, Adventurousness.BALANCED, Adventurousness.ADVENTUROUS)
    var position by remember(level) { mutableStateOf(steps.indexOf(level).toFloat()) }
    Column {
        androidx.compose.material3.Slider(
            value = position,
            onValueChange = { v ->
                if (Math.round(v) != Math.round(position)) haptics.perform(Haptic.Tick)
                position = v
            },
            onValueChangeFinished = { onChange(steps[Math.round(position).coerceIn(0, 2)]) },
            valueRange = 0f..2f,
            steps = 1,
            modifier = Modifier.fillMaxWidth().semantics { stateDescription = steps[Math.round(position).coerceIn(0, 2)].name.lowercase() },
        )
        Row(Modifier.fillMaxWidth()) {
            val type = LocalEncoreType.current
            Text("Familiar", style = type.caption, modifier = Modifier.weight(1f))
            Text("Balanced", style = type.caption)
            Text("Adventurous", style = type.caption, modifier = Modifier.weight(1f), textAlign = androidx.compose.ui.text.style.TextAlign.End)
        }
    }
}

@Composable
private fun Label(text: String) {
    Text(text, style = LocalEncoreType.current.groupLabel, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = (Grid.base * 3).dp, top = (Grid.base * 3).dp))
}

@Composable
private fun PlaybackPage(host: CoreHost, onBack: () -> Unit, contentPadding: PaddingValues) {
    val qualities = listOf(StreamQuality.ORIGINAL to "Original", StreamQuality.HIGH to "High", StreamQuality.NORMAL to "Normal", StreamQuality.DATA_SAVER to "Data saver")
    var wifi by remember { mutableStateOf(runCatching { host.core.streamQuality(Network.WIFI) }.getOrDefault(StreamQuality.ORIGINAL)) }
    var cell by remember { mutableStateOf(runCatching { host.core.streamQuality(Network.CELLULAR) }.getOrDefault(StreamQuality.HIGH)) }
    Page("Playback and quality", onBack, contentPadding) {
        item { Label("Streaming on Wi-Fi") }
        item { Choices(qualities, wifi) { q -> runCatching { host.core.setStreamQuality(Network.WIFI, q) }.onSuccess { wifi = q } } }
        item { Label("Streaming on cellular") }
        item { Choices(qualities, cell) { q -> runCatching { host.core.setStreamQuality(Network.CELLULAR, q) }.onSuccess { cell = q } } }
        item {
            Text(
                "Original keeps the file as it is, gapless information included. High is 320 kbps, Normal 160, Data saver 96. It applies from the next song.",
                style = LocalEncoreType.current.body,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = (Grid.base * 3).dp, vertical = Grid.base.dp),
            )
        }
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
        item {
            Box(Modifier.padding(horizontal = (Grid.base * 3).dp)) {
                AdventurousSlider(level) { l -> runCatching { host.core.setAdventurousness(l) }.onSuccess { level = l } }
            }
        }
        item {
            Text(
                when (level) {
                    Adventurousness.FAMILIAR -> "About 1 in 20 picks will be new to you."
                    Adventurousness.BALANCED -> "About 1 in 10 picks will be new to you."
                    Adventurousness.ADVENTUROUS -> "About 1 in 5 picks will be new to you."
                },
                style = LocalEncoreType.current.body,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = (Grid.base * 3).dp),
            )
        }
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
        item { Choices(ThemeMode.entries.map { it to it.label }, choice.mode) { Appearance.set(context, choice.copy(mode = it)) } }
        item { Label("Accent color") }
        item {
            val options = ACCENTS + if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S) listOf(WALLPAPER) else emptyList()
            LazyRow(contentPadding = PaddingValues(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp), horizontalArrangement = Arrangement.spacedBy((Grid.base * 2).dp)) {
                items(options) { name ->
                    val colour = remember(name) { accentScheme(context, name, dark = false).primary }
                    val selected = name == choice.accent
                    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.clickable {
                        haptics.perform(Haptic.Tick)
                        Appearance.set(context, choice.copy(accent = name))
                    }.semantics { stateDescription = if (selected) "Selected" else "" }) {
                        Box(
                            Modifier
                                .size(48.dp)
                                .background(colour, if (selected) RoundedCornerShape(16.dp) else CircleShape)
                                .let { if (selected) it.border(3.dp, MaterialTheme.colorScheme.onSurface, RoundedCornerShape(16.dp)) else it },
                        )
                        Spacer(Modifier.height(4.dp))
                        Text(name, style = LocalEncoreType.current.caption)
                    }
                }
            }
        }
        item {
            Text(
                "Reduce motion and larger text follow the system settings.",
                style = LocalEncoreType.current.body,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = (Grid.base * 3).dp, vertical = Grid.base.dp),
            )
        }
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
    Page("Servers and sources", onBack, contentPadding) {
        group("Media servers", servers.map { s ->
            Row0(
                s.name,
                listOfNotNull(
                    "Jellyfin ${s.version.orEmpty()}".trim(),
                    "${s.songCount} songs",
                    s.lastSyncUtcMs?.let { "synced " + fmt.format(Date(it)) },
                    if (!s.signedIn) "Sign in again" else null,
                ).joinToString(" · "),
                { Icons.servers() },
                trailing = {
                    // Item 2: the health dot.
                    Box(Modifier.size(10.dp).background(if (s.signedIn) Color(0xFF2E7D32) else MaterialTheme.colorScheme.error, CircleShape))
                },
            )
        } + listOf(
            Row0("Sync now", if (sync.running) "Syncing…" else null, { Icons.sync() }) { host.sync.syncNow() },
            Row0("Add a server", null, { Icons.add() }, onClick = onAddServer),
        ))
        // Item 5: the companion server.
        val st = status
        if (st == null || st.state == SyncState.NOT_CONNECTED) {
            item { Label("Companion server") }
            item {
                Column(Modifier.padding(horizontal = (Grid.base * 2).dp)) {
                    Text("Optional. It syncs history, likes and playlists between your devices and makes handoff possible. Without it everything works on this phone alone.", style = LocalEncoreType.current.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Spacer(Modifier.height(Grid.base.dp))
                    OutlinedTextField(companion, { companion = it }, placeholder = { Text("encore:8097") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    TextButton(enabled = companion.isNotBlank(), onClick = {
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
    Page("Privacy and data · ${profile?.displayName.orEmpty()}", onBack, contentPadding) {
        item {
            // Item 1: the promise, matching what the app does.
            Text(
                "There is no account with the app's makers. This app only talks to the servers you added, and nothing about your listening leaves them.",
                style = LocalEncoreType.current.body,
                modifier = Modifier.padding(horizontal = (Grid.base * 3).dp),
            )
        }
        group("Learning", listOf(Row0("Learn from my listening", "Off stops all recommendation learning and hides Mixes", trailing = {
            Switch(learning, onCheckedChange = { on ->
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
            Row0("Source code", "github.com/Gr3y50n1213/encore · MPL-2.0", { Icons.chevron() }),
        ))
        item {
            Text(
                text.ifBlank { "The license list ships with release builds." } + "\n\n" + inter,
                style = LocalEncoreType.current.caption,
                modifier = Modifier.padding((Grid.base * 3).dp),
            )
        }
    }
}
