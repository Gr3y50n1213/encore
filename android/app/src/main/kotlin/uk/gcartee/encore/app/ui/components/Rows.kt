package uk.gcartee.encore.app.ui.components

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.sp
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.AlbumSummary
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.core.Song
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.TouchTarget

/** 3:42, or 1:02:05 past an hour. */
fun formatDuration(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, s / 60 % 60, s % 60) else "%d:%02d".format(s / 60, s % 60)
}

/** "46 min", or "1 hr 12 min". */
fun formatLength(ms: Long): String {
    val min = (ms / 60_000).coerceAtLeast(0)
    return if (min >= 60) "${min / 60} hr ${min % 60} min" else "$min min"
}

/**
 * A song row (board 08, item 5). [leading] is the track number on an album, the cover
 * elsewhere. The playing row shows level bars instead of its number.
 */
@Composable
fun SongRow(
    song: Song,
    playing: Boolean,
    onClick: () -> Unit,
    onLongClick: (() -> Unit)? = null,
    trackNumber: Int? = null,
    showArtwork: Boolean = trackNumber == null,
    subtitle: String? = song.artist,
    contentColor: Color = MaterialTheme.colorScheme.onSurface,
    secondaryColor: Color = MaterialTheme.colorScheme.onSurfaceVariant,
) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = (TouchTarget.android + Grid.base).dp)
            .combinedClickable(
                onClick = onClick,
                onLongClick = onLongClick?.let { { haptics.perform(Haptic.LongPress); it() } },
                onLongClickLabel = onLongClick?.let { "More options" },
            )
            .semantics { if (playing) stateDescription = "Playing" }
            .padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        when {
            // The cover is held to its row size; without it, it fills the row width.
            showArtwork -> Artwork(ArtworkOf.SONG, song.id, ROW_ART_DP.dp, Modifier.size(ROW_ART_DP.dp))
            else -> Column(Modifier.width(NUMBER_DP.dp), horizontalAlignment = Alignment.Start) {
                if (playing) LevelBars(contentColor) else Text(
                    trackNumber?.toString().orEmpty(),
                    style = type.rowValue,
                    color = secondaryColor,
                    textAlign = TextAlign.Start,
                )
            }
        }
        Spacer(Modifier.width((Grid.base * 2).dp))
        Column(Modifier.weight(1f)) {
            Text(song.title, style = type.songRowTitle, color = contentColor, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (subtitle != null && showArtwork) {
                Text(subtitle, style = type.body, color = secondaryColor, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        // A long title ends in its ellipsis with room to spare, never against the time.
        Spacer(Modifier.width(Grid.base.dp))
        if (showArtwork && playing) {
            LevelBars(contentColor)
            Spacer(Modifier.width(Grid.base.dp))
        }
        Text(formatDuration(song.durationMs), style = type.rowValue, color = secondaryColor)
    }
}

/** Three bars that move while a song plays (board 08, item 5). Decorative. */
@Composable
fun LevelBars(colour: Color, size: Dp = 16.dp) {
    val t = rememberInfiniteTransition(label = "bars")
    val a by t.animateFloat(0.3f, 1f, infiniteRepeatable(tween(420), RepeatMode.Reverse), label = "a")
    val b by t.animateFloat(1f, 0.4f, infiniteRepeatable(tween(530), RepeatMode.Reverse), label = "b")
    val c by t.animateFloat(0.5f, 0.9f, infiniteRepeatable(tween(370), RepeatMode.Reverse), label = "c")
    Canvas(Modifier.size(size)) {
        val w = this.size.width / 5f
        listOf(a, b, c).forEachIndexed { i, h ->
            val bar = this.size.height * h
            drawRoundRect(
                colour,
                topLeft = Offset(i * 2 * w, this.size.height - bar),
                size = Size(w, bar),
                cornerRadius = CornerRadius(w / 2),
            )
        }
    }
}

/** An album tile in a grid or a row (boards 05 and 06). */
@Composable
fun AlbumTile(album: AlbumSummary, onClick: () -> Unit, onLongClick: (() -> Unit)? = null, modifier: Modifier = Modifier, card: Boolean = false) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    // Board 05: a tile presses to 0.97 on a spring instead of a flat ripple over it.
    val press = androidx.compose.runtime.remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val pressed = press.collectIsPressedAsState().value
    val scale = androidx.compose.animation.core.animateFloatAsState(if (pressed) TILE_PRESSED else 1f, androidx.compose.animation.core.spring(dampingRatio = 0.6f, stiffness = androidx.compose.animation.core.Spring.StiffnessMedium), label = "press").value
    Column(
        modifier
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            }
            .combinedClickable(
                interactionSource = press,
                indication = null,
                onClick = onClick,
                onLongClick = onLongClick?.let { { haptics.perform(Haptic.LongPress); it() } },
            ),
    ) {
        if (card) {
            // The Library grid: a tonal card, the cover inset with concentric corners and
            // the words inside it, so each album reads as one object.
            androidx.compose.material3.Surface(
                shape = androidx.compose.foundation.shape.RoundedCornerShape(CARD_RADIUS_DP.dp),
                color = MaterialTheme.colorScheme.surfaceContainerLow,
            ) {
                Column(Modifier.padding(CARD_INSET_DP.dp)) {
                    Artwork(ArtworkOf.ALBUM, album.id, TILE_ART_DP.dp, Modifier.fillMaxWidth(), cornerRadius = (CARD_RADIUS_DP - CARD_INSET_DP).dp)
                    Column(Modifier.padding(start = 6.dp, end = 6.dp, top = 10.dp, bottom = 6.dp)) {
                        Text(album.title, style = type.tileTitle.copy(fontSize = 14.sp, lineHeight = 18.sp), color = MaterialTheme.colorScheme.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(album.artist, style = type.tileSubtitle.copy(fontSize = 13.sp, lineHeight = 17.sp), color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        } else {
            Artwork(ArtworkOf.ALBUM, album.id, TILE_ART_DP.dp, Modifier.fillMaxWidth(), cornerRadius = TILE_RADIUS_DP.dp)
            Spacer(Modifier.size(TILE_TEXT_GAP_DP.dp))
            Text(album.title, style = type.tileTitle, color = MaterialTheme.colorScheme.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(album.artist, style = type.tileSubtitle, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/** A plain list row: artists, playlists, genres. */
@Composable
fun ListRow(title: String, subtitle: String?, onClick: () -> Unit, leading: @Composable (() -> Unit)? = null) {
    val type = LocalEncoreType.current
    // A rounded press highlight inset from the edges, larger type, room to breathe.
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = Grid.base.dp)
            .clip(androidx.compose.foundation.shape.RoundedCornerShape(LIST_ROW_RADIUS_DP.dp))
            .combinedClickable(onClick = onClick)
            .heightIn(min = LIST_ROW_DP.dp)
            .padding(horizontal = Grid.base.dp, vertical = Grid.base.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        leading?.let {
            it()
            Spacer(Modifier.width((Grid.base * 2).dp))
        }
        Column(Modifier.weight(1f)) {
            Text(title, style = type.listRowTitle.copy(fontSize = 16.sp, lineHeight = 22.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Medium), color = MaterialTheme.colorScheme.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
            subtitle?.let { Text(it, style = type.body.copy(fontSize = 14.sp, lineHeight = 20.sp), color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis) }
        }
    }
}

// Sizes measured from boards 06 and 08 (Android phone).
const val ROW_ART_DP = 48
const val NUMBER_DP = 28
const val TILE_ART_DP = 172
const val TILE_RADIUS_DP = 20
private const val TILE_PRESSED = 0.97f
private const val TILE_TEXT_GAP_DP = 8
private const val CARD_RADIUS_DP = 24
private const val CARD_INSET_DP = 8
private const val LIST_ROW_DP = 72
private const val LIST_ROW_RADIUS_DP = 16
/** List rows' art: 56 dp, 12 dp corners (a circle for artists). */
const val LIST_ART_DP = 56
const val LIST_ART_RADIUS_DP = 12
