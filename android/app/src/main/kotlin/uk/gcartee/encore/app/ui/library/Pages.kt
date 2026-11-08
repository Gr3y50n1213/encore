package uk.gcartee.encore.app.ui.library

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CornerSize
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FilledTonalIconToggleButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.ui.components.AlbumTile
import uk.gcartee.encore.app.ui.components.ConnectedButtons
import uk.gcartee.encore.app.ui.components.GroupButton
import uk.gcartee.encore.app.ui.components.SystemBarIconsFor
import uk.gcartee.encore.app.ui.components.Artwork
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.MenuExtra
import uk.gcartee.encore.app.ui.components.MenuPlace
import uk.gcartee.encore.app.ui.components.SongItem
import uk.gcartee.encore.app.ui.components.SwipeAction
import uk.gcartee.encore.app.ui.components.TILE_ART_DP
import uk.gcartee.encore.app.ui.components.describe
import uk.gcartee.encore.app.ui.components.formatLength
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.core.ArtworkPalette
import uk.gcartee.encore.core.ChosenBy
import uk.gcartee.encore.core.MixPage
import uk.gcartee.encore.core.PlaylistDetail
import uk.gcartee.encore.core.Song
import uk.gcartee.encore.core.StartedFrom
import uk.gcartee.encore.tokens.EncoreTokens.AlbumGradient
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Radius

/** A palette colour from the core ("3D0000" or "#3D0000"). */
fun paletteColor(hex: String): Color = Color(0xFF000000 or hex.removePrefix("#").toLong(16))

/**
 * A page's tint (board 08, item 2): the palette's base and text, else the theme. [themed]
 * is the no-artwork case, where Play takes the profile accent (board 08, States).
 */
data class PageTint(val field: Color, val content: Color, val secondary: Color, val themed: Boolean = false)

@Composable
fun rememberTint(load: suspend () -> ArtworkPalette?, key: Any): PageTint {
    var palette by remember(key) { mutableStateOf<ArtworkPalette?>(null) }
    LaunchedEffect(key) { palette = runCatching { load() }.getOrNull() }
    val p = palette
    val field = p?.let { paletteColor(it.base) } ?: MaterialTheme.colorScheme.surface
    val content = p?.let { paletteColor(it.text) } ?: MaterialTheme.colorScheme.onSurface
    val secondary = if (p != null) content.copy(alpha = 0.72f) else MaterialTheme.colorScheme.onSurfaceVariant
    val fade = tween<Color>(AlbumGradient.crossfadeMs)
    return PageTint(
        animateColorAsState(field, fade, label = "field").value,
        animateColorAsState(content, fade, label = "content").value,
        animateColorAsState(secondary, fade, label = "secondary").value,
        themed = p == null,
    )
}

@Composable
fun rememberAlbumTint(host: CoreHost, albumId: Long): PageTint = rememberTint({ host.core.albumPalette(albumId) }, albumId)

