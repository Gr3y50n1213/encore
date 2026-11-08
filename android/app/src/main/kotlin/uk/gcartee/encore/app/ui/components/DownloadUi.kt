package uk.gcartee.encore.app.ui.components

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.CircularWavyProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.DownloadView
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.DownloadProgress
import uk.gcartee.encore.core.DownloadState
import uk.gcartee.encore.core.DownloadedItem

/** What is downloaded and what plays offline, for every list (Phase 8). */
val LocalDownloads = staticCompositionLocalOf { DownloadView() }

/** The transfer under way, for the rings. */
val LocalDownloadProgress = staticCompositionLocalOf<State<DownloadProgress?>> { error("No download progress provided") }

/** Board 20: what can't play offline dims to this. */
const val OFFLINE_DIM = 0.38f

/** Board 35's blocked action: a 4 dp shake, 3 cycles, 240 ms (none with Reduce Motion). */
class Shake(private val scope: CoroutineScope, private val offset: Animatable<Float, androidx.compose.animation.core.AnimationVector1D>, private val still: Boolean) {
    val x: Float get() = offset.value

    fun go() {
        if (still) return
        scope.launch {
            offset.snapTo(0f)
            repeat(SHAKE_CYCLES) {
                offset.animateTo(SHAKE_DP, tween(SHAKE_MS / (SHAKE_CYCLES * 2)))
                offset.animateTo(-SHAKE_DP, tween(SHAKE_MS / (SHAKE_CYCLES * 2)))
            }
            offset.animateTo(0f, tween(SHAKE_MS / (SHAKE_CYCLES * 2)))
        }
    }
}

@Composable
fun rememberShake(): Shake {
    val scope = rememberCoroutineScope()
    val still = reduceMotion()
    val offset = remember { Animatable(0f) }
    return remember(scope, still) { Shake(scope, offset, still) }
}

/** Moves a refused control sideways while it shakes. */
fun Modifier.shaking(shake: Shake): Modifier = graphicsLayer { translationX = shake.x * density }

/** How far an item has got, counting the song moving now. */
@Composable
fun downloadFraction(item: DownloadedItem?): Float {
    val progress by LocalDownloadProgress.current
    if (item == null || item.total == 0u) return 0f
    val p = progress
    val part = if (item.state == DownloadState.DOWNLOADING && p != null) {
        p.bytesTotal?.takeIf { it > 0uL }?.let { (p.bytesDone.toDouble() / it.toDouble()).toFloat().coerceIn(0f, 1f) } ?: 0f
    } else {
        0f
    }
    return ((item.saved.toFloat() + part) / item.total.toFloat()).coerceIn(0f, 1f)
}

/**
 * Board 06, item 5 on a cover: a small pill. Saved (the offline pin), a ring while it
 * moves, a warning dot when some songs failed; nothing when it isn't downloaded.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun DownloadBadge(item: DownloadedItem?, modifier: Modifier = Modifier) {
    val state = item?.state ?: return
    val fraction = downloadFraction(item)
    val scheme = MaterialTheme.colorScheme
    AnimatedContent(
        targetState = state,
        transitionSpec = { (scaleIn() + fadeIn()) togetherWith (scaleOut() + fadeOut()) },
        label = "badge",
        modifier = modifier,
    ) { s ->
        Box(
            Modifier
                .size(BADGE_DP.dp)
                .background(scheme.surfaceContainerHighest, CircleShape)
                .semantics { contentDescription = badgeWords(s) },
            contentAlignment = Alignment.Center,
        ) {
            when (s) {
                DownloadState.SAVED -> Icon(Icons.saved(), contentDescription = null, tint = scheme.primary, modifier = Modifier.size(18.dp))
                DownloadState.FAILED -> Icon(Icons.error(), contentDescription = null, tint = scheme.error, modifier = Modifier.size(18.dp))
                DownloadState.DOWNLOADING -> CircularProgressIndicator(
                    progress = { fraction },
                    strokeWidth = 2.5.dp,
                    color = scheme.primary,
                    trackColor = scheme.primary.copy(alpha = 0.2f),
                    modifier = Modifier.size(18.dp),
                )
                else -> CircularProgressIndicator(
                    progress = { fraction },
                    strokeWidth = 2.5.dp,
                    color = scheme.onSurfaceVariant,
                    trackColor = scheme.onSurfaceVariant.copy(alpha = 0.2f),
                    modifier = Modifier.size(18.dp),
                )
            }
        }
    }
}

private fun badgeWords(s: DownloadState) = when (s) {
    DownloadState.SAVED -> "Downloaded"
    DownloadState.DOWNLOADING -> "Downloading"
    DownloadState.FAILED -> "Some songs couldn't download"
    DownloadState.PAUSED -> "Download paused"
    DownloadState.WAITING -> "Waiting to download"
}

/** A song row's quiet mark that it is on the device (board 08). */
@Composable
fun SavedMark(colour: Color, modifier: Modifier = Modifier) {
    Icon(
        Icons.saved(),
        contentDescription = "Downloaded",
        tint = colour,
        modifier = modifier.size(SAVED_MARK_DP.dp),
    )
}

