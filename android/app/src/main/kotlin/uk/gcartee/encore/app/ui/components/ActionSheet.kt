package uk.gcartee.encore.app.ui.components

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/**
 * One action in an [ActionSheet]. [reason] set: it can't be done now (board 13,
 * "Offline: Download and Share dim with a reason"); it dims, and a tap says why.
 */
class SheetAction(val label: String, val icon: Painter? = null, val destructive: Boolean = false, val reason: String? = null, val onClick: () -> Unit)

/**
 * The app's one menu for a song, album or mix (boards 13 and 24, Material 3 Expressive):
 * a sheet with the item at the top, the three commonest actions as a connected group of
 * tonal tiles, then the rest in rounded tonal groups like Settings (24 dp outer, 6 dp
 * inner corners), the destructive group last and in red. The sheet closes first and the
 * action runs after, so nothing happens behind a closing sheet. The owner chose this over
 * board 13's popup on 7 November 2026.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ActionSheet(
    onDismiss: () -> Unit,
    header: (@Composable () -> Unit)? = null,
    quick: List<SheetAction> = emptyList(),
    groups: List<List<SheetAction>>,
) {
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = rememberCoroutineScope()
    val haptics = rememberHaptics()
    val toaster = LocalAppActions.current.toaster
    fun run(a: SheetAction) {
        a.reason?.let {
            haptics.perform(Haptic.Reject)
            toaster.show(it)
            return
        }
        haptics.perform(Haptic.Soft)
        scope.launch { sheet.hide() }.invokeOnCompletion {
            onDismiss()
            a.onClick()
        }
    }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheet,
        containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
    ) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(bottom = (Grid.base * 2).dp)) {
            header?.let {
                Box(Modifier.padding(horizontal = (Grid.base * 3).dp)) { it() }
                Spacer(Modifier.height((Grid.base * 2.5f).dp))
            }
            if (quick.isNotEmpty()) {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp),
                    horizontalArrangement = Arrangement.spacedBy(QUICK_GAP_DP.dp),
                ) {
                    quick.forEachIndexed { i, a -> QuickTile(a, first = i == 0, last = i == quick.lastIndex, modifier = Modifier.weight(1f)) { run(a) } }
                }
                Spacer(Modifier.height((Grid.base * 2).dp))
            }
            groups.filter { it.isNotEmpty() }.forEach { group ->
                Column(Modifier.padding(horizontal = (Grid.base * 2).dp), verticalArrangement = Arrangement.spacedBy(ROW_GAP_DP.dp)) {
                    group.forEachIndexed { i, a -> SheetRow(a, top = if (i == 0) OUTER_DP else INNER_DP, bottom = if (i == group.lastIndex) OUTER_DP else INNER_DP) { run(a) } }
                }
                Spacer(Modifier.height((Grid.base * 1.5f).dp))
            }
        }
    }
}

/** A connected tile: outer corners round, facing corners tight; pressed, it rounds fully. */
@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun QuickTile(a: SheetAction, first: Boolean, last: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val press = remember { MutableInteractionSource() }
    val pressed by press.collectIsPressedAsState()
    val facing by animateDpAsState(if (pressed) OUTER_DP.dp else FACING_DP.dp, MaterialTheme.motionScheme.fastSpatialSpec(), label = "facing")
    val outer = OUTER_DP.dp
    val start = if (first) outer else facing
    val end = if (last) outer else facing
    Surface(
        onClick = onClick,
        interactionSource = press,
        shape = RoundedCornerShape(topStart = start, bottomStart = start, topEnd = end, bottomEnd = end),
        color = MaterialTheme.colorScheme.secondaryContainer,
        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
        modifier = modifier.heightIn(min = QUICK_HEIGHT_DP.dp).graphicsLayer { alpha = if (a.reason != null) OFFLINE_DIM else 1f },
    ) {
        Column(Modifier.padding(vertical = (Grid.base * 1.5f).dp, horizontal = Grid.base.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
            a.icon?.let { Icon(it, contentDescription = null, modifier = Modifier.size(24.dp)) }
            Spacer(Modifier.height(6.dp))
            Text(a.label, style = LocalEncoreType.current.buttonLabelSelected.copy(fontSize = 13.sp), textAlign = TextAlign.Center, maxLines = 2)
        }
    }
}

@Composable
private fun SheetRow(a: SheetAction, top: Int, bottom: Int, onClick: () -> Unit) {
    val colour = if (a.destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface
    Surface(
        onClick = onClick,
        shape = RoundedCornerShape(topStart = top.dp, topEnd = top.dp, bottomStart = bottom.dp, bottomEnd = bottom.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier.fillMaxWidth().heightIn(min = ROW_HEIGHT_DP.dp).graphicsLayer { alpha = if (a.reason != null) OFFLINE_DIM else 1f },
    ) {
        Row(Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
            a.icon?.let {
                Icon(it, contentDescription = null, tint = if (a.destructive) colour else MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width((Grid.base * 2).dp))
            }
            Text(a.label, style = LocalEncoreType.current.listRowTitle.copy(fontSize = 16.sp), color = colour, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

private const val OUTER_DP = 24
private const val INNER_DP = 6
private const val FACING_DP = 8
private const val QUICK_GAP_DP = 4
private const val QUICK_HEIGHT_DP = 84
private const val ROW_GAP_DP = 2
private const val ROW_HEIGHT_DP = 56
