package uk.gcartee.encore.app.ui.components

import android.content.Context
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.unit.sp
import android.os.Build
import android.view.ViewGroup
import android.view.WindowManager
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Surface
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.layout.positionOnScreen
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import kotlin.math.roundToInt
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.ChosenBy
import uk.gcartee.encore.core.DropTarget
import uk.gcartee.encore.core.DropTargetKind
import uk.gcartee.encore.core.NotInterestedReason
import uk.gcartee.encore.core.PickReason
import uk.gcartee.encore.core.ReasonKind
import uk.gcartee.encore.core.Song
import uk.gcartee.encore.core.StartedFrom
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/**
 * What every song row, tile and menu can do, done once through the core: queue edits
 * with an Undo toast (boards 11, 13, 42), likes, playlists, navigation and Not interested.
 */
@Stable
class AppActions(
    val host: CoreHost,
    val toaster: Toaster,
    private val context: Context,
    val open: (Route) -> Unit,
    val openNowPlaying: () -> Unit,
    /** The profile sheet (board 14), which also leads to Settings (board 17). */
    val openProfiles: () -> Unit = {},
) {
    private val core get() = host.core

    /** Board 08, item 4 and board 15, item 3: replaces the queue; Undo for 5 s if music was playing. */
    fun play(ids: List<Long>, index: Int, from: StartedFrom, contextId: String?, label: String?, shuffle: Boolean = false) {
        if (ids.isEmpty()) return
        val hadQueue = host.queue.value?.current != null
        runCatching {
            core.play(ids, index.coerceIn(0, ids.lastIndex).toUInt(), from, contextId)
            core.setShuffle(shuffle)
        }.onFailure { toaster.show(it.userMessage()); return }
        host.playingFrom.value = label
        PlaybackHub.playCurrent(context)
        if (hadQueue) toaster.show("Replaced queue") { runCatching { core.undoQueue() }; PlaybackHub.playCurrent(context) }
    }

    fun playNext(songs: List<Song>) = queueEdit("Playing next") { core.playNext(songs.map { it.id }) }

    fun addToQueue(songs: List<Song>) = queueEdit("Added to Up Next") { core.addToUpNext(songs.map { it.id }) }

    private fun queueEdit(message: String, edit: () -> Unit) {
        runCatching(edit)
            .onSuccess { toaster.show(message) { runCatching { core.undoQueue() } } }
            .onFailure { toaster.show(it.userMessage()) }
    }

    fun isLiked(song: Song): Boolean = runCatching { core.areLiked(listOf(song.id)).first() }.getOrDefault(false)

    fun setLiked(song: Song, liked: Boolean): Boolean = runCatching {
        if (liked) core.like(listOf(song.id)) else core.unlike(listOf(song.id))
    }.fold(
        onSuccess = { true },
        onFailure = { toaster.show(it.userMessage()); false },
    )

    /** Board 13, item 5: hides it from suggestions and trains the recommender; Undo for 5 s. */
    fun notInterested(song: Song) {
        runCatching { core.notInterestedSong(song.id, NotInterestedReason.NOT_MY_TASTE) }
            .onSuccess { item -> toaster.show("You'll hear less like this") { runCatching { core.removeNotInterested(item) } } }
            .onFailure { toaster.show(it.userMessage()) }
    }

    fun goToAlbum(song: Song) {
        song.albumId?.let { open(Route.Album(it)) }
    }

    /** The song's artist page, found by name among the library's artists. */
    fun goToArtist(song: Song) {
        val name = song.albumArtist.ifBlank { song.artist }
        val id = runCatching { core.search(name).artists.firstOrNull { it.name.equals(name, ignoreCase = true) }?.id }.getOrNull()
        if (id != null) open(Route.Artist(id)) else toaster.show("$name isn't in your library")
    }

    fun startRadio(song: Song) {
        runCatching { core.startRadio(uk.gcartee.encore.core.RadioSeed.SONG, song.id) }
            .onSuccess {
                host.playingFrom.value = "${song.title} radio"
                PlaybackHub.playCurrent(context)
            }
            .onFailure { toaster.show(it.userMessage()) }
    }
}

val LocalAppActions = staticCompositionLocalOf<AppActions> { error("No AppActions provided") }

/** Where a menu is opened from: items that point to the page you are on are hidden (board 13, item 4). */
data class MenuPlace(
    val onAlbum: Long? = null,
    val onArtist: Boolean = false,
    val chosenBy: ChosenBy = ChosenBy.USER,
    /** Extra rows a page adds, such as "Not in this mix" or "Remove from queue". */
    val extra: List<MenuExtra> = emptyList(),
)

data class MenuExtra(val label: String, val destructive: Boolean, val icon: (@Composable () -> Painter)? = null, val action: () -> Unit)

