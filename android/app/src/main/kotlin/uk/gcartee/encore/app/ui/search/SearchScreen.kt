package uk.gcartee.encore.app.ui.search

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.ui.components.Artwork
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.ListRow
import uk.gcartee.encore.app.ui.components.SongRow
import uk.gcartee.encore.app.ui.library.EmptyState
import uk.gcartee.encore.app.ui.library.SectionTitle
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.core.ResultKind
import uk.gcartee.encore.core.SearchResults
import uk.gcartee.encore.core.Song
import uk.gcartee.encore.core.StartedFrom
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/** Board 07 on Android: the field docked at the top, results as you type. */
@Composable
fun SearchScreen(host: CoreHost, playingSongId: Long?, onOpen: (Route) -> Unit, contentPadding: PaddingValues, onLibrary: () -> Unit = {}) {
    val type = LocalEncoreType.current
    val context = LocalContext.current
    val keyboard = LocalSoftwareKeyboardController.current
    var query by rememberSaveable { mutableStateOf("") }
    var recents by remember { mutableStateOf(runCatching { host.core.recentSearches() }.getOrDefault(emptyList())) }
    var shown by remember { mutableStateOf<SearchResults?>(null) }
    var scope by rememberSaveable { mutableStateOf("All") }
    // GUARD: results are frozen from touch-down until release, so a row that re-ranks
    // under the finger never launches the wrong song.
    var touching by remember { mutableStateOf(false) }
    var pending by remember { mutableStateOf<SearchResults?>(null) }

    LaunchedEffect(query) {
        val q = query.trim()
        val r = if (q.isEmpty()) null else runCatching { host.core.search(q) }.getOrNull()
        if (touching) pending = r else shown = r
    }
    LaunchedEffect(Unit) {
        snapshotFlow { touching }.collect { if (!it && pending != null) { shown = pending; pending = null } }
    }

    val actions = uk.gcartee.encore.app.ui.components.LocalAppActions.current
    fun playSong(song: Song, list: List<Song>) {
        host.core.addRecentSearch(query.trim())
        // A tapped song plays and Autoplay continues from it (board 07).
        actions.play(list.map { it.id }, list.indexOf(song).coerceAtLeast(0), StartedFrom.SEARCH, null, "Search")
    }

    fun open(route: Route) {
        host.core.addRecentSearch(query.trim())
        onOpen(route)
    }

    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        // Item 1: Android docks a filled search bar at the top, the avatar inside it.
        androidx.compose.material3.TextField(
            value = query,
            onValueChange = { query = it },
            placeholder = { Text("Songs, albums, artists, lyrics", maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis) },
            leadingIcon = { Icon(Icons.search(), contentDescription = null) },
            trailingIcon = {
                if (query.isNotEmpty()) IconButton(onClick = { query = "" }) { Icon(Icons.close(), contentDescription = "Clear") }
                else uk.gcartee.encore.app.ui.home.ProfileButton(host, uk.gcartee.encore.app.ui.components.LocalAppActions.current.openProfiles)
            },
            singleLine = true,
            shape = CircleShape,
            colors = androidx.compose.material3.TextFieldDefaults.colors(
                focusedContainerColor = androidx.compose.material3.MaterialTheme.colorScheme.surfaceContainerHigh,
                unfocusedContainerColor = androidx.compose.material3.MaterialTheme.colorScheme.surfaceContainerHigh,
                focusedIndicatorColor = androidx.compose.ui.graphics.Color.Transparent,
                unfocusedIndicatorColor = androidx.compose.ui.graphics.Color.Transparent,
            ),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = {
                // Item 1: the return key saves the query.
                host.core.addRecentSearch(query.trim())
                recents = runCatching { host.core.recentSearches() }.getOrDefault(recents)
                keyboard?.hide()
            }),
            modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
        )
        // Item 1: Android's scopes, as the same connected group as Library's shelves.
        // Downloaded searches downloads, which come in Phase 8.
        uk.gcartee.encore.app.ui.library.ChoiceGroup(
            options = listOf("All" to "All", "Library" to "Library", "Downloaded" to "Downloaded"),
            selected = scope,
            onSelect = { scope = it },
            enabled = { it != "Downloaded" },
            modifier = Modifier.padding(horizontal = (Grid.base * 2).dp).padding(bottom = Grid.base.dp),
        )
        val results = shown
        val listState = rememberLazyListState()
        LaunchedEffect(listState) {
            // SWIPE down on results dismisses the keyboard.
            snapshotFlow { listState.isScrollInProgress }.collect { if (it) keyboard?.hide() }
        }
        val freeze = Modifier.pointerInput(Unit) {
            awaitEachGesture {
                awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                touching = true
                waitForUpOrCancellation(PointerEventPass.Initial)
                touching = false
            }
        }
        when {
            query.isBlank() -> LazyColumn(freeze, state = listState, contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding())) {
                if (recents.isNotEmpty()) {
                    item {
                        androidx.compose.foundation.layout.Row(Modifier.fillMaxWidth(), verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                            androidx.compose.foundation.layout.Box(Modifier.weight(1f)) { SectionTitle("Recent searches") }
                            TextButton(onClick = {
                                host.core.clearRecentSearches()
                                recents = emptyList()
                            }, modifier = Modifier.padding(end = Grid.base.dp)) { Text("Clear all") }
                        }
                    }
                    // Item 2: a tap fills the field and runs it; Remove deletes one.
                    items(recents, key = { it }) { r ->
                        androidx.compose.foundation.layout.Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                            androidx.compose.foundation.layout.Box(Modifier.weight(1f)) {
                                ListRow(r, null, onClick = { query = r })
                            }
                            TextButton(onClick = {
                                host.core.removeRecentSearch(r)
                                recents = recents - r
                            }, modifier = Modifier.padding(end = Grid.base.dp)) { Text("Remove", style = type.caption) }
                        }
                    }
                }
                // Item 3: the tile grid under the recents. Moods come from audio analysis
                // (Phase 10), so for now the tiles open the library's own ways in.
                item { SectionTitle("Browse") }
                item { BrowseTiles(host, onLibrary) }
            }
            results == null || isEmpty(results) -> EmptyState("Nothing matches “${query.trim()}”", "Check the spelling, or try fewer words.")
            else -> LazyColumn(freeze, state = listState, contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding())) {
                results.top?.let { top ->
                    item { SectionTitle("Top result") }
                    item {
                        when (top.kind) {
                            ResultKind.SONG -> top.song?.let { s -> uk.gcartee.encore.app.ui.components.SongItem(s, s.id == playingSongId, onClick = { playSong(s, results.songs.ifEmpty { listOf(s) }) }) }
                            ResultKind.ALBUM -> top.album?.let { a -> ListRow(a.title, "Album · ${a.artist}", onClick = { open(Route.Album(a.id)) }) { Artwork(ArtworkOf.ALBUM, a.id, 48.dp, Modifier.size(48.dp)) } }
                            ResultKind.ARTIST -> top.artist?.let { a -> ListRow(a.name, "Artist", onClick = { open(Route.Artist(a.id)) }) { Artwork(ArtworkOf.ARTIST, a.id, 48.dp, Modifier.size(48.dp), cornerRadius = 24.dp) } }
                            ResultKind.PLAYLIST -> top.playlist?.let { p -> ListRow(p.name, "Playlist", onClick = { open(Route.Playlist(p.id)) }) }
                        }
                    }
                }
                if (results.songs.isNotEmpty() || results.lyrics.isNotEmpty()) {
                    item { SectionTitle("Songs") }
                    items(results.songs, key = { "s${it.id}" }) { s -> uk.gcartee.encore.app.ui.components.SongItem(s, s.id == playingSongId, onClick = { playSong(s, results.songs) }) }
                    // Lyric matches after the title matches, with the line under the title.
                    items(results.lyrics, key = { "l${it.song.id}" }) { m ->
                        uk.gcartee.encore.app.ui.components.SongItem(m.song, m.song.id == playingSongId, subtitle = "“${m.line}”", onClick = { playSong(m.song, listOf(m.song)) })
                    }
                }
                if (results.albums.isNotEmpty()) {
                    item { SectionTitle("Albums") }
                    items(results.albums, key = { "a${it.id}" }) { a ->
                        ListRow(a.title, a.artist, onClick = { open(Route.Album(a.id)) }) { Artwork(ArtworkOf.ALBUM, a.id, 48.dp, Modifier.size(48.dp)) }
                    }
                }
                if (results.artists.isNotEmpty()) {
                    item { SectionTitle("Artists") }
                    items(results.artists, key = { "r${it.id}" }) { a ->
                        ListRow(a.name, null, onClick = { open(Route.Artist(a.id)) }) { Artwork(ArtworkOf.ARTIST, a.id, 48.dp, Modifier.size(48.dp), cornerRadius = 24.dp) }
                    }
                }
                if (results.playlists.isNotEmpty()) {
                    item { SectionTitle("Playlists") }
                    items(results.playlists, key = { "p${it.id}" }) { p -> ListRow(p.name, null, onClick = { open(Route.Playlist(p.id)) }) }
                }
            }
        }
    }
}

