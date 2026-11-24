package uk.gcartee.encore.app.ui.components

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
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.Song
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/*
 * Mock-up 46 (owner, 24 November 2026: "adding each one individually like that is very time
 * consuming and annoying"): pick as many songs as you like, then act on them all at once.
 * One selection for the whole app; it ends when you leave the page, press Back or tap ✕.
 */

@Stable
class SongSelection {
    private val picked = mutableStateMapOf<Long, Song>()
    var active by mutableStateOf(false)
        private set
    /** The songs of the page you're on, in its order, for Select all. */
    var available by mutableStateOf<List<Song>>(emptyList())
        internal set
    /** What this page adds to the bar's More: Remove from this playlist. */
    var removeHere by mutableStateOf<((List<Song>) -> Unit)?>(null)
        internal set
    val count: Int get() = picked.size
    val allPicked: Boolean get() = available.isNotEmpty() && available.all { it.id in picked }

    fun isPicked(id: Long): Boolean = id in picked
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
fun SelectableSongs(songs: List<Song>, removeHere: ((List<Song>) -> Unit)? = null) {
    val selection = LocalSongSelection.current ?: return
    DisposableEffect(selection, songs, removeHere) {
        selection.available = songs
        selection.removeHere = removeHere
        onDispose {
            if (selection.available === songs) {
                selection.available = emptyList()
                selection.removeHere = null
            }
        }
    }
}

/**
 * The leading mark while picking: an empty ring, or the theme colour with a tick that pops
 * in (Material 3 Expressive's quick spatial spring). Dragging down the marks picks a run of
 * songs, as Google Photos does; the run follows the finger and the first row says whether
 * it picks or unpicks.
 */
@Composable
internal fun SelectMark(selection: SongSelection, song: Song, size: Dp) {
    val scheme = MaterialTheme.colorScheme
    val on = selection.isPicked(song.id)
    val pop by animateFloatAsState(
        if (on) 1f else 0.86f,
        spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessMedium),
        label = "pick",
    )
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
            .graphicsLayer {
                scaleX = pop
                scaleY = pop
            }
            .clip(CircleShape)
            .background(if (on) scheme.primary else Color.Transparent)
            .border(if (on) 0.dp else 2.dp, scheme.onSurfaceVariant.copy(alpha = 0.7f), CircleShape)
            .semantics {
                role = Role.Checkbox
                stateDescription = if (on) "Selected" else "Not selected"
            },
        contentAlignment = Alignment.Center,
    ) {
        if (on) Icon(Icons.check(), contentDescription = null, tint = scheme.onPrimary, modifier = Modifier.size(size * 0.55f))
    }
}

/** Each row on screen while picking says where it is, for drag to pick a run. */
internal fun Modifier.selectableRow(selection: SongSelection?, song: Song): Modifier =
    if (selection == null || !selection.active) this
    else onGloballyPositioned { selection.rows[song.id] = song to it.boundsInRoot() }

/** The bar across the top while picking: ✕, how many, Select all. */
@Composable
fun SelectionTopBar(selection: SongSelection, modifier: Modifier = Modifier) {
    val type = LocalEncoreType.current
    AnimatedVisibility(
        selection.active,
        enter = fadeIn(tween(BAR_MS)) + slideInVertically(tween(BAR_MS, easing = FastOutSlowInEasing)) { -it / 2 },
        exit = fadeOut(tween(BAR_MS)) + slideOutVertically(tween(BAR_MS, easing = FastOutSlowInEasing)) { -it / 2 },
        modifier = modifier,
    ) {
        Surface(color = MaterialTheme.colorScheme.surfaceContainer, modifier = Modifier.fillMaxWidth()) {
            Row(
                Modifier.statusBarsPadding().height(64.dp).padding(horizontal = Grid.base.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = { selection.clear() }) { Icon(Icons.close(), contentDescription = "Stop selecting") }
                Spacer(Modifier.width(Grid.base.dp))
                val n = selection.count
                Text(
                    if (n == 0) "Select songs" else "$n selected",
                    style = type.sectionHeader,
                    modifier = Modifier.weight(1f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (selection.available.isNotEmpty()) {
                    TextButton(onClick = { if (selection.allPicked) selection.available.forEach { selection.set(it, false) } else selection.selectAll() }) {
                        Text(if (selection.allPicked) "Select none" else "Select all")
                    }
                }
            }
        }
    }
}

/**
 * The floating bar while picking (Material 3 Expressive's floating toolbar): Add to playlist
 * first and filled, then Play next, Add to queue and More (Like, and Remove on a playlist).
 */
@Composable
fun SelectionBar(selection: SongSelection, modifier: Modifier = Modifier) {
    val actions = LocalAppActions.current
    var adding by remember { mutableStateOf<List<Song>?>(null) }
    var more by remember { mutableStateOf(false) }
    AnimatedVisibility(
        selection.active,
        enter = fadeIn(tween(BAR_MS)) + slideInVertically(spring(dampingRatio = Spring.DampingRatioLowBouncy, stiffness = Spring.StiffnessMediumLow)) { it },
        exit = fadeOut(tween(BAR_MS)) + slideOutVertically(tween(BAR_MS, easing = FastOutSlowInEasing)) { it },
        modifier = modifier,
    ) {
        val none = selection.count == 0
        Surface(
            shape = RoundedCornerShape(BAR_RADIUS_DP.dp),
            color = MaterialTheme.colorScheme.surfaceContainerHighest,
            modifier = Modifier.padding(Grid.base.dp).shadow(6.dp, RoundedCornerShape(BAR_RADIUS_DP.dp)),
        ) {
            Row(Modifier.padding(6.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.CenterVertically) {
                Button(onClick = { adding = selection.songs() }, enabled = !none, modifier = Modifier.height(48.dp)) {
                    Icon(Icons.playlistAdd(), contentDescription = null, modifier = Modifier.size(20.dp))
                    Spacer(Modifier.width(Grid.base.dp))
                    Text("Add to playlist")
                }
                IconButton(onClick = { actions.playNext(selection.songs()); selection.clear() }, enabled = !none) {
                    Icon(Icons.playNext(), contentDescription = "Play next")
                }
                IconButton(onClick = { actions.addToQueue(selection.songs()); selection.clear() }, enabled = !none) {
                    Icon(Icons.queue(), contentDescription = "Add to queue")
                }
                IconButton(onClick = { more = true }, enabled = !none) {
                    Icon(Icons.more(), contentDescription = "More")
                }
            }
        }
    }
    if (more) {
        val like = Icons.favorite(false)
        val remove = Icons.remove()
        val songs = selection.songs()
        val here = selection.removeHere
        ActionSheet(
            onDismiss = { more = false },
            header = { Text("${songs.size} songs", style = LocalEncoreType.current.sheetTitle, modifier = Modifier.padding(horizontal = Grid.base.dp)) },
            quick = emptyList(),
            groups = listOfNotNull(
                listOf(
                    SheetAction("Like all", like) {
                        val n = songs.count { actions.setLiked(it, true) }
                        actions.toaster.show(if (n == 1) "Liked 1 song" else "Liked $n songs")
                        selection.clear()
                    },
                ),
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

private const val BAR_MS = 220
private const val BAR_RADIUS_DP = 30
