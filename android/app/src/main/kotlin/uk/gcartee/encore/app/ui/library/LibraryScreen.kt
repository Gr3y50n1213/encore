package uk.gcartee.encore.app.ui.library

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.clickable
import uk.gcartee.encore.app.ui.components.LIST_ART_RADIUS_DP
import uk.gcartee.encore.app.ui.components.LIST_ART_DP
import kotlin.math.roundToInt
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.text.NumberFormat
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.SyncUi
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.ui.components.AlbumTile
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.ListRow
import uk.gcartee.encore.app.ui.components.SongRow
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.AlbumSort
import uk.gcartee.encore.core.SongSort
import uk.gcartee.encore.core.StartedFrom
import uk.gcartee.encore.tokens.EncoreTokens.Grid

enum class Shelf(val label: String) { Albums("Albums"), Playlists("Playlists"), Artists("Artists"), Songs("Songs") }

/** Board 06's sort menu, limited to the orders the core offers today. */
private enum class SortChoice(val label: String, val album: AlbumSort?, val song: SongSort?) {
    RecentlyAdded("Recently added", AlbumSort.RECENTLY_ADDED, SongSort.RECENTLY_ADDED),
    Title("Title", AlbumSort.TITLE, SongSort.TITLE),
    Artist("Artist", AlbumSort.ARTIST, SongSort.ARTIST),
    Year("Year", AlbumSort.YEAR, null),
    Album("Album", null, SongSort.ALBUM),
}

/** Board 06: the last chip, sort and grid-or-list per chip, remembered per profile. */
private class LibraryPrefs(context: Context, profile: String) {
    private val prefs = context.getSharedPreferences("library-$profile", Context.MODE_PRIVATE)
    var shelf: Shelf
        get() = runCatching { Shelf.valueOf(prefs.getString("shelf", null)!!) }.getOrDefault(Shelf.Albums)
        set(v) = prefs.edit().putString("shelf", v.name).apply()
    fun sort(s: Shelf): SortChoice = runCatching { SortChoice.valueOf(prefs.getString("sort-${s.name}", null)!!) }.getOrDefault(SortChoice.RecentlyAdded)
    fun setSort(s: Shelf, v: SortChoice) = prefs.edit().putString("sort-${s.name}", v.name).apply()
    fun grid(s: Shelf): Boolean = prefs.getBoolean("grid-${s.name}", s == Shelf.Albums)
    fun setGrid(s: Shelf, v: Boolean) = prefs.edit().putBoolean("grid-${s.name}", v).apply()
}