private fun isEmpty(r: SearchResults) =
    r.top == null && r.songs.isEmpty() && r.albums.isEmpty() && r.artists.isEmpty() && r.playlists.isEmpty() && r.lyrics.isEmpty()

/**
 * Board 07, item 3: plain-colour tiles two to a row, alternating the two leaf shapes (round
 * at one pair of corners, tight at the other) so the grid reads as a pattern.
 */
@Composable
private fun BrowseTiles(host: CoreHost, onLibrary: () -> Unit) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val actions = uk.gcartee.encore.app.ui.components.LocalAppActions.current
    val scheme = androidx.compose.material3.MaterialTheme.colorScheme
    val type = LocalEncoreType.current
    val haptics = uk.gcartee.encore.app.ui.components.rememberHaptics()
    fun library(shelf: uk.gcartee.encore.app.ui.library.Shelf) {
        uk.gcartee.encore.app.ui.library.showLibraryShelf(context, host, shelf)
        onLibrary()
    }
    val tiles: List<Triple<String, androidx.compose.ui.graphics.Color, () -> Unit>> = listOf(
        Triple("Shuffle all", scheme.primaryContainer, {
            val ids = runCatching { host.core.songs(uk.gcartee.encore.core.SongSort.RECENTLY_ADDED, 0u, SHUFFLE_ALL_SONGS.toUInt()).map { it.id } }.getOrDefault(emptyList())
            if (ids.isNotEmpty()) actions.play(ids, ids.indices.random(), uk.gcartee.encore.core.StartedFrom.SONGS, null, "Your library", shuffle = true)
        }),
        Triple("Recently added", scheme.secondaryContainer, { library(uk.gcartee.encore.app.ui.library.Shelf.Albums) }),
        Triple("Playlists", scheme.secondaryContainer, { library(uk.gcartee.encore.app.ui.library.Shelf.Playlists) }),
        Triple("Artists", scheme.tertiaryContainer, { library(uk.gcartee.encore.app.ui.library.Shelf.Artists) }),
    )
    androidx.compose.foundation.layout.Column(
        Modifier.padding(horizontal = (Grid.base * 2).dp),
        verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(Grid.base.dp * 1.5f),
    ) {
        tiles.chunked(2).forEachIndexed { row, pair ->
            androidx.compose.foundation.layout.Row(horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(Grid.base.dp * 1.5f)) {
                pair.forEachIndexed { col, (label, colour, open) ->
                    val big = LEAF_BIG_DP.dp
                    val small = LEAF_SMALL_DP.dp
                    // The two leaves alternate like a checkerboard.
                    val leaf = if ((row + col) % 2 == 0) androidx.compose.foundation.shape.RoundedCornerShape(big, small, big, small)
                    else androidx.compose.foundation.shape.RoundedCornerShape(small, big, small, big)
                    androidx.compose.material3.Surface(
                        onClick = { haptics.perform(uk.gcartee.encore.app.ui.components.Haptic.Soft); open() },
                        shape = leaf,
                        color = colour,
                        modifier = Modifier.weight(1f).height(TILE_HEIGHT_DP.dp),
                    ) {
                        androidx.compose.foundation.layout.Box(Modifier.padding(horizontal = (Grid.base * 2).dp), contentAlignment = androidx.compose.ui.Alignment.CenterStart) {
                            Text(label, style = type.sectionHeader)
                        }
                    }
                }
            }
        }
    }
}

private const val LEAF_BIG_DP = 28
private const val LEAF_SMALL_DP = 8
private const val TILE_HEIGHT_DP = 84
private const val SHUFFLE_ALL_SONGS = 300