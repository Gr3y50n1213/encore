package uk.gcartee.encore.app.ui.sound

import android.content.Context
import android.os.Build
import android.os.PowerManager
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.text.NumberFormat
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.sound.SoundScheduler
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.settings.CheckSwitch
import uk.gcartee.encore.app.ui.settings.OUTER
import uk.gcartee.encore.app.ui.settings.Page
import uk.gcartee.encore.app.ui.settings.Row0
import uk.gcartee.encore.app.ui.settings.formatBytes
import uk.gcartee.encore.app.ui.settings.group
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.EventKind
import uk.gcartee.encore.core.MoodTile
import uk.gcartee.encore.core.Network
import uk.gcartee.encore.core.SoundSettings
import uk.gcartee.encore.core.SoundStatus
import uk.gcartee.encore.core.SoundStop
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/*
 * Sound analysis as people meet it (Phase 10; mock-up design/wireframes/html/44, approved by
 * the owner on 23 November 2026): Settings > Mixes and suggestions > Sound analysis with every
 * stop in words, Home's card shown once, Browse by mood on Search, the Mixes tab's line, and
 * (in SongActions) Why this song's "Sounds like" songs.
 */

/** The live status: read once, then from every `SoundAnalysisChanged`. Null with no profile. */
@Composable
fun rememberSoundStatus(host: CoreHost): State<SoundStatus?> {
    val first = remember(host) { runCatching { host.core.soundStatus() }.getOrNull() }
    return produceState(first, host) {
        host.events.collect { e ->
            if (e.kind == EventKind.SOUND_ANALYSIS_CHANGED) value = e.sound ?: runCatching { host.core.soundStatus() }.getOrNull()
        }
    }
}

/** Songs that still wait (neither analysed nor unreadable). */
val SoundStatus.waiting: Int get() = (songs.toLong() - analysed.toLong() - unavailable.toLong()).coerceAtLeast(0).toInt()

/** How far along, 0 to 100, counting unreadable songs as done (they are never guessed at). */
val SoundStatus.percent: Int get() = if (songs == 0u) 100 else (((analysed + unavailable).toLong() * 100) / songs.toLong()).toInt().coerceIn(0, 100)

private fun count(n: Number): String = NumberFormat.getIntegerInstance().format(n)

private fun songs(n: Int): String = if (n == 1) "1 song" else "${count(n)} songs"

/** "about 5 hours", "about 40 minutes": the core's estimate, rounded the way people say it. */
private fun duration(minutes: Int): String = when {
    minutes < 2 -> "about a minute"
    minutes < 55 -> "about ${(minutes + 4) / 5 * 5} minutes"
    minutes < 90 -> "about an hour"
    else -> "about ${(minutes + 30) / 60} hours"
}

/** "while charging on Wi-Fi": the person's own rules, for the estimate line and Home's card. */
private fun whileRules(s: SoundSettings): String = when {
    s.wifiOnly && s.chargingOnly -> "while charging on Wi-Fi"
    s.chargingOnly -> "while charging"
    s.wifiOnly -> "on Wi-Fi"
    else -> ""
}

/** One state of the page: a bold line and a sentence that says what to do, or that nothing needs doing. */
data class SoundState(val title: String, val line: String, val resumable: Boolean = false)

/**
 * Board 6 of the mock-up, "Every stop has words": the conditions as they are now come first
 * (Android simply doesn't start the job without Wi-Fi or the charger), then why the last run
 * stopped.
 */
