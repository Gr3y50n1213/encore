package uk.gcartee.encore.app.ui.player

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.playback.PlayerState
import uk.gcartee.encore.app.ui.components.Artwork
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Radius

/**
 * Board 05, item 5 and board 42: the mini player above the bar. Its body opens Now
 * Playing (tap or swipe up); left skips forward and right goes back, the next song
 * sliding in under the finger; play and next stay put and act in place, debounced
 * 200 ms, following the Android shape rule (board 35). TalkBack gets Next song and
 * Previous song as custom actions (rule 12).
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MiniPlayer(host: CoreHost, state: PlayerState, onExpand: () -> Unit, modifier: Modifier = Modifier, bar: Boolean = false) {
    val entry = state.entry ?: return
    val context = LocalContext.current
    val type = LocalEncoreType.current
    var endOfQueue by remember { mutableStateOf(false) }
    // Board 04: a tonal container lightly tinted by the album's dominant stop.
    val field = rememberFieldColours(host, entry.song.albumId)
    val container = androidx.compose.ui.graphics.lerp(MaterialTheme.colorScheme.secondaryContainer, field.stops.first(), MINI_TINT)
    // Board 42: Next is a tonal button, round on the side its arrow points to.
    val tonal = IconButtonDefaults.filledTonalIconButtonColors(
        containerColor = MaterialTheme.colorScheme.surfaceContainerLowest,
        contentColor = MaterialTheme.colorScheme.onSurface,
    )
    Surface(
        color = container,
        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
        shape = RoundedCornerShape(Radius.androidCard.dp),
        modifier = modifier
            .testTag("mini-player")
            .fillMaxWidth()
            .padding(horizontal = Grid.base.dp)
            .semantics(mergeDescendants = false) {
                customActions = listOf(
                    CustomAccessibilityAction("Next song") { PlaybackHub.next(context); true },
                    CustomAccessibilityAction("Previous song") { PlaybackHub.previous(context); true },
                )
            },
    ) {
        Row(Modifier.padding(Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
            SkipSwipe(
                host,
                current = entry,
                onNext = { PlaybackHub.swipeNext(context) },
                onPrevious = { PlaybackHub.swipePrevious(context) },
                onSwipeUp = onExpand,
                onEndOfQueue = { endOfQueue = it },
                modifier = Modifier.weight(1f),
            ) { e, slot ->
                val song = e?.song ?: return@SkipSwipe
                Row(
                    Modifier
                        .fillMaxWidth()
                        .let { if (slot == Slot.Current) it.clickable(onClickLabel = "Open Now Playing", onClick = onExpand) else it }
                        .semantics(mergeDescendants = true) {
                            contentDescription = "${song.title}, ${song.artist}"
                            if (slot == Slot.Current) onClick("Open Now Playing") { onExpand(); true }
                        },
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // Concentric with the container: its radius less the 8 dp gap, so the cover's
                    // corners follow the pill's curve instead of poking into it.
                    Artwork(ArtworkOf.SONG, song.id, ART_DP.dp, Modifier.size(ART_DP.dp), cornerRadius = (Radius.androidCard - Grid.base).dp)
                    Spacer(Modifier.width((Grid.base * 1.5f).dp))
                    Column(Modifier.weight(1f)) {
                        Text(song.title, style = type.songRowTitle, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(
                            if (endOfQueue && slot == Slot.Current) "End of queue" else song.artist,
                            style = type.body,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
            // Board 29, item 5: on wider windows a bar with the whole transport, 48 dp or more
            // apart, so a stray touch only expands the bar.
            if (bar) {
                SkipButton(next = false, onClick = { PlaybackHub.previous(context) }, size = TransportSize.Bar, colors = tonal)
                Spacer(Modifier.width(Grid.base.dp))
            }
            Box {
                PlayPauseButton(
                    playing = state.isPlaying,
                    loading = state.buffering,
                    onToggle = { PlaybackHub.togglePlay(context) },
                    size = if (bar) TransportSize.Bar else TransportSize.Compact,
                    // Board 42: the filled accent button, shaped by its state (board 35).
                    container = MaterialTheme.colorScheme.primary,
                    onContainer = MaterialTheme.colorScheme.onPrimary,
                )
            }
            Spacer(Modifier.width(if (bar) Grid.base.dp else 4.dp))
            SkipButton(next = true, onClick = { PlaybackHub.next(context) }, size = if (bar) TransportSize.Bar else TransportSize.Compact, colors = tonal)
        }
    }
}

private const val ART_DP = 48
private const val MINI_TINT = 0.14f
