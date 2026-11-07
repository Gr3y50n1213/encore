package uk.gcartee.encore.app.ui.onboarding

import android.provider.Settings
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import androidx.graphics.shapes.Morph
import kotlin.math.cos
import kotlin.math.sin
import kotlinx.coroutines.delay
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.components.toPath
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.app.ui.theme.tokenColor
import uk.gcartee.encore.core.appName
import uk.gcartee.encore.tokens.EncoreTokens.Color.Accents.Iris
import uk.gcartee.encore.tokens.EncoreTokens.Color.WireframeAccent
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Motion.Timing

/** Whether the person turned animations off (Settings, Remove animations). */
@Composable
fun reduceMotion(): Boolean {
    val resolver = LocalContext.current.contentResolver
    return remember(resolver) {
        Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
    }
}

/**
 * Board 22. Dark in both themes by design; the gradient and the shape are decorative and
 * hidden from TalkBack, so the buttons are the only focus stops.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun WelcomeScreen(onGetStarted: () -> Unit, onExistingProfile: (() -> Unit)?) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val still = reduceMotion()
    // Item 3, GUARD: Get started ignores taps for 300 ms after the screen appears.
    var armed by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(GUARD_MS)
        armed = true
    }
    // Item 1: the name and the line fade up over 500 ms, staggered by 120 ms.
    val nameIn = remember { Animatable(if (still) 1f else 0f) }
    val lineIn = remember { Animatable(if (still) 1f else 0f) }
    LaunchedEffect(Unit) {
        if (still) return@LaunchedEffect
        nameIn.animateTo(1f, tween(FADE_UP_MS))
    }
    LaunchedEffect(Unit) {
        if (still) return@LaunchedEffect
        delay(STAGGER_MS)
        lineIn.animateTo(1f, tween(FADE_UP_MS))
    }

    Box(Modifier.fillMaxSize().background(tokenColor(Iris.Dark.surfaceContainerLowest))) {
        DriftingField(still, Modifier.fillMaxSize().clearAndSetSemantics { })
        Column(
            modifier = Modifier
                .align(Alignment.Center)
                .fillMaxSize()
                .safeDrawingPadding()
                .widthIn(max = CENTRED_COLUMN_DP.dp)
                .padding(horizontal = (Grid.base * 3).dp, vertical = (Grid.base * 2).dp),
        ) {
            Spacer(Modifier.weight(0.6f))
            WelcomeShape(still, Modifier.align(Alignment.CenterHorizontally).size(SHAPE_DP.dp).clearAndSetSemantics { })
            Spacer(Modifier.height((Grid.base * 5).dp))
            Text(
                appName(),
                style = type.displayTitle,
                color = Color.White,
                modifier = Modifier.graphicsLayer {
                    alpha = nameIn.value
                    translationY = (1f - nameIn.value) * RISE_PX
                },
            )
            Spacer(Modifier.height(Grid.base.dp))
            Text(
                // Item 2: one sentence, the privacy stance, no feature list.
                "Your music, from your own server, with nothing watching.",
                style = type.sectionHeader.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Normal),
                color = Color.White.copy(alpha = 0.78f),
                modifier = Modifier.graphicsLayer {
                    alpha = lineIn.value
                    translationY = (1f - lineIn.value) * RISE_PX
                },
            )
            Spacer(Modifier.weight(1f))
            Button(
                onClick = {
                    if (!armed) return@Button
                    haptics.perform(Haptic.Soft)
                    onGetStarted()
                },
                colors = ButtonDefaults.buttonColors(containerColor = Color.White, contentColor = tokenColor(Iris.Light.onPrimaryContainer)),
                modifier = Modifier.fillMaxWidth().height(BUTTON_DP.dp),
            ) { Text("Get started", style = type.sectionHeader) }
            Spacer(Modifier.height(Grid.base.dp))
            FilledTonalButton(
                onClick = { onExistingProfile?.invoke() },
                enabled = onExistingProfile != null,
                colors = ButtonDefaults.filledTonalButtonColors(
                    containerColor = Color.White.copy(alpha = 0.16f),
                    contentColor = Color.White,
                    disabledContainerColor = Color.White.copy(alpha = 0.08f),
                    disabledContentColor = Color.White.copy(alpha = 0.38f),
                ),
                modifier = Modifier.fillMaxWidth().height(BUTTON_DP.dp),
            ) { Text("I already have a profile", style = type.sectionHeader) }
            Spacer(Modifier.height(Grid.base.dp))
            // Item 5: the policy links. TODO(owner): the privacy policy page does not exist
            // yet (docs/DECISIONS.md section 4); both stay disabled until it does.
            Row(Modifier.align(Alignment.CenterHorizontally), horizontalArrangement = Arrangement.Center) {
                TextButton(onClick = {}, enabled = false) { Text("Privacy policy", style = type.caption) }
                TextButton(onClick = {}, enabled = false) { Text("Open-source licenses", style = type.caption) }
            }
        }
    }
}

/** The cookie shape, turning slowly and morphing to sunny and back every 8 s (item 1). */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun WelcomeShape(still: Boolean, modifier: Modifier) {
    val morph = remember { Morph(MaterialShapes.Cookie9Sided, MaterialShapes.Sunny) }
    val transition = rememberInfiniteTransition(label = "welcome-shape")
    val progress by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(MORPH_HALF_MS, easing = androidx.compose.animation.core.FastOutSlowInEasing), RepeatMode.Reverse),
        label = "morph",
    )
    val turn by transition.animateFloat(
        initialValue = 0f,
        targetValue = 360f,
        animationSpec = infiniteRepeatable(tween(TURN_MS, easing = LinearEasing)),
        label = "turn",
    )
    val colour = Color.White.copy(alpha = 0.92f)
    Canvas(modifier.rotate(if (still) 0f else turn)) {
        drawPath(morph.toPath(if (still) 0f else progress, size), colour)
    }
}

