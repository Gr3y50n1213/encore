package uk.gcartee.encore.app.ui.components

import androidx.compose.foundation.layout.Box
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import uk.gcartee.encore.core.Song

/** The default row swipes (board 42): right is Play Next, left is Add to Queue. */
@Composable
fun defaultSwipes(song: Song): Pair<SwipeAction, SwipeAction> {
    val actions = LocalAppActions.current
    val next = Icons.playNext()
    val queue = Icons.queue()
    return remember(song.id) {
        SwipeAction("Play Next", destructive = false, icon = { next }) { actions.playNext(listOf(song)) } to
            SwipeAction("Add to Queue", destructive = false, icon = { queue }) { actions.addToQueue(listOf(song)) }
    }
}

/**
 * A song in any list: tap plays, a long press (400 ms, the system's timing) opens the
 * song menu with one LONG_PRESS haptic (board 13), and the row swipes of board 42.
 * Pass [toStart] or [toEnd] to replace a swipe (Remove in the queue, Not in this mix).
 */
@Composable
fun SongItem(
    song: Song,
    playing: Boolean,
    onClick: () -> Unit,
    trackNumber: Int? = null,
    subtitle: String? = song.artist,
    place: MenuPlace = MenuPlace(),
    contentColor: Color = MaterialTheme.colorScheme.onSurface,
    secondaryColor: Color = MaterialTheme.colorScheme.onSurfaceVariant,
    rowColor: Color = MaterialTheme.colorScheme.surface,
    toEnd: SwipeAction? = defaultSwipes(song).first,
    toStart: SwipeAction? = defaultSwipes(song).second,
) {
    var menu by remember { mutableStateOf(false) }
    SwipeRow(toEnd = toEnd, toStart = toStart, rowColor = rowColor) {
        Box {
            SongRow(
                song,
                playing = playing,
                onClick = onClick,
                onLongClick = { menu = true },
                trackNumber = trackNumber,
                subtitle = subtitle,
                contentColor = contentColor,
                secondaryColor = secondaryColor,
            )
            SongMenu(song, expanded = menu, onDismiss = { menu = false }, place = place)
        }
    }
}