fun soundState(context: Context, host: CoreHost, s: SoundStatus): SoundState {
    val pm = context.getSystemService(PowerManager::class.java)
    val battery = context.getSystemService(android.os.BatteryManager::class.java)
    val saver = pm?.isPowerSaveMode == true
    val hot = Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q &&
        (pm?.currentThermalStatus ?: PowerManager.THERMAL_STATUS_NONE) >= PowerManager.THERMAL_STATUS_MODERATE
    val charging = battery?.isCharging == true
    val online = !host.connectivity.offline.value
    val done = "${count(s.analysed.toInt() + s.unavailable.toInt())} of ${count(s.songs.toInt())} songs are done."
    return when {
        !s.settings.enabled -> SoundState("Off", "Suggestions use tags and your listening. Turn it on to hear the difference.")
        s.running -> SoundState("Analysing now", "Liked and most played songs first, then the rest.")
        s.waiting == 0 -> SoundState(
            "All ${count(s.songs.toInt())} songs analysed",
            (if (s.unavailable > 0u) "${count(s.unavailable.toInt())} couldn't be read. " else "") + "New songs are added as they arrive.",
        )
        !s.modelLoaded && s.lastStop == SoundStop.NO_MODEL -> SoundState("Can't start", "This copy of Encore is missing its sound model. Installing it again fixes that.")
        s.lastStop == SoundStop.SIGN_IN_NEEDED -> SoundState("Sign in to your server again", "Analysis needs to read your songs.")
        !online -> SoundState("Waiting for a connection", "It carries on when this phone is back online.")
        s.settings.wifiOnly && host.network() != Network.WIFI -> SoundState("Waiting for Wi-Fi", "Analysis only runs on Wi-Fi. It carries on when you're back on it.")
        s.settings.chargingOnly && !charging -> SoundState("Waiting for the charger", "It runs while charging. $done")
        saver -> SoundState("Paused: battery saver is on", "Turn it off, or charge the phone, and it carries on.")
        hot -> SoundState("Paused: the phone is warm", "It waits until the phone cools down.")
        s.lastStop == SoundStop.SERVER_BUSY -> SoundState("Your server is busy", "It stopped for now, so it doesn't slow your server. It tries again tonight.")
        s.lastStop == SoundStop.STOPPED && !SoundScheduler.isScheduled(context) -> SoundState("Paused", "It carries on later by itself, or now with Resume. $done", resumable = true)
        // The run job ended with songs left that another device has claimed.
        s.lastStop == SoundStop.DONE -> SoundState("Another device is on it", "Some songs are being analysed by another of your devices and shared through your server.")
        else -> SoundState("Ready to carry on", "It carries on in a moment, when Android gives it the time. $done")
    }
}