/** Library opens on [shelf] the next time it shows (Search's Browse tiles). */
fun showLibraryShelf(context: Context, host: CoreHost, shelf: Shelf) {
    LibraryPrefs(context, host.core.currentProfile().orEmpty()).shelf = shelf
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LibraryScreen(host: CoreHost, onOpen: (Route) -> Unit, playingSongId: Long?, contentPadding: PaddingValues) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val context = LocalContext.current
    val profile = remember { host.core.currentProfile().orEmpty() }
    val prefs = remember(profile) { LibraryPrefs(context, profile) }
    var shelf by remember { mutableStateOf(prefs.shelf) }
    var sort by remember(shelf) { mutableStateOf(prefs.sort(shelf)) }
    var grid by remember(shelf) { mutableStateOf(prefs.grid(shelf)) }
    var sortOpen by remember { mutableStateOf(false) }
    val sync by host.sync.state.collectAsState()

    // The header (title, the shelf group, sort and grid or list) slides away as the list
    // scrolls down and returns as soon as it scrolls up, so browsing is just the albums.
    val headerState = rememberTopAppBarState()
    val collapse = TopAppBarDefaults.enterAlwaysScrollBehavior(headerState)
    var headerHeight by remember { mutableIntStateOf(0) }
    LaunchedEffect(headerHeight) { headerState.heightOffsetLimit = -headerHeight.toFloat() }
    val sortable = shelf == Shelf.Albums || shelf == Shelf.Songs
    Column(Modifier.fillMaxSize().statusBarsPadding().nestedScroll(collapse.nestedScrollConnection)) {
        Layout(
            content = {
                Column(Modifier.onSizeChanged { headerHeight = it.height }) {
                    Row(
                        Modifier.fillMaxWidth().padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = (Grid.base * 2).dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text("Library", style = type.pageTitle, color = MaterialTheme.colorScheme.onSurface, modifier = Modifier.weight(1f))
                        // Board 14: the avatar on Library too; its sheet holds Settings (board 17).
                        uk.gcartee.encore.app.ui.home.ProfileButton(host, uk.gcartee.encore.app.ui.components.LocalAppActions.current.openProfiles)
                    }
                    // Item 1: Albums, Playlists, Artists, Songs as one connected group, the
                    // chosen one round and filled (boards 06, 24), like every group in the app.
                    ChoiceGroup(
                        options = Shelf.entries.map { it to it.label },
                        selected = shelf,
                        onSelect = { s ->
                            shelf = s
                            prefs.shelf = s
                        },
                        modifier = Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
                    )
                    Row(
                        Modifier.fillMaxWidth().padding(start = Grid.base.dp, end = (Grid.base * 2).dp, bottom = Grid.base.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        // Item 2: the order is its own menu, named by what it is.
                        Box(Modifier.weight(1f)) {
                            if (sortable) {
                                androidx.compose.material3.TextButton(onClick = { sortOpen = true }) {
                                    Text(sort.label, style = type.groupLabel, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    Spacer(Modifier.size(4.dp))
                                    Icon(Icons.expandMore(), contentDescription = "Sort", tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(18.dp))
                                }
                                DropdownMenu(expanded = sortOpen, onDismissRequest = { sortOpen = false }) {
                                    SortChoice.entries.filter { if (shelf == Shelf.Songs) it.song != null else it.album != null }.forEach { choice ->
                                        DropdownMenuItem(
                                            text = { Text(choice.label, style = if (choice == sort) type.buttonLabelSelected else type.buttonLabel) },
                                            trailingIcon = if (choice == sort) ({ Icon(Icons.check(), contentDescription = null) }) else null,
                                            onClick = {
                                                sortOpen = false
                                                sort = choice
                                                prefs.setSort(shelf, choice)
                                            },
                                        )
                                    }
                                }
                            } else {
                                Text(shelf.label, style = type.groupLabel, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = Grid.base.dp))
                            }
                        }
                        if (shelf == Shelf.Albums) {
                            // Item 3: grid or list, a compact connected pair of icons.
                            Row(horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
                                ToggleButton(
                                    checked = grid,
                                    onCheckedChange = {
                                        if (!grid) haptics.perform(Haptic.ToggleOn)
                                        grid = true
                                        prefs.setGrid(shelf, true)
                                    },
                                    shapes = ButtonGroupDefaults.connectedLeadingButtonShapes(),
                                    colors = tonalToggle(),
                                    modifier = Modifier.height(VIEW_TOGGLE_DP.dp).width(VIEW_TOGGLE_W_DP.dp),
                                ) { Icon(Icons.grid(), contentDescription = "Grid", modifier = Modifier.size(20.dp)) }
                                ToggleButton(
                                    checked = !grid,
                                    onCheckedChange = {
                                        if (grid) haptics.perform(Haptic.ToggleOn)
                                        grid = false
                                        prefs.setGrid(shelf, false)
                                    },
                                    shapes = ButtonGroupDefaults.connectedTrailingButtonShapes(),
                                    colors = tonalToggle(),
                                    modifier = Modifier.height(VIEW_TOGGLE_DP.dp).width(VIEW_TOGGLE_W_DP.dp),
                                ) { Icon(Icons.list(), contentDescription = "List", modifier = Modifier.size(20.dp)) }
                            }
                        }
                    }
                }
            },
            modifier = Modifier.clipToBounds(),
        ) { measurables, constraints ->
            val header = measurables.first().measure(constraints.copy(minHeight = 0, maxHeight = Constraints.Infinity))
            val offset = headerState.heightOffset.roundToInt()
            layout(header.width, (header.height + offset).coerceAtLeast(0)) { header.place(0, offset) }
        }
        SyncRow(sync)
        PullToRefreshBox(isRefreshing = sync.running, onRefresh = { host.sync.syncNow() }, modifier = Modifier.weight(1f)) {
            when (shelf) {
                Shelf.Albums -> AlbumsList(host, sort.album ?: AlbumSort.RECENTLY_ADDED, grid, sync.generation, onOpen, contentPadding)
                Shelf.Songs -> SongsList(host, sort.song ?: SongSort.RECENTLY_ADDED, sync.generation, playingSongId, contentPadding)
                Shelf.Artists -> ArtistsList(host, sync.generation, onOpen, contentPadding)
                Shelf.Playlists -> PlaylistsList(host, sync.generation, onOpen, contentPadding)
            }
        }
    }
}

/** Board 06 states: "Syncing: top row with count and a thin progress line, content stays usable." */
@Composable
fun SyncRow(sync: SyncUi) {
    val type = LocalEncoreType.current
    if (!sync.running && sync.problem == null) return
    val nf = remember { NumberFormat.getIntegerInstance() }
    Column(Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = 4.dp)) {
        if (sync.running) {
            Text(
                if (sync.total > 0) "Indexing ${nf.format(sync.done)} of ${nf.format(sync.total)}" else "Syncing your library",
                style = type.caption,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.size(4.dp))
            if (sync.total > 0) {
                LinearProgressIndicator(progress = { (sync.done.toFloat() / sync.total).coerceIn(0f, 1f) }, modifier = Modifier.fillMaxWidth())
            } else {
                LinearProgressIndicator(Modifier.fillMaxWidth())
            }
        } else {
            Text(sync.problem.orEmpty(), style = type.caption, color = MaterialTheme.colorScheme.error)
        }
    }
}

