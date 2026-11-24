package uk.gcartee.encore.app.ui.components

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.graphics.shapes.Morph
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.PlaylistSummary
import uk.gcartee.encore.core.Song
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/*
 * Mock-ups 46 and 47 (owner, 24 November 2026: adding songs one at a time was "very time
 * consuming and annoying", and the first picking screen looked "very plain, very basic"):
 * pick as many songs as you like, then act on them all at once. One selection for the whole
 * app; it ends when you leave the page, press Back or tap ✕.
 */

@Stable
class SongSelection {
    private val picked = mutableStateMapOf<Long, Song>()
    var active by mutableStateOf(false)
        private set
    /** The songs of the page you're on, in its order, for Select all and for joining rows. */
    var available by mutableStateOf<List<Song>>(emptyList())
        internal set
    /** Where the songs come from, for the bar's line ("from Songs"). */
    var label by mutableStateOf<String?>(null)
        internal set
    /** What this page adds to the bar's More: Remove from this playlist. */
    var removeHere by mutableStateOf<((List<Song>) -> Unit)?>(null)
        internal set
    private val order by derivedStateOf { available.withIndex().associate { it.value.id to it.index } }

    val count: Int get() = picked.size
    val allPicked: Boolean get() = available.isNotEmpty() && available.all { it.id in picked }
    val totalMs: Long get() = picked.values.sumOf { it.durationMs }

    fun isPicked(id: Long): Boolean = id in picked

    /** Whether the songs just before and after this one on the page are picked too. */
    fun neighbours(id: Long): Pair<Boolean, Boolean> {
        val i = order[id] ?: return false to false
        val before = i > 0 && available[i - 1].id in picked
        val after = i < available.lastIndex && available[i + 1].id in picked
        return before to after
    }

    fun start(song: Song? = null) {
        active = true
        song?.let { picked[it.id] = it }
    }
    fun toggle(song: Song) {
        if (picked.remove(song.id) == null) picked[song.id] = song
    }
    fun set(song: Song, on: Boolean) {
        if (on) picked[song.id] = song else picked.remove(song.id)
    }
    fun selectAll() = available.forEach { picked[it.id] = it }
    fun clear() {
        active = false
        picked.clear()
    }

    /** The picked songs in the page's order, then any picked before the page changed. */
    fun songs(): List<Song> {
        val inOrder = available.filter { it.id in picked }
        val seen = inOrder.mapTo(HashSet()) { it.id }
        return inOrder + picked.values.filter { it.id !in seen }
    }

    // Drag to pick a run: each row on screen says where it is.
    internal val rows = HashMap<Long, Pair<Song, Rect>>()
}

/** Null where picking makes no sense (Now Playing's queue). */
val LocalSongSelection = staticCompositionLocalOf<SongSelection?> { null }

/** A page's songs, in order, for Select all (and Remove, on a playlist). */
@Composable
fun SelectableSongs(songs: List<Song>, removeHere: ((List<Song>) -> Unit)? = null, label: String? = null) {
    val selection = LocalSongSelection.current ?: return
    DisposableEffect(selection, songs, removeHere, label) {
        selection.available = songs
        selection.removeHere = removeHere
        selection.label = label
        onDispose {
            if (selection.available === songs) {
                selection.available = emptyList()
                selection.removeHere = null
                selection.label = null
            }
        }
    }
}

