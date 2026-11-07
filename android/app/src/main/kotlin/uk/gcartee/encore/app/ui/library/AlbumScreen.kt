package uk.gcartee.encore.app.ui.library

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.foundation.shape.CornerSize
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.ui.components.Artwork
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.SongRow
import uk.gcartee.encore.app.ui.components.formatLength
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.AlbumDetail
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.core.ArtworkPalette
import uk.gcartee.encore.core.PaletteScheme
import uk.gcartee.encore.core.StartedFrom
import uk.gcartee.encore.tokens.EncoreTokens.AlbumGradient
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Radius

/** A palette colour from the core ("3D0000" or "#3D0000"). */
fun paletteColor(hex: String): Color = Color(0xFF000000 or hex.removePrefix("#").toLong(16))

/** The page tint: the palette's leading stop over its base (board 08, item 2), else the theme. */
data class PageTint(val field: Color, val content: Color, val secondary: Color)

@Composable
fun rememberAlbumTint(host: CoreHost, albumId: Long): PageTint {
    var palette by remember(albumId) { mutableStateOf<ArtworkPalette?>(null) }
    LaunchedEffect(albumId) { palette = runCatching { host.core.albumPalette(albumId) }.getOrNull() }
    // No artwork: the profile accent, which is the theme (board 08 states).
    val p = palette
    val field = p?.let { paletteColor(it.base) } ?: MaterialTheme.colorScheme.surface
    val content = p?.let { paletteColor(it.text) } ?: MaterialTheme.colorScheme.onSurface
    val secondary = if (p != null) content.copy(alpha = 0.72f) else MaterialTheme.colorScheme.onSurfaceVariant
    val crossfade = tween<Color>(AlbumGradient.crossfadeMs)
    return PageTint(
        field = animateColorAsState(field, crossfade, label = "field").value,
        content = animateColorAsState(content, crossfade, label = "content").value,
        secondary = animateColorAsState(secondary, crossfade, label = "secondary").value,
    )
}

