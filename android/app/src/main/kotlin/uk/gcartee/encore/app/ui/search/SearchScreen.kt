package uk.gcartee.encore.app.ui.search

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
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
fun SearchScreen(host: CoreHost, playingSongId: Long?, onOpen: (Route) -> Unit, contentPadding: PaddingValues) {
    val type = LocalEncoreType.current
    val context = LocalContext.current
    val keyboard = LocalSoftwareKeyboardController.current
    var query by rememberSaveable { mutableStateOf("") }
    var recents by remember { mutableStateOf(runCatching { host.core.recentSearches() }.getOrDefault(emptyList())) }
    var shown by remember { mutableStateOf<SearchResults?>(null) }
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
        OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            placeholder = { Text("Songs, albums, artists, lyrics") },
            leadingIcon = { Icon(Icons.search(), contentDescription = null) },
            trailingIcon = {
                if (query.isNotEmpty()) IconButton(onClick = { query = "" }) { Icon(Icons.close(), contentDescription = "Clear") }
            },
            singleLine = true,
            shape = CircleShape,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = {
                // Item 1: the return key saves the query.
                host.core.addRecentSearch(query.trim())
                recents = runCatching { host.core.recentSearches() }.getOrDefault(recents)
                keyboard?.hide()
            }),
            modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
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
                    items(recents, key = { it }) { r ->
                        androidx.compose.foundation.layout.Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                            androidx.compose.foundation.layout.Box(Modifier.weight(1f)) {
                                ListRow(r, null, onClick = { query = r }) { Icon(Icons.history(), contentDescription = null) }
                            }
                            IconButton(onClick = {
                                host.core.removeRecentSearch(r)
                                recents = recents - r
                            }) { Icon(Icons.close(), contentDescription = "Remove $r") }
                        }
                    }
                }
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
