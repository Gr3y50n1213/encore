package uk.gcartee.encore.app

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlin.math.abs
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uk.gcartee.encore.app.ui.player.AlbumField
import uk.gcartee.encore.app.ui.player.FieldColours

/**
 * The album field has the same composition in portrait and in landscape (owner,
 * 11 November 2026: turning the phone changed the gradient too much). The same field is
 * drawn on a tall box and on the same box turned on its side; the colour at each
 * screen-relative point must match. The shader had measured its blobs against the width,
 * so on a wide box each blob covered most of the height and the four colours smeared
 * into one. API 28 runs the brush field and newer Androids the shader.
 */
@RunWith(AndroidJUnit4::class)
class AlbumFieldAspectTest {
    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    // Golden Hour, the app's artwork fixture: base 3D0000, stops CF4630 1B1030 C02D56 986E00.
    private val colours = FieldColours(
        base = Color(0xFF3D0000),
        stops = listOf(Color(0xFFCF4630), Color(0xFF1B1030), Color(0xFFC02D56), Color(0xFF986E00)),
        text = Color.White,
        secondary = Color.White.copy(alpha = 0.72f),
        dark = true,
    )

    // The shader's anchors (AlbumField.kt): where the four blobs sit, as fractions of the screen.
    private val points = listOf(0.20f to 0.18f, 0.85f to 0.35f, 0.25f to 0.75f, 0.80f to 0.92f, 0.5f to 0.5f)

    @Test
    fun landscapeKeepsPortraitsColoursAtTheSameScreenPoints() {
        // The drift's phase stays at 0 (infinite animations are cancelled in tests), so
        // both boxes show the same still frame.
        rule.mainClock.autoAdvance = false
        rule.setContent {
            // 120x264 dp and 264x120 dp side by side: 384 dp, which fits every CI screen.
            Row {
                Box(Modifier.size(120.dp, 264.dp).testTag("portrait")) { AlbumField(colours, Modifier.size(120.dp, 264.dp)) }
                Box(Modifier.size(264.dp, 120.dp).testTag("landscape")) { AlbumField(colours, Modifier.size(264.dp, 120.dp)) }
            }
        }
        rule.mainClock.advanceTimeByFrame()
        rule.waitForIdle()

        val portrait = rule.onNodeWithTag("portrait").captureToImage().toPixelMap()
        val landscape = rule.onNodeWithTag("landscape").captureToImage().toPixelMap()

        fun at(map: androidx.compose.ui.graphics.PixelMap, rx: Float, ry: Float): Color =
            map[(rx * (map.width - 1)).toInt(), (ry * (map.height - 1)).toInt()]

        // Eight 8-bit steps: dithering and the two resolutions' rounding.
        val tolerance = 8f / 255f
        val samples = points.map { (rx, ry) -> at(portrait, rx, ry) to at(landscape, rx, ry) }
        samples.forEachIndexed { i, (p, l) ->
            val diff = maxOf(abs(p.red - l.red), abs(p.green - l.green), abs(p.blue - l.blue))
            assertTrue("point $i: portrait $p, landscape $l (largest channel difference $diff)", diff <= tolerance)
        }
        // A control: the field really has structure (the four colours differ from each other).
        val distinct = samples.map { it.first }.distinctBy { (it.red * 8).toInt() * 100 + (it.green * 8).toInt() * 10 + (it.blue * 8).toInt() }
        assertTrue("the field shows more than one colour: $distinct", distinct.size >= 3)
    }
}
