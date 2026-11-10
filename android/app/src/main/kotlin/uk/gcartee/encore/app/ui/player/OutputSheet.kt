package uk.gcartee.encore.app.ui.player

import android.content.Intent
import android.provider.Settings
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.ui.components.GroupRow
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.IconDisc
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LevelBars
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.SHEET_ROW_GAP_DP
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.DeviceState
import uk.gcartee.encore.core.OutputDevice
import uk.gcartee.encore.core.OutputState
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/**
 * Board 12, item 2: Play on, from Output on Now Playing. Drawn like the app's other
 * sheets (ActionSheet, Settings): this device and Android's own output switcher as one
 * rounded tonal group, the device lit while it is the one playing; then the other devices
 * as a second group, or a card saying why there are none. It takes the theme it is given,
 * which over Now Playing is the album's colours (FieldSheetTheme). The owner, 9 November
 * 2026: the plain list it was looked basic beside the rest of the app.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun OutputSheet(host: CoreHost, playingHere: Boolean, onDismiss: () -> Unit) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val actions = LocalAppActions.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val scheme = MaterialTheme.colorScheme
    var version by remember { mutableIntStateOf(0) }
    val devices = remember(version) { runCatching { host.core.outputDevices() }.getOrNull() }
    var moving by remember { mutableStateOf<String?>(null) }
    var rename by remember { mutableStateOf<OutputDevice?>(null) }
    val tablet = LocalConfiguration.current.smallestScreenWidthDp >= TABLET_DP
    val rowTitle = type.listRowTitle.copy(fontSize = 16.sp)
    LaunchedEffect(Unit) {
        host.events.collect { e -> if (e.kind == uk.gcartee.encore.core.EventKind.DEVICES_CHANGED) version++ }
    }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = scheme.surfaceContainerLow,
    ) {
        Column(
            Modifier
                .navigationBarsPadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = (Grid.base * 2).dp)
                .padding(bottom = (Grid.base * 2).dp),
        ) {
            Text("Play on", style = type.sheetTitle, modifier = Modifier.padding(horizontal = Grid.base.dp))
            Spacer(Modifier.height((Grid.base * 2).dp))
            Column(verticalArrangement = Arrangement.spacedBy(SHEET_ROW_GAP_DP.dp)) {
                // Item 3: this device, lit while the music is coming from it.
                GroupRow(
                    first = true,
                    last = false,
                    color = if (playingHere) scheme.secondaryContainer else scheme.surfaceContainerHigh,
                    contentColor = if (playingHere) scheme.onSecondaryContainer else scheme.onSurface,
                ) {
                    IconDisc(
                        Icons.phone(),
                        color = if (playingHere) scheme.primary else scheme.surfaceContainerHighest,
                        tint = if (playingHere) scheme.onPrimary else scheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.width((Grid.base * 2).dp))
                    Text(if (tablet) "This tablet" else "This phone", style = rowTitle, modifier = Modifier.weight(1f))
                    if (playingHere) LevelBars(scheme.onSecondaryContainer)
                }
                // Item 4: Android's own media output switcher (Bluetooth, speakers).
                GroupRow(first = false, last = true, onClick = {
                    val panel = Intent("com.android.settings.panel.action.MEDIA_OUTPUT").putExtra("com.android.settings.panel.extra.PACKAGE_NAME", context.packageName)
                    runCatching { context.startActivity(panel) }.onFailure {
                        runCatching { context.startActivity(Intent(Settings.ACTION_BLUETOOTH_SETTINGS)) }
                    }
                }) {
                    IconDisc(Icons.speaker())
                    Spacer(Modifier.width((Grid.base * 2).dp))
                    Text("Speakers and headphones", style = rowTitle, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                    Icon(Icons.chevron(), contentDescription = null, tint = scheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
                }
            }
            Spacer(Modifier.height((Grid.base * 2.5f).dp))
            val d = devices
            when {
                d == null || d.state == OutputState.NO_COMPANION ->
                    OutputNote(Icons.devices(), "Connect a companion server in Settings to move music between your devices.")
                d.state == OutputState.NOT_AVAILABLE -> OutputNote(Icons.lock(), "Handoff is off in Guest.")
                d.state == OutputState.NO_OTHER_DEVICES -> OutputNote(Icons.devices(), "Sign in on another device to hand off music.")
                d.state == OutputState.UNREACHABLE ->
                    OutputNote(Icons.wifi(), d.reason ?: "The companion server can't be reached.", action = "Retry") { version++ }
                else -> {
                    val others = d.devices.filter { !it.thisDevice }
                    Text(
                        "Your other devices",
                        style = type.groupLabel,
                        color = scheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = Grid.base.dp, bottom = Grid.base.dp),
                    )
                    Column(verticalArrangement = Arrangement.spacedBy(SHEET_ROW_GAP_DP.dp)) {
                        others.forEachIndexed { i, dev ->
                            key(dev.deviceId) {
                                var menu by remember { mutableStateOf(false) }
                                val offline = dev.state == DeviceState.OFFLINE
                                GroupRow(
                                    first = i == 0,
                                    last = i == others.lastIndex,
                                    modifier = Modifier.alpha(if (offline) OFFLINE_ALPHA else 1f),
                                    onClick = {
                                        if (offline) {
                                            haptics.perform(Haptic.Reject)
                                            actions.toaster.show("${dev.name} is offline")
                                        } else {
                                            // Item 5: the snapshot goes there; this device fades out once it starts.
                                            moving = dev.deviceId
                                            scope.launch {
                                                runCatching { host.core.handOffTo(dev.deviceId) }
                                                    .onSuccess { haptics.perform(Haptic.Confirm); onDismiss() }
                                                    .onFailure { haptics.perform(Haptic.Reject); actions.toaster.show(it.userMessage()) }
                                                moving = null
                                            }
                                        }
                                    },
                                    onLongClick = { haptics.perform(Haptic.LongPress); menu = true },
                                ) {
                                    IconDisc(if (dev.platform == "android" || dev.platform == "ios") Icons.phone() else Icons.computer())
                                    Spacer(Modifier.width((Grid.base * 2).dp))
                                    Column(Modifier.weight(1f)) {
                                        Text(dev.name, style = rowTitle, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                        Text(
                                            when {
                                                offline -> "Offline"
                                                dev.privateListening -> "Playing"
                                                dev.song != null -> "${dev.song?.title} · ${dev.song?.artist}"
                                                else -> dev.state.name.lowercase().replaceFirstChar(Char::uppercase)
                                            },
                                            style = type.body,
                                            color = scheme.onSurfaceVariant,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                    }
                                    if (moving == dev.deviceId) {
                                        Spacer(Modifier.width(Grid.base.dp))
                                        LoadingIndicator(Modifier.size(MOVING_DP.dp), color = scheme.primary)
                                    }
                                    DropdownMenu(menu, onDismissRequest = { menu = false }, shape = RoundedCornerShape(16.dp), containerColor = scheme.surfaceContainerHigh) {
                                        DropdownMenuItem(text = { Text("Control only") }, onClick = {
                                            menu = false
                                            runCatching { host.core.controlOnly(dev.deviceId) }.onSuccess { actions.toaster.show("Controlling ${dev.name}") }
                                        })
                                        DropdownMenuItem(text = { Text("Rename") }, onClick = { menu = false; rename = dev })
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    rename?.let { dev ->
        var name by remember { mutableStateOf(dev.name) }
        AlertDialog(
            onDismissRequest = { rename = null },
            title = { Text("Rename device") },
            text = { OutlinedTextField(name, { name = it.take(100) }, singleLine = true) },
            confirmButton = {
                TextButton(enabled = name.isNotBlank(), onClick = {
                    rename = null
                    scope.launch { runCatching { host.core.renameDevice(dev.deviceId, name.trim()) }.onSuccess { version++ } }
                }) { Text("Rename") }
            },
            dismissButton = { TextButton(onClick = { rename = null }) { Text("Cancel") } },
        )
    }
}

/** Why there are no other devices to play on, as a card in the sheet's own look. */
@Composable
private fun OutputNote(icon: Painter, line: String, action: String? = null, onAction: () -> Unit = {}) {
    GroupRow(first = true, last = true) {
        IconDisc(icon)
        Spacer(Modifier.width((Grid.base * 2).dp))
        Text(line, style = LocalEncoreType.current.body, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
        if (action != null) {
            Spacer(Modifier.width(Grid.base.dp))
            FilledTonalButton(onClick = onAction) { Text(action) }
        }
    }
}

/**
 * Board 12, item 1: "Continue from <device>" when another device is playing and this
 * one isn't. Tap pulls the music here; a swipe up dismisses; it fades after 8 s. It never
 * moves the music by itself, and shows once a session per device (the core decides).
 */
@Composable
fun ContinueBanner(host: CoreHost, device: OutputDevice?, onDone: () -> Unit) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val actions = LocalAppActions.current
    val scope = rememberCoroutineScope()
    LaunchedEffect(device?.deviceId) {
        if (device != null) {
            delay(BANNER_MS)
            runCatching { host.core.dismissContinue(device.deviceId) }
            onDone()
        }
    }
    AnimatedVisibility(device != null, enter = slideInVertically { -it }, exit = slideOutVertically { -it } + fadeOut()) {
        val dev = device ?: return@AnimatedVisibility
        Surface(
            color = MaterialTheme.colorScheme.tertiaryContainer,
            shape = RoundedCornerShape(28.dp),
            modifier = Modifier
                .statusBarsPadding()
                .padding((Grid.base * 2).dp)
                .fillMaxWidth()
                .pointerInput(dev.deviceId) {
                    detectVerticalDragGestures { _, amount ->
                        if (amount < -8) {
                            runCatching { host.core.dismissContinue(dev.deviceId) }
                            onDone()
                        }
                    }
                },
        ) {
            Row(Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.devices(), contentDescription = null)
                Spacer(Modifier.width((Grid.base * 1.5f).dp))
                Column(Modifier.weight(1f)) {
                    Text("Continue from ${dev.name}", style = type.songRowTitle)
                    dev.song?.let { Text("${it.title} · ${it.artist}", style = type.body) }
                }
                FilledTonalButton(onClick = {
                    scope.launch {
                        runCatching { host.core.pullFrom(dev.deviceId) }
                            .onSuccess { haptics.perform(Haptic.Confirm) }
                            .onFailure { haptics.perform(Haptic.Reject); actions.toaster.show(it.userMessage()) }
                        onDone()
                    }
                }) { Text("Continue") }
            }
        }
    }
}

private const val BANNER_MS = 8_000L
private const val TABLET_DP = 600
private const val OFFLINE_ALPHA = 0.45f
private const val MOVING_DP = 28