/**
 * Board 13. Android's popup scales from the row. Groups: queue, library, navigate, then the
 * destructive group apart and in red. Lifting the finger without sliding picks nothing,
 * and the first item is never pre-selected (DropdownMenu does neither).
 */
@Composable
fun SongMenu(
    song: Song,
    expanded: Boolean,
    onDismiss: () -> Unit,
    place: MenuPlace = MenuPlace(),
    anchor: Rect? = null,
    lifted: (@Composable () -> Unit)? = null,
) {
    val actions = LocalAppActions.current
    val haptics = rememberHaptics()
    var playlistSheet by remember { mutableStateOf(false) }
    var why by remember { mutableStateOf(false) }
    val liked = remember(song.id, expanded) { if (expanded) actions.isLiked(song) else false }
    // The render: one rounded card, text only, groups split by a band of the darker tone,
    // sentence case.
    val items: @Composable () -> Unit = {
        MenuItem("Play next") { onDismiss(); actions.playNext(listOf(song)) }
        MenuItem("Add to queue") { onDismiss(); actions.addToQueue(listOf(song)) }
        MenuItem("Add to playlist") { onDismiss(); playlistSheet = true }
        GroupGap()
        MenuItem(if (liked) "Unlike" else "Like") {
            onDismiss()
            if (actions.setLiked(song, !liked)) haptics.perform(if (liked) Haptic.ToggleOff else Haptic.ToggleOn)
        }
        MenuItem("Start radio") { onDismiss(); actions.startRadio(song) }
        GroupGap()
        if (song.albumId != null && song.albumId != place.onAlbum) MenuItem("Go to album") { onDismiss(); actions.goToAlbum(song) }
        if (!place.onArtist) MenuItem("Go to artist") { onDismiss(); actions.goToArtist(song) }
        if (place.chosenBy == ChosenBy.APP) MenuItem("Why this song") { onDismiss(); why = true }
        place.extra.filter { !it.destructive }.forEach { e -> MenuItem(e.label) { onDismiss(); e.action() } }
        GroupGap()
        place.extra.filter { it.destructive }.forEach { e -> MenuItem(e.label, destructive = true) { onDismiss(); e.action() } }
        MenuItem("Not interested", destructive = true) { onDismiss(); actions.notInterested(song) }
    }
    if (anchor != null && lifted != null) {
        // Item 1: from a row, the row lifts and the menu grows out of it.
        if (expanded) LiftedMenu(anchor, onDismiss, lifted, items)
    } else {
        DropdownMenu(
            expanded = expanded,
            onDismissRequest = onDismiss,
            shape = RoundedCornerShape(MENU_RADIUS_DP.dp),
            containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        ) { items() }
    }
    if (playlistSheet) AddToPlaylistSheet(listOf(song)) { playlistSheet = false }
    if (why) WhyThisSongSheet(song) { why = false }
}

@Composable
private fun MenuItem(label: String, destructive: Boolean = false, onClick: () -> Unit) {
    val colour = if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface
    DropdownMenuItem(
        text = { Text(label, style = LocalEncoreType.current.listRowTitle.copy(fontSize = MENU_TEXT_SP.sp), color = colour) },
        onClick = onClick,
    )
}

/** Board 13: the gap between groups, drawn as a band of the darker tone. */
@Composable
private fun GroupGap() {
    Box(Modifier.fillMaxWidth().padding(vertical = 4.dp).height(MENU_GAP_DP.dp).background(MaterialTheme.colorScheme.surfaceContainerHighest))
}

/**
 * Board 13, item 1: the pressed row lifts to 1.03 with a shadow in the tonal highlight,
 * everything else dims (and blurs from Android 12, where the window can), and the menu
 * grows out of the row on the expressive spring, below it or above it if there is no room.
 * A tap outside closes it with no change.
 */
