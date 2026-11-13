package uk.gcartee.encore.app.ui.player

import android.text.format.DateFormat
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.util.Date
import java.util.Locale
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.EventKind
import uk.gcartee.encore.core.SleepTimer
import uk.gcartee.encore.core.SleepTimerMode
import uk.gcartee.encore.core.SleepTimerSpec
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Motion.Timing

/**
 * Board 31's sleep timer sheet, in two states drawn in one place. Not running: the presets
 * (5, 10, 15, 30, 45 and 60 minutes, End of this song, End of this album), when the music
 * would pause, and Start. Running: the time left, when the music fades out, a bar from when
 * it was set to when it ends, Add 15 min, Cancel timer, and Change time. The core owns the
 * timer (CORE-API, "Sleep timer"); this only sets, extends and cancels it, and follows
 * `SleepTimerChanged`. A tap on the scrim or a swipe down closes the sheet and leaves a
 * running timer alone. It draws in the theme it is given, so over Now Playing it takes the
 * album's colours (FieldSheetTheme).
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SleepTimerSheet(host: CoreHost, onDismiss: () -> Unit) {
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    val context = LocalContext.current
    val haptics = rememberHaptics()
    val actions = LocalAppActions.current
    val toaster = actions.toaster
    val scope = rememberCoroutineScope()
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val timerState = rememberSleepTimer(host)
    val timer = timerState.value
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) {
        while (true) {
            now = System.currentTimeMillis()
            delay(TICK_MS)
        }
    }
    var pick by rememberSaveable { mutableStateOf(Pick.THIRTY) }
    // The app's toast is drawn in the window under this sheet and could not be seen (Phase 9
    // follow-up, fixed 12 November 2026). A success shows in the sheet itself: it turns to its
    // running state, "Music fades out at 11:30 PM". A failure is written under the title.
    var notice by remember { mutableStateOf<String?>(null) }

    /** Starts [chosen] (from Start or from Change time); the sheet then shows when the music ends. */
    fun start(chosen: Pick, firstStart: Boolean) {
        runCatching { host.core.setSleepTimer(SleepTimerSpec(chosen.mode, chosen.minutes.toUInt())) }
            .onSuccess { t ->
                timerState.value = t
                notice = null
                // Start is a light confirm; picking another time while running is a tick.
                haptics.perform(if (firstStart) Haptic.Confirm else Haptic.Tick)
                if (firstStart) actions.askForNotificationsOnce(forSleepTimer = true)
            }
            .onFailure {
                haptics.perform(Haptic.Reject)
                notice = it.userMessage()
            }
    }

    fun addFifteen() {
        runCatching { host.core.extendSleepTimer(ADD_MINUTES.toUInt()) }
            .onSuccess { t ->
                timerState.value = t
                notice = null
                haptics.perform(Haptic.Tick)
            }
            .onFailure {
                haptics.perform(Haptic.Reject)
                notice = it.userMessage()
            }
    }

    /** No confirmation: the sheet closes, then "Timer cancelled" with Undo for 5 s. */
    fun cancel() {
        runCatching { host.core.cancelSleepTimer() }
            .onSuccess { cancelled ->
                scope.launch { sheet.hide() }.invokeOnCompletion {
                    onDismiss()
                    toaster.show(
                        "Timer cancelled",
                        undo = cancelled?.let { c ->
                            {
                                runCatching { host.core.restoreSleepTimer(c) }
                                    .onSuccess { toaster.show("Sleep timer is back") }
                                    .onFailure { e -> toaster.show(e.userMessage()) }
                            }
                        },
                    )
                }
            }
            .onFailure { notice = it.userMessage() }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheet,
        containerColor = scheme.surfaceContainerLow,
    ) {
        Column(
            Modifier
                .navigationBarsPadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = (Grid.base * 3).dp)
                .padding(bottom = (Grid.base * 2).dp),
        ) {
            Text("Sleep timer", style = type.sheetTitle, modifier = Modifier.semantics { heading() })
            notice?.let {
                Text(
                    it,
                    style = type.caption,
                    color = scheme.error,
                    modifier = Modifier.padding(top = 2.dp).semantics { liveRegion = LiveRegionMode.Polite },
                )
            }
            if (timer == null) {
                // Not running: the presets, then Start.
                Text(
                    if (pick.minutes > 0) "Music fades out over the last ${Timing.sleepTimerFadeOutMs / MS_PER_SECOND} seconds." else "Music stops after ${pick.thing}.",
                    style = type.caption,
                    color = scheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 2.dp),
                )
                Spacer(Modifier.height((Grid.base * 2).dp))
                Column(verticalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
                    Pick.MINUTE_PRESETS.chunked(PRESETS_PER_ROW).forEach { row ->
                        Row(horizontalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
                            row.forEach { p ->
                                ChoiceCell(p.label, pick == p, solid = true, onClick = { haptics.perform(Haptic.Tick); pick = p }, modifier = Modifier.weight(1f))
                            }
                        }
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
                        listOf(Pick.SONG, Pick.ALBUM).forEach { p ->
                            ChoiceCell(p.label, pick == p, solid = true, onClick = { haptics.perform(Haptic.Tick); pick = p }, modifier = Modifier.weight(1f))
                        }
                    }
                }
                Spacer(Modifier.height((Grid.base * 2).dp))
                Button(
                    onClick = { start(pick, firstStart = true) },
                    shapes = ButtonDefaults.shapes(),
                    modifier = Modifier.fillMaxWidth().height(START_HEIGHT_DP.dp),
                ) { Text(if (pick.minutes > 0) "Start for ${pick.minutes} minutes" else "Start", style = type.sectionHeader) }
                Text(
                    if (pick.minutes > 0) "Music pauses at ${sleepClock(context, now + pick.minutes * MS_PER_MINUTE)}" else "Music pauses when ${pick.thing} ends",
                    style = type.caption,
                    color = scheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth().padding(top = Grid.base.dp),
                )
            } else {
                // Running: the clock, the bar, Add 15 min and Cancel timer, then Change time.
                val left = (timer.endsAtUtcMs - now).coerceAtLeast(0)
                val span = (timer.endsAtUtcMs - timer.setUtcMs).coerceAtLeast(1)
                Text(
                    when (timer.mode) {
                        SleepTimerMode.MINUTES -> "Music fades out at ${sleepClock(context, timer.endsAtUtcMs)}"
                        SleepTimerMode.END_OF_SONG -> "Music stops after this song"
                        SleepTimerMode.END_OF_ALBUM -> "Music stops after this album"
                    },
                    style = type.caption,
                    color = scheme.onSurfaceVariant,
                    // Read out when it changes: it replaces the confirmation toast the sheet hid.
                    modifier = Modifier.padding(top = 2.dp).semantics { liveRegion = LiveRegionMode.Polite },
                )
                Text(
                    countdown(left),
                    style = type.pageTitle.copy(fontSize = COUNTDOWN_SP.sp, lineHeight = (COUNTDOWN_SP + 8).sp, fontFeatureSettings = "tnum"),
                    modifier = Modifier
                        .padding(top = Grid.base.dp, bottom = Grid.base.dp)
                        .semantics { contentDescription = "Sleep timer, ${spokenLeft(left)} left" },
                )
                LinearWavyProgressIndicator(
                    progress = { ((now - timer.setUtcMs).toFloat() / span).coerceIn(0f, 1f) },
                    color = scheme.primary,
                    trackColor = scheme.primary.copy(alpha = TRACK_ALPHA),
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height((Grid.base * 2).dp))
                Row(horizontalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
                    FilledTonalButton(
                        onClick = { addFifteen() },
                        shapes = ButtonDefaults.shapes(),
                        modifier = Modifier.weight(1f).heightIn(min = ACTION_HEIGHT_DP.dp).semantics { contentDescription = "Add $ADD_MINUTES minutes" },
                    ) { Text("Add $ADD_MINUTES min", style = type.sectionHeader) }
                    OutlinedButton(
                        onClick = { cancel() },
                        shapes = ButtonDefaults.shapes(),
                        modifier = Modifier.weight(1f).heightIn(min = ACTION_HEIGHT_DP.dp),
                    ) { Text("Cancel timer", style = type.sectionHeader) }
                }
                Spacer(Modifier.height((Grid.base * 2.5f).dp))
                Text("CHANGE TIME", style = type.caption, color = scheme.onSurfaceVariant, modifier = Modifier.semantics { heading() })
                Spacer(Modifier.height(Grid.base.dp))
                // The choice in force is lit: a timer of that many minutes in all, or the
                // song or album mode.
                val total = ((timer.endsAtUtcMs - timer.setUtcMs) / MS_PER_MINUTE.toDouble()).let { Math.round(it).toInt() }
                fun current(p: Pick) = if (p.minutes > 0) timer.mode == SleepTimerMode.MINUTES && total == p.minutes else timer.mode == p.mode
                Column(verticalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
                        Pick.CHANGE_PRESETS.forEach { p ->
                            ChoiceCell(p.shortLabel, current(p), solid = false, onClick = { start(p, firstStart = false) }, modifier = Modifier.weight(1f))
                        }
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
                        listOf(Pick.SONG, Pick.ALBUM).forEach { p ->
                            ChoiceCell(p.shortLabel, current(p), solid = false, onClick = { start(p, firstStart = false) }, modifier = Modifier.weight(1f))
                        }
                    }
                }
            }
        }
    }
}

