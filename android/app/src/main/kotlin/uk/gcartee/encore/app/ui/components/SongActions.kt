package uk.gcartee.encore.app.ui.components

import android.content.Context
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.style.TextOverflow
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
import androidx.compose.ui.semantics.stateDescription
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

    /** Board 35: a blocked action's haptic (EncoreRoot sets it); the toast says why. */
    var onRefused: () -> Unit = {}

    private fun refuse(e: Throwable) {
        if (e is uk.gcartee.encore.core.EncoreException.NotAllowed) onRefused()
        toaster.show(e.userMessage())
    }

    /** Board 08, item 4 and board 15, item 3: replaces the queue; Undo for 5 s if music was playing. */
    fun play(ids: List<Long>, index: Int, from: StartedFrom, contextId: String?, label: String?, shuffle: Boolean = false) {
        if (ids.isEmpty()) return
        val hadQueue = host.queue.value?.current != null
        // Offline, Play on an album, playlist or mix starts from its first song that can
        // play; with none, the tap is refused with the reason (board 06, item 5).
        var start = index.coerceIn(0, ids.lastIndex)
        val view = host.downloads.view.value
        if (!view.canPlay(ids[start])) {
            val playable = ids.indices.filter { view.canPlay(ids[it]) }
            if (playable.isEmpty()) {
                onRefused()
                toaster.show(NOT_DOWNLOADED)
                return
            }
            start = if (shuffle) playable.random() else playable.first()
        }
        runCatching {
            core.play(ids, start.toUInt(), from, contextId)
            core.setShuffle(shuffle)
        }.onFailure { refuse(it); return }
        host.playingFrom.value = label
        PlaybackHub.playCurrent(context)
        if (hadQueue) toaster.show("Replaced queue") { runCatching { core.undoQueue() }; PlaybackHub.playCurrent(context) }
    }

    fun playNext(songs: List<Song>) = queueEdit("Playing next") { core.playNext(songs.map { it.id }) }

    fun addToQueue(songs: List<Song>) = queueEdit("Added to Up Next") { core.addToUpNext(songs.map { it.id }) }

    private fun queueEdit(message: String, edit: () -> Unit) {
        runCatching(edit)
            .onSuccess { toaster.show(message) { runCatching { core.undoQueue() } } }
            .onFailure(::refuse)
    }

    // --- Downloads (Phase 8). ---

    /**
     * Keep it on the device (board 13, item 3): the core queues its songs and the job
     * fetches them; the first time, board 25's primer asks about notifications.
     */
    fun download(target: uk.gcartee.encore.core.DownloadTarget, title: String) {
        runCatching { core.download(target) }
            .onSuccess {
                uk.gcartee.encore.app.downloads.DownloadScheduler.start(context, byUser = true)
                host.downloads.refreshNow()
                val view = host.downloads.view.value
                val wifiOnly = runCatching { core.downloadSettings().wifiOnly }.getOrDefault(true)
                toaster.show(
                    when {
                        !view.online -> "$title downloads when you're back online"
                        wifiOnly && host.network() == uk.gcartee.encore.core.Network.CELLULAR -> "$title downloads on Wi-Fi"
                        else -> "Downloading $title"
                    },
                )
                askForNotificationsOnce()
            }
            .onFailure(::refuse)
    }

    /** Board 13: Remove download (after the confirm sheet where the caller asks). */
    fun removeDownload(target: uk.gcartee.encore.core.DownloadTarget, title: String) {
        runCatching { core.removeDownload(target) }
            .onSuccess {
                host.downloads.refreshNow()
                toaster.show("Removed $title from downloads")
            }
            .onFailure(::refuse)
    }

    /** Board 06, item 5: the ring pauses, the warning dot retries. */
    fun pauseDownload(target: uk.gcartee.encore.core.DownloadTarget, paused: Boolean) {
        runCatching { core.pauseDownload(target, paused) }
            .onSuccess {
                if (!paused) uk.gcartee.encore.app.downloads.DownloadScheduler.start(context, byUser = true)
                host.downloads.refreshNow()
            }
            .onFailure(::refuse)
    }

    fun retryDownloads() {
        runCatching { core.retryFailedDownloads() }
            .onSuccess {
                uk.gcartee.encore.app.downloads.DownloadScheduler.start(context, byUser = true)
                host.downloads.refreshNow()
            }
            .onFailure(::refuse)
    }

    /** One tap on the Download button, whatever its state. */
    fun tapDownload(target: uk.gcartee.encore.core.DownloadTarget, title: String, item: uk.gcartee.encore.core.DownloadedItem?, confirmRemove: () -> Unit) {
        when (item?.state) {
            null -> download(target, title)
            uk.gcartee.encore.core.DownloadState.SAVED -> confirmRemove()
            uk.gcartee.encore.core.DownloadState.FAILED -> retryDownloads()
            uk.gcartee.encore.core.DownloadState.PAUSED -> pauseDownload(target, paused = false)
            else -> pauseDownload(target, paused = true)
        }
    }

    /** Board 25: notifications are asked for once, in context, after the primer. */
    fun askForNotificationsOnce(forSleepTimer: Boolean = false) {
        if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.TIRAMISU) return
        val granted = androidx.core.content.ContextCompat.checkSelfPermission(context, android.Manifest.permission.POST_NOTIFICATIONS) ==
            android.content.pm.PackageManager.PERMISSION_GRANTED
        if (granted) return
        val prefs = context.getSharedPreferences("notifications", Context.MODE_PRIVATE)
        if (prefs.getBoolean("primer-shown", false)) return
        prefs.edit().putBoolean("primer-shown", true).apply()
        // The primer's words: downloads' (EncoreRoot) unless the sleep timer asked.
        uk.gcartee.encore.app.ui.player.PrimerCopy.sleepTimer.value = forSleepTimer
        host.notificationPrimer.value = true
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

    /**
     * Section 21: a radio from a song. The owner (8 November 2026) found it did nothing:
     * with the queue's Autoplay off a radio has nowhere to go, so starting one turns it on;
     * from the song already playing it carries on where it is instead of starting again;
     * the player follows the core even when the picks fail; and a toast says what happened.
     */
    fun startRadio(song: Song) {
        val before = runCatching { core.queue() }.getOrNull()
        val same = before?.current?.song?.id == song.id
        if (before != null && !before.autoplayEnabled) runCatching { core.setAutoplay(true) }
        val result = runCatching { core.startRadio(uk.gcartee.encore.core.RadioSeed.SONG, song.id) }
        val now = runCatching { core.queue().current?.uid }.getOrNull()
        if (now != null && now != before?.current?.uid) {
            host.playingFrom.value = "${song.title} radio"
            if (same) PlaybackHub.continueCurrent(context) else PlaybackHub.playCurrent(context)
        }
        result
            .onSuccess { q ->
                toaster.show(if (q.autoplay.isEmpty()) "Started ${song.title} radio. More picks come as you listen" else "Started ${song.title} radio")
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
 * Board 13's actions, in the app's one action sheet (ActionSheet). The song and a Like
 * toggle head it; Play next, Add to queue and Add to playlist are the quick tiles; then
 * radio and the ways to the album and artist (hidden where you already are), Why this
 * song for the app's picks, the page's extras, and Not interested apart and in red.
 */
@Composable
fun SongMenu(song: Song, expanded: Boolean, onDismiss: () -> Unit, place: MenuPlace = MenuPlace()) {
    val actions = LocalAppActions.current
    val haptics = rememberHaptics()
    var playlistSheet by remember { mutableStateOf(false) }
    var why by remember { mutableStateOf(false) }
    if (expanded) {
        var liked by remember(song.id) { mutableStateOf(actions.isLiked(song)) }
        val next = Icons.playNext()
        val queue = Icons.queue()
        val add = Icons.playlistAdd()
        val radio = Icons.radio()
        val album = Icons.album()
        val artist = Icons.artist()
        val info = Icons.info()
        val block = Icons.block()
        val remove = Icons.remove()
        val extras = place.extra.map { e -> Triple(e, e.icon?.invoke(), e.destructive) }
        // Board 13, item 3: Download becomes Remove download once the song is kept;
        // offline it dims with the reason.
        val downloads = LocalDownloads.current
        val saved = song.id in downloads.saved || song.id in downloads.pending
        val downloadIcon = if (saved) Icons.delete() else Icons.download()
        val target = uk.gcartee.encore.app.core.songTarget(song.id)
        ActionSheet(
            onDismiss = onDismiss,
            header = {
                SongSheetHeader(song, liked) {
                    val want = !liked
                    if (actions.setLiked(song, want)) {
                        liked = want
                        haptics.perform(if (want) Haptic.ToggleOn else Haptic.ToggleOff)
                    }
                }
            },
            quick = listOf(
                SheetAction("Play next", next) { actions.playNext(listOf(song)) },
                SheetAction("Add to queue", queue) { actions.addToQueue(listOf(song)) },
                SheetAction("Add to playlist", add) { playlistSheet = true },
            ),
            groups = listOf(
                listOfNotNull(
                    if (saved) {
                        SheetAction("Remove download", downloadIcon) { actions.removeDownload(target, song.title) }
                    } else {
                        SheetAction("Download", downloadIcon, reason = if (downloads.online) null else "Download when you're back online") { actions.download(target, song.title) }
                    },
                    SheetAction("Start radio", radio, reason = if (downloads.canPlay(song.id)) null else NOT_DOWNLOADED) { actions.startRadio(song) },
                    if (song.albumId != null && song.albumId != place.onAlbum) SheetAction("Go to album", album) { actions.goToAlbum(song) } else null,
                    if (!place.onArtist) SheetAction("Go to artist", artist) { actions.goToArtist(song) } else null,
                    if (place.chosenBy == ChosenBy.APP) SheetAction("Why this song", info) { why = true } else null,
                ) + extras.filter { !it.third }.map { (e, icon, _) -> SheetAction(e.label, icon) { e.action() } },
                extras.filter { it.third }.map { (e, icon, _) -> SheetAction(e.label, icon ?: remove, destructive = true) { e.action() } } +
                    SheetAction("Not interested", block, destructive = true) { actions.notInterested(song) },
            ),
        )
    }
    if (playlistSheet) AddToPlaylistSheet(listOf(song)) { playlistSheet = false }
    if (why) WhyThisSongSheet(song) { why = false }
}

/** The sheet's head: the cover, title and artist, and Like (its heart pops, board 35). */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun SongSheetHeader(song: Song, liked: Boolean, onLike: () -> Unit) {
    val type = LocalEncoreType.current
    val heart = rememberPop(liked)
    Row(verticalAlignment = Alignment.CenterVertically) {
        Artwork(uk.gcartee.encore.core.ArtworkOf.SONG, song.id, HEADER_ART_DP.dp, Modifier.size(HEADER_ART_DP.dp), cornerRadius = HEADER_ART_RADIUS_DP.dp)
        Spacer(Modifier.width((Grid.base * 2).dp))
        Column(Modifier.weight(1f)) {
            Text(song.title, style = type.sectionHeader, maxLines = 2, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
            Text(
                listOfNotNull(song.artist, song.albumTitle).joinToString(" · "),
                style = type.body,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
            )
        }
        Spacer(Modifier.width(Grid.base.dp))
        androidx.compose.material3.FilledTonalIconToggleButton(
            checked = liked,
            onCheckedChange = { onLike() },
            shapes = androidx.compose.material3.IconButtonDefaults.toggleableShapes(
                shape = RoundedCornerShape(50),
                pressedShape = RoundedCornerShape(12.dp),
                checkedShape = RoundedCornerShape(16.dp),
            ),
            modifier = Modifier.size(48.dp).semantics { stateDescription = if (liked) "Liked" else "Not liked" },
        ) {
            Icon(Icons.favorite(liked), contentDescription = "Like", modifier = Modifier.graphicsLayer { scaleX = heart; scaleY = heart })
        }
    }
}

private const val HEADER_ART_DP = 56
private const val HEADER_ART_RADIUS_DP = 14
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
    val scheme = MaterialTheme.colorScheme
    val rowTitle = type.listRowTitle.copy(fontSize = 16.sp)
    val nameField = remember { FocusRequester() }
    // The app's sheet look (ActionSheet): one rounded tonal group, New playlist first, a
    // ticked playlist lit. Over Now Playing it comes in the album's colours.
    ModalBottomSheet(
        onDismissRequest = onDone,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = scheme.surfaceContainerLow,
    ) {
        Column(Modifier.navigationBarsPadding().padding(horizontal = (Grid.base * 2).dp)) {
            Text(if (songs.size == 1) "Add “${songs[0].title}” to" else "Add ${songs.size} songs to", style = type.sheetTitle, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = Grid.base.dp))
            Spacer(Modifier.height((Grid.base * 2).dp))
            LazyColumn(
                Modifier.heightIn(max = 420.dp).clip(RoundedCornerShape(SHEET_LIST_RADIUS_DP.dp)),
                verticalArrangement = Arrangement.spacedBy(SHEET_ROW_GAP_DP.dp),
            ) {
                item {
                    // Tapped, the row becomes the name's field, with the keyboard up.
                    GroupRow(first = true, last = playlists.isEmpty(), onClick = if (creating) null else ({ creating = true })) {
                        IconDisc(Icons.add())
                        Spacer(Modifier.width((Grid.base * 2).dp))
                        if (creating) {
                            OutlinedTextField(
                                newName,
                                { newName = it },
                                placeholder = { Text("Playlist name") },
                                singleLine = true,
                                modifier = Modifier.weight(1f).focusRequester(nameField),
                            )
                            LaunchedEffect(Unit) { runCatching { nameField.requestFocus() } }
                        } else {
                            Text("New playlist", style = rowTitle)
                        }
                    }
                }
                itemsIndexed(playlists, key = { _, p -> p.id }) { i, p ->
                    val on = p.id in ticked
                    GroupRow(
                        first = false,
                        last = i == playlists.lastIndex,
                        color = if (on) scheme.secondaryContainer else scheme.surfaceContainerHigh,
                        contentColor = if (on) scheme.onSecondaryContainer else scheme.onSurface,
                        role = Role.Checkbox,
                        onClick = { if (on) ticked.remove(p.id) else ticked.add(p.id) },
                    ) {
                        IconDisc(Icons.queue())
                        Spacer(Modifier.width((Grid.base * 2).dp))
                        Text(p.name, style = rowTitle, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                        Spacer(Modifier.width(Grid.base.dp))
                        Checkbox(checked = on, onCheckedChange = null)
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
    val scheme = MaterialTheme.colorScheme
    val rowTitle = type.listRowTitle.copy(fontSize = 16.sp)
    // The app's sheet look (ActionSheet): each reason a row of one rounded tonal group with
    // its own icon, then the songs it sounds like with their covers.
    ModalBottomSheet(onDismissRequest = onDone, containerColor = scheme.surfaceContainerLow) {
        Column(
            Modifier
                .navigationBarsPadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = (Grid.base * 2).dp)
                .padding(bottom = (Grid.base * 3).dp),
        ) {
            Text("Why this song", style = type.sheetTitle, modifier = Modifier.padding(horizontal = Grid.base.dp))
            Text(
                "${song.title} · ${song.artist}",
                style = type.body,
                color = scheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(horizontal = Grid.base.dp),
            )
            Spacer(Modifier.height((Grid.base * 2).dp))
            val reasons = why?.reasons.orEmpty()
            Column(verticalArrangement = Arrangement.spacedBy(SHEET_ROW_GAP_DP.dp)) {
                if (reasons.isEmpty()) {
                    GroupRow(first = true, last = true) {
                        IconDisc(Icons.spark())
                        Spacer(Modifier.width((Grid.base * 2).dp))
                        Text("It fits what you've been playing.", style = rowTitle, modifier = Modifier.weight(1f))
                    }
                }
                reasons.forEachIndexed { i, r ->
                    GroupRow(first = i == 0, last = i == reasons.lastIndex) {
                        IconDisc(reasonIcon(r))
                        Spacer(Modifier.width((Grid.base * 2).dp))
                        Text(describe(r), style = rowTitle, modifier = Modifier.weight(1f))
                    }
                }
            }
            why?.soundsLike?.takeIf { it.isNotEmpty() }?.let { like ->
                Spacer(Modifier.height((Grid.base * 2.5f).dp))
                Text(
                    "Sounds like",
                    style = type.groupLabel,
                    color = scheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = Grid.base.dp, bottom = Grid.base.dp),
                )
                Column(verticalArrangement = Arrangement.spacedBy(SHEET_ROW_GAP_DP.dp)) {
                    like.forEachIndexed { i, s ->
                        GroupRow(first = i == 0, last = i == like.lastIndex) {
                            Artwork(uk.gcartee.encore.core.ArtworkOf.SONG, s.id, LIKE_ART_DP.dp, Modifier.size(LIKE_ART_DP.dp), cornerRadius = LIKE_ART_RADIUS_DP.dp)
                            Spacer(Modifier.width((Grid.base * 2).dp))
                            Column(Modifier.weight(1f)) {
                                Text(s.title, style = rowTitle, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text(s.artist, style = type.body, color = scheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                    }
                }
            }
        }
    }
}

/** The picture beside a reason: what kind of reason it is, at a glance. */
@Composable
private fun reasonIcon(r: PickReason): Painter = when (r.kind) {
    ReasonKind.FINISHED_OFTEN -> Icons.check()
    ReasonKind.PLAYED_TOGETHER -> Icons.queue()
    ReasonKind.TIME_OF_DAY, ReasonKind.REDISCOVER -> Icons.history()
    ReasonKind.YOU_LIKED_IT -> Icons.favorite(true)
    ReasonKind.SAME_ARTIST -> Icons.artist()
    ReasonKind.SAME_GENRE -> Icons.album()
    else -> Icons.spark()
}

private const val SHEET_LIST_RADIUS_DP = 24
private const val LIKE_ART_DP = 40
private const val LIKE_ART_RADIUS_DP = 10

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
