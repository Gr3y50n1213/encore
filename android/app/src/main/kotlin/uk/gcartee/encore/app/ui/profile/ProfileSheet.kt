package uk.gcartee.encore.app.ui.profile

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.height
import androidx.compose.ui.unit.sp
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.graphics.Color
import androidx.compose.material3.Surface
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.theme.Appearance
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.ProfileSummary
import uk.gcartee.encore.core.SourceToken
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/** Board 14. [onSwitched] resets the app to Home; [onAddAccount] opens the sign-in flow. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun ProfileSheet(host: CoreHost, playing: Boolean, onDismiss: () -> Unit, onSwitched: () -> Unit, onAddAccount: () -> Unit) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val actions = LocalAppActions.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var version by remember { mutableStateOf(0) }
    val profiles = remember(version) { runCatching { host.core.profiles() }.getOrDefault(emptyList()) }
    val guest = remember(version) { runCatching { host.core.guestSession() }.getOrNull() }
    var choosing by remember { mutableStateOf<ProfileSummary?>(null) }
    var pinFor by remember { mutableStateOf<ProfileSummary?>(null) }
    var confirm by remember { mutableStateOf<Pair<String, () -> Unit>?>(null) }

    fun tokensOf(p: ProfileSummary) = p.credentials.mapNotNull { c -> host.tokens.read(c.key)?.let { SourceToken(c.sourceId, it) } }

    fun switchTo(p: ProfileSummary, keepPlaying: Boolean) {
        scope.launch {
            runCatching { host.core.switchProfile(p.profileId, tokensOf(p), keepPlaying) }
                .onSuccess { q ->
                    host.profileSwitched(q)
                    Appearance.load(context, p.profileId)
                    if (keepPlaying) PlaybackHub.playCurrent(context) else PlaybackHub.withPlayback(context) { it.pause() }
                    haptics.perform(Haptic.Confirm)
                    onSwitched()
                    onDismiss()
                }
                .onFailure {
                    haptics.perform(Haptic.Reject)
                    actions.toaster.show(it.userMessage())
                }
        }
    }

    fun pick(p: ProfileSummary) {
        if (p.active) return
        when {
            p.locked -> pinFor = p
            playing -> choosing = p
            else -> switchTo(p, keepPlaying = false)
        }
    }

    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(Modifier.navigationBarsPadding().padding(bottom = (Grid.base * 2).dp)) {
            Text("Who's listening?", style = type.sheetTitle, modifier = Modifier.padding(horizontal = (Grid.base * 3).dp, vertical = Grid.base.dp))
            if (guest != null) {
                Row(Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 3).dp, vertical = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Guest, nothing is saved", style = type.listRowTitle, modifier = Modifier.weight(1f))
                    FilledTonalButton(onClick = {
                        runCatching { host.core.endGuest() }.onSuccess { q ->
                            host.profileSwitched(q)
                            version++
                        }
                    }) { Text("End Guest") }
                }
            }
            profiles.forEach { p ->
                var menu by remember { mutableStateOf(false) }
                Box {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 64.dp)
                            // Board 14: the active profile sits in a tonal highlight.
                            .padding(horizontal = Grid.base.dp)
                            .clip(RoundedCornerShape(ROW_RADIUS_DP.dp))
                            .background(if (p.active) MaterialTheme.colorScheme.secondaryContainer else androidx.compose.ui.graphics.Color.Transparent)
                            .combinedClickable(onClick = { pick(p) }, onLongClick = { haptics.perform(Haptic.LongPress); menu = true })
                            .padding(horizontal = (Grid.base * 2).dp)
                            .semantics { stateDescription = if (p.active) "Current profile" else if (p.needsSignIn) "Sign in again" else "" },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Avatar(if (p.active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceContainerHighest) {
                            Text(p.initials, style = type.buttonLabelSelected, color = if (p.active) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        Spacer(Modifier.width((Grid.base * 2).dp))
                        Column(Modifier.weight(1f)) {
                            Text(p.displayName, style = type.listRowTitle)
                            if (p.needsSignIn) Text("Session ended. Sign in again", style = type.caption, color = MaterialTheme.colorScheme.error)
                            else if (p.active) Text("Active", style = type.caption, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        // Board 14: a locked profile wears a small PIN pill.
                        if (p.locked) Surface(shape = CircleShape, color = Color.Transparent, border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outline), modifier = Modifier.semantics { contentDescription = "Locked with a PIN" }) {
                            Text("PIN", style = type.caption, modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp))
                        }
                        if (p.active) Icon(Icons.check(), contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                    }
                    // Item 3: row actions. Removing is not deleting (that lives in Privacy and data).
                    DropdownMenu(menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(text = { Text("Remove from this device", color = MaterialTheme.colorScheme.error) }, onClick = {
                            menu = false
                            confirm = "Remove ${p.displayName} from this phone? Their history stays on the companion server." to {
                                runCatching { host.core.removeProfileFromDevice(p.profileId) }
                                    .onSuccess {
                                        p.credentials.forEach { c -> host.tokens.delete(c.key) }
                                        version++
                                        if (p.active) host.profileClosed()
                                    }
                                    .onFailure { actions.toaster.show(it.userMessage()) }
                            }
                        })
                    }
                }
            }
            // Item 4: Add account, drawn like a profile row with a + in its circle.
            Row(
                Modifier.fillMaxWidth().heightIn(min = 64.dp).padding(horizontal = Grid.base.dp).clip(RoundedCornerShape(ROW_RADIUS_DP.dp)).combinedClickable(onClick = { onDismiss(); onAddAccount() }).padding(horizontal = (Grid.base * 2).dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Avatar(MaterialTheme.colorScheme.surfaceContainerHighest) { Icon(Icons.add(), contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant) }
                Spacer(Modifier.width((Grid.base * 2).dp))
                Text("Add account", style = type.listRowTitle)
            }
            if (guest == null) {
                // Item 5: Guest records nothing and ends after 4 hours idle.
                Row(
                    Modifier.fillMaxWidth().heightIn(min = 64.dp).padding(horizontal = Grid.base.dp).clip(RoundedCornerShape(ROW_RADIUS_DP.dp)).combinedClickable(onClick = {
                        runCatching { host.core.startGuest() }
                            .onSuccess { q ->
                                host.profileSwitched(q)
                                haptics.perform(Haptic.Confirm)
                                actions.toaster.show("Guest, nothing is saved")
                                onSwitched()
                                onDismiss()
                            }
                            .onFailure { actions.toaster.show(it.userMessage()) }
                    }).padding(horizontal = (Grid.base * 2).dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Avatar(MaterialTheme.colorScheme.surfaceContainerHighest) { Text("G", style = type.buttonLabelSelected, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                    Spacer(Modifier.width((Grid.base * 2).dp))
                    Column {
                        Text("Guest", style = type.listRowTitle)
                        Text("Nothing is saved", style = type.caption, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
            // Where board 14 has "Manage profiles": a tonal pill to Settings (board 17), the
            // phone's way in to every setting and the profile page.
            FilledTonalButton(
                onClick = { onDismiss(); actions.open(uk.gcartee.encore.app.ui.nav.Route.Settings()) },
                modifier = Modifier.padding(start = (Grid.base * 3).dp, top = (Grid.base * 2).dp).height(48.dp),
            ) {
                Icon(Icons.settings(), contentDescription = null, modifier = Modifier.size(20.dp))
                Spacer(Modifier.width(Grid.base.dp))
                Text("Settings", style = type.sectionHeader.copy(fontSize = 16.sp))
            }
        }
    }

    choosing?.let { p ->
        // Item 2: music is playing, so ask first.
        AlertDialog(
            onDismissRequest = { choosing = null },
            title = { Text("Switch to ${p.displayName}?") },
            text = { Text("Keep the music going for ${p.displayName}, or stop and open their own queue.") },
            confirmButton = { TextButton(onClick = { choosing = null; switchTo(p, keepPlaying = true) }) { Text("Keep playing") } },
            dismissButton = { TextButton(onClick = { choosing = null; switchTo(p, keepPlaying = false) }) { Text("Stop and switch") } },
        )
    }
    pinFor?.let { p ->
        var pin by remember { mutableStateOf("") }
        var error by remember { mutableStateOf<String?>(null) }
        AlertDialog(
            onDismissRequest = { pinFor = null },
            title = { Text("Enter ${p.displayName}'s PIN") },
            text = {
                Column {
                    OutlinedTextField(
                        pin,
                        { if (it.length <= 6 && it.all(Char::isDigit)) pin = it },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword),
                    )
                    error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = type.caption) }
                }
            },
            confirmButton = {
                TextButton(enabled = pin.length >= 4, onClick = {
                    runCatching { host.core.unlockProfile(p.profileId, pin) }
                        .onSuccess {
                            pinFor = null
                            if (playing) choosing = p else switchTo(p, keepPlaying = false)
                        }
                        .onFailure {
                            haptics.perform(Haptic.Reject)
                            error = it.userMessage()
                            pin = ""
                        }
                }) { Text("Unlock") }
            },
            dismissButton = { TextButton(onClick = { pinFor = null }) { Text("Cancel") } },
        )
    }
    confirm?.let { (text, act) ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            text = { Text(text) },
            confirmButton = { TextButton(onClick = { confirm = null; haptics.perform(Haptic.Confirm); act() }) { Text("Remove", color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text("Cancel") } },
        )
    }
}


/** Board 14's avatar circle, 48 dp, for profiles, Add account and Guest alike. */
@Composable
private fun Avatar(color: Color, content: @Composable () -> Unit) {
    Box(Modifier.size(AVATAR_DP.dp).background(color, CircleShape), contentAlignment = Alignment.Center) { content() }
}

private const val AVATAR_DP = 48
private const val ROW_RADIUS_DP = 20