/**
 * One choice as an expressive button: round while idle, squarer once chosen, squarer still
 * while pressed, on the theme's fast spatial spring. [solid] is the presets' filled look
 * (chosen: the primary colour); otherwise it is outlined, and chosen is tonal.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ChoiceCell(label: String, isSelected: Boolean, solid: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    val press = remember { MutableInteractionSource() }
    val pressed by press.collectIsPressedAsState()
    val corner by animateDpAsState(
        when {
            pressed -> CELL_PRESSED_DP
            isSelected -> CELL_SELECTED_DP
            else -> CELL_IDLE_DP
        }.dp,
        MaterialTheme.motionScheme.fastSpatialSpec(),
        label = "corner",
    )
    val container = when {
        solid && isSelected -> scheme.primary
        solid -> scheme.surfaceContainerHigh
        isSelected -> scheme.secondaryContainer
        else -> androidx.compose.ui.graphics.Color.Transparent
    }
    val content = when {
        solid && isSelected -> scheme.onPrimary
        !solid && isSelected -> scheme.onSecondaryContainer
        else -> scheme.onSurface
    }
    Surface(
        onClick = onClick,
        interactionSource = press,
        shape = RoundedCornerShape(corner),
        color = container,
        contentColor = content,
        border = if (solid || isSelected) null else BorderStroke(1.dp, scheme.outlineVariant),
        modifier = modifier
            .heightIn(min = if (solid) CELL_SOLID_HEIGHT_DP.dp else CELL_OUTLINED_HEIGHT_DP.dp)
            .semantics { role = Role.RadioButton; selected = isSelected },
    ) {
        Row(
            Modifier.padding(horizontal = Grid.base.dp, vertical = Grid.base.dp).heightIn(min = CELL_TEXT_MIN_DP.dp),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
        ) {
            Text(label, style = LocalEncoreType.current.buttonLabelSelected, maxLines = 1, textAlign = TextAlign.Center)
        }
    }
}

/** What the sheet offers; [minutes] 0 is a song or album mode. */
private enum class Pick(val label: String, val shortLabel: String, val mode: SleepTimerMode, val minutes: Int, val thing: String) {
    FIVE("5 min", "5", SleepTimerMode.MINUTES, 5, ""),
    TEN("10 min", "10", SleepTimerMode.MINUTES, 10, ""),
    FIFTEEN("15 min", "15", SleepTimerMode.MINUTES, 15, ""),
    THIRTY("30 min", "30", SleepTimerMode.MINUTES, 30, ""),
    FORTY_FIVE("45 min", "45", SleepTimerMode.MINUTES, 45, ""),
    SIXTY("60 min", "60", SleepTimerMode.MINUTES, 60, ""),
    SONG("End of this song", "End of song", SleepTimerMode.END_OF_SONG, 0, "this song"),
    ALBUM("End of this album", "End of album", SleepTimerMode.END_OF_ALBUM, 0, "this album"),
    ;