@Composable
private fun AlbumsList(host: CoreHost, sort: AlbumSort, grid: Boolean, generation: Int, onOpen: (Route) -> Unit, padding: PaddingValues) {
    val pager = rememberPager(sort, generation) { o, l -> host.core.albums(sort, o.toUInt(), l.toUInt()) }
    if (pager.items.isEmpty()) {
        EmptyState("Nothing here yet", "Albums appear once your library has synced.")
        return
    }
    if (grid) {
        val state = rememberLazyGridState()
        LoadMore(state, pager)
        LazyVerticalGrid(
            columns = GridCells.Adaptive(TILE_MIN_DP.dp),
            state = state,
            contentPadding = PaddingValues(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = Grid.base.dp, bottom = padding.calculateBottomPadding() + (Grid.base * 2).dp),
            horizontalArrangement = Arrangement.spacedBy((Grid.base * 1.5f).dp),
            verticalArrangement = Arrangement.spacedBy((Grid.base * 1.5f).dp),
        ) {
            itemsIndexed(pager.items, key = { _, a -> a.id }) { _, album ->
                AlbumTile(album, onClick = { onOpen(Route.Album(album.id)) }, card = true)
            }
        }
    } else {
        val state = rememberLazyListState()
        LoadMore(state, pager)
        LazyColumn(state = state, contentPadding = PaddingValues(bottom = padding.calculateBottomPadding())) {
            items(pager.items, key = { it.id }) { album ->
                ListRow(album.title, album.artist, onClick = { onOpen(Route.Album(album.id)) }) {
                    uk.gcartee.encore.app.ui.components.Artwork(uk.gcartee.encore.core.ArtworkOf.ALBUM, album.id, LIST_ART_DP.dp, Modifier.size(LIST_ART_DP.dp), cornerRadius = LIST_ART_RADIUS_DP.dp)
                }
            }
        }
    }
}

