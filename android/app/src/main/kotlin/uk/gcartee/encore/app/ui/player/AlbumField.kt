package uk.gcartee.encore.app.ui.player

import android.graphics.RuntimeShader
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.withInfiniteAnimationFrameMillis
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.FloatState
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ShaderBrush
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.semantics.clearAndSetSemantics
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.ui.library.paletteColor
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.app.ui.theme.irisDarkScheme
import uk.gcartee.encore.app.ui.theme.irisLightScheme
import uk.gcartee.encore.core.ArtworkPalette
import uk.gcartee.encore.core.PaletteScheme
import uk.gcartee.encore.tokens.EncoreTokens.AlbumGradient
import uk.gcartee.encore.tokens.EncoreTokens.Motion.Timing

/** The colours Now Playing is drawn in, from the album's palette (board 04). */
data class FieldColours(val base: Color, val stops: List<Color>, val text: Color, val secondary: Color, val dark: Boolean)

/**
 * The album's palette for the field, cross-faded over 600 ms when the album changes; the
 * same album keeps its field (`albumGradient.sameAlbumKeepsField`). No artwork: the
 * profile accent (the theme).
 */
@Composable
fun rememberFieldColours(host: CoreHost, albumId: Long?): FieldColours {
    var palette by remember { mutableStateOf<ArtworkPalette?>(null) }
    LaunchedEffect(albumId) {
        palette = albumId?.let { id -> runCatching { host.core.albumPalette(id) }.getOrNull() }
    }
    val scheme = MaterialTheme.colorScheme
    val p = palette
    val base = p?.let { paletteColor(it.base) } ?: scheme.surfaceContainerHighest
    val stops = p?.stops?.map(::paletteColor)?.takeIf { it.size >= 4 }
        ?: listOf(scheme.primaryContainer, scheme.tertiaryContainer, scheme.secondaryContainer, scheme.surfaceContainer)
    val text = p?.let { paletteColor(it.text) } ?: scheme.onSurface
    val fade = tween<Color>(AlbumGradient.crossfadeMs)
    val b by animateColorAsState(base, fade, label = "base")
    val s0 by animateColorAsState(stops[0], fade, label = "s0")
    val s1 by animateColorAsState(stops[1], fade, label = "s1")
    val s2 by animateColorAsState(stops[2], fade, label = "s2")
    val s3 by animateColorAsState(stops[3], fade, label = "s3")
    val t by animateColorAsState(text, fade, label = "text")
    val dark = p?.scheme != PaletteScheme.LIGHT
    return FieldColours(b, listOf(s0, s1, s2, s3), t, t.copy(alpha = 0.72f), dark)
}

/**
 * The app's theme re-tinted with the album's colours, for the sheets opened over Now
 * Playing (More's menu and the sheets it leads to, Play on). The sheet takes the field's
 * base, its groups a little of the text colour over that, and its accents the text colour,
 * so it reads as part of the page instead of a grey panel laid over it (owner, 9 November
 * 2026: the menu should match Now Playing better). Shapes, type and motion stay the theme's.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun FieldSheetTheme(colours: FieldColours, content: @Composable () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val tinted = remember(colours, scheme) {
        fun tone(share: Float) = colours.text.copy(alpha = share).compositeOver(colours.base)
        // The theme's red is picked for the theme's own surfaces. Over an album colour, the
        // light or the dark scheme's red, whichever stands out more.
        val error = listOf(irisDarkScheme().error, irisLightScheme().error).maxBy { contrast(it, colours.base) }
        scheme.copy(
            primary = colours.text,
            onPrimary = colours.base,
            // A checked tonal toggle (the sheet's Like) fills with secondary: the album's
            // text colour, so a liked song stands out in the album's own colours.
            secondary = colours.text,
            onSecondary = colours.base,
            secondaryContainer = tone(TONE_SELECTED),
            onSecondaryContainer = colours.text,
            surface = colours.base,
            onSurface = colours.text,
            surfaceVariant = tone(TONE_GROUP),
            onSurfaceVariant = colours.secondary,
            surfaceContainerLowest = colours.base,
            surfaceContainerLow = colours.base,
            surfaceContainer = tone(TONE_FAINT),
            surfaceContainerHigh = tone(TONE_GROUP),
            surfaceContainerHighest = tone(TONE_RAISED),
            outline = tone(TONE_OUTLINE),
            outlineVariant = tone(TONE_SELECTED),
            error = error,
        )
    }
    MaterialTheme(
        colorScheme = tinted,
        motionScheme = MaterialTheme.motionScheme,
        shapes = MaterialTheme.shapes,
        typography = MaterialTheme.typography,
        content = content,
    )
}

/** The contrast ratio of two colours (WCAG), 1 to 21. */
private fun contrast(a: Color, b: Color): Float {
    val la = a.luminance() + 0.05f
    val lb = b.luminance() + 0.05f
    return if (la > lb) la / lb else lb / la
}

// How much of the text colour is laid over the base for each tone of a tinted sheet.
private const val TONE_FAINT = 0.06f
private const val TONE_GROUP = 0.1f
private const val TONE_RAISED = 0.17f
private const val TONE_SELECTED = 0.22f
private const val TONE_OUTLINE = 0.5f