/** The tonal box every state is drawn in (mock-up: `.state`). */
@Composable
fun StateBox(state: SoundState, modifier: Modifier = Modifier) {
    val type = LocalEncoreType.current
    Surface(color = MaterialTheme.colorScheme.surfaceContainerHighest, shape = RoundedCornerShape(18.dp), modifier = modifier.fillMaxWidth()) {
        Column(Modifier.padding(horizontal = (Grid.base * 1.75f).dp, vertical = (Grid.base * 1.5f).dp)) {
            Text(state.title, style = type.body.copy(fontSize = 15.sp, lineHeight = 21.sp, fontWeight = FontWeight.SemiBold), color = MaterialTheme.colorScheme.onSurface)
            Text(state.line, style = type.body.copy(fontSize = 14.sp, lineHeight = 20.sp), color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

private fun changeSettings(context: Context, host: CoreHost, s: SoundSettings): Boolean {
    val ok = runCatching { host.core.setSoundSettings(s) }.isSuccess
    if (!s.enabled) host.core.stopSoundAnalysis()
    // The run job's conditions are the settings: schedule it again with the new ones.
    if (ok) SoundScheduler.resume(context, replace = true)
    return ok
}

/** Board 2 of the mock-up: Settings > Mixes and suggestions > Sound analysis. */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SoundAnalysisPage(host: CoreHost, onBack: () -> Unit, contentPadding: PaddingValues) {
    val context = LocalContext.current
    val actions = LocalAppActions.current
    val haptics = rememberHaptics()
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    val status by rememberSoundStatus(host)
    var confirmDelete by remember { mutableStateOf(false) }
    val s = status
    if (s == null) {
        Page("Sound analysis", onBack, contentPadding) {
            item { uk.gcartee.encore.app.ui.settings.Body("Open a profile to use sound analysis.") }
        }
        return
    }
    val state = soundState(context, host, s)
    Page("Sound analysis", onBack, contentPadding) {
        item {
            Surface(color = scheme.surfaceContainer, shape = RoundedCornerShape(OUTER.dp), modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp)) {
                Column(Modifier.padding((Grid.base * 2).dp)) {
                    if (s.settings.enabled && s.songs > 0u) {
                        Row(verticalAlignment = Alignment.Bottom) {
                            Text(count(s.analysed.toInt()), style = type.pageTitle.copy(fontSize = 34.sp, lineHeight = 38.sp), color = scheme.onSurface)
                            Spacer(Modifier.width(Grid.base.dp))
                            Text("of ${songs(s.songs.toInt())}", style = type.sectionHeader.copy(fontSize = 17.sp), color = scheme.onSurfaceVariant, modifier = Modifier.padding(bottom = 4.dp))
                        }
                        Spacer(Modifier.height((Grid.base * 1.5f).dp))
                        val progress = s.percent / 100f
                        if (s.running) {
                            androidx.compose.material3.LinearWavyProgressIndicator(progress = { progress }, modifier = Modifier.fillMaxWidth())
                        } else {
                            androidx.compose.material3.LinearProgressIndicator(progress = { progress }, modifier = Modifier.fillMaxWidth())
                        }
                        Spacer(Modifier.height(Grid.base.dp))
                        val left = if (s.waiting > 0) {
                            listOf(duration(s.minutesEstimate.toInt()), formatBytes(s.bytesEstimate.toLong())).joinToString(" and ") + " left" +
                                whileRules(s.settings).let { if (it.isEmpty()) "" else ", $it" }
                        } else {
                            null
                        }
                        Text(listOfNotNull("${s.percent}% done", left).joinToString(" · "), style = type.body.copy(fontSize = 14.sp, lineHeight = 20.sp), color = scheme.onSurfaceVariant)
                        Spacer(Modifier.height((Grid.base * 1.5f).dp))
                    }
                    StateBox(state)
                    // Pause stops after the song it is on and keeps everything done.
                    if (s.settings.enabled && s.running) {
                        Spacer(Modifier.height((Grid.base * 1.5f).dp))
                        FilledTonalButton(onClick = {
                            haptics.perform(Haptic.Tick)
                            host.core.stopSoundAnalysis()
                        }, modifier = Modifier.height(48.dp)) { Text("Pause") }
                    } else if (s.settings.enabled && s.waiting > 0 && state.resumable) {
                        Spacer(Modifier.height((Grid.base * 1.5f).dp))
                        FilledTonalButton(onClick = {
                            haptics.perform(Haptic.Tick)
                            SoundScheduler.resume(context, replace = true)
                        }, modifier = Modifier.height(48.dp)) { Text("Resume") }
                    }
                }
            }
        }
        group("When it runs", listOf(
            Row0("Analyse my library", if (s.settings.enabled) "About 30 seconds of each song, once" else "Off until you turn it on", trailing = {
                CheckSwitch(s.settings.enabled, onCheckedChange = { on ->
                    haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
                    if (!changeSettings(context, host, s.settings.copy(enabled = on))) actions.toaster.show("Couldn't change that. Try again.")
                })
            }),
            Row0("Only on Wi-Fi", "About 1 MB per song", trailing = {
                CheckSwitch(s.settings.wifiOnly, onCheckedChange = { on ->
                    haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
                    changeSettings(context, host, s.settings.copy(wifiOnly = on))
                })
            }),
            Row0("Only while charging", "It also waits for battery saver and heat", trailing = {
                CheckSwitch(s.settings.chargingOnly, onCheckedChange = { on ->
                    haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
                    changeSettings(context, host, s.settings.copy(chargingOnly = on))
                })
            }),
        ))
        val songRows = buildList {
            if (s.unavailable > 0u) add(
                Row0(
                    if (s.unavailable == 1u) "1 song couldn't be read" else "${count(s.unavailable.toInt())} songs couldn't be read",
                    "Opus files and damaged files. Never guessed at.",
                    { Icons.info() },
                    trailing = {
                        TextButton(onClick = {
                            runCatching { host.core.retryUnreadableSounds() }
                                .onSuccess { n ->
                                    SoundScheduler.resume(context, replace = true)
                                    actions.toaster.show(if (n == 1u) "1 song will be tried again" else "${count(n.toInt())} songs will be tried again")
                                }
                                .onFailure { actions.toaster.show(it.userMessage()) }
                        }) { Text("Try again") }
                    },
                ),
            )
            if (s.analysed > 0u || s.unavailable > 0u) add(
                Row0("Delete sound analysis", "Suggestions fall back to tags and your listening", { Icons.delete() }, destructive = true) {
                    haptics.perform(Haptic.Reject)
                    confirmDelete = true
                },
            )
        }
        group("Songs", songRows)
        item {
            uk.gcartee.encore.app.ui.settings.Body("Only about 130 numbers per song are kept. No audio leaves your devices or your server.")
        }
    }
    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text("Delete sound analysis?") },
            text = { Text("Encore forgets what every song sounds like and turns sound analysis off. Suggestions fall back to tags and your listening. Turning it on again starts over.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmDelete = false
                    changeSettings(context, host, s.settings.copy(enabled = false))
                    runCatching { host.core.deleteSoundAnalysis() }
                        .onSuccess { haptics.perform(Haptic.Confirm); actions.toaster.show("Sound analysis deleted") }
                        .onFailure { actions.toaster.show(it.userMessage()) }
                }) { Text("Delete", color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text("Cancel") } },
        )
    }
}

/** The value beside the row on Mixes and suggestions: Off, 42% or On. */
fun soundSummary(s: SoundStatus?): String? = when {
    s == null -> null
    !s.settings.enabled -> "Off"
    s.waiting > 0 -> "${s.percent}%"
    else -> "On"
}