    companion object {
        val MINUTE_PRESETS = listOf(FIVE, TEN, FIFTEEN, THIRTY, FORTY_FIVE, SIXTY)
        val CHANGE_PRESETS = listOf(FIFTEEN, THIRTY, FORTY_FIVE, SIXTY)
    }
}

/**
 * The running timer, kept in step with `SleepTimerChanged`; null when none runs. The sheet
 * and Settings read it; the sheet also writes the answer of its own calls so it moves at once.
 */
@Composable
fun rememberSleepTimer(host: CoreHost): MutableState<SleepTimer?> {
    val state = remember { mutableStateOf<SleepTimer?>(runCatching { host.core.sleepTimer() }.getOrNull()) }
    LaunchedEffect(host) {
        state.value = runCatching { host.core.sleepTimer() }.getOrNull()
        host.events.collect { e -> if (e.kind == EventKind.SLEEP_TIMER_CHANGED) state.value = e.sleepTimer }
    }
    return state
}

/** A time of day in the device's own 12 or 24 hour style. */
fun sleepClock(context: android.content.Context, utcMs: Long): String = DateFormat.getTimeFormat(context).format(Date(utcMs))

/** Settings > Playback and quality's row value: "Off", or when it ends. */
fun sleepSummary(context: android.content.Context, timer: SleepTimer?): String = when (timer?.mode) {
    null -> "Off"
    SleepTimerMode.MINUTES -> "Ends at ${sleepClock(context, timer.endsAtUtcMs)}"
    SleepTimerMode.END_OF_SONG -> "After this song"
    SleepTimerMode.END_OF_ALBUM -> "After this album"
}