/**
 * The page's Download button (boards 08 and 24), in the Play and Shuffle group: an
 * arrow, a wavy ring that fills as songs arrive (tap pauses), a filled check when
 * every song is here (tap asks to remove), or the error mark (tap retries).
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun DownloadGlyph(item: DownloadedItem?, tint: Color) {
    val fraction = downloadFraction(item)
    val animated by animateFloatAsState(fraction, label = "fill")
    val state = item?.state
    AnimatedContent(
        targetState = state,
        transitionSpec = { (scaleIn(initialScale = 0.6f) + fadeIn()) togetherWith (scaleOut(targetScale = 0.6f) + fadeOut()) },
        label = "download",
    ) { s ->
        Box(contentAlignment = Alignment.Center) {
            when (s) {
                null -> Icon(Icons.download(), contentDescription = null, tint = tint)
                DownloadState.SAVED -> Icon(Icons.downloadDone(), contentDescription = null, tint = tint)
                DownloadState.FAILED -> Icon(Icons.error(), contentDescription = null, tint = tint)
                DownloadState.PAUSED -> {
                    CircularProgressIndicator(progress = { animated }, strokeWidth = 2.5.dp, color = tint, trackColor = tint.copy(alpha = 0.22f), modifier = Modifier.size(26.dp))
                    Icon(Icons.download(), contentDescription = null, tint = tint, modifier = Modifier.size(14.dp))
                }
                else -> {
                    CircularWavyProgressIndicator(
                        progress = { animated.coerceAtLeast(0.02f) },
                        color = tint,
                        trackColor = tint.copy(alpha = 0.22f),
                        modifier = Modifier.size(28.dp),
                    )
                    Icon(Icons.pause(), contentDescription = null, tint = tint, modifier = Modifier.size(12.dp))
                }
            }
        }
    }
}

/** What the Download button says to TalkBack, and does. */
fun downloadLabel(item: DownloadedItem?): String = when (item?.state) {
    null -> "Download"
    DownloadState.SAVED -> "Downloaded. Remove download"
    DownloadState.FAILED -> "Some songs couldn't download. Try again"
    DownloadState.PAUSED -> "Download paused. Resume"
    DownloadState.DOWNLOADING, DownloadState.WAITING -> "Downloading, ${item.saved} of ${item.total}. Pause"
}

/** Board 08: "7 of 12 saved", or why it waits. Null when there is nothing to say. */
fun downloadLine(item: DownloadedItem?): String? = when (item?.state) {
    null, DownloadState.SAVED -> null
    DownloadState.FAILED -> "${item.saved} of ${item.total} saved · ${item.failed} couldn't download"
    DownloadState.PAUSED -> "${item.saved} of ${item.total} saved · Paused"
    else -> "${item.saved} of ${item.total} saved"
}

/** A small "Offline" chip (Now Playing, the Mixes stamp). */
@Composable
fun OfflineChip(colour: Color, modifier: Modifier = Modifier) {
    Row(
        modifier
            .background(colour.copy(alpha = 0.14f), RoundedCornerShape(50))
            .padding(horizontal = 10.dp, vertical = 4.dp)
            .semantics { stateDescription = "Offline" },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.offline(), contentDescription = null, tint = colour, modifier = Modifier.size(14.dp))
        Spacer(Modifier.width(6.dp))
        Text("Offline", style = LocalEncoreType.current.caption.copy(fontSize = 12.sp), color = colour)
    }
}

private const val BADGE_DP = 30
private const val SAVED_MARK_DP = 16
private const val SHAKE_DP = 4f
private const val SHAKE_CYCLES = 3
private const val SHAKE_MS = 240

/**
 * Board 03: removing a download asks first, in the app's sheet, with the red action
 * apart from the one that keeps it. What was saved stays in the library.
 */
@Composable
fun ConfirmRemoveDownload(title: String, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    val keep = Icons.downloadDone()
    val remove = Icons.delete()
    ActionSheet(
        onDismiss = onDismiss,
        header = {
            androidx.compose.foundation.layout.Column {
                Text("Remove $title from downloads?", style = LocalEncoreType.current.sectionHeader)
                Text(
                    "It stays in your library and plays when you're online.",
                    style = LocalEncoreType.current.body,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        groups = listOf(
            listOf(SheetAction("Keep it", keep) {}),
            listOf(SheetAction("Remove download", remove, destructive = true, onClick = onConfirm)),
        ),
    )
}
