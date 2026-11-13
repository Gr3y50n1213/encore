package uk.gcartee.encore.app

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PixelMap
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
 * Turning the phone must keep the album field's colours without distorting them (owner,
 * 11 and 12 November 2026). Two things are checked on a tall box and on the same box turned
 * on its side: every blob stays a circle (the first fix stretched the portrait picture, which
 * made oval blobs), and each orientation shows about the same amount of colour (the original
 * shader measured blobs against the width, so a wide screen smeared the four colours into
 * one). API 28 runs the brush field and newer Androids the shader.
 */
@RunWith(AndroidJUnit4::class)
class AlbumFieldAspectTest {
    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    private val base = Color(0xFF3D0000)

    // Golden Hour, the app's artwork fixture: base 3D0000, stops CF4630 1B1030 C02D56 986E00.
    private val golden = FieldColours(
        base = base,
        stops = listOf(Color(0xFFCF4630), Color(0xFF1B1030), Color(0xFFC02D56), Color(0xFF986E00)),
        text = Color.White,
        secondary = Color.White.copy(alpha = 0.72f),
        dark = true,
    )

    // Only the first blob shows: the other stops equal the base, so mixing them changes nothing.
    private val single = golden.copy(stops = listOf(Color(0xFFCF4630), base, base, base))

    private fun render(colours: FieldColours): Pair<PixelMap, PixelMap> {
        // The drift's phase stays at 0 (infinite animations are cancelled in tests), so both
        // boxes show the same still frame.
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
        return rule.onNodeWithTag("portrait").captureToImage().toPixelMap() to
            rule.onNodeWithTag("landscape").captureToImage().toPixelMap()
    }

    private fun spread(a: Color, b: Color) = maxOf(abs(a.red - b.red), abs(a.green - b.green), abs(a.blue - b.blue))

    /**
     * The four colours do not smear into one: at each blob's centre the nearest of the four stops
     * is that blob's own, in a tall box and in a wide one. The original landscape measured the
     * blobs against the width, so each covered most of the height and the colours merged. A blob's
     * roundness is not tested pixel by pixel: the other three blobs overlap every sample and shift
     * it by about as much as an oval would (CI: 0.05 apart even in portrait), so the landscape
     * look is judged on the screenshot this test saves. The shader path only: the brush fallback
     * (below Android 13) draws far larger blobs that do blend.
     */
    @Test
    fun theFourColoursStayApartInBothOrientations() {
        if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.TIRAMISU) return
        val (portrait, landscape) = render(golden)
        // At phase 0 blob i is at (ax + 0.08 cos(1.7 i), ay + 0.08 sin(1.1 i)) of the box.
        val anchors = listOf(0.20f to 0.18f, 0.85f to 0.35f, 0.25f to 0.75f, 0.80f to 0.92f)
        fun dist(a: Color, b: Color) = abs(a.red - b.red) + abs(a.green - b.green) + abs(a.blue - b.blue)
        listOf(portrait, landscape).forEach { map ->
            anchors.forEachIndexed { i, (ax, ay) ->
                val x = ((ax + 0.08f * kotlin.math.cos(1.7f * i)) * map.width).toInt().coerceIn(0, map.width - 1)
                val y = ((ay + 0.08f * kotlin.math.sin(1.1f * i)) * map.height).toInt().coerceIn(0, map.height - 1)
                val got = map[x, y]
                val nearest = golden.stops.indices.minBy { dist(got, golden.stops[it]) }
                assertTrue("a ${map.width}x${map.height} box: at blob $i the colour $got is nearest to stop $nearest", nearest == i)
            }
        }
    }

    /** A picture for the owner to judge the landscape look: both boxes, large, on screen. */
    @Test
    fun saveAScreenshotOfBothOrientations() {
        rule.setContent {
            Row {
                Box(Modifier.size(120.dp, 264.dp)) { AlbumField(golden, Modifier.size(120.dp, 264.dp)) }
                Box(Modifier.size(264.dp, 120.dp)) { AlbumField(golden, Modifier.size(264.dp, 120.dp)) }
            }
        }
        rule.waitForIdle()
        Thread.sleep(600)
        val automation = androidx.test.platform.app.InstrumentationRegistry.getInstrumentation().uiAutomation
        val pfd = automation.executeShellCommand("screencap -p /data/local/tmp/encore-shot-album-field-orientations.png")
        android.os.ParcelFileDescriptor.AutoCloseInputStream(pfd).use { it.readBytes() }
    }
    /** Each orientation shows about the same amount of colour: the mean distance from the base. */
    @Test
    fun landscapeShowsAboutAsMuchColourAsPortrait() {
        val (portrait, landscape) = render(golden)
        fun tint(map: PixelMap): Float {
            var sum = 0f
            var n = 0
            for (y in 0 until map.height step 4) for (x in 0 until map.width step 4) {
                sum += spread(map[x, y], base)
                n++
            }
            return sum / n
        }
        val p = tint(portrait)
        val l = tint(landscape)
        assertTrue("portrait tint $p, landscape tint $l (should be within 40 % of each other)", l in (p * 0.6f)..(p * 1.4f))
    }
}