/** The words the one notification primer (EncoreRoot, board 25) shows. */
object PrimerCopy {
    /** True while the primer is for the sleep timer, false for downloads. */
    val sleepTimer = MutableStateFlow(false)
}

/** m:ss, or h:mm:ss from an hour. */
private fun countdown(ms: Long): String {
    val total = (ms + MS_PER_SECOND - 1) / MS_PER_SECOND
    val h = total / SECONDS_PER_HOUR
    val m = total % SECONDS_PER_HOUR / SECONDS_PER_MINUTE
    val s = total % SECONDS_PER_MINUTE
    return if (h > 0) String.format(Locale.ROOT, "%d:%02d:%02d", h, m, s) else String.format(Locale.ROOT, "%02d:%02d", m, s)
}

/** For TalkBack: "24 minutes", rounded up to the minute, "less than a minute" at the end. */
private fun spokenLeft(ms: Long): String {
    val minutes = (ms + MS_PER_MINUTE - 1) / MS_PER_MINUTE
    val h = minutes / MINUTES_PER_HOUR
    val m = minutes % MINUTES_PER_HOUR
    fun plural(n: Long, unit: String) = if (n == 1L) "$n $unit" else "$n ${unit}s"
    return when {
        minutes <= 0 -> "less than a minute"
        h == 0L -> plural(m, "minute")
        m == 0L -> plural(h, "hour")
        else -> "${plural(h, "hour")} ${plural(m, "minute")}"
    }
}

private const val TICK_MS = 1000L
private const val MS_PER_SECOND = 1000
private const val MS_PER_MINUTE = 60_000L
private const val SECONDS_PER_MINUTE = 60L
private const val SECONDS_PER_HOUR = 3600L
private const val MINUTES_PER_HOUR = 60L

/** Add 15 min, as the core's `extend_sleep_timer` takes it. */
private const val ADD_MINUTES = 15
private const val PRESETS_PER_ROW = 3
private const val COUNTDOWN_SP = 64
private const val TRACK_ALPHA = 0.2f
private const val START_HEIGHT_DP = 56
private const val ACTION_HEIGHT_DP = 52
private const val CELL_SOLID_HEIGHT_DP = 52
private const val CELL_OUTLINED_HEIGHT_DP = 48
private const val CELL_TEXT_MIN_DP = 24
private const val CELL_IDLE_DP = 28
private const val CELL_SELECTED_DP = 16
private const val CELL_PRESSED_DP = 12
