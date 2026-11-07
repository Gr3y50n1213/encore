package uk.gcartee.encore.app.ui.player

import android.content.Intent
import android.provider.Settings
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LevelBars
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.DeviceState
import uk.gcartee.encore.core.OutputDevice
import uk.gcartee.encore.core.OutputState
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/** Board 12, item 2: from Output on Now Playing, the mini player, or the banner. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun OutputSheet(host: CoreHost, playingHere: Boolean, onDismiss: () -> Unit) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val actions = LocalAppActions.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var version by remember { mutableIntStateOf(0) }
    val devices = remember(version) { runCatching { host.core.outputDevices() }.getOrNull() }
    var moving by remember { mutableStateOf<String?>(null) }
    var rename by remember { mutableStateOf<OutputDevice?>(null) }
    LaunchedEffect(Unit) {
        host.events.collect { e -> if (e.kind == uk.gcartee.encore.core.EventKind.DEVICES_CHANGED) version++ }
    }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(Modifier.navigationBarsPadding().padding(bottom = (Grid.base * 2).dp)) {
            Text("Play on", style = type.sheetTitle, modifier = Modifier.padding(horizontal = (Grid.base * 3).dp, vertical = Grid.base.dp))
            // Item 3: this device.
            Row(Modifier.fillMaxWidth().heightIn(min = 64.dp).padding(horizontal = (Grid.base * 3).dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.phone(), contentDescription = null)
                Spacer(Modifier.width((Grid.base * 2).dp))
                Text("This phone", style = type.listRowTitle, modifier = Modifier.weight(1f))
                if (playingHere) LevelBars(MaterialTheme.colorScheme.primary)
            }
            // Item 4: Android's own media output switcher (Bluetooth, speakers).
            TextButton(onClick = {
                val panel = Intent("com.android.settings.panel.action.MEDIA_OUTPUT").putExtra("com.android.settings.panel.extra.PACKAGE_NAME", context.packageName)
                runCatching { context.startActivity(panel) }.onFailure {
                    runCatching { context.startActivity(Intent(Settings.ACTION_BLUETOOTH_SETTINGS)) }
                }
            }, modifier = Modifier.padding(horizontal = (Grid.base * 2).dp)) { Text("Speakers and headphones") }
            HorizontalDivider(Modifier.padding(vertical = Grid.base.dp))
            val d = devices
            when {
                d == null || d.state == OutputState.NO_COMPANION -> Text(
                    "Connect a companion server in Settings to move music between your devices.",
                    style = type.body,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = (Grid.base * 3).dp),
                )
                d.state == OutputState.NOT_AVAILABLE -> Text("Handoff is off in Guest.", style = type.body, modifier = Modifier.padding(horizontal = (Grid.base * 3).dp))
                d.state == OutputState.NO_OTHER_DEVICES -> Text("Sign in on another device to hand off music.", style = type.body, modifier = Modifier.padding(horizontal = (Grid.base * 3).dp))
                d.state == OutputState.UNREACHABLE -> Row(Modifier.padding(horizontal = (Grid.base * 3).dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(d.reason ?: "The companion server can't be reached.", style = type.body, modifier = Modifier.weight(1f))
                    TextButton(onClick = { version++ }) { Text("Retry") }
                }
                else -> d.devices.filter { !it.thisDevice }.forEach { dev ->
                    var menu by remember { mutableStateOf(false) }
                    val offline = dev.state == DeviceState.OFFLINE
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 64.dp)
                            .alpha(if (offline) 0.45f else 1f)
                            .combinedClickable(
                                onClick = {
                                    if (offline) {
                                        haptics.perform(Haptic.Reject)
                                        actions.toaster.show("${dev.name} is offline")
                                        return@combinedClickable
                                    }
                                    // Item 5: the snapshot goes there; this device fades out once it starts.
                                    moving = dev.deviceId
                                    scope.launch {
                                        runCatching { host.core.handOffTo(dev.deviceId) }
                                            .onSuccess { haptics.perform(Haptic.Confirm); onDismiss() }
                                            .onFailure { haptics.perform(Haptic.Reject); actions.toaster.show(it.userMessage()) }
                                        moving = null
                                    }
                                },
                                onLongClick = { haptics.perform(Haptic.LongPress); menu = true },
                            )
                            .padding(horizontal = (Grid.base * 3).dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(if (dev.platform == "android" || dev.platform == "ios") Icons.phone() else Icons.computer(), contentDescription = null)
                        Spacer(Modifier.width((Grid.base * 2).dp))
                        Column(Modifier.weight(1f)) {
                            Text(dev.name, style = type.listRowTitle)
                            Text(
                                when {
                                    offline -> "Offline"
                                    dev.privateListening -> "Playing"
                                    dev.song != null -> "${dev.song?.title} · ${dev.song?.artist}"
                                    else -> dev.state.name.lowercase().replaceFirstChar(Char::uppercase)
                                },
                                style = type.body,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        if (moving == dev.deviceId) CircularProgressIndicator(Modifier.width(24.dp))
                        DropdownMenu(menu, onDismissRequest = { menu = false }) {
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