/** A page header (boards 08 and 15): back, the cover, title, subtitle lines, Play and Shuffle. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun Header(
    tint: PageTint,
    title: String,
    subtitle: String?,
    meta: String?,
    onBack: () -> Unit,
    onPlay: (() -> Unit)?,
    onShuffle: (() -> Unit)?,
    onSubtitle: (() -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
    cover: @Composable () -> Unit,
) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    Column(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = (Grid.base * 2).dp)) {
        Row(Modifier.fillMaxWidth().padding(vertical = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
            // Item 1: tonal rounded-square buttons over the tinted page, rounding when pressed.
            PageButton(tint, onBack) { Icon(Icons.back(), contentDescription = "Back") }
            Spacer(Modifier.weight(1f))
            trailing?.invoke()
        }
        Box(Modifier.align(Alignment.CenterHorizontally).widthIn(max = COVER_DP.dp).fillMaxWidth(0.72f)) { cover() }
        Spacer(Modifier.height((Grid.base * 3).dp))
        Text(title, style = type.collectionTitle, color = tint.content, modifier = Modifier.semantics { heading() })
        subtitle?.let {
            // Item 3: the artist link, the full text width plus 8 dp of padding.
            Text(
                it,
                style = type.nowPlayingSubtitle,
                color = tint.content.copy(alpha = 0.86f),
                modifier = Modifier
                    .let { m -> if (onSubtitle != null) m.clickable(onClick = onSubtitle) else m }
                    .padding(vertical = Grid.base.dp),
            )
        }
        meta?.let { Text(it, style = type.body, color = tint.secondary) }
        Spacer(Modifier.height((Grid.base * 2).dp))
        if (onPlay != null || onShuffle != null) {
            // Item 4: one connected group; the pressed button rounds fully and widens.
            ConnectedButtons(
                listOf(
                    if (tint.themed) {
                        GroupButton("Play", MaterialTheme.colorScheme.primary, MaterialTheme.colorScheme.onPrimary, icon = Icons.play(), enabled = onPlay != null) { onPlay?.invoke() }
                    } else {
                        GroupButton("Play", tint.content, tint.field, icon = Icons.play(), enabled = onPlay != null) { onPlay?.invoke() }
                    },
                    if (tint.themed) {
                        GroupButton("Shuffle", MaterialTheme.colorScheme.secondaryContainer, MaterialTheme.colorScheme.onSecondaryContainer, icon = Icons.shuffle(), enabled = onShuffle != null) { onShuffle?.invoke() }
                    } else {
                        GroupButton("Shuffle", tint.content.copy(alpha = 0.16f), tint.content, icon = Icons.shuffle(), enabled = onShuffle != null) { onShuffle?.invoke() }
                    },
                ),
                height = BUTTON_DP.dp,
            )
            Spacer(Modifier.height((Grid.base * 2).dp))
        }
    }
}

private fun songs(n: Int) = if (n == 1) "1 song" else "$n songs"

/** Board 08, item 1: Back and More, tonal rounded squares that round fully when pressed. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PageButton(tint: PageTint, onClick: () -> Unit, content: @Composable () -> Unit) {
    FilledTonalIconButton(
        onClick = onClick,
        shapes = IconButtonDefaults.shapes(RoundedCornerShape(PAGE_BUTTON_RADIUS_DP.dp), RoundedCornerShape(50)),
        colors = IconButtonDefaults.filledTonalIconButtonColors(containerColor = tint.content.copy(alpha = 0.14f), contentColor = tint.content),
        modifier = Modifier.size(PAGE_BUTTON_DP.dp),
    ) { content() }
}


/** Board 08. */
@Composable
fun AlbumScreen(host: CoreHost, albumId: Long, playingSongId: Long?, onBack: () -> Unit, contentPadding: PaddingValues) {
    val actions = LocalAppActions.current
    val detail = remember(albumId) { runCatching { host.core.album(albumId) }.getOrNull() }
    val tint = rememberAlbumTint(host, albumId)
    if (detail == null) {
        EmptyState("This album isn't in your library", "It may have been removed from the server.")
        return
    }
    val album = detail.album
    val ids = remember(detail) { detail.songs.map { it.id } }
    val multiDisc = remember(detail) { detail.songs.mapNotNull { it.discNumber }.distinct().size > 1 }
    SystemBarIconsFor(tint.field)
    val more = remember(album.id) {
        runCatching { host.core.search(album.artist).albums.filter { it.artist == album.artist && it.id != album.id }.take(12) }.getOrDefault(emptyList())
    }
    val type = LocalEncoreType.current
    Box(Modifier.fillMaxSize().background(tint.field)) {
        LazyColumn(contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp)) {
            item {
                Header(
                    tint,
                    album.title,
                    album.artist,
                    listOfNotNull(album.year?.toString(), songs(album.songCount.toInt()), formatLength(album.durationMs)).joinToString(" · "),
                    onBack,
                    onPlay = { actions.play(ids, 0, StartedFrom.ALBUM, album.id.toString(), album.title) },
                    onShuffle = { actions.play(ids, ids.indices.random(), StartedFrom.ALBUM, album.id.toString(), album.title, shuffle = true) },
                    onSubtitle = { detail.songs.firstOrNull()?.let(actions::goToArtist) },
                    trailing = {
                        var open by remember { mutableStateOf(false) }
                        Box {
                            PageButton(tint, { open = true }) { Icon(Icons.more(), contentDescription = "More") }
                            DropdownMenu(open, onDismissRequest = { open = false }) {
                                DropdownMenuItem(text = { Text("Play Next") }, leadingIcon = { Icon(Icons.playNext(), null) }, onClick = { open = false; actions.playNext(detail.songs) })
                                DropdownMenuItem(text = { Text("Add to Queue") }, leadingIcon = { Icon(Icons.queue(), null) }, onClick = { open = false; actions.addToQueue(detail.songs) })
                                DropdownMenuItem(text = { Text("Go to Artist") }, leadingIcon = { Icon(Icons.artist(), null) }, onClick = { open = false; detail.songs.firstOrNull()?.let(actions::goToArtist) })
                            }
                        }
                    },
                ) {
                    Artwork(ArtworkOf.ALBUM, album.id, COVER_DP.dp, Modifier.fillMaxWidth(), cornerRadius = COVER_RADIUS_DP.dp)
                }
                // States: a partial album.
                if (detail.songs.size.toUInt() < album.songCount) {
                    Text("${detail.songs.size} of ${album.songCount} songs in your library", style = type.caption, color = tint.secondary, modifier = Modifier.padding(horizontal = (Grid.base * 2).dp))
                }
            }
            itemsIndexed(detail.songs, key = { _, s -> s.id }) { i, song ->
                val disc = song.discNumber
                if (multiDisc && disc != null && (i == 0 || detail.songs[i - 1].discNumber != disc)) {
                    Text("Disc $disc", style = type.groupLabel, color = tint.secondary, modifier = Modifier.padding(start = (Grid.base * 2).dp, top = (Grid.base * 2).dp, bottom = 4.dp).semantics { heading() })
                }
                // Item 5: a tap plays the song and queues the rest of the album.
                SongItem(
                    song,
                    playing = song.id == playingSongId,
                    onClick = { actions.play(ids, i, StartedFrom.ALBUM, album.id.toString(), album.title) },
                    trackNumber = song.trackNumber?.toInt() ?: (i + 1),
                    place = MenuPlace(onAlbum = album.id),
                    contentColor = tint.content,
                    secondaryColor = tint.secondary,
                    rowColor = tint.field,
                )
                HorizontalDivider(Modifier.padding(horizontal = (Grid.base * 2).dp), color = tint.content.copy(alpha = 0.12f))
            }
            // Footer: release info, then More by this artist.
            item {
                Text(
                    listOfNotNull(album.year?.let { "Released $it" }, songs(album.songCount.toInt()) + ", " + formatLength(album.durationMs)).joinToString("\n"),
                    style = type.caption,
                    color = tint.secondary,
                    modifier = Modifier.padding((Grid.base * 2).dp),
                )
            }
            if (more.isNotEmpty()) {
                item { SectionTitle("More by ${album.artist}", tint.content) }
                item {
                    LazyRow(contentPadding = PaddingValues(horizontal = (Grid.base * 2).dp), horizontalArrangement = Arrangement.spacedBy((Grid.base * 2).dp)) {
                        items(more, key = { it.id }) { a -> AlbumTile(a, onClick = { actions.open(Route.Album(a.id)) }, modifier = Modifier.width(ROW_TILE_DP.dp)) }
                    }
                }
            }
        }
    }
}