@Composable
private fun SongsList(host: CoreHost, sort: SongSort, generation: Int, playingSongId: Long?, padding: PaddingValues) {
    val actions = uk.gcartee.encore.app.ui.components.LocalAppActions.current
    val pager = rememberPager(sort, generation) { o, l -> host.core.songs(sort, o.toUInt(), l.toUInt()) }
    if (pager.items.isEmpty()) {
        EmptyState("Nothing here yet", "Songs appear once your library has synced.")
        return
    }
    val state = rememberLazyListState()
    LoadMore(state, pager)
    LazyColumn(state = state, contentPadding = PaddingValues(bottom = padding.calculateBottomPadding())) {
        itemsIndexed(pager.items, key = { _, s -> s.id }) { i, song ->
            uk.gcartee.encore.app.ui.components.SongItem(song, playing = song.id == playingSongId, onClick = {
                // The loaded pages play from this song (StartedFrom Songs).
                actions.play(pager.items.map { it.id }, i, StartedFrom.SONGS, null, "Songs")
            })
        }
    }
}

@Composable
private fun ArtistsList(host: CoreHost, generation: Int, onOpen: (Route) -> Unit, padding: PaddingValues) {
    val type = LocalEncoreType.current
    val pager = rememberPager(generation) { o, l -> host.core.artists(o.toUInt(), l.toUInt()) }
    if (pager.items.isEmpty()) {
        EmptyState("Nothing here yet", "Artists appear once your library has synced.")
        return
    }
    // Artists as round portraits in a grid, the name under each.
    val state = rememberLazyGridState()
    LoadMore(state, pager)
    LazyVerticalGrid(
        columns = GridCells.Adaptive(ARTIST_MIN_DP.dp),
        state = state,
        contentPadding = PaddingValues(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = Grid.base.dp, bottom = padding.calculateBottomPadding() + (Grid.base * 2).dp),
        horizontalArrangement = Arrangement.spacedBy((Grid.base * 1.5f).dp),
        verticalArrangement = Arrangement.spacedBy((Grid.base * 2.5f).dp),
    ) {
        items(pager.items, key = { it.id }) { artist ->
            val n = artist.albumCount.toInt()
            Column(
                Modifier
                    .clip(androidx.compose.foundation.shape.RoundedCornerShape(ARTIST_PRESS_RADIUS_DP.dp))
                    .clickable { onOpen(Route.Artist(artist.id)) }
                    .padding(6.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                uk.gcartee.encore.app.ui.components.Artwork(uk.gcartee.encore.core.ArtworkOf.ARTIST, artist.id, ARTIST_ART_DP.dp, Modifier.fillMaxWidth().clip(androidx.compose.foundation.shape.CircleShape), cornerRadius = (ARTIST_ART_DP / 2).dp)
                Spacer(Modifier.size(Grid.base.dp))
                Text(artist.name, style = type.tileTitle.copy(fontSize = 14.sp, lineHeight = 18.sp), maxLines = 2, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
                Text(if (n == 1) "1 album" else "$n albums", style = type.caption.copy(fontSize = 12.sp), color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
            }
        }
    }
}

@Composable
private fun PlaylistsList(host: CoreHost, generation: Int, onOpen: (Route) -> Unit, padding: PaddingValues) {
    val pager = rememberPager(generation) { o, l -> host.core.playlists(o.toUInt(), l.toUInt()) }
    if (pager.items.isEmpty()) {
        EmptyState("Nothing here yet", "Playlists from your server appear here.")
        return
    }
    val state = rememberLazyListState()
    LoadMore(state, pager)
    LazyColumn(state = state, contentPadding = PaddingValues(bottom = padding.calculateBottomPadding())) {
        items(pager.items, key = { it.id }) { p ->
            val n = p.songCount.toInt()
            ListRow(p.name, if (n == 1) "1 song" else "$n songs", onClick = { onOpen(Route.Playlist(p.id)) }) {
                uk.gcartee.encore.app.ui.components.Artwork(uk.gcartee.encore.core.ArtworkOf.PLAYLIST, p.id, LIST_ART_DP.dp, Modifier.size(LIST_ART_DP.dp), cornerRadius = LIST_ART_RADIUS_DP.dp)
            }
        }
    }
}

/** Board 20: "Nothing here yet" with the reason. */
@Composable
fun EmptyState(title: String, reason: String) {
    val type = LocalEncoreType.current
    Column(
        Modifier.fillMaxSize().padding((Grid.base * 4).dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        // Board 20, item 2: one sentence on a tonal card, no cartoons.
        androidx.compose.material3.Surface(
            color = MaterialTheme.colorScheme.surfaceContainer,
            shape = androidx.compose.foundation.shape.RoundedCornerShape(EMPTY_RADIUS_DP.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(Modifier.padding((Grid.base * 2).dp)) {
                Text(title, style = type.sectionHeader.copy(fontSize = CHIP_TEXT_SP.sp), color = MaterialTheme.colorScheme.onSurface)
                Spacer(Modifier.size(4.dp))
                Text(reason, style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

private const val EMPTY_RADIUS_DP = 24

private const val TILE_MIN_DP = 150
private const val GROUP_TEXT_SP = 14
private const val ARTIST_MIN_DP = 104
private const val ARTIST_ART_DP = 112
private const val ARTIST_PRESS_RADIUS_DP = 20
private const val GROUP_SIDE_DP = 8
private const val GROUP_WEIGHT_BASE = 3f
private const val VIEW_TOGGLE_DP = 40
private const val VIEW_TOGGLE_W_DP = 52

/** Board 06, item 3: the selected one filled with the tonal colour, the other lighter. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun tonalToggle() = androidx.compose.material3.ToggleButtonDefaults.colors(
    containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
    contentColor = MaterialTheme.colorScheme.onSurfaceVariant,
    checkedContainerColor = MaterialTheme.colorScheme.inversePrimary,
    checkedContentColor = MaterialTheme.colorScheme.onPrimaryContainer,
)
/** Board 06: chip labels at 16 sp. */
internal const val CHIP_TEXT_SP = 16
internal const val CHIP_RADIUS_DP = 12

/** A chip's label, with the check the selected chip adds (board 35). */
@Composable
internal fun ChipLabel(label: String, on: Boolean) {
    val type = LocalEncoreType.current
    Row(Modifier.padding(start = if (on) 12.dp else 20.dp, end = 20.dp).fillMaxHeight(), verticalAlignment = Alignment.CenterVertically) {
        androidx.compose.animation.AnimatedVisibility(on) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.check(), contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
            }
        }
        Text(label, style = type.sectionHeader.copy(fontSize = CHIP_TEXT_SP.sp, fontWeight = if (on) androidx.compose.ui.text.font.FontWeight.Medium else androidx.compose.ui.text.font.FontWeight.Normal))
    }
}

/**
 * A one-of choice drawn as a connected button group (boards 06 and 24): outer corners
 * round, facing corners 10 dp, the chosen one fully round and filled. Library's shelves
 * and Search's scopes both use it, like the app's other groups.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun <T> ChoiceGroup(options: List<Pair<T, String>>, selected: T, onSelect: (T) -> Unit, modifier: Modifier = Modifier, enabled: (T) -> Boolean = { true }) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
        options.forEachIndexed { i, (value, label) ->
            val on = value == selected
            ToggleButton(
                checked = on,
                onCheckedChange = {
                    if (!on) {
                        haptics.perform(Haptic.Tick)
                        onSelect(value)
                    }
                },
                enabled = enabled(value),
                shapes = when (i) {
                    0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                    options.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                    else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                },
                colors = tonalToggle(),
                // Labels never cut: each button is as wide as its word wants, with slim sides,
                // so large text sizes still fit four in a row.
                contentPadding = PaddingValues(horizontal = GROUP_SIDE_DP.dp),
                modifier = Modifier.weight(label.length + GROUP_WEIGHT_BASE).semantics {
                    role = Role.Tab
                    this.selected = on
                },
            ) {
                Text(label, style = type.sectionHeader.copy(fontSize = GROUP_TEXT_SP.sp, fontWeight = if (on) androidx.compose.ui.text.font.FontWeight.SemiBold else androidx.compose.ui.text.font.FontWeight.Medium), maxLines = 1, softWrap = false)
            }
        }
    }
}