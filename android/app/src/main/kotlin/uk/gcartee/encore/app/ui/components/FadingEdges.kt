package uk.gcartee.encore.app.ui.components

import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Lists drawn straight on an album's field (Lyrics and the Queue on Now Playing) fade out
 * at their top and bottom edges instead of stopping at a hard line, so they melt into the
 * field the way the lyrics board draws them.
 */
fun Modifier.fadingEdges(top: Dp = 20.dp, bottom: Dp = 28.dp): Modifier = this
    .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
    .drawWithContent {
        drawContent()
        if (size.height <= 0f) return@drawWithContent
        val t = (top.toPx() / size.height).coerceIn(0f, 0.5f)
        val b = (bottom.toPx() / size.height).coerceIn(0f, 0.5f)
        drawRect(
            Brush.verticalGradient(0f to Color.Transparent, t to Color.Black, 1f - b to Color.Black, 1f to Color.Transparent),
            blendMode = BlendMode.DstIn,
        )
    }