/** A playlist. A song that left the library stays listed, dimmed (board 20, item 5). */
@Composable
fun PlaylistScreen(host: CoreHost, playlistId: Long, playingSongId: Long?, onBack: () -> Unit, contentPadding: PaddingValues) {
    val actions = LocalAppActions.current
    val type = LocalEncoreType.current
    var version by remember { mutableIntStateOf(0) }
    val remote by host.remoteChanges.collectAsState()
    val detail: PlaylistDetail? = remember(playlistId, version, remote) { runCatching { host.core.playlist(playlistId) }.getOrNull() }
    val tint = rememberTint({ host.core.playlistPalette(playlistId) }, playlistId)
    if (detail == null) {
        EmptyState("This playlist isn't in your library", "It may have been removed from the server.")
        return
    }
    SystemBarIconsFor(tint.field)
    val playable = remember(detail) { detail.entries.mapNotNull { it.song } }
    val ids = remember(playable) { playable.map { it.id } }
    val name = detail.playlist.name
    val removeIcon = Icons.remove()
    Box(Modifier.fillMaxSize().background(tint.field)) {
        LazyColumn(contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp)) {
            item {
                Header(
                    tint,
                    name,
                    null,
                    "${songs(detail.playlist.songCount.toInt())} · ${formatLength(detail.playlist.durationMs)}",
                    onBack,
                    onPlay = if (ids.isEmpty()) null else ({ actions.play(ids, 0, StartedFrom.PLAYLIST, playlistId.toString(), name) }),
                    onShuffle = if (ids.isEmpty()) null else ({ actions.play(ids, ids.indices.random(), StartedFrom.PLAYLIST, playlistId.toString(), name, shuffle = true) }),
                ) {
                    // The server's playlist image, over a mosaic of its first songs for playlists without one.
                    Box {
                        Mosaic(playable.take(4).map { it.id })
                        Artwork(ArtworkOf.PLAYLIST, playlistId, COVER_DP.dp, Modifier.fillMaxWidth(), cornerRadius = COVER_RADIUS_DP.dp, placeholder = false)
                    }
                }
            }
            if (detail.entries.isEmpty()) item { EmptyState("This playlist is empty", "Add songs from their menu, Add to Playlist.") }
            itemsIndexed(detail.entries, key = { i, e -> "${e.position}-$i" }) { _, entry ->
                val remove = SwipeAction("Remove", destructive = true, icon = { removeIcon }) {
                    runCatching { host.core.removeFromPlaylist(playlistId, listOf(entry.position)) }
                        .onSuccess { r ->
                            version++
                            actions.toaster.show("Removed from $name") { r.editId?.let { runCatching { host.core.undo(it) } }; version++ }
                        }
                        .onFailure { actions.toaster.show(it.userMessage()) }
                }
                val song = entry.song
                if (song == null) {
                    Text(
                        "A song that's no longer in your library",
                        style = type.body,
                        color = tint.secondary.copy(alpha = 0.6f),
                        modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = (Grid.base * 2).dp),
                    )
                } else {
                    SongItem(
                        song,
                        playing = song.id == playingSongId,
                        onClick = { actions.play(ids, playable.indexOf(song).coerceAtLeast(0), StartedFrom.PLAYLIST, playlistId.toString(), name) },
                        place = MenuPlace(extra = listOf(MenuExtra("Remove from this playlist", destructive = true) { remove.perform() })),
                        contentColor = tint.content,
                        secondaryColor = tint.secondary,
                        rowColor = tint.field,
                        toStart = remove,
                    )
                }
            }
        }
    }
}

