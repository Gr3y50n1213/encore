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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
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

    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        Row(
            Modifier.fillMaxWidth().padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = (Grid.base * 2).dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("Library", style = type.pageTitle, color = MaterialTheme.colorScheme.onSurface, modifier = Modifier.weight(1f))
            // Board 17: Settings sits in the Library tab on phones.
            FilledTonalIconButton(onClick = { onOpen(Route.Settings()) }) { Icon(Icons.settings(), contentDescription = "Settings") }
            Spacer(Modifier.size(Grid.base.dp))
            val sortable = shelf == Shelf.Albums || shelf == Shelf.Songs
            Box {
                // Item 2: Sort.
                FilledTonalIconButton(onClick = { sortOpen = true }, enabled = sortable) {
                    Icon(Icons.sort(), contentDescription = "Sort, ${sort.label}")
                }
                DropdownMenu(expanded = sortOpen, onDismissRequest = { sortOpen = false }) {
                    SortChoice.entries.filter { if (shelf == Shelf.Songs) it.song != null else it.album != null }.forEach { choice ->
                        DropdownMenuItem(
                            text = { Text(choice.label, style = if (choice == sort) type.buttonLabelSelected else type.buttonLabel) },
                            onClick = {
                                sortOpen = false
                                sort = choice
                                prefs.setSort(shelf, choice)
                            },
                        )
                    }
                }
            }
        }
        // Item 1: filter chips; one is always active.
        LazyRow(
            contentPadding = PaddingValues(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
            horizontalArrangement = Arrangement.spacedBy(Grid.base.dp),
        ) {
            items(Shelf.entries) { s ->
                FilterChip(
                    selected = s == shelf,
                    onClick = {
                        if (s != shelf) {
                            haptics.perform(Haptic.Tick)
                            shelf = s
                            prefs.shelf = s
                        }
                    },
                    label = { Text(s.label, style = type.sectionHeader) },
                    modifier = Modifier.semantics { role = Role.Tab },
                )
            }
        }
        SyncRow(sync)
        Row(
            Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                if (shelf == Shelf.Albums || shelf == Shelf.Songs) sort.label else shelf.label,
                style = type.groupLabel,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            if (shelf == Shelf.Albums) {
                // Item 3: grid or list, a connected group of two.
                Row(horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
                    ToggleButton(
                        checked = grid,
                        onCheckedChange = {
                            if (!grid) haptics.perform(Haptic.ToggleOn)
                            grid = true
                            prefs.setGrid(shelf, true)
                        },
                        shapes = ButtonGroupDefaults.connectedLeadingButtonShapes(),
                    ) { Text("Grid", style = type.buttonLabel) }
                    ToggleButton(
                        checked = !grid,
                        onCheckedChange = {
                            if (grid) haptics.perform(Haptic.ToggleOn)
                            grid = false
                            prefs.setGrid(shelf, false)
                        },
                        shapes = ButtonGroupDefaults.connectedTrailingButtonShapes(),
                    ) { Text("List", style = type.buttonLabel) }
                }
            }
        }
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
            horizontalArrangement = Arrangement.spacedBy((Grid.base * 2).dp),
            verticalArrangement = Arrangement.spacedBy((Grid.base * 3).dp),
        ) {
            itemsIndexed(pager.items, key = { _, a -> a.id }) { _, album ->
                AlbumTile(album, onClick = { onOpen(Route.Album(album.id)) })
            }
        }
    } else {
        val state = rememberLazyListState()
        LoadMore(state, pager)
        LazyColumn(state = state, contentPadding = PaddingValues(bottom = padding.calculateBottomPadding())) {
            items(pager.items, key = { it.id }) { album ->
                ListRow(album.title, album.artist, onClick = { onOpen(Route.Album(album.id)) }) {
                    uk.gcartee.encore.app.ui.components.Artwork(uk.gcartee.encore.core.ArtworkOf.ALBUM, album.id, 48.dp, Modifier.size(48.dp))
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
    val pager = rememberPager(generation) { o, l -> host.core.artists(o.toUInt(), l.toUInt()) }
    if (pager.items.isEmpty()) {
        EmptyState("Nothing here yet", "Artists appear once your library has synced.")
        return
    }
    val state = rememberLazyListState()
    LoadMore(state, pager)
    LazyColumn(state = state, contentPadding = PaddingValues(bottom = padding.calculateBottomPadding())) {
        items(pager.items, key = { it.id }) { artist ->
            val n = artist.albumCount.toInt()
            ListRow(artist.name, if (n == 1) "1 album" else "$n albums", onClick = { onOpen(Route.Artist(artist.id)) }) {
                uk.gcartee.encore.app.ui.components.Artwork(uk.gcartee.encore.core.ArtworkOf.ARTIST, artist.id, 48.dp, Modifier.size(48.dp), cornerRadius = 24.dp)
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
                uk.gcartee.encore.app.ui.components.Artwork(uk.gcartee.encore.core.ArtworkOf.PLAYLIST, p.id, 48.dp, Modifier.size(48.dp))
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
        Text(title, style = type.sectionHeader, color = MaterialTheme.colorScheme.onSurface)
        Spacer(Modifier.size(Grid.base.dp))
        Text(reason, style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

private const val TILE_MIN_DP = 150