/** Board 08. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun AlbumScreen(host: CoreHost, albumId: Long, playingSongId: Long?, onBack: () -> Unit, onOpen: (Route) -> Unit, contentPadding: PaddingValues) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val context = LocalContext.current
    val detail: AlbumDetail? = remember(albumId) { runCatching { host.core.album(albumId) }.getOrNull() }
    val tint = rememberAlbumTint(host, albumId)
    if (detail == null) {
        EmptyState("This album isn't in your library", "It may have been removed from the server.")
        return
    }
    val album = detail.album
    val ids = remember(detail) { detail.songs.map { it.id } }
    val multiDisc = remember(detail) { detail.songs.mapNotNull { it.discNumber }.distinct().size > 1 }

    fun playFrom(index: Int, shuffle: Boolean) {
        haptics.perform(Haptic.Soft)
        host.core.play(ids, index.toUInt(), StartedFrom.ALBUM, album.id.toString())
        // Play keeps the album's order; Shuffle shuffles what follows the first song.
        host.core.setShuffle(shuffle)
        PlaybackHub.playCurrent(context)
    }

    Box(Modifier.fillMaxSize().background(tint.field)) {
        LazyColumn(contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp)) {
            item {
                Column(Modifier.fillMaxWidth().statusBarsPadding().padding(horizontal = (Grid.base * 2).dp)) {
                    Row(Modifier.fillMaxWidth().padding(vertical = Grid.base.dp)) {
                        // Item 1: tonal buttons over the tinted page.
                        FilledTonalIconButton(onClick = onBack, colors = IconButtonDefaults.filledTonalIconButtonColors(containerColor = tint.content.copy(alpha = 0.14f), contentColor = tint.content)) {
                            Icon(Icons.back(), contentDescription = "Back")
                        }
                        // More (Add to Library, Download, Share, Go to Artist) comes with the menus in Phase 3.
                    }
                    Artwork(
                        ArtworkOf.ALBUM,
                        album.id,
                        ART_DP.dp,
                        Modifier.align(Alignment.CenterHorizontally).widthIn(max = ART_DP.dp).fillMaxWidth(0.72f),
                        cornerRadius = ART_RADIUS_DP.dp,
                    )
                    Spacer(Modifier.height((Grid.base * 3).dp))
                    Text(album.title, style = type.collectionTitle, color = tint.content, modifier = Modifier.semantics { heading() })
                    Spacer(Modifier.height(Grid.base.dp))
                    // Item 3: the artist link, the full text width plus 8 dp of padding.
                    Text(
                        album.artist,
                        style = type.nowPlayingSubtitle,
                        color = tint.content.copy(alpha = 0.86f),
                        modifier = Modifier
                            .clickable { artistOf(host, detail)?.let { onOpen(Route.Artist(it)) } }
                            .padding(vertical = Grid.base.dp),
                    )
                    Text(
                        listOfNotNull(album.year?.toString(), songCount(album.songCount.toInt()), formatLength(album.durationMs)).joinToString(" · "),
                        style = type.body,
                        color = tint.secondary,
                    )
                    Spacer(Modifier.height((Grid.base * 2).dp))
                    // Item 4: Play and Shuffle, one connected group. (Download joins it in Phase 8.)
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
                        // Outer corners fully round, facing corners 10 dp (tokens radius.buttonGroupFacingCorner).
                        val facing = Radius.buttonGroupFacingCorner.dp
                        Button(
                            onClick = { playFrom(0, shuffle = false) },
                            shape = RoundedCornerShape(CornerSize(50), CornerSize(facing), CornerSize(facing), CornerSize(50)),
                            colors = ButtonDefaults.buttonColors(containerColor = tint.content, contentColor = tint.field),
                            modifier = Modifier.weight(1f).height(BUTTON_DP.dp),
                        ) { Text("Play", style = type.sectionHeader) }
                        Button(
                            onClick = { if (ids.isNotEmpty()) playFrom(ids.indices.random(), shuffle = true) },
                            shape = RoundedCornerShape(CornerSize(facing), CornerSize(50), CornerSize(50), CornerSize(facing)),
                            colors = ButtonDefaults.buttonColors(containerColor = tint.content.copy(alpha = 0.16f), contentColor = tint.content),
                            modifier = Modifier.weight(1f).height(BUTTON_DP.dp),
                        ) { Text("Shuffle", style = type.sectionHeader) }
                    }
                    Spacer(Modifier.height((Grid.base * 2).dp))
                }
            }
            itemsIndexed(detail.songs, key = { _, s -> s.id }) { i, song ->
                val disc = song.discNumber
                if (multiDisc && disc != null && (i == 0 || detail.songs[i - 1].discNumber != disc)) {
                    Text(
                        "Disc $disc",
                        style = type.groupLabel,
                        color = tint.secondary,
                        modifier = Modifier.padding(start = (Grid.base * 2).dp, top = (Grid.base * 2).dp, bottom = 4.dp).semantics { heading() },
                    )
                }
                // Item 5: a tap plays the song and queues the rest of the album.
                SongRow(
                    song,
                    playing = song.id == playingSongId,
                    onClick = { playFrom(i, shuffle = false) },
                    trackNumber = song.trackNumber?.toInt() ?: (i + 1),
                    contentColor = tint.content,
                    secondaryColor = tint.secondary,
                )
                HorizontalDivider(Modifier.padding(horizontal = (Grid.base * 2).dp), color = tint.content.copy(alpha = 0.12f))
            }
        }
    }
}

private fun songCount(n: Int) = if (n == 1) "1 song" else "$n songs"

/** The album artist's page, found by name among the library's artists. */
private fun artistOf(host: CoreHost, detail: AlbumDetail): Long? {
    val name = detail.songs.firstOrNull()?.albumArtist?.takeIf { it.isNotBlank() } ?: detail.album.artist
    return runCatching { host.core.search(name).artists.firstOrNull { it.name.equals(name, ignoreCase = true) }?.id }.getOrNull()
}

private const val ART_DP = 280
private const val ART_RADIUS_DP = 28
private const val BUTTON_DP = 56
