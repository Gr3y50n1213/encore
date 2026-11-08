package uk.gcartee.encore.app.ui.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CornerSize
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.tokens.EncoreTokens.Radius

/** One button of a connected group. [label] null draws an icon-only square-ish button. */
class GroupButton(
    val label: String?,
    val container: Color,
    val content: Color,
    val icon: Painter? = null,
    val description: String? = null,
    val weight: Float = 1f,
    val enabled: Boolean = true,
    /** Drawn instead of [icon] (the Download button's ring); given the content colour. */
    val custom: (@Composable (Color) -> Unit)? = null,
    val onClick: () -> Unit,
)

/**
 * A connected button group (boards 08 and 29, Material 3 Expressive): the outer corners
 * are fully round and the corners facing a neighbour 10 dp
 * (`radius.buttonGroupFacingCorner`). Pressed, a button morphs fully round and widens
 * while its neighbours narrow, on the expressive motion scheme's fast spatial spring.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ConnectedButtons(buttons: List<GroupButton>, height: Dp, modifier: Modifier = Modifier) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val sources = buttons.map { remember { MutableInteractionSource() } }
    val pressed = sources.map { it.collectIsPressedAsState().value }
    val anyPressed = pressed.any { it }
    val spring = MaterialTheme.motionScheme.fastSpatialSpec<Float>()
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
        buttons.forEachIndexed { i, b ->
            val grow = when {
                pressed[i] -> PRESSED_GROW
                anyPressed -> NEIGHBOUR_SQUEEZE
                else -> 1f
            }
            val weight by animateFloatAsState(b.weight * grow, spring, label = "weight")
            val round = CornerSize(50)
            val facing = CornerSize(Radius.buttonGroupFacingCorner.dp)
            val start = if (i == 0) round else facing
            val end = if (i == buttons.lastIndex) round else facing
            Button(
                onClick = {
                    haptics.perform(Haptic.Soft)
                    b.onClick()
                },
                shapes = ButtonDefaults.shapes(
                    shape = RoundedCornerShape(start, end, end, start),
                    pressedShape = RoundedCornerShape(50),
                ),
                enabled = b.enabled,
                colors = ButtonDefaults.buttonColors(containerColor = b.container, contentColor = b.content),
                // An icon-only end button (Download, Pin) is narrow; its icon needs the room.
                contentPadding = if (b.label == null) androidx.compose.foundation.layout.PaddingValues(0.dp) else androidx.compose.foundation.layout.PaddingValues(horizontal = LABEL_PAD_DP.dp),
                interactionSource = sources[i],
                modifier = Modifier
                    .weight(weight)
                    .height(height)
                    .let { m -> if (b.custom != null && b.description != null) m.semantics { contentDescription = b.description } else m },
            ) {
                b.custom?.invoke(b.content)
                if (b.custom == null) b.icon?.let {
                    Icon(it, contentDescription = b.description, modifier = Modifier.size(22.dp))
                    if (b.label != null) Spacer(Modifier.size(8.dp))
                }
                b.label?.let { Text(it, style = type.sectionHeader.copy(fontSize = LABEL_SP.sp), maxLines = 1, softWrap = false) }
            }
        }
    }
}

private const val PRESSED_GROW = 1.15f
private const val NEIGHBOUR_SQUEEZE = 0.93f
// Labels at 16 sp with 16 dp sides, so "Shuffle" fits beside Play and Download even
// while a neighbour is pressed and this button squeezes.
private const val LABEL_SP = 16
private const val LABEL_PAD_DP = 16