/** Board 22, item 1: the app's own palette drifting on a 40 s loop; one still frame for Reduce Motion. */
@Composable
private fun DriftingField(still: Boolean, modifier: Modifier) {
    val transition = rememberInfiniteTransition(label = "welcome-field")
    val t by transition.animateFloat(
        initialValue = 0f,
        targetValue = (2 * Math.PI).toFloat(),
        animationSpec = infiniteRepeatable(tween(Timing.GradientBlobLoopSeconds.min * 1000, easing = LinearEasing)),
        label = "drift",
    )
    val blobs = remember {
        listOf(
            Blob(tokenColor(WireframeAccent.ios), 0.22f, 0.18f, 0.75f, 0f),
            Blob(tokenColor(Iris.Light.primary), 0.92f, 0.42f, 0.7f, 2.1f),
            Blob(tokenColor(Iris.Dark.tertiary), 0.55f, 0.98f, 0.8f, 4.2f),
        )
    }
    Canvas(modifier.alpha(0.95f)) { blobs.forEach { drawBlob(it, if (still) 0f else t) } }
}

private data class Blob(val colour: Color, val x: Float, val y: Float, val radius: Float, val phase: Float)

private fun DrawScope.drawBlob(b: Blob, t: Float) {
    // Travel 6 to 10 % of the width (tokens albumGradient.blobTravelPercentOfWidth).
    val travel = size.width * 0.08f
    val centre = Offset(
        size.width * b.x + cos(t + b.phase) * travel,
        size.height * b.y + sin(t * 1.3f + b.phase) * travel,
    )
    val r = size.width * b.radius
    drawCircle(Brush.radialGradient(listOf(b.colour.copy(alpha = 0.85f), Color.Transparent), centre, r), r, centre)
}

// Board 22's own numbers.
private const val GUARD_MS = 300L
private const val FADE_UP_MS = 500
private const val STAGGER_MS = 120L
private const val MORPH_HALF_MS = 4_000
private const val TURN_MS = 40_000
private const val SHAPE_DP = 136
private const val BUTTON_DP = 56
private const val CENTRED_COLUMN_DP = 420
private const val RISE_PX = 24f