private const val CARD_PREFS = "sound-card"

private fun cardKey(host: CoreHost) = "seen-${host.core.currentProfile().orEmpty()}"

/**
 * Board 1 of the mock-up: Home's card, shown once per profile. It says what it does, what it
 * costs and what stays private before anything runs. Not now hides it for good; Turn on and
 * Settings > Sound analysis remain the only ways to change it.
 */
@Composable
fun SoundHomeCard(host: CoreHost, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val actions = LocalAppActions.current
    val haptics = rememberHaptics()
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    val prefs = remember { context.getSharedPreferences(CARD_PREFS, Context.MODE_PRIVATE) }
    var seen by remember { mutableStateOf(prefs.getBoolean(cardKey(host), false)) }
    val status by rememberSoundStatus(host)
    val s = status ?: return
    val guest = remember { runCatching { host.core.guestSession() }.getOrNull() != null }
    if (seen || guest || s.settings.enabled || s.songs == 0u || s.analysed > 0u) return
    fun hide() {
        prefs.edit().putBoolean(cardKey(host), true).apply()
        seen = true
    }
    Surface(color = scheme.surfaceContainer, shape = RoundedCornerShape(OUTER.dp), modifier = modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp)) {
        Column(Modifier.padding((Grid.base * 2).dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(48.dp).background(scheme.secondaryContainer, RoundedCornerShape(16.dp)), contentAlignment = Alignment.Center) {
                    Icon(Icons.spark(), contentDescription = null, tint = scheme.onSecondaryContainer)
                }
                Spacer(Modifier.width((Grid.base * 1.5f).dp))
                Text("Let Encore learn what your songs sound like", style = type.sectionHeader.copy(fontSize = 17.sp, lineHeight = 22.sp), modifier = Modifier.weight(1f).semantics { heading() })
            }
            Spacer(Modifier.height((Grid.base * 1.5f).dp))
            Text(
                "Suggestions get better when Encore knows a song is slow and warm, or fast and bright, whatever its tags say. It listens to about 30 seconds of each song, once.",
                style = type.body.copy(fontSize = 14.sp, lineHeight = 20.sp),
                color = scheme.onSurfaceVariant,
            )
            Spacer(Modifier.height((Grid.base * 1.5f).dp))
            // "8,106 songs: about 9.7 GB over Wi-Fi and about 5 hours of work, while charging."
            val over = if (s.settings.wifiOnly) " over Wi-Fi" else ""
            val power = if (s.settings.chargingOnly) ", while charging" else ""
            val stops = listOfNotNull(
                "when you unplug".takeIf { s.settings.chargingOnly },
                "when Wi-Fi drops".takeIf { s.settings.wifiOnly },
                "when your server is busy",
            )
            val stopsWords = if (stops.size == 1) stops[0] else stops.dropLast(1).joinToString(", ") + " or " + stops.last()
            StateBox(
                SoundState(
                    "What it costs",
                    "${songs(s.waiting)}: about ${formatBytes(s.bytesEstimate.toLong())}$over and ${duration(s.minutesEstimate.toInt())} of work$power. It stops $stopsWords.",
                ),
            )
            Spacer(Modifier.height((Grid.base * 1.5f).dp))
            Text("Only about 130 numbers per song are kept. No audio leaves your devices or your server.", style = type.body.copy(fontSize = 14.sp, lineHeight = 20.sp), color = scheme.onSurfaceVariant)
            Spacer(Modifier.height((Grid.base * 2).dp))
            Row(horizontalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
                Button(onClick = {
                    haptics.perform(Haptic.ToggleOn)
                    if (changeSettings(context, host, s.settings.copy(enabled = true))) {
                        hide()
                        actions.toaster.show("Sound analysis is on. Settings shows how far along it is.")
                    } else {
                        actions.toaster.show("Couldn't turn it on. Try again from Settings.")
                    }
                }, modifier = Modifier.height(48.dp)) { Text("Turn on") }
                TextButton(onClick = { hide() }, modifier = Modifier.height(48.dp)) { Text("Not now") }
            }
        }
    }
}

/**
 * Board 5 of the mock-up: one honest line on the Mixes tab while analysis runs. It disappears
 * once every song is done.
 */
@Composable
fun SoundMixesLine(host: CoreHost, modifier: Modifier = Modifier) {
    val status by rememberSoundStatus(host)
    val s = status ?: return
    if (!s.settings.enabled || s.waiting == 0) return
    StateBox(
        SoundState(
            "Getting better as your library is analysed · ${s.percent}%",
            "These mixes use what songs sound like where they know it, and tags and your listening for the rest.",
        ),
        modifier,
    )
}

