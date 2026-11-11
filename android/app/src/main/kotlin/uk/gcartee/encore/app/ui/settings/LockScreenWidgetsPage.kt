package uk.gcartee.encore.app.ui.settings

import android.app.StatusBarManager
import android.content.Context
import android.graphics.drawable.Icon as AndroidIcon
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import uk.gcartee.encore.app.R
import uk.gcartee.encore.app.notifications.LiveUpdates
import uk.gcartee.encore.app.tile.StartMixTileService
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.app.widgets.WidgetConfig
import uk.gcartee.encore.app.widgets.WidgetProfileChoice
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/**
 * Phase 9 (WP8), board 31's Android Settings page "Lock screen and widgets": the Live Update
 * cards (sleep timer, downloads), the history rule for lock-screen surfaces, the Profile new
 * widgets start with, and the Quick Settings tile. A toggle here only stops the Live Update
 * promotion: a timer or download keeps going and keeps its normal notification.
 */
@Composable
internal fun LockScreenWidgetsPage(onBack: () -> Unit, contentPadding: PaddingValues) {
    val context = LocalContext.current
    val actions = LocalAppActions.current
    val haptics = rememberHaptics()

    var sleepOn by remember { mutableStateOf(LiveUpdates.isOn(context, LiveUpdates.Card.SLEEP_TIMER)) }
    var downloadsOn by remember { mutableStateOf(LiveUpdates.isOn(context, LiveUpdates.Card.DOWNLOADS)) }
    var hideHistory by remember { mutableStateOf(LiveUpdates.hideHistoryWhileLocked(context)) }
    var choice by remember { mutableStateOf(WidgetConfig.defaultChoice(context)) }
    var profileSheet by remember { mutableStateOf(false) }
    val profiles = remember { WidgetConfig.profileState(context).profiles }

    // The person may switch Live Updates on in Android's settings and come back.
    var canPromote by remember { mutableStateOf(LiveUpdates.canPromote(context)) }
    DisposableEffect(context) {
        val owner = context as? LifecycleOwner
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) canPromote = LiveUpdates.canPromote(context)
        }
        owner?.lifecycle?.addObserver(observer)
        onDispose { owner?.lifecycle?.removeObserver(observer) }
    }

    fun toggle(on: Boolean, set: (Boolean) -> Unit) {
        haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
        set(on)
    }

    Page("Lock screen and widgets", onBack, contentPadding) {
        if (LiveUpdates.apiAllows()) {
            if (!canPromote) item { LiveUpdatesOffBanner { LiveUpdates.openSettings(context) } }
        } else {
            item { Body("Live Updates need Android 16 or later. Timers and downloads use a normal notification.") }
        }
        group("Live Updates", listOf(
            Row0("Sleep timer Live Update", "Time left and Add 15 min", { Icons.sleepTimer() }, trailing = {
                CheckSwitch(sleepOn, onCheckedChange = { on ->
                    toggle(on) { LiveUpdates.setOn(context, LiveUpdates.Card.SLEEP_TIMER, it); sleepOn = it }
                })
            }),
            Row0("Download progress", "Only downloads you start", { Icons.downloading() }, trailing = {
                CheckSwitch(downloadsOn, onCheckedChange = { on ->
                    toggle(on) { LiveUpdates.setOn(context, LiveUpdates.Card.DOWNLOADS, it); downloadsOn = it }
                })
            }),
        ))
        item { Body("Turning one off only stops the pinned card. The timer or download keeps going with a normal notification.") }
        group("Privacy", listOf(
            Row0("Hide listening history while locked", "Widgets and notifications don't show what you played until you unlock", { Icons.visibilityOff() }, trailing = {
                CheckSwitch(hideHistory, onCheckedChange = { on ->
                    toggle(on) { LiveUpdates.setHideHistoryWhileLocked(context, it); hideHistory = it }
                })
            }),
        ))
        group("Widgets", listOf(
            Row0("Widget profile", "The profile a new widget starts with", { Icons.devices() }, right = profileName(choice, profiles)) { profileSheet = true },
        ))
        group("Quick Settings", listOf(
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                Row0("Quick Settings tile", "Start a mix", { Icons.mixes() }, trailing = {
                    FilledTonalButton(onClick = { addTile(context) { actions.toaster.show(it) } }) { Text("Add") }
                })
            } else {
                Row0("Quick Settings tile", "Add it from the Quick Settings edit screen", { Icons.mixes() })
            },
        ))
    }

    if (profileSheet) {
        WidgetProfileSheet(profiles, choice, onDismiss = { profileSheet = false }) { picked ->
            haptics.perform(Haptic.Tick)
            WidgetConfig.setDefaultChoice(context, picked)
            choice = picked
            profileSheet = false
        }
    }
}