/** An artist: albums newest first, then their songs. */
@Composable
fun ArtistScreen(host: CoreHost, artistId: Long, playingSongId: Long?, onBack: () -> Unit, contentPadding: PaddingValues) {
    val actions = LocalAppActions.current
    val detail = remember(artistId) { runCatching { host.core.artist(artistId) }.getOrNull() }
    val songs = remember(artistId) { runCatching { host.core.artistSongs(artistId, 0u, ARTIST_SONGS.toUInt()) }.getOrDefault(emptyList()) }
    if (detail == null) {
        EmptyState("This artist isn't in your library", "They may have been removed from the server.")
        return
    }
    val name = detail.artist.name
    val ids = remember(songs) { songs.map { it.id } }
    val tint = PageTint(MaterialTheme.colorScheme.surface, MaterialTheme.colorScheme.onSurface, MaterialTheme.colorScheme.onSurfaceVariant, themed = true)
    LazyColumn(Modifier.fillMaxSize().background(tint.field), contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp)) {
        item {
            val n = detail.albums.size
            Header(
                tint,
                name,
                null,
                if (n == 1) "1 album" else "$n albums",
                onBack,
                onPlay = if (ids.isEmpty()) null else ({ actions.play(ids, 0, StartedFrom.ARTIST, artistId.toString(), name) }),
                onShuffle = if (ids.isEmpty()) null else ({ actions.play(ids, ids.indices.random(), StartedFrom.ARTIST, artistId.toString(), name, shuffle = true) }),
            ) {
                Artwork(ArtworkOf.ARTIST, artistId, COVER_DP.dp, Modifier.fillMaxWidth().clip(RoundedCornerShape(50)), cornerRadius = (COVER_DP / 2).dp)
            }
        }
        if (detail.albums.isNotEmpty()) {
            item { SectionTitle("Albums", tint.content) }
            item {
                LazyRow(contentPadding = PaddingValues(horizontal = (Grid.base * 2).dp), horizontalArrangement = Arrangement.spacedBy((Grid.base * 2).dp)) {
                    items(detail.albums, key = { it.id }) { a -> AlbumTile(a, onClick = { actions.open(Route.Album(a.id)) }, modifier = Modifier.width(ROW_TILE_DP.dp)) }
                }
            }
        }
        if (songs.isNotEmpty()) {
            item { SectionTitle("Songs", tint.content) }
            itemsIndexed(songs, key = { _, s -> s.id }) { i, song ->
                SongItem(
                    song,
                    playing = song.id == playingSongId,
                    onClick = { actions.play(ids, i, StartedFrom.ARTIST, artistId.toString(), name) },
                    subtitle = song.albumTitle,
                    place = MenuPlace(onArtist = true),
                )
            }
        }
    }
}