/**
 * The leading mark while picking. Not picked: the cover, dimmed (or an empty ring where a
 * row shows a track number). Picked: a badge in the theme colour that grows from a circle
 * into Material 3 Expressive's scalloped "cookie" and turns as it does, with a tick that pops.
 * Dragging down the marks picks a run of songs, as Google Photos does; the first row says
 * whether the run picks or unpicks.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun SelectMark(selection: SongSelection, song: Song, size: Dp, cover: (@Composable () -> Unit)? = null) {
    val scheme = MaterialTheme.colorScheme
    val still = reduceMotion()
    val on = selection.isPicked(song.id)
    val grow by animateFloatAsState(
        if (on) 1f else 0f,
        if (still) tween(0) else spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessMediumLow),
        label = "pick",
    )
    val morph = remember { Morph(MaterialShapes.Circle, MaterialShapes.Cookie9Sided) }
    val path = remember { Path() }
    val badge = scheme.primary
    var where by remember { mutableStateOf<LayoutCoordinates?>(null) }
    Box(
        Modifier
            .size(size)
            .onGloballyPositioned { where = it }
            .pointerInput(song.id) {
                var paint = true
                var fromY = 0f
                detectDragGestures(
                    onDragStart = { at ->
                        paint = !selection.isPicked(song.id)
                        selection.set(song, paint)
                        fromY = where?.localToRoot(at)?.y ?: 0f
                    },
                    onDrag = { change, _ ->
                        change.consume()
                        val y = where?.localToRoot(change.position)?.y ?: return@detectDragGestures
                        val top = minOf(fromY, y)
                        val bottom = maxOf(fromY, y)
                        selection.rows.values.forEach { (s, r) -> if (r.bottom >= top && r.top <= bottom) selection.set(s, paint) }
                    },
                )
            }
            .semantics {
                role = Role.Checkbox
                stateDescription = if (on) "Selected" else "Not selected"
            },
        contentAlignment = Alignment.Center,
    ) {
        if (grow < 0.999f) {
            Box(Modifier.graphicsLayer { alpha = (1f - grow) * UNPICKED_ALPHA + 0f }) {
                if (cover != null) {
                    cover()
                } else {
                    Box(Modifier.size(size).border(2.dp, scheme.onSurfaceVariant, CircleShape))
                }
            }
        }
        if (grow > 0.001f) {
            Box(
                Modifier
                    .size(size)
                    .graphicsLayer {
                        val s = BADGE_FROM + (1f - BADGE_FROM) * grow
                        scaleX = s
                        scaleY = s
                    }
                    .drawBehind {
                        morph.toPath(grow.coerceIn(0f, 1f), this.size, path)
                        rotate(TURN_DEGREES * (1f - grow)) { drawPath(path, badge) }
                    },
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    Icons.check(),
                    contentDescription = null,
                    tint = scheme.onPrimary,
                    modifier = Modifier.size(size * 0.5f).graphicsLayer { scaleX = grow; scaleY = grow },
                )
            }
        }
    }
}

/** Each row on screen while picking says where it is, for drag to pick a run. */
internal fun Modifier.selectableRow(selection: SongSelection?, song: Song): Modifier =
    if (selection == null || !selection.active) this
    else onGloballyPositioned { selection.rows[song.id] = song to it.boundsInRoot() }

/**
 * Mock-up 47, board 1: a rounded block in the theme's soft colour across the top: ✕ (a tonal
 * square), the count large (it rolls as it changes) with how long the picked songs last and
 * where they're from, and All (Select all, or none when all are picked).
 */