/** Shown only where Live Updates exist and the person has switched them off for the app. */
@Composable
private fun LiveUpdatesOffBanner(onOpenSettings: () -> Unit) {
    Surface(
        color = MaterialTheme.colorScheme.secondaryContainer,
        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
        shape = RoundedCornerShape(OUTER.dp),
        modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
    ) {
        Column(Modifier.padding((Grid.base * 2).dp)) {
            Row(verticalAlignment = Alignment.Top) {
                Icon(Icons.info(), contentDescription = null)
                Spacer(Modifier.width((Grid.base * 1.5f).dp))
                Column(Modifier.weight(1f)) {
                    Text("Live updates are off for Encore", style = rowTitle())
                    Text(
                        "Timers and downloads use a normal notification instead.",
                        style = rowValue(),
                        modifier = Modifier.padding(top = 2.dp),
                    )
                }
            }
            Row(Modifier.fillMaxWidth().padding(top = Grid.base.dp), horizontalArrangement = androidx.compose.foundation.layout.Arrangement.End) {
                FilledTonalButton(onClick = onOpenSettings) { Text("Open settings", style = LocalEncoreType.current.buttonLabel) }
            }
        }
    }
}

/** The row's right-hand value: Active profile, or the chosen profile's name. */
private fun profileName(choice: WidgetProfileChoice, profiles: List<uk.gcartee.encore.app.widgets.KnownProfile>): String = when (choice) {
    WidgetProfileChoice.Active -> "Active profile"
    is WidgetProfileChoice.Fixed -> profiles.firstOrNull { it.id == choice.profileId }?.name ?: "A removed profile"
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun WidgetProfileSheet(
    profiles: List<uk.gcartee.encore.app.widgets.KnownProfile>,
    choice: WidgetProfileChoice,
    onDismiss: () -> Unit,
    onPick: (WidgetProfileChoice) -> Unit,
) {
    // "" is Active profile; a fixed profile that is gone stays chosen until another is picked.
    val selected = (choice as? WidgetProfileChoice.Fixed)?.profileId.orEmpty()
    val options = buildList {
        add(Triple("", "Active profile", "Follows whichever profile is open"))
        profiles.forEach { add(Triple(it.id, it.name, if (it.locked) "Has a PIN" else null)) }
    }
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.padding(bottom = (Grid.base * 3).dp)) {
            Text("Widget profile", style = LocalEncoreType.current.sectionHeader, modifier = Modifier.padding(horizontal = (Grid.base * 3).dp))
            Body("New widgets start with this. A widget set to one profile keeps showing it after you switch.")
            RadioRows(options, selected, { id -> onPick(if (id.isEmpty()) WidgetProfileChoice.Active else WidgetProfileChoice.Fixed(id)) })
            Spacer(Modifier.height(Grid.base.dp))
        }
    }
}

/** Android 13 and later: ask the system to add the tile; it shows its own confirmation. */
private fun addTile(context: Context, say: (String) -> Unit) {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
    requestAdd(context, say)
}

@RequiresApi(Build.VERSION_CODES.TIRAMISU)
private fun requestAdd(context: Context, say: (String) -> Unit) {
    val manager = context.getSystemService(StatusBarManager::class.java)
    if (manager == null) {
        say("Add it from the Quick Settings edit screen")
        return
    }
    manager.requestAddTileService(
        StartMixTileService.component(context),
        context.getString(R.string.tile_start_mix_label),
        AndroidIcon.createWithResource(context, R.drawable.sym_graphic_eq),
        context.mainExecutor,
    ) { result ->
        say(
            when (result) {
                StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_ADDED -> "Tile added"
                StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_ALREADY_ADDED -> "The tile is already added"
                StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_NOT_ADDED -> "Tile not added"
                else -> "Add it from the Quick Settings edit screen"
            },
        )
    }
}