/**
 * Board 15, a Made for you mix: a 2x2 mosaic of four of its songs' covers, tinted from
 * the first; a plain description and when it was made; Play and Shuffle; Pin (the button
 * fills); each song with one reason; swipe left is Not in this mix, right Play Next.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MixScreen(host: CoreHost, mixId: String, playingSongId: Long?, onBack: () -> Unit, contentPadding: PaddingValues) {
    val actions = LocalAppActions.current
    val haptics = rememberHaptics()
    val type = LocalEncoreType.current
    var version by remember { mutableIntStateOf(0) }
    val page: MixPage? = remember(mixId, version) { runCatching { host.core.mix(mixId) }.getOrNull() }
    if (page == null) {
        EmptyState("This mix has been replaced", "Today's mixes are on the Mixes tab.")
        return
    }
    val summary = page.summary
    val first = summary.coverSongIds.firstOrNull()
    val firstSong = remember(first) { first?.let { runCatching { host.core.song(it) }.getOrNull() } }
    val tint = rememberTint({ firstSong?.albumId?.let { host.core.albumPalette(it) } }, first ?: 0L)
    SystemBarIconsFor(tint.field)
    val ids = remember(page) { page.songs.map { it.song.id } }
    var pinned by remember(summary.pinned) { mutableStateOf(summary.pinned) }
    val notIcon = Icons.block()
    Box(Modifier.fillMaxSize().background(tint.field)) {
        LazyColumn(contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp)) {
            item {
                Header(
                    tint,
                    summary.title,
                    summary.subtitle,
                    "${songs(summary.songCount.toInt())} · ${madeWhen(summary.madeUtcMs)}",
                    onBack,
                    onPlay = { actions.play(ids, 0, StartedFrom.MIX, mixId, summary.title) },
                    onShuffle = { actions.play(ids, ids.indices.random(), StartedFrom.MIX, mixId, summary.title, shuffle = true) },
                    trailing = {
                        // Item 4: Pin; the button fills, with the success haptic.
                        FilledTonalIconToggleButton(
                            checked = pinned,
                            onCheckedChange = { on ->
                                runCatching { host.core.pinMix(mixId, on) }
                                    .onSuccess {
                                        pinned = on
                                        haptics.perform(if (on) Haptic.Confirm else Haptic.ToggleOff)
                                    }
                                    .onFailure { actions.toaster.show(it.userMessage()) }
                            },
                            colors = IconButtonDefaults.filledTonalIconToggleButtonColors(
                                containerColor = tint.content.copy(alpha = 0.14f),
                                contentColor = tint.content,
                                checkedContainerColor = tint.content,
                                checkedContentColor = tint.field,
                            ),
                            modifier = Modifier.semantics { stateDescription = if (pinned) "Pinned" else "Not pinned" },
                        ) { Icon(Icons.pin(pinned), contentDescription = "Pin") }
                    },
                ) { Mosaic(summary.coverSongIds) }
            }
            items(page.songs, key = { it.song.id }) { ms ->
                val notInMix = SwipeAction("Not in this mix", destructive = true, icon = { notIcon }) {
                    runCatching { host.core.notInThisMix(mixId, ms.song.id) }
                        .onSuccess {
                            version++
                            actions.toaster.show("Removed from ${summary.title}")
                        }
                        .onFailure { actions.toaster.show(it.userMessage()) }
                }
                // Item 5: one short reason under the title.
                SongItem(
                    ms.song,
                    playing = ms.song.id == playingSongId,
                    onClick = { actions.play(ids, ids.indexOf(ms.song.id), StartedFrom.MIX, mixId, summary.title) },
                    subtitle = ms.reason?.let(::describe) ?: if (ms.newToYou) "New to you" else ms.song.artist,
                    place = MenuPlace(chosenBy = ChosenBy.APP, extra = listOf(MenuExtra("Not in this mix", destructive = true) { notInMix.perform() })),
                    contentColor = tint.content,
                    secondaryColor = tint.secondary,
                    rowColor = tint.field,
                    toStart = notInMix,
                )
            }
            item {
                Row(Modifier.fillMaxWidth().padding((Grid.base * 2).dp), horizontalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
                    androidx.compose.material3.TextButton(onClick = {
                        runCatching { host.core.saveMixAsPlaylist(mixId) }
                            .onSuccess { actions.toaster.show("Saved as a playlist") }
                            .onFailure { actions.toaster.show(it.userMessage()) }
                    }) { Text("Save as playlist", color = tint.content) }
                    androidx.compose.material3.TextButton(onClick = {
                        runCatching { host.core.hideMix(mixId) }.onSuccess { onBack(); actions.toaster.show("Hidden from today's mixes") }
                    }) { Text("Hide this mix", color = tint.content) }
                }
            }
        }
    }
}

/** Board 15, item 1: the mix's cover, a 2x2 mosaic of four of its songs' covers. */
@Composable
fun Mosaic(songIds: List<Long>, modifier: Modifier = Modifier, size: Int = COVER_DP) {
    val ids = songIds.take(4)
    Column(modifier.fillMaxWidth().clip(RoundedCornerShape(COVER_RADIUS_DP.dp))) {
        ids.chunked(2).forEach { row ->
            Row(Modifier.fillMaxWidth()) {
                row.forEach { id -> Artwork(ArtworkOf.SONG, id, (size / 2).dp, Modifier.weight(1f), cornerRadius = 0.dp) }
                if (row.size == 1) Box(Modifier.weight(1f))
            }
        }
    }
}

