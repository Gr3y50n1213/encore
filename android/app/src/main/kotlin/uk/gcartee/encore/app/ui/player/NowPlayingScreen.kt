package uk.gcartee.encore.app.ui.player

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.IconToggleButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlayerState
import uk.gcartee.encore.app.ui.components.Artwork
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.formatDuration
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.library.rememberAlbumTint
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/**
 * Board 09, Phase 2: what playing an album needs. The moving gradient, the swipes, the
 * scrub slow-down, lyrics, output and queue come with Phase 3 (boards 09, 10, 11, 42).
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun NowPlayingScreen(
    host: CoreHost,
    state: PlayerState,
    onCollapse: () -> Unit,
    onOpen: (Route) -> Unit,
    onToggle: () -> Unit,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
    onSeek: (Long) -> Unit,
) {
    val entry = state.entry ?: return
    val song = entry.song
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val still = reduceMotion()
    val tint = song.albumId?.let { rememberAlbumTint(host, it) }
    val field = tint?.field ?: androidx.compose.material3.MaterialTheme.colorScheme.surface
    val content = tint?.content ?: androidx.compose.material3.MaterialTheme.colorScheme.onSurface
    val secondary = tint?.secondary ?: androidx.compose.material3.MaterialTheme.colorScheme.onSurfaceVariant

    // Item 2: paused the art shrinks to 88 %, playing it is full size.
    val scale by animateFloatAsState(if (state.isPlaying || still) 1f else PAUSED_SCALE, label = "art")
    var liked by remember(song.id) { mutableStateOf(runCatching { host.core.areLiked(listOf(song.id)).first() }.getOrDefault(false)) }
    // Item 5: a tap on the right time swaps remaining and total.
    var showRemaining by rememberSaveable { mutableStateOf(false) }
    var scrubbing by remember { mutableStateOf<Float?>(null) }

    Box(Modifier.fillMaxSize().background(field)) {
        Column(
            Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = (Grid.base * 3).dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Row(Modifier.fillMaxWidth().padding(vertical = Grid.base.dp)) {
                // Item 1: the arrow collapses (Android).
                FilledTonalIconButton(
                    onClick = onCollapse,
                    colors = IconButtonDefaults.filledTonalIconButtonColors(containerColor = content.copy(alpha = 0.14f), contentColor = content),
                ) { Icon(Icons.collapse(), contentDescription = "Close Now Playing") }
            }
            Spacer(Modifier.weight(0.4f))
            Artwork(
                ArtworkOf.SONG,
                song.id,
                ART_DP.dp,
                Modifier.widthIn(max = ART_DP.dp).fillMaxWidth().graphicsLayer {
                    scaleX = scale
                    scaleY = scale
                },
                cornerRadius = ART_RADIUS_DP.dp,
            )
            Spacer(Modifier.weight(0.4f))
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    // Item 3: the title opens the album, the artist the artist page, collapsing first.
                    Text(
                        song.title,
                        style = type.nowPlayingTitle,
                        color = content,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.semantics { heading() }.clickable(enabled = song.albumId != null) {
                            onCollapse()
                            song.albumId?.let { onOpen(Route.Album(it)) }
                        },
                    )
                    Text(song.artist, style = type.nowPlayingSubtitle, color = secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                // Item 4: like, the "love" signal; syncs through the core.
                IconToggleButton(
                    checked = liked,
                    onCheckedChange = { on ->
                        runCatching { if (on) host.core.like(listOf(song.id)) else host.core.unlike(listOf(song.id)) }
                            .onSuccess {
                                liked = on
                                haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
                            }
                            .onFailure { haptics.perform(Haptic.Reject) }
                    },
                    colors = IconButtonDefaults.iconToggleButtonColors(contentColor = content, checkedContentColor = content),
                    modifier = Modifier.semantics { stateDescription = if (liked) "Liked" else "Not liked" },
                ) { Icon(Icons.favorite(liked), contentDescription = "Like") }
            }
            Spacer(Modifier.height((Grid.base * 2).dp))

            // Item 5: progress. Haptics at the start and end of a scrub only.
            val duration = state.durationMs.coerceAtLeast(1)
            val position = scrubbing?.let { (it * duration).toLong() } ?: state.positionMs
            Slider(
                value = (position.toFloat() / duration).coerceIn(0f, 1f),
                onValueChange = {
                    if (scrubbing == null) haptics.perform(Haptic.ScrubStart)
                    scrubbing = it
                },
                onValueChangeFinished = {
                    scrubbing?.let { onSeek((it * duration).toLong()) }
                    scrubbing = null
                    haptics.perform(Haptic.ScrubEnd)
                },
                colors = SliderDefaults.colors(thumbColor = content, activeTrackColor = content, inactiveTrackColor = content.copy(alpha = 0.24f)),
                modifier = Modifier.fillMaxWidth().semantics { stateDescription = "${formatDuration(position)} of ${formatDuration(duration)}" },
            )
            Row(Modifier.fillMaxWidth()) {
                Text(formatDuration(position), style = type.caption, color = secondary)
                Spacer(Modifier.weight(1f))
                Text(
                    if (showRemaining) "-" + formatDuration(duration - position) else formatDuration(duration),
                    style = type.caption,
                    color = secondary,
                    modifier = Modifier.clickable { showRemaining = !showRemaining },
                )
            }
            Spacer(Modifier.height((Grid.base * 3).dp))
            // Item 6: three separate buttons 8 dp apart.
            TransportRow(
                playing = state.isPlaying,
                loading = state.buffering,
                onPrevious = onPrevious,
                onToggle = onToggle,
                onNext = onNext,
            )
            Spacer(Modifier.weight(0.5f))
        }
    }
    LaunchedEffect(song.id) { scrubbing = null }
}

private const val PAUSED_SCALE = 0.88f
private const val ART_DP = 340
private const val ART_RADIUS_DP = 28