@Composable
private fun LiftedMenu(anchor: Rect, onDismiss: () -> Unit, lifted: @Composable () -> Unit, items: @Composable () -> Unit) {
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        val view = LocalView.current
        DisposableEffect(view) {
            (view.parent as? DialogWindowProvider)?.window?.let { w ->
                w.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
                w.addFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
                w.setDimAmount(MENU_DIM)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                    w.addFlags(WindowManager.LayoutParams.FLAG_BLUR_BEHIND)
                    w.attributes = w.attributes.apply { blurBehindRadius = MENU_BLUR_PX }
                }
            }
            onDispose { }
        }
        val still = reduceMotion()
        val appear = remember { Animatable(if (still) 1f else 0f) }
        LaunchedEffect(Unit) { appear.animateTo(1f, spring(dampingRatio = APPEAR_DAMPING, stiffness = Spring.StiffnessMediumLow)) }
        val density = LocalDensity.current
        var origin by remember { mutableStateOf(Offset.Zero) }
        var root by remember { mutableStateOf(IntSize.Zero) }
        var card by remember { mutableStateOf(IntSize.Zero) }
        Box(
            Modifier
                .fillMaxSize()
                .onGloballyPositioned {
                    origin = it.positionOnScreen()
                    root = it.size
                }
                .pointerInput(Unit) { detectTapGestures { onDismiss() } },
        ) {
            val left = anchor.left - origin.x
            val top = anchor.top - origin.y
            Box(
                Modifier
                    .offset { IntOffset(left.roundToInt(), top.roundToInt()) }
                    .size(with(density) { anchor.width.toDp() }, with(density) { anchor.height.toDp() })
                    .graphicsLayer {
                        val s = 1f + LIFT_SCALE * appear.value
                        scaleX = s
                        scaleY = s
                    }
                    .shadow((LIFT_SHADOW_DP * appear.value).dp, RoundedCornerShape(LIFT_RADIUS_DP.dp))
                    .background(MaterialTheme.colorScheme.secondaryContainer, RoundedCornerShape(LIFT_RADIUS_DP.dp)),
            ) { lifted() }
            val gap = with(density) { MENU_GAP_FROM_ROW_DP.dp.toPx() }
            val edge = with(density) { MENU_EDGE_DP.dp.toPx() }
            val below = top + anchor.height + gap + card.height <= root.height - edge
            val cardTop = (if (below) top + anchor.height + gap else top - gap - card.height)
                .coerceIn(edge, (root.height - edge - card.height).coerceAtLeast(edge))
            val cardLeft = (left + edge).coerceAtMost((root.width - edge - card.width).coerceAtLeast(edge))
            Surface(
                shape = RoundedCornerShape(MENU_RADIUS_DP.dp),
                color = MaterialTheme.colorScheme.surfaceContainerHigh,
                shadowElevation = MENU_SHADOW_DP.dp,
                modifier = Modifier
                    .offset { IntOffset(cardLeft.roundToInt(), cardTop.roundToInt()) }
                    .onSizeChanged { card = it }
                    .graphicsLayer {
                        transformOrigin = TransformOrigin(0.15f, if (below) 0f else 1f)
                        val s = MENU_START_SCALE + (1f - MENU_START_SCALE) * appear.value
                        scaleX = s
                        scaleY = s
                        alpha = appear.value.coerceIn(0f, 1f)
                    },
            ) {
                Column(Modifier.width(IntrinsicSize.Max).padding(vertical = Grid.base.dp)) { items() }
            }
        }
    }
}

private const val MENU_RADIUS_DP = 16
private const val MENU_DIM = 0.32f
private const val MENU_BLUR_PX = 40
private const val APPEAR_DAMPING = 0.8f
private const val LIFT_SCALE = 0.03f
private const val LIFT_SHADOW_DP = 8
private const val LIFT_RADIUS_DP = 20
private const val MENU_GAP_FROM_ROW_DP = 8
private const val MENU_EDGE_DP = 16
private const val MENU_SHADOW_DP = 6
private const val MENU_START_SCALE = 0.85f
private const val MENU_GAP_DP = 6
private const val MENU_TEXT_SP = 15