/** The same mosaic filling any frame (a carousel item), each cell cropped to fit. */
@Composable
fun MosaicFill(songIds: List<Long>, modifier: Modifier = Modifier) {
    val ids = songIds.take(4)
    Column(modifier) {
        ids.chunked(2).forEach { row ->
            Row(Modifier.weight(1f).fillMaxWidth()) {
                row.forEach { id -> Artwork(ArtworkOf.SONG, id, 160.dp, Modifier.weight(1f).fillMaxSize(), cornerRadius = 0.dp, fill = true) }
                if (row.size == 1) Box(Modifier.weight(1f))
            }
        }
    }
}

fun madeWhen(utcMs: Long?): String {
    if (utcMs == null || utcMs <= 0) return "Made today"
    val mins = (System.currentTimeMillis() - utcMs) / 60_000
    return when {
        mins < 60 -> "Made just now"
        mins < 24 * 60 -> "Made ${mins / 60} h ago"
        else -> "Made ${mins / (24 * 60)} days ago"
    }
}

@Composable
fun SectionTitle(text: String, colour: Color = MaterialTheme.colorScheme.onSurface) {
    Text(
        text,
        style = LocalEncoreType.current.sectionHeader,
        color = colour,
        modifier = Modifier.padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = (Grid.base * 3).dp, bottom = Grid.base.dp).semantics { heading() },
    )
}

private const val COVER_DP = 280
private const val COVER_RADIUS_DP = 28
private const val BUTTON_DP = 56
private const val ARTIST_SONGS = 200
private const val ROW_TILE_DP = TILE_ART_DP * 3 / 4
private const val PAGE_BUTTON_DP = 48
private const val PAGE_BUTTON_RADIUS_DP = 14
