package uk.gcartee.encore.app.ui.player

import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.IconToggleButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import kotlinx.coroutines.delay
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.foundation.background
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.Spring
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.rememberTurn
import uk.gcartee.encore.app.ui.components.rememberPop
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.MenuPlace
import uk.gcartee.encore.app.ui.components.SongItem
import uk.gcartee.encore.app.ui.components.SwipeAction
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.ChosenBy
import uk.gcartee.encore.core.NotInterestedReason
import uk.gcartee.encore.core.QueueEntry
import uk.gcartee.encore.core.QueueState
import uk.gcartee.encore.core.RepeatMode
import uk.gcartee.encore.tokens.EncoreTokens.AndroidToggleShape
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/**
 * Board 11's queue, drawn in place on Now Playing (board 09, item 8) straight on the
 * album's field, as Lyrics is and as board 29 draws Up next: the field's text colour for
 * words, tonal tints of it for containers. [onColour] is the field's base, the glyph on a
 * toggle that is on.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun QueuePanel(host: CoreHost, colour: Color, onPlayer: () -> Unit, modifier: Modifier = Modifier, onColour: Color = Color.Black) {
    val highlight = colour.copy(alpha = HIGHLIGHT_ALPHA)
    val queue by host.queue.collectAsState()
    val q = queue ?: return
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val actions = LocalAppActions.current
    var historyOpen by rememberSaveable { mutableStateOf(false) }
    var confirmClear by remember { mutableStateOf(false) }

    fun edit(message: String?, undoable: Boolean = true, change: () -> QueueState) {
        runCatching(change)
            .onSuccess { if (message != null) actions.toaster.show(message, if (undoable) ({ runCatching { host.core.undoQueue() } }) else null) }
            .onFailure { actions.toaster.show(it.userMessage()) }
    }

    var adventurous by remember { mutableStateOf(false) }
    val upNextDrag = remember { Reorder() }
    val autoDrag = remember { Reorder() }
    // The queue came back changed: show it, not the order held since the drop.
    LaunchedEffect(q.upNext) { upNextDrag.holding = false }
    LaunchedEffect(q.autoplay) { autoDrag.holding = false }
    LaunchedEffect(upNextDrag.holding, autoDrag.holding) {
        if (upNextDrag.holding || autoDrag.holding) {
            delay(HOLD_TIMEOUT_MS)
            upNextDrag.holding = false
            autoDrag.holding = false
        }
    }
    LazyColumn(modifier, contentPadding = PaddingValues(bottom = (Grid.base * 2).dp)) {
        item {
            // Item 1: the title, then Shuffle, Repeat and Autoplay as tonal toggles whose
            // shape tells the state (section 22).
            Row(Modifier.fillMaxWidth().padding(start = (Grid.base * 2).dp, end = Grid.base.dp, top = (Grid.base * 2).dp, bottom = Grid.base.dp), horizontalArrangement = Arrangement.spacedBy(Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
                Text("Queue", style = type.sheetTitle, color = colour, modifier = Modifier.weight(1f))
                // Each glyph answers its change: Shuffle flips, Repeat and Autoplay turn once,
                // all with a small pop, while the button morphs round <-> square (board 35).
                val shuffleFlip = rememberTurn(q.shuffle)
                val repeatTurn = rememberTurn(q.repeat)
                val autoTurn = rememberTurn(q.autoplayEnabled)
                val shufflePop = rememberPop(q.shuffle, TOGGLE_POP)
                val repeatPop = rememberPop(q.repeat, TOGGLE_POP)
                val autoPop = rememberPop(q.autoplayEnabled, TOGGLE_POP)
                StateToggle(q.shuffle, if (q.shuffle) "Shuffle on" else "Shuffle off", colour, onColour, onToggle = { on ->
                    haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
                    edit(null) { host.core.setShuffle(on) }
                }) { Icon(Icons.shuffle(), contentDescription = "Shuffle", modifier = Modifier.graphicsLayer { rotationY = shuffleFlip; scaleX = shufflePop; scaleY = shufflePop }) }
                StateToggle(q.repeat != RepeatMode.OFF, "Repeat ${q.repeat.name.lowercase()}", colour, onColour, onToggle = {
                    val next = when (q.repeat) {
                        RepeatMode.OFF -> RepeatMode.ALL
                        RepeatMode.ALL -> RepeatMode.ONE
                        RepeatMode.ONE -> RepeatMode.OFF
                    }
                    haptics.perform(if (next == RepeatMode.OFF) Haptic.ToggleOff else Haptic.ToggleOn)
                    edit(null) { host.core.setRepeat(next) }
                }) { Icon(if (q.repeat == RepeatMode.ONE) Icons.repeatOne() else Icons.repeat(), contentDescription = "Repeat", modifier = Modifier.graphicsLayer { rotationZ = repeatTurn; scaleX = repeatPop; scaleY = repeatPop }) }
                StateToggle(q.autoplayEnabled, if (q.autoplayEnabled) "Autoplay on" else "Autoplay off", colour, onColour, onToggle = { on ->
                    haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
                    edit(null) { host.core.setAutoplay(on) }
                }) { Icon(Icons.autoplay(), contentDescription = "Autoplay", modifier = Modifier.graphicsLayer { rotationZ = autoTurn; scaleX = autoPop; scaleY = autoPop }) }
            }
        }
        // Item 2: History, collapsed by default, newest at the bottom.
        if (q.history.isNotEmpty()) {
            item {
                Row(Modifier.fillMaxWidth().clickable { historyOpen = !historyOpen }.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("History", style = type.caption, color = colour.copy(alpha = 0.72f), modifier = Modifier.weight(1f))
                    Text(if (historyOpen) "Hide" else "Show ${q.history.size}", style = type.caption, color = colour.copy(alpha = 0.72f))
                }
            }
            if (historyOpen) itemsIndexed(q.history, key = { _, e -> "h${e.uid}" }) { _, e ->
                Box(Modifier.graphicsLayer { alpha = 0.6f }) {
                    SongItem(e.song, playing = false, onClick = { edit(null, undoable = false) { host.core.replayFromHistory(e.uid) } }, contentColor = colour, secondaryColor = colour.copy(alpha = 0.72f), rowColor = Color.Transparent)
                }
            }
        }
        // Item 3: the now playing row; no swipe, so a slip can't dismiss it.
        q.current?.let { c ->
            item(key = "now${c.uid}") {
                // Item 3: highlighted, with live level bars; no heading, the highlight says it.
                androidx.compose.material3.Surface(
                    // Concentric with the 28 dp sheet it sits 8 dp inside.
                    shape = androidx.compose.foundation.shape.RoundedCornerShape(20.dp),
                    color = highlight,
                    modifier = Modifier.padding(horizontal = Grid.base.dp, vertical = Grid.base.dp),
                ) {
                    SongItem(c.song, playing = true, onClick = onPlayer, contentColor = colour, secondaryColor = colour.copy(alpha = 0.72f), rowColor = Color.Transparent, toEnd = null, toStart = null)
                }
            }
        }
        item {
            Row(Modifier.fillMaxWidth().padding(start = (Grid.base * 2).dp, end = Grid.base.dp, top = (Grid.base * 2).dp), verticalAlignment = Alignment.CenterVertically) {
                Text("Up next", style = type.caption, color = colour.copy(alpha = 0.72f), modifier = Modifier.weight(1f))
                if (q.upNext.isNotEmpty()) TextButton(onClick = { confirmClear = true }) { Text("Clear", style = type.caption, color = colour.copy(alpha = 0.72f)) }
            }
        }
        if (q.upNext.isEmpty()) item {
            Text("Nothing queued. Autoplay continues with songs you'll like.", style = type.body, color = colour.copy(alpha = 0.72f), modifier = Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp))
        }
        reorderable(upNextDrag.shown(q.upNext), host, colour, isAutoplay = false, ::edit, upNextDrag)
        item {
            Row(Modifier.fillMaxWidth().padding(start = (Grid.base * 2).dp, end = Grid.base.dp, top = (Grid.base * 2).dp), verticalAlignment = Alignment.CenterVertically) {
                Text("Autoplay", style = type.caption, color = colour.copy(alpha = 0.72f), modifier = Modifier.weight(1f))
                // Item 1, HOLD on Autoplay: the Familiar to Adventurous slider, here as a link.
                TextButton(onClick = { adventurous = true }) { Text("Familiar to adventurous", style = type.caption, color = colour.copy(alpha = 0.72f)) }
            }
        }
        when {
            !q.autoplayEnabled -> item {
                Text("Autoplay is off", style = type.body, color = colour.copy(alpha = 0.72f), modifier = Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp))
            }
            // The core fills Autoplay when a song starts with fewer than five upcoming
            // (CORE-API, smart picks), so a long Up Next is no reason to show a spinner.
            q.autoplay.isEmpty() && q.upNext.size >= AUTOPLAY_FILLS_BELOW -> item {
                Text("Autoplay picks songs as Up Next runs low.", style = type.body, color = colour.copy(alpha = 0.72f), modifier = Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp))
            }
            q.autoplay.isEmpty() -> item {
                // States: refilling shows the expressive loading indicator, not a sentence alone.
                Row(Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
                    androidx.compose.material3.LoadingIndicator(Modifier.size(32.dp), color = colour.copy(alpha = 0.72f))
                    Spacer(Modifier.width(Grid.base.dp))
                    Text("Picking songs you'll like…", style = type.body, color = colour.copy(alpha = 0.72f))
                }
            }
            else -> reorderable(autoDrag.shown(q.autoplay), host, colour, isAutoplay = true, ::edit, autoDrag)
        }
    }

    if (adventurous) {
        var level by remember { mutableStateOf(runCatching { host.core.adventurousness() }.getOrDefault(uk.gcartee.encore.core.Adventurousness.BALANCED)) }
        AlertDialog(
            onDismissRequest = { adventurous = false },
            title = { Text("How adventurous") },
            text = { uk.gcartee.encore.app.ui.settings.AdventurousChoice(level) { l -> runCatching { host.core.setAdventurousness(l) }.onSuccess { level = l } } },
            confirmButton = { TextButton(onClick = { adventurous = false }) { Text("Done") } },
        )
    }
    if (confirmClear) {
        // Item 6: the destructive button sits apart from Cancel.
        AlertDialog(
            onDismissRequest = { confirmClear = false },
            title = { Text("Clear Up Next?") },
            text = { Text("Autoplay is not cleared; it carries on from the current song.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmClear = false
                    haptics.perform(Haptic.Confirm)
                    edit("Cleared Up Next") { host.core.clearUpNext() }
                }) { Text("Clear Up Next", color = androidx.compose.material3.MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { confirmClear = false }) { Text("Cancel") } },
        )
    }
}


/** A round toggle that turns into a rounded square when on (section 22). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StateToggle(on: Boolean, state: String, colour: Color, onColour: Color, onToggle: (Boolean) -> Unit, icon: @Composable () -> Unit) {
    val h = AndroidToggleShape.Sizes.Compact.height
    IconToggleButton(
        checked = on,
        onCheckedChange = onToggle,
        shapes = IconButtonDefaults.toggleableShapes(
            shape = RoundedCornerShape(percent = AndroidToggleShape.restRadiusPercent),
            pressedShape = RoundedCornerShape((h * AndroidToggleShape.pressedRadiusPercentOfHeight / 100).dp),
            checkedShape = RoundedCornerShape(AndroidToggleShape.Sizes.Compact.checkedRadius.dp),
        ),
        colors = IconButtonDefaults.iconToggleButtonColors(
            // Board 35: off is round and tonal; on is squarer and filled, like Like.
            containerColor = colour.copy(alpha = TONAL_ALPHA),
            contentColor = colour,
            checkedContainerColor = colour,
            checkedContentColor = onColour,
        ),
        modifier = Modifier.size(AndroidToggleShape.Sizes.Compact.touchTarget.dp).semantics { stateDescription = state },
    ) { icon() }
}

/**
 * A drag in progress within one section of the queue. While a row is held, [order] is
 * the section as it now stands; the core is told once, on release, and the new order
 * stays on screen until the queue comes back changed, so nothing flicks back.
 */