@Composable
fun SelectionTopBar(selection: SongSelection, modifier: Modifier = Modifier) {
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    val still = reduceMotion()
    AnimatedVisibility(
        selection.active,
        enter = fadeIn(tween(BAR_MS)) + slideInVertically(tween(BAR_MS, easing = FastOutSlowInEasing)) { -it / 2 },
        exit = fadeOut(tween(BAR_MS)) + slideOutVertically(tween(BAR_MS, easing = FastOutSlowInEasing)) { -it / 2 },
        modifier = modifier,
    ) {
        Box(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = Grid.base.dp, vertical = 4.dp)) {
            Surface(
                shape = RoundedCornerShape(TOP_RADIUS_DP.dp),
                color = scheme.primaryContainer,
                contentColor = scheme.onPrimaryContainer,
                modifier = Modifier.fillMaxWidth().shadow(2.dp, RoundedCornerShape(TOP_RADIUS_DP.dp)),
            ) {
                Row(Modifier.padding(start = 10.dp, end = 12.dp, top = 10.dp, bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(
                        Modifier
                            .size(48.dp)
                            .clip(RoundedCornerShape(16.dp))
                            .background(scheme.onPrimaryContainer.copy(alpha = 0.10f))
                            .clickable(onClickLabel = "Stop selecting") { selection.clear() }
                            .semantics { contentDescription = "Stop selecting" },
                        contentAlignment = Alignment.Center,
                    ) { Icon(Icons.close(), contentDescription = null) }
                    Spacer(Modifier.width((Grid.base * 1.5f).dp))
                    Column(Modifier.weight(1f)) {
                        Row(verticalAlignment = Alignment.Bottom) {
                            val n = selection.count
                            AnimatedContent(
                                targetState = n,
                                transitionSpec = {
                                    if (still) {
                                        fadeIn(tween(0)) togetherWith fadeOut(tween(0))
                                    } else if (targetState > initialState) {
                                        (slideInVertically(tween(ROLL_MS)) { it } + fadeIn(tween(ROLL_MS))) togetherWith (slideOutVertically(tween(ROLL_MS)) { -it } + fadeOut(tween(ROLL_MS)))
                                    } else {
                                        (slideInVertically(tween(ROLL_MS)) { -it } + fadeIn(tween(ROLL_MS))) togetherWith (slideOutVertically(tween(ROLL_MS)) { it } + fadeOut(tween(ROLL_MS)))
                                    }
                                },
                                label = "count",
                            ) { value ->
                                Text(
                                    value.toString(),
                                    style = type.sectionHeader.copy(fontSize = 34.sp, lineHeight = 36.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.5).sp),
                                )
                            }
                            Spacer(Modifier.width(Grid.base.dp))
                            Text("selected", style = type.body.copy(fontWeight = FontWeight.SemiBold), modifier = Modifier.padding(bottom = 4.dp))
                        }
                        val line = listOfNotNull(
                            if (selection.count == 0) "Tap songs to pick them" else formatLength(selection.totalMs),
                            selection.label?.let { "from $it" },
                        ).joinToString(" · ")
                        Text(line, style = type.caption, color = scheme.onPrimaryContainer.copy(alpha = 0.8f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                    if (selection.available.isNotEmpty()) {
                        Spacer(Modifier.width(Grid.base.dp))
                        val all = selection.allPicked
                        androidx.compose.material3.Button(
                            onClick = { if (all) selection.available.forEach { selection.set(it, false) } else selection.selectAll() },
                            modifier = Modifier.heightIn(min = 40.dp),
                        ) {
                            Text(if (all) "None" else "All")
                        }
                    }
                }
            }
        }
    }
}

/**
 * Mock-up 47, boards 1 and 2: a floating toolbar of labelled actions (Next, Queue, Like,
 * Save) beside a large Add button. Once a playlist has been added to, Add becomes a split
 * button above the toolbar: the left half adds straight to that playlist, the arrow opens
 * the sheet for any other. More holds Remove from this playlist on a playlist page.
 */
@Composable
fun SelectionBar(selection: SongSelection, modifier: Modifier = Modifier) {
    val actions = LocalAppActions.current
    val scheme = MaterialTheme.colorScheme
    var adding by remember { mutableStateOf<List<Song>?>(null) }
    var more by remember { mutableStateOf(false) }
    val last by produceState<PlaylistSummary?>(null, actions.playlistMemory, selection.active) {
        value = if (selection.active) withContext(Dispatchers.IO) { actions.lastPlaylist() } else null
    }
    AnimatedVisibility(
        selection.active,
        enter = fadeIn(tween(BAR_MS)) + slideInVertically(spring(dampingRatio = Spring.DampingRatioLowBouncy, stiffness = Spring.StiffnessMediumLow)) { it },
        exit = fadeOut(tween(BAR_MS)) + slideOutVertically(tween(BAR_MS, easing = FastOutSlowInEasing)) { it },
        modifier = modifier,
    ) {
        val none = selection.count == 0
        Column(Modifier.padding(horizontal = Grid.base.dp, vertical = Grid.base.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            last?.let { p ->
                SplitAdd(p.name, enabled = !none, onAdd = { actions.quickAdd(selection.songs(), p); selection.clear() }, onMore = { adding = selection.songs() })
                Spacer(Modifier.height(Grid.base.dp))
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Surface(
                    shape = RoundedCornerShape(TOOLBAR_RADIUS_DP.dp),
                    color = scheme.surfaceContainerHighest,
                    modifier = Modifier.shadow(6.dp, RoundedCornerShape(TOOLBAR_RADIUS_DP.dp)),
                ) {
                    Row(Modifier.height(TOOLBAR_DP.dp).padding(horizontal = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        ToolbarItem(Icons.playNext(), "Next", !none) { actions.playNext(selection.songs()); selection.clear() }
                        ToolbarItem(Icons.queue(), "Queue", !none) { actions.addToQueue(selection.songs()); selection.clear() }
                        ToolbarItem(Icons.favorite(false), "Like", !none) {
                            val songs = selection.songs()
                            val n = songs.count { actions.setLiked(it, true) }
                            actions.toaster.show(if (n == 1) "Liked 1 song" else "Liked $n songs")
                            selection.clear()
                        }
                        val online = LocalDownloads.current.online
                        ToolbarItem(Icons.download(), "Save", !none && online) { actions.downloadSongs(selection.songs()); selection.clear() }
                        if (selection.removeHere != null) ToolbarItem(Icons.more(), "More", !none) { more = true }
                    }
                }
                if (last == null) {
                    Spacer(Modifier.width(Grid.base.dp))
                    // The big Add: a large, square-ish button in the theme colour.
                    Surface(
                        onClick = { adding = selection.songs() },
                        enabled = !none,
                        shape = RoundedCornerShape(FAB_RADIUS_DP.dp),
                        color = if (none) scheme.surfaceContainerHigh else scheme.primary,
                        contentColor = if (none) scheme.onSurfaceVariant else scheme.onPrimary,
                        modifier = Modifier.size(FAB_DP.dp).shadow(6.dp, RoundedCornerShape(FAB_RADIUS_DP.dp)).semantics { contentDescription = "Add to playlist" },
                    ) {
                        Box(contentAlignment = Alignment.Center) { Icon(Icons.playlistAdd(), contentDescription = null, modifier = Modifier.size(28.dp)) }
                    }
                }
            }
        }
    }
    if (more) {
        val remove = Icons.remove()
        val songs = selection.songs()
        val here = selection.removeHere
        ActionSheet(
            onDismiss = { more = false },
            header = { Text(if (songs.size == 1) "1 song" else "${songs.size} songs", style = LocalEncoreType.current.sheetTitle, modifier = Modifier.padding(horizontal = Grid.base.dp)) },
            quick = emptyList(),
            groups = listOfNotNull(
                here?.let { listOf(SheetAction("Remove from this playlist", remove, destructive = true) { it(songs); selection.clear() }) },
            ),
        )
    }
    adding?.let { songs ->
        AddToPlaylistSheet(songs) {
            adding = null
            selection.clear()
        }
    }
}

/** One labelled action in the floating toolbar. */
@Composable
private fun ToolbarItem(icon: Painter, label: String, enabled: Boolean, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val ink = if (enabled) scheme.onSurface else scheme.onSurface.copy(alpha = 0.38f)
    Column(
        Modifier
            .widthIn(min = TOOL_DP.dp)
            .clip(RoundedCornerShape(16.dp))
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 6.dp, vertical = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(icon, contentDescription = null, tint = ink, modifier = Modifier.size(22.dp))
        Text(label, style = LocalEncoreType.current.caption.copy(fontSize = 11.sp), color = ink, maxLines = 1)
    }
}

/** "Add to My Favorites ▾": the left half adds there at once, the arrow opens the sheet. */
@Composable
private fun SplitAdd(name: String, enabled: Boolean, onAdd: () -> Unit, onMore: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val bg = if (enabled) scheme.primary else scheme.surfaceContainerHigh
    val ink = if (enabled) scheme.onPrimary else scheme.onSurfaceVariant
    Row(Modifier.shadow(6.dp, RoundedCornerShape(SPLIT_RADIUS_DP.dp)), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
        Surface(
            onClick = onAdd,
            enabled = enabled,
            shape = RoundedCornerShape(topStart = SPLIT_RADIUS_DP.dp, bottomStart = SPLIT_RADIUS_DP.dp, topEnd = 8.dp, bottomEnd = 8.dp),
            color = bg,
            contentColor = ink,
            modifier = Modifier.height(SPLIT_DP.dp).widthIn(max = SPLIT_MAX_DP.dp),
        ) {
            Row(Modifier.padding(start = 18.dp, end = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.playlistAdd(), contentDescription = null, modifier = Modifier.size(22.dp))
                Spacer(Modifier.width(Grid.base.dp))
                Text("Add to $name", style = LocalEncoreType.current.buttonLabel, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        Surface(
            onClick = onMore,
            enabled = enabled,
            shape = RoundedCornerShape(topStart = 8.dp, bottomStart = 8.dp, topEnd = SPLIT_RADIUS_DP.dp, bottomEnd = SPLIT_RADIUS_DP.dp),
            color = bg,
            contentColor = ink,
            modifier = Modifier.size(width = 48.dp, height = SPLIT_DP.dp).semantics { contentDescription = "Add to another playlist" },
        ) {
            Box(contentAlignment = Alignment.Center) { Icon(Icons.expandMore(), contentDescription = null) }
        }
    }
}

private const val BAR_MS = 220
private const val ROLL_MS = 180
private const val TOP_RADIUS_DP = 28
private const val TOOLBAR_RADIUS_DP = 30
private const val TOOLBAR_DP = 64
private const val TOOL_DP = 56
private const val FAB_DP = 64
private const val FAB_RADIUS_DP = 20
private const val SPLIT_DP = 52
private const val SPLIT_RADIUS_DP = 26
private const val SPLIT_MAX_DP = 280
private const val UNPICKED_ALPHA = 0.55f
private const val BADGE_FROM = 0.6f
private const val TURN_DEGREES = -60f