/**
 * The moving field behind Now Playing: four blobs of the album's stops over its base,
 * drifting 6 to 10 % of the width, one turn every 40 s. An AGSL shader from Android 13
 * (tokens albumGradient.android), radial brushes before; one still frame with Reduce
 * Motion. Decorative, so hidden from TalkBack. Drawn in its own layer at most
 * [DRIFT_FRAME_MS] apart, reading the phase where it draws: the slow drift doesn't need
 * the display's 120 Hz, and nothing else on the page redraws with it.
 */
@Composable
fun AlbumField(colours: FieldColours, modifier: Modifier = Modifier) {
    val phase = rememberDriftPhase(still = reduceMotion(), turnMs = Timing.GradientBlobLoopSeconds.min * 1000)
    Box(modifier.background(colours.base).clearAndSetSemantics { }) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ShaderField(colours, phase)
        } else {
            BrushField(colours, phase)
        }
    }
}

/**
 * The drift's angle for the blobs' `cos(t)` and `sin(1.3 t)`. Those two only meet their
 * starting point again after ten turns of t, so one loop is ten turns: a one-turn loop
 * snapped the blobs back every 40 s, which showed as a flicker (owner, 9 November 2026).
 * Steps at most every [DRIFT_FRAME_MS]; an infinite animation, so UI tests can pause it.
 */
@Composable
internal fun rememberDriftPhase(still: Boolean, turnMs: Int): FloatState {
    val phase = remember { mutableFloatStateOf(0f) }
    LaunchedEffect(still, turnMs) {
        if (still) {
            phase.floatValue = 0f
            return@LaunchedEffect
        }
        val loopMs = turnMs.toLong() * DRIFT_TURNS_PER_LOOP
        val start = withInfiniteAnimationFrameMillis { it }
        var shown = start - DRIFT_FRAME_MS
        while (true) {
            withInfiniteAnimationFrameMillis { now ->
                if (now - shown >= DRIFT_FRAME_MS) {
                    shown = now
                    phase.floatValue = ((now - start) % loopMs).toFloat() / turnMs * (2 * PI).toFloat()
                }
            }
        }
    }
    return phase
}

private const val DRIFT_TURNS_PER_LOOP = 10
private const val DRIFT_FRAME_MS = 33L

private val travel = (AlbumGradient.BlobTravelPercentOfWidth.min + AlbumGradient.BlobTravelPercentOfWidth.max) / 200f

private val anchors = listOf(Offset(0.2f, 0.18f), Offset(0.85f, 0.35f), Offset(0.25f, 0.75f), Offset(0.8f, 0.92f))

@Composable
private fun BrushField(colours: FieldColours, phase: FloatState) {
    Canvas(Modifier.fillMaxSize().graphicsLayer { }) {
        val t = phase.floatValue
        colours.stops.forEachIndexed { i, c ->
            val a = anchors[i]
            val centre = Offset(
                size.width * (a.x + travel * cos(t + i * 1.7f)),
                size.height * (a.y + travel * sin(t * 1.3f + i * 1.1f)),
            )
            val r = size.maxDimension * 0.55f
            drawCircle(Brush.radialGradient(listOf(c.copy(alpha = 0.9f), Color.Transparent), centre, r), r, centre)
        }
    }
}

private const val SHADER = """
uniform float2 size;
uniform float t;
uniform float travel;
layout(color) uniform half4 base;
layout(color) uniform half4 c0;
layout(color) uniform half4 c1;
layout(color) uniform half4 c2;
layout(color) uniform half4 c3;

float blob(float2 uv, float2 centre) {
    float2 d = uv - centre;
    return exp(-dot(d, d) * 5.0);
}

half4 main(float2 p) {
    float2 uv = p / size;
    float aspect = size.y / max(size.x, 1.0);
    uv.y *= aspect;
    float2 p0 = float2(0.20 + travel * cos(t),        (0.18 + travel * sin(t * 1.3)) * aspect);
    float2 p1 = float2(0.85 + travel * cos(t + 1.7),  (0.35 + travel * sin(t * 1.3 + 1.1)) * aspect);
    float2 p2 = float2(0.25 + travel * cos(t + 3.4),  (0.75 + travel * sin(t * 1.3 + 2.2)) * aspect);
    float2 p3 = float2(0.80 + travel * cos(t + 5.1),  (0.92 + travel * sin(t * 1.3 + 3.3)) * aspect);
    half3 col = base.rgb;
    col = mix(col, c0.rgb, half(blob(uv, p0)));
    col = mix(col, c1.rgb, half(blob(uv, p1)));
    col = mix(col, c2.rgb, half(blob(uv, p2)));
    col = mix(col, c3.rgb, half(blob(uv, p3)));
    return half4(col, 1.0);
}
"""

@RequiresApi(Build.VERSION_CODES.TIRAMISU)
@Composable
private fun ShaderField(colours: FieldColours, phase: FloatState) {
    val shader = remember { RuntimeShader(SHADER) }
    Canvas(Modifier.fillMaxSize().graphicsLayer { }) {
        shader.setFloatUniform("size", size.width, size.height)
        shader.setFloatUniform("t", phase.floatValue)
        shader.setFloatUniform("travel", travel)
        shader.setColorUniform("base", colours.base.toArgb())
        shader.setColorUniform("c0", colours.stops[0].toArgb())
        shader.setColorUniform("c1", colours.stops[1].toArgb())
        shader.setColorUniform("c2", colours.stops[2].toArgb())
        shader.setColorUniform("c3", colours.stops[3].toArgb())
        drawRect(ShaderBrush(shader))
    }
}