private class Reorder {
    var dragging by mutableStateOf<Long?>(null)
    var holding by mutableStateOf(false)
    var offset by mutableFloatStateOf(0f)
    var order by mutableStateOf<List<QueueEntry>>(emptyList())
    var startIndex = -1

    fun shown(live: List<QueueEntry>): List<QueueEntry> = if (dragging != null || holding) order else live
}

/**
 * Items 4 and 5: tap jumps, the handle lifts the row to reorder (DRAG_START on pick-up,
 * GESTURE_END on drop; the row body never drags). As the row passes half of a neighbour
 * they swap places with a tick, and the others slide out of the way on a spring. Swipe
 * right is Play Next, left past 60 % is Remove (Undo for 5 s); on Autoplay rows left is
 * "Not this", a strong dislike, and a dragged Autoplay row moves into Up Next.
 */
private fun LazyListScope.reorderable(
    entries: List<QueueEntry>,
    host: CoreHost,
    colour: Color,
    isAutoplay: Boolean,
    edit: (String?, Boolean, () -> QueueState) -> Unit,
    drag: Reorder,
) {
    itemsIndexed(entries, key = { _, e -> (if (isAutoplay) "a" else "u") + e.uid }) { index, e ->
        val haptics = rememberHaptics()
        var rowH by remember { mutableFloatStateOf(1f) }
        // The drag handler outlives recompositions; it must see the section as it is now.
        val section by androidx.compose.runtime.rememberUpdatedState(entries)
        val held = drag.dragging == e.uid
        val lift by animateFloatAsState(if (held) 1f else 0f, tween(LIFT_MS), label = "lift")
        val removeIcon = Icons.remove()
        val blockIcon = Icons.block()
        val nextIcon = Icons.playNext()
        val toStart = if (isAutoplay) {
            SwipeAction("Not this", destructive = true, icon = { blockIcon }) {
                edit("Removed. You'll hear less like this", false) {
                    runCatching { host.core.notInterestedSong(e.song.id, NotInterestedReason.NOT_MY_TASTE) }
                    host.core.removeFromQueue(e.uid)
                }
            }
        } else {
            SwipeAction("Remove", destructive = true, icon = { removeIcon }) { edit("Removed from Up Next", true) { host.core.removeFromQueue(e.uid) } }
        }
        val toEnd = SwipeAction("Play Next", destructive = false, icon = { nextIcon }) { edit("Playing next", true) { host.core.playNext(listOf(e.song.id)) } }
        Row(
            Modifier
                // The held row follows the finger; the rest glide into their new places.
                .animateItem(fadeInSpec = null, fadeOutSpec = null, placementSpec = if (held) null else spring(stiffness = Spring.StiffnessMediumLow))
                .zIndex(if (held) 1f else 0f)
                .onSizeChanged { rowH = it.height.toFloat().coerceAtLeast(1f) }
                .graphicsLayer {
                    translationY = if (held) drag.offset else 0f
                    val s = 1f + LIFT_SCALE * lift
                    scaleX = s
                    scaleY = s
                }
                .background(colour.copy(alpha = HELD_ALPHA * lift), RoundedCornerShape(HELD_RADIUS_DP.dp))
                .semantics {
                    customActions = listOf(
                        CustomAccessibilityAction("Move up") { edit(null, true) { host.core.moveInQueue(e.uid, (index - 1).coerceAtLeast(0).toUInt()) }; true },
                        CustomAccessibilityAction("Move down") { edit(null, true) { host.core.moveInQueue(e.uid, (index + 1).toUInt()) }; true },
                    )
                },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(Modifier.weight(1f)) {
                SongItem(
                    e.song,
                    playing = false,
                    onClick = { edit("Skipped to ${e.song.title}", true) { host.core.jumpTo(e.uid) } },
                    place = MenuPlace(chosenBy = e.chosenBy),
                    contentColor = colour,
                    secondaryColor = colour.copy(alpha = 0.72f),
                    rowColor = Color.Transparent,
                    toEnd = toEnd,
                    toStart = toStart,
                )
            }
            if (e.chosenBy == ChosenBy.APP) Icon(Icons.spark(), contentDescription = "Picked for you", tint = colour.copy(alpha = 0.6f), modifier = Modifier.size(16.dp))
            Icon(
                Icons.dragHandle(),
                contentDescription = "Reorder",
                tint = colour.copy(alpha = 0.72f),
                modifier = Modifier
                    .heightIn(min = 48.dp)
                    .padding(horizontal = 12.dp)
                    .pointerInput(e.uid) {
                        detectDragGestures(
                            onDragStart = {
                                haptics.perform(Haptic.DragStart)
                                drag.order = section
                                drag.startIndex = section.indexOfFirst { it.uid == e.uid }
                                drag.offset = 0f
                                drag.dragging = e.uid
                            },
                            onDragEnd = {
                                val final = drag.order.indexOfFirst { it.uid == e.uid }
                                val moved = final >= 0 && final != drag.startIndex
                                haptics.perform(Haptic.ScrubEnd)
                                drag.holding = moved
                                drag.dragging = null
                                drag.offset = 0f
                                if (moved) {
                                    // Reordering an autoplay row moves it into Up Next.
                                    val to = if (isAutoplay) AUTOPLAY_TO_UP_NEXT else final
                                    edit(null, true) { host.core.moveInQueue(e.uid, to.toUInt()) }
                                }
                            },
                            onDragCancel = {
                                drag.dragging = null
                                drag.holding = false
                                drag.offset = 0f
                            },
                        ) { change, amount ->
                            change.consume()
                            drag.offset += amount.y
                            val list = drag.order.toMutableList()
                            var i = list.indexOfFirst { it.uid == e.uid }
                            if (i < 0) return@detectDragGestures
                            var swapped = false
                            while (drag.offset > rowH / 2 && i < list.lastIndex) {
                                java.util.Collections.swap(list, i, i + 1)
                                i++
                                drag.offset -= rowH
                                swapped = true
                            }
                            while (drag.offset < -rowH / 2 && i > 0) {
                                java.util.Collections.swap(list, i, i - 1)
                                i--
                                drag.offset += rowH
                                swapped = true
                            }
                            if (swapped) {
                                haptics.perform(Haptic.Tick)
                                drag.order = list
                            }
                        }
                    },
            )
        }
    }
}
private const val HIGHLIGHT_ALPHA = 0.14f
private const val TONAL_ALPHA = 0.16f
private const val AUTOPLAY_FILLS_BELOW = 5
private const val TOGGLE_POP = 1.15f
private const val LIFT_MS = 150
private const val LIFT_SCALE = 0.02f
private const val HELD_ALPHA = 0.14f
private const val HELD_RADIUS_DP = 16
private const val AUTOPLAY_TO_UP_NEXT = 10_000
private const val HOLD_TIMEOUT_MS = 2_000L
