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
import androidx.compose.animation.core.withInfiniteAnimationFrameMillis
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerId
import androidx.compose.ui.layout.LocalPinnableContainer
import androidx.compose.ui.platform.LocalDensity
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
import androidx.compose.runtime.mutableIntStateOf
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
    val drag = remember { Reorder() }
    // The queue came back changed: show it, not the order held since the drop.
    LaunchedEffect(q.upNext, q.autoplay) { drag.holding = false }
    LaunchedEffect(drag.holding) {
        if (drag.holding) {
            delay(HOLD_TIMEOUT_MS)
            drag.holding = false
        }
    }
    // Up Next and Autoplay are one list while a row is held, split at [Reorder.boundary].
    val live = q.upNext + q.autoplay
    val shown = drag.shown(live)
    val boundary = if (drag.dragging != null || drag.holding) drag.boundary else q.upNext.size
    val upShown = shown.take(boundary)
    val autoShown = shown.drop(boundary)
    val autoUids = remember(q.autoplay) { q.autoplay.map { it.uid }.toSet() }
    val listState = rememberLazyListState()
    // The held row changed places: a tick, and the list keeps its place by position. Left
    // alone it would follow whichever row was first on screen, and jump when that row is
    // the one that just moved.
    val onStep = remember(listState, haptics) {
        {
            haptics.perform(Haptic.Tick)
            listState.requestScrollToItem(listState.firstVisibleItemIndex, listState.firstVisibleItemScrollOffset)
        }
    }
    // The owner (9 November 2026): a song could only go as far as the screen showed, "move
    // it up, scroll up, move it up, scroll up". Held near the top or bottom edge, the list
    // now scrolls under the row, faster the nearer the edge, and the row goes on trading
    // places as songs pass beneath it. It needs the finger to be heading that way, so a
    // row picked up at the edge doesn't run off by itself.
    val density = LocalDensity.current
    LaunchedEffect(drag.dragging, listState) {
        val uid = drag.dragging ?: return@LaunchedEffect
        val key = "q$uid"
        val edge = with(density) { SCROLL_EDGE_DP.dp.toPx() }
        val slow = with(density) { SCROLL_SLOW_DP.dp.toPx() }
        val fast = with(density) { SCROLL_FAST_DP.dp.toPx() }
        val lean = with(density) { LEAN_DP.dp.toPx() }
        var last = withInfiniteAnimationFrameMillis { it }
        while (true) {
            val now = withInfiniteAnimationFrameMillis { it }
            val dt = ((now - last) / 1000f).coerceIn(0f, MAX_FRAME_S)
            last = now
            val info = listState.layoutInfo
            val slot = info.visibleItemsInfo.firstOrNull { it.key == key } ?: continue
            val top = slot.offset + drag.offset
            val above = (info.viewportStartOffset + edge - top) / edge
            val below = (top + slot.size - (info.viewportEndOffset - edge)) / edge
            val speed = when {
                above > 0f && drag.lean < -lean -> -(slow + (fast - slow) * above.coerceAtMost(1f).let { it * it })
                below > 0f && drag.lean > lean -> slow + (fast - slow) * below.coerceAtMost(1f).let { it * it }
                else -> 0f
            }
            if (speed != 0f) {
                drag.offset += listState.dispatchRawDelta(speed * dt)
                if (drag.step()) onStep()
            }
        }
    }
    LazyColumn(
        // And the other way the owner asked for: one finger holds the row, another scrolls.
        modifier.secondFingerScroll(drag, listState, onStep),
        state = listState,
        contentPadding = PaddingValues(bottom = (Grid.base * 2).dp),
    ) {
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
        if (upShown.isEmpty() && drag.dragging == null) item {
            Text("Nothing queued. Autoplay continues with songs you'll like.", style = type.body, color = colour.copy(alpha = 0.72f), modifier = Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp))
        }
        reorderable(upShown, shown, host, colour, autoUids, ::edit, drag, onStep)
        item(key = "autoplay-heading") {
            // Measured: a pick dragged up past this heading joins Up Next.
            Row(Modifier.fillMaxWidth().onSizeChanged { drag.headerH = it.height.toFloat() }.padding(start = (Grid.base * 2).dp, end = Grid.base.dp, top = (Grid.base * 2).dp), verticalAlignment = Alignment.CenterVertically) {
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
            autoShown.isEmpty() && q.autoplay.isEmpty() && q.upNext.size >= AUTOPLAY_FILLS_BELOW -> item {
                Text("Autoplay picks songs as Up Next runs low.", style = type.body, color = colour.copy(alpha = 0.72f), modifier = Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp))
            }
            autoShown.isEmpty() && q.autoplay.isEmpty() -> item {
                // States: refilling shows the expressive loading indicator, not a sentence alone.
                Row(Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
                    androidx.compose.material3.LoadingIndicator(Modifier.size(32.dp), color = colour.copy(alpha = 0.72f))
                    Spacer(Modifier.width(Grid.base.dp))
                    Text("Picking songs you'll like…", style = type.body, color = colour.copy(alpha = 0.72f))
                }
            }
            else -> reorderable(autoShown, shown, host, colour, autoUids, ::edit, drag, onStep)
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
 * A drag in progress across Up Next and Autoplay, held as one list split at [boundary].
 * The core takes an Autoplay song into Up Next ("Moving an Autoplay song puts it in Up
 * Next", CORE-API) but has no way to make an Up Next song an Autoplay pick or to reorder
 * the picks. So: Up Next rows swap freely and stop above the Autoplay heading; a pick only
 * moves once dragged up past the heading, where it joins Up Next, and dragged back down
 * it returns to its place. The core is told once, on release, and the new order stays on
 * screen until the queue comes back changed, so nothing flicks back.
 */
private class Reorder {
    var dragging by mutableStateOf<Long?>(null)
    var holding by mutableStateOf(false)
    var offset by mutableFloatStateOf(0f)
    var order by mutableStateOf<List<QueueEntry>>(emptyList())
    var boundary by mutableIntStateOf(0)
    var headerH = 0f
    var rowH = 1f
    var startIndex = -1
    var fromAutoplay = false
    var autoplayIndex = 0

    /** Which way the finger has been heading lately, in pixels: up is negative. */
    var lean = 0f

    fun shown(live: List<QueueEntry>): List<QueueEntry> = if (dragging != null || holding) order else live

    /** The finger moved by [dy]: [lean] follows it, and turns round within [most] pixels. */
    fun leaned(dy: Float, most: Float) {
        lean = (lean + dy).coerceIn(-most, most)
    }

    /**
     * Moves the held row through the list to where [offset] has carried it, whether the
     * finger moved or the list scrolled beneath it. True when it changed places.
     */
    fun step(): Boolean {
        val uid = dragging ?: return false
        var i = order.indexOfFirst { it.uid == uid }
        if (i < 0) return false
        val item = order[i]
        // Copied only when the row does change places.
        var changed: MutableList<QueueEntry>? = null
        fun list() = changed ?: order.toMutableList().also { changed = it }
        var b = boundary
        while (true) {
            if (i < b) {
                when {
                    offset < -rowH / 2 && i > 0 -> {
                        java.util.Collections.swap(list(), i, i - 1)
                        i--
                        offset += rowH
                    }
                    offset > rowH / 2 && i < b - 1 -> {
                        java.util.Collections.swap(list(), i, i + 1)
                        i++
                        offset -= rowH
                    }
                    // A pick taken into Up Next goes back to its place.
                    fromAutoplay && i == b - 1 && offset > headerH / 2 + autoplayIndex * rowH -> {
                        val l = list()
                        l.removeAt(i)
                        b--
                        val slot = (b + autoplayIndex).coerceAtMost(l.size)
                        l.add(slot, item)
                        i = slot
                        offset -= headerH + autoplayIndex * rowH
                    }
                    else -> break
                }
            } else {
                // A pick moves only by joining Up Next, past the heading.
                val k = i - b
                if (offset < -(headerH / 2 + k * rowH)) {
                    val l = list()
                    l.removeAt(i)
                    l.add(b, item)
                    offset += headerH + k * rowH
                    i = b
                    b++
                } else {
                    break
                }
            }
        }
        // An Up Next row stops above the heading; a pick does not wander among the other
        // picks.
        if (i == b - 1 && !fromAutoplay) offset = offset.coerceAtMost(rowH / 2)
        if (i >= b) offset = offset.coerceAtMost(rowH / 2)
        if (i == 0) offset = offset.coerceAtLeast(-rowH / 2)
        val moved = changed ?: return false
        order = moved
        boundary = b
        return true
    }
}

/**
 * While a row is held by its handle, any other finger put down on the queue scrolls it.
 * The list's own scrolling can't: it only starts with a gesture's first finger, and that
 * one belongs to the handle. So this takes the later fingers before the rows see them (a
 * second finger never taps or swipes a row), moves the list by hand and keeps the held row
 * under the first finger.
 */
private fun Modifier.secondFingerScroll(drag: Reorder, list: LazyListState, onStep: () -> Unit): Modifier = pointerInput(drag, list, onStep) {
    awaitPointerEventScope {
        val others = mutableSetOf<PointerId>()
        while (true) {
            val event = awaitPointerEvent(PointerEventPass.Initial)
            var dy = 0f
            for (change in event.changes) {
                if (drag.dragging != null && change.pressed && !change.previousPressed) others += change.id
                if (change.id !in others) continue
                if (!change.pressed) {
                    others -= change.id
                } else if (change.previousPressed && dy == 0f) {
                    dy = change.position.y - change.previousPosition.y
                }
                change.consume()
            }
            if (dy != 0f) {
                // A finger moving up brings later songs into view.
                val used = list.dispatchRawDelta(-dy)
                if (drag.dragging != null) {
                    drag.offset += used
                    if (drag.step()) onStep()
                }
            }
        }
    }
}

/**
 * Items 4 and 5: tap jumps, the handle lifts the row to reorder (DRAG_START on pick-up,
 * GESTURE_END on drop; the row body never drags). As the row passes half of a neighbour
 * they swap places with a tick, and the others slide out of the way on a spring. Swipe
 * right is Play Next, left past 60 % is Remove (Undo for 5 s); on Autoplay rows left is
 * "Not this", a strong dislike.
 */
private fun LazyListScope.reorderable(
    entries: List<QueueEntry>,
    all: List<QueueEntry>,
    host: CoreHost,
    colour: Color,
    autoUids: Set<Long>,
    edit: (String?, Boolean, () -> QueueState) -> Unit,
    drag: Reorder,
    onStep: () -> Unit,
) {
    itemsIndexed(entries, key = { _, e -> "q" + e.uid }) { index, e ->
        val haptics = rememberHaptics()
        var rowH by remember { mutableFloatStateOf(1f) }
        // The drag handler outlives recompositions; it must see the queue as it is now.
        val everything by androidx.compose.runtime.rememberUpdatedState(all)
        val picks by androidx.compose.runtime.rememberUpdatedState(autoUids)
        val stepped by androidx.compose.runtime.rememberUpdatedState(onStep)
        val isPick = e.uid in autoUids
        val held = drag.dragging == e.uid
        // The held row's place may scroll off screen while its drag goes on (the row is
        // drawn under the finger, wherever its place is); pinned, the list keeps it, and
        // the drag with it.
        val pinnable = LocalPinnableContainer.current
        DisposableEffect(held, pinnable) {
            val pin = if (held) pinnable?.pin() else null
            onDispose { pin?.release() }
        }
        val lift by animateFloatAsState(if (held) 1f else 0f, tween(LIFT_MS), label = "lift")
        val removeIcon = Icons.remove()
        val blockIcon = Icons.block()
        val nextIcon = Icons.playNext()
        val toStart = if (isPick) {
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
                                val list = everything
                                val firstPick = list.indexOfFirst { it.uid in picks }.let { if (it < 0) list.size else it }
                                drag.order = list
                                drag.boundary = firstPick
                                drag.startIndex = list.indexOfFirst { it.uid == e.uid }
                                drag.fromAutoplay = e.uid in picks
                                drag.autoplayIndex = (drag.startIndex - firstPick).coerceAtLeast(0)
                                drag.rowH = rowH
                                drag.lean = 0f
                                drag.offset = 0f
                                drag.dragging = e.uid
                            },
                            onDragEnd = {
                                val final = drag.order.indexOfFirst { it.uid == e.uid }
                                val inUpNext = final in 0 until drag.boundary
                                val moved = if (drag.fromAutoplay) inUpNext else final >= 0 && final != drag.startIndex
                                haptics.perform(Haptic.ScrubEnd)
                                drag.holding = moved
                                drag.dragging = null
                                drag.offset = 0f
                                if (moved) edit(null, true) { host.core.moveInQueue(e.uid, final.toUInt()) }
                            },
                            onDragCancel = {
                                drag.dragging = null
                                drag.holding = false
                                drag.offset = 0f
                            },
                        ) { change, amount ->
                            change.consume()
                            drag.leaned(amount.y, LEAN_DP.dp.toPx() * LEAN_TURN)
                            drag.offset += amount.y
                            if (drag.step()) stepped()
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
private const val HOLD_TIMEOUT_MS = 2_000L

// Scrolling under a held row: the edge zone, the speed at its start and at the very edge
// (dp a second), how far the finger must have headed that way, and how many times that
// far it takes to turn round.
private const val SCROLL_EDGE_DP = 72
private const val SCROLL_SLOW_DP = 120
private const val SCROLL_FAST_DP = 760
private const val LEAN_DP = 6
private const val LEAN_TURN = 3
private const val MAX_FRAME_S = 0.05f
