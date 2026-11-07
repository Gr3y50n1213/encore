package uk.gcartee.encore.app.ui.player

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledIconToggleButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.playback.PlayerState
import uk.gcartee.encore.app.ui.components.Artwork
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.tokens.EncoreTokens.AndroidToggleShape
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Radius
import uk.gcartee.encore.tokens.EncoreTokens.TouchTarget

/**
 * Board 05, item 5: the mini player above the bar. A tap on its body opens Now Playing;
 * play and next act in place, debounced 200 ms. TalkBack gets Next song and Previous
 * song as custom actions (section 26, rule 14). The swipes come with board 42.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MiniPlayer(state: PlayerState, onExpand: () -> Unit, onToggle: () -> Unit, onNext: () -> Unit, onPrevious: () -> Unit, modifier: Modifier = Modifier) {
    val entry = state.entry ?: return
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val next = rememberDebounced {
        haptics.perform(Haptic.Transport)
        onNext()
    }
    val toggle = rememberDebounced {
        haptics.perform(Haptic.Transport)
        onToggle()
    }
    Surface(
        color = MaterialTheme.colorScheme.secondaryContainer,
        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
        shape = RoundedCornerShape(Radius.androidCard.dp),
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = Grid.base.dp)
            .semantics(mergeDescendants = false) {
                customActions = listOf(
                    CustomAccessibilityAction("Next song") { onNext(); true },
                    CustomAccessibilityAction("Previous song") { onPrevious(); true },
                )
            },
    ) {
        Row(Modifier.padding(Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
            Row(
                Modifier
                    .weight(1f)
                    .clickable(onClickLabel = "Open Now Playing", onClick = onExpand)
                    .semantics(mergeDescendants = true) {
                        contentDescription = "${entry.song.title}, ${entry.song.artist}"
                        onClick("Open Now Playing") { onExpand(); true }
                    },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Artwork(ArtworkOf.SONG, entry.song.id, ART_DP.dp, Modifier.size(ART_DP.dp), cornerRadius = Grid.base.dp)
                Spacer(Modifier.width((Grid.base * 1.5f).dp))
                Column(Modifier.weight(1f)) {
                    Text(entry.song.title, style = type.songRowTitle, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(entry.song.artist, style = type.body, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            // The compact toggle: 44 dp, keeping a 48 dp touch target.
            FilledIconToggleButton(
                checked = state.isPlaying,
                onCheckedChange = { toggle() },
                shapes = IconButtonDefaults.toggleableShapes(
                    shape = RoundedCornerShape(percent = AndroidToggleShape.restRadiusPercent),
                    pressedShape = RoundedCornerShape((AndroidToggleShape.Sizes.Compact.height * AndroidToggleShape.pressedRadiusPercentOfHeight / 100).dp),
                    checkedShape = RoundedCornerShape(AndroidToggleShape.Sizes.Compact.checkedRadius.dp),
                ),
                colors = IconButtonDefaults.filledIconToggleButtonColors(
                    containerColor = MaterialTheme.colorScheme.secondaryContainer,
                    contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                    checkedContainerColor = MaterialTheme.colorScheme.secondaryContainer,
                    checkedContentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                ),
                modifier = Modifier.size(AndroidToggleShape.Sizes.Compact.touchTarget.dp).semantics {
                    contentDescription = if (state.isPlaying) "Pause" else "Play"
                },
            ) { Icon(if (state.isPlaying) Icons.pause() else Icons.play(), contentDescription = null) }
            IconButton(onClick = next, modifier = Modifier.size(TouchTarget.android.dp)) {
                Icon(Icons.next(), contentDescription = "Next")
            }
        }
    }
}

private const val ART_DP = 48
