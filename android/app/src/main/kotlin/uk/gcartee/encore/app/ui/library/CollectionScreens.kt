package uk.gcartee.encore.app.ui.library

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.ui.components.AlbumTile
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.SongRow
import uk.gcartee.encore.app.ui.components.TILE_ART_DP
import uk.gcartee.encore.app.ui.components.formatLength
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.StartedFrom
import uk.gcartee.encore.tokens.EncoreTokens.Grid

@Composable
private fun PageHeader(title: String, subtitle: String?, onBack: () -> Unit) {
    val type = LocalEncoreType.current
    Column(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = (Grid.base * 2).dp)) {
        FilledTonalIconButton(onClick = onBack, modifier = Modifier.padding(vertical = Grid.base.dp)) {
            Icon(Icons.back(), contentDescription = "Back")
        }
        Text(title, style = type.collectionTitle, color = MaterialTheme.colorScheme.onSurface, modifier = Modifier.semantics { heading() })
        subtitle?.let {
            Spacer(Modifier.height(4.dp))
            Text(it, style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Spacer(Modifier.height((Grid.base * 2).dp))
    }
}

/** An artist: albums newest first, then songs. */
@Composable
fun ArtistScreen(host: CoreHost, artistId: Long, playingSongId: Long?, onBack: () -> Unit, onOpen: (Route) -> Unit, contentPadding: PaddingValues) {
    val type = LocalEncoreType.current
    val context = LocalContext.current
    val haptics = rememberHaptics()
    val detail = remember(artistId) { runCatching { host.core.artist(artistId) }.getOrNull() }
    val songs = remember(artistId) { runCatching { host.core.artistSongs(artistId, 0u, ARTIST_SONGS.toUInt()) }.getOrDefault(emptyList()) }
    if (detail == null) {
        EmptyState("This artist isn't in your library", "They may have been removed from the server.")
        return
    }
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        LazyColumn(contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp)) {
            item {
                val n = detail.albums.size
                PageHeader(detail.artist.name, if (n == 1) "1 album" else "$n albums", onBack)
            }
            if (songs.isNotEmpty()) item {
                Button(
                    onClick = {
                        haptics.perform(Haptic.Soft)
                        host.core.play(songs.map { it.id }, 0u, StartedFrom.ARTIST, artistId.toString())
                        PlaybackHub.playCurrent(context)
                    },
                    modifier = Modifier.padding(horizontal = (Grid.base * 2).dp).fillMaxWidth().height(BUTTON_DP.dp),
                ) { Text("Play", style = type.sectionHeader) }
            }
            if (detail.albums.isNotEmpty()) {
                item { SectionTitle("Albums") }
                item {
                    LazyRow(
                        contentPadding = PaddingValues(horizontal = (Grid.base * 2).dp),
                        horizontalArrangement = Arrangement.spacedBy((Grid.base * 2).dp),
                    ) {
                        items(detail.albums, key = { it.id }) { a ->
                            AlbumTile(a, onClick = { onOpen(Route.Album(a.id)) }, modifier = Modifier.width(ROW_TILE_DP.dp))
                        }
                    }
                }
            }
            if (songs.isNotEmpty()) {
                item { SectionTitle("Songs") }
                itemsIndexed(songs, key = { _, s -> s.id }) { i, song ->
                    SongRow(song, playing = song.id == playingSongId, subtitle = song.albumTitle, onClick = {
                        host.core.play(songs.map { it.id }, i.toUInt(), StartedFrom.ARTIST, artistId.toString())
                        PlaybackHub.playCurrent(context)
                    })
                }
            }
        }
    }
}

/** A playlist. A song that left the library stays listed, dimmed (board 20). */
@Composable
fun PlaylistScreen(host: CoreHost, playlistId: Long, playingSongId: Long?, onBack: () -> Unit, contentPadding: PaddingValues) {
    val type = LocalEncoreType.current
    val context = LocalContext.current
    val haptics = rememberHaptics()
    val detail = remember(playlistId) { runCatching { host.core.playlist(playlistId) }.getOrNull() }
    if (detail == null) {
        EmptyState("This playlist isn't in your library", "It may have been removed from the server.")
        return
    }
    val playable = remember(detail) { detail.entries.mapNotNull { it.song } }
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        LazyColumn(contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp)) {
            item {
                val n = detail.playlist.songCount.toInt()
                PageHeader(detail.playlist.name, "${if (n == 1) "1 song" else "$n songs"} · ${formatLength(detail.playlist.durationMs)}", onBack)
            }
            if (playable.isNotEmpty()) item {
                Button(
                    onClick = {
                        haptics.perform(Haptic.Soft)
                        host.core.play(playable.map { it.id }, 0u, StartedFrom.PLAYLIST, playlistId.toString())
                        PlaybackHub.playCurrent(context)
                    },
                    modifier = Modifier.padding(horizontal = (Grid.base * 2).dp).fillMaxWidth().height(BUTTON_DP.dp),
                ) { Text("Play", style = type.sectionHeader) }
            }
            itemsIndexed(detail.entries, key = { _, e -> e.position.toLong() }) { _, entry ->
                val song = entry.song
                if (song == null) {
                    Row(Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = (Grid.base * 2).dp), verticalAlignment = Alignment.CenterVertically) {
                        Text("A song that's no longer in your library", style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f))
                    }
                } else {
                    SongRow(song, playing = song.id == playingSongId, onClick = {
                        host.core.play(playable.map { it.id }, playable.indexOf(song).coerceAtLeast(0).toUInt(), StartedFrom.PLAYLIST, playlistId.toString())
                        PlaybackHub.playCurrent(context)
                    })
                }
            }
        }
    }
}

@Composable
fun SectionTitle(text: String) {
    Text(
        text,
        style = LocalEncoreType.current.sectionHeader,
        color = MaterialTheme.colorScheme.onSurface,
        modifier = Modifier.padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = (Grid.base * 3).dp, bottom = Grid.base.dp).semantics { heading() },
    )
}

private const val ARTIST_SONGS = 200
private const val BUTTON_DP = 56
private const val ROW_TILE_DP = TILE_ART_DP * 3 / 4