/** Board 13, item 2: several playlists can be ticked at once, plus a New Playlist row. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AddToPlaylistSheet(songs: List<Song>, onDone: () -> Unit) {
    val actions = LocalAppActions.current
    val core = actions.host.core
    val type = LocalEncoreType.current
    val playlists = remember { runCatching { core.playlists(0u, 500u) }.getOrDefault(emptyList()) }
    val ticked = remember { mutableStateListOf<Long>() }
    var newName by remember { mutableStateOf("") }
    var creating by remember { mutableStateOf(false) }
    ModalBottomSheet(onDismissRequest = onDone) {
        Column(Modifier.navigationBarsPadding().padding(horizontal = (Grid.base * 2).dp)) {
            Text(if (songs.size == 1) "Add “${songs[0].title}” to" else "Add ${songs.size} songs to", style = type.sheetTitle)
            Spacer(Modifier.heightIn(min = Grid.base.dp))
            LazyColumn(Modifier.heightIn(max = 420.dp)) {
                item {
                    Row(
                        Modifier.fillMaxWidth().heightIn(min = 56.dp).clickable { creating = true },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(Icons.add(), contentDescription = null)
                        Spacer(Modifier.width((Grid.base * 2).dp))
                        Text("New playlist", style = type.listRowTitle)
                    }
                    if (creating) {
                        OutlinedTextField(newName, { newName = it }, placeholder = { Text("Playlist name") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    }
                }
                items(playlists, key = { it.id }) { p ->
                    val on = p.id in ticked
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 56.dp)
                            .clickable { if (on) ticked.remove(p.id) else ticked.add(p.id) }
                            .semantics { role = Role.Checkbox },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Checkbox(checked = on, onCheckedChange = null)
                        Spacer(Modifier.width((Grid.base * 2).dp))
                        Text(p.name, style = type.listRowTitle, modifier = Modifier.weight(1f))
                    }
                }
            }
            Row(Modifier.fillMaxWidth().padding(vertical = (Grid.base * 2).dp)) {
                TextButton(onClick = onDone) { Text("Cancel") }
                Spacer(Modifier.weight(1f))
                Button(
                    enabled = ticked.isNotEmpty() || (creating && newName.isNotBlank()),
                    onClick = {
                        val ids = songs.map { it.id }
                        val edits = mutableListOf<String>()
                        var added = 0u
                        var already = 0u
                        val failure = runCatching {
                            ticked.forEach { pid ->
                                val r = core.addTo(DropTarget(DropTargetKind.PLAYLIST, pid), ids, null, false)
                                r.editId?.let(edits::add)
                                added += r.added
                                already += r.alreadyThere
                            }
                            if (creating && newName.isNotBlank()) {
                                val r = core.addTo(DropTarget(DropTargetKind.NEW_PLAYLIST, null), ids, null, false)
                                r.editId?.let(edits::add)
                                added += r.added
                                r.playlistId?.let { core.renamePlaylist(it, newName.trim()) }
                            }
                        }.exceptionOrNull()
                        onDone()
                        if (failure != null) {
                            actions.toaster.show(failure.userMessage())
                        } else {
                            // The toast of section 26: "Added {added} · {already_there} already there".
                            val msg = if (already > 0u) "Added $added · $already already there" else "Added $added"
                            actions.toaster.show(msg) { edits.reversed().forEach { runCatching { core.undo(it) } } }
                        }
                    },
                ) { Text("Done") }
            }
        }
    }
}

/** Board 38: one or two reasons in plain words, and the songs it sounds like. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WhyThisSongSheet(song: Song, onDone: () -> Unit) {
    val actions = LocalAppActions.current
    val type = LocalEncoreType.current
    val why = remember(song.id) { runCatching { actions.host.core.whyThisSong(song.id) }.getOrNull() }
    ModalBottomSheet(onDismissRequest = onDone) {
        Column(Modifier.navigationBarsPadding().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp)) {
            Text("Why this song", style = type.sheetTitle)
            Spacer(Modifier.heightIn(min = (Grid.base * 2).dp))
            val reasons = why?.reasons.orEmpty()
            if (reasons.isEmpty()) Text("It fits what you've been playing.", style = type.body)
            reasons.forEach { r -> Text("• " + describe(r), style = type.listRowTitle, modifier = Modifier.padding(vertical = 4.dp)) }
            why?.soundsLike?.takeIf { it.isNotEmpty() }?.let { like ->
                Spacer(Modifier.heightIn(min = (Grid.base * 2).dp))
                Text("Sounds like", style = type.groupLabel, color = MaterialTheme.colorScheme.onSurfaceVariant)
                like.forEach { s -> Text("${s.title} · ${s.artist}", style = type.body, modifier = Modifier.padding(vertical = 4.dp)) }
            }
            Spacer(Modifier.heightIn(min = (Grid.base * 3).dp))
        }
    }
}

/** A reason in words, from its kind and fields (CORE-API "Smart picks"). */
fun describe(r: PickReason): String = when (r.kind) {
    ReasonKind.FINISHED_OFTEN -> if (r.count != null && r.of != null) "You finished it ${r.count} of ${r.of} times" else "You usually play it to the end"
    ReasonKind.PLAYED_TOGETHER -> r.song?.let { "You often play it with “${it.title}”" } ?: "You often play these together"
    ReasonKind.TIME_OF_DAY -> r.timeOfDay?.let { "You play it in the ${it.name.lowercase()}" } ?: "You play it at this time of day"
    ReasonKind.YOU_LIKED_IT -> "You liked it"
    ReasonKind.REDISCOVER -> r.days?.let { "You loved it and haven't played it in $it days" } ?: "Something you loved a while ago"
    ReasonKind.SAME_ARTIST -> r.artist?.let { "More from $it" } ?: "More from this artist"
    ReasonKind.SAME_GENRE -> r.genre?.let { "Like the other $it you play" } ?: "Like other songs you play"
    else -> when (r.kind.name) {
        "PLAYED_ELSEWHERE" -> "You played it on another device"
        "NEW_TO_YOU" -> "New to you"
        "RECENTLY_ADDED" -> "Recently added to your library"
        else -> "It fits what you've been playing"
    }
}