/**
 * Board 3 of the mock-up: Browse by mood on Search, plain-colour leaf tiles two to a row, a tap
 * opens that mood's mix. Moods appear as the library is analysed; with analysis off, one line
 * says where to turn it on.
 */
@Composable
fun MoodTiles(host: CoreHost, onOpen: (Route) -> Unit) {
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    val haptics = rememberHaptics()
    val status by rememberSoundStatus(host)
    val s = status ?: return
    var moods by remember { mutableStateOf<List<MoodTile>>(emptyList()) }
    // A new list as songs are analysed: every 50 songs is enough to notice a new mood.
    val step = s.analysed.toInt() / MOOD_REFRESH_SONGS
    LaunchedEffect(step, s.settings.enabled) {
        moods = withContext(Dispatchers.IO) { runCatching { host.core.moods() }.getOrDefault(emptyList()) }
    }
    if (moods.isEmpty() && !s.settings.enabled && s.analysed == 0u) {
        uk.gcartee.encore.app.ui.library.SectionTitle("Browse by mood")
        Surface(
            onClick = { onOpen(Route.Settings("sound")) },
            color = scheme.surfaceContainerHighest,
            shape = RoundedCornerShape(18.dp),
            modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp),
        ) {
            Row(Modifier.padding(horizontal = (Grid.base * 1.75f).dp, vertical = (Grid.base * 1.5f).dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text("Moods need sound analysis", style = type.body.copy(fontSize = 15.sp, lineHeight = 21.sp, fontWeight = FontWeight.SemiBold))
                    Text("Turn it on in Settings, and moods made from how your songs sound appear here.", style = type.body.copy(fontSize = 14.sp, lineHeight = 20.sp), color = scheme.onSurfaceVariant)
                }
                Icon(Icons.chevron(), contentDescription = null, tint = scheme.onSurfaceVariant)
            }
        }
        return
    }
    if (moods.isEmpty() && !s.settings.enabled) return
    uk.gcartee.encore.app.ui.library.SectionTitle("Browse by mood")
    Text(
        "Made from how your songs sound, not from their tags.",
        style = type.body.copy(fontSize = 14.sp, lineHeight = 20.sp),
        color = scheme.onSurfaceVariant,
        modifier = Modifier.padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, bottom = Grid.base.dp),
    )
    val colours: List<Pair<Color, Color>> = listOf(
        scheme.primaryContainer to scheme.onPrimaryContainer,
        scheme.secondaryContainer to scheme.onSecondaryContainer,
        scheme.tertiaryContainer to scheme.onTertiaryContainer,
        scheme.surfaceContainerHighest to scheme.onSurface,
    )
    Column(Modifier.padding(horizontal = (Grid.base * 2).dp), verticalArrangement = Arrangement.spacedBy(Grid.base.dp * 1.5f)) {
        moods.chunked(2).forEachIndexed { row, pair ->
            Row(horizontalArrangement = Arrangement.spacedBy(Grid.base.dp * 1.5f)) {
                pair.forEachIndexed { col, m ->
                    val big = LEAF_BIG_DP.dp
                    val small = LEAF_SMALL_DP.dp
                    // Corners alternate like a checkerboard, as Browse's tiles above.
                    val leaf = if ((row + col) % 2 == 0) RoundedCornerShape(big, small, big, small) else RoundedCornerShape(small, big, small, big)
                    val (bg, fg) = colours[(row * 2 + col) % colours.size]
                    Surface(
                        onClick = { haptics.perform(Haptic.Soft); onOpen(Route.Mix(m.mixId)) },
                        shape = leaf,
                        color = bg,
                        contentColor = fg,
                        modifier = Modifier.weight(1f).height(MOOD_TILE_DP.dp),
                    ) {
                        Column(Modifier.fillMaxSize().padding((Grid.base * 1.75f).dp), verticalArrangement = Arrangement.Bottom) {
                            Text(m.title, style = type.sectionHeader)
                            Text(songs(m.songCount.toInt()), style = type.caption, color = fg.copy(alpha = 0.8f))
                        }
                    }
                }
                if (pair.size == 1) Spacer(Modifier.weight(1f))
            }
        }
        if (s.settings.enabled && s.waiting > 0) {
            StateBox(SoundState("Still learning your library", "${s.percent}% analysed. More moods appear as it goes. A mood needs at least 25 songs."))
        }
    }
}

private const val LEAF_BIG_DP = 30
private const val LEAF_SMALL_DP = 8
private const val MOOD_TILE_DP = 92
private const val MOOD_REFRESH_SONGS = 50
