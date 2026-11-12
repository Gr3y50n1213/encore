package uk.gcartee.encore.app.ui.player

import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.sin

/**
 * One still frame of the album field behind Now Playing, in plain Kotlin: four blobs of the
 * album's stops over its base. It is the same picture as the AGSL shader and the brush
 * fallback in `AlbumField.kt` at phase 0 (what Reduce Motion shows), drawn on the portrait
 * composition (the long side measured over the short one) so any shape it is stretched to
 * keeps the colors in the same places. Widgets draw their background from it (CORE-API,
 * "Widgets", Colors). No Android classes and no core: JVM-testable, and safe for a widget.
 *
 * The anchors, the travel and the blob falloff (`exp(-5 d²)`, d in widths) are `AlbumField`'s.
 */
object FieldMath {
    private val ANCHOR_X = floatArrayOf(0.20f, 0.85f, 0.25f, 0.80f)
    private val ANCHOR_Y = floatArrayOf(0.18f, 0.35f, 0.75f, 0.92f)

    /** The shader's drift phases for the four blobs: `cos(t + PHASE_X)` and `sin(1.3 t + PHASE_Y)` at t = 0. */
    private val PHASE_X = floatArrayOf(0f, 1.7f, 3.4f, 5.1f)
    private val PHASE_Y = floatArrayOf(0f, 1.1f, 2.2f, 3.3f)

    /** The blobs drift 6 to 10 % of the width (the album gradient token's middle). */
    const val TRAVEL = 0.08f

    /** A phone held upright: about 2.2 tall for every 1 wide. */
    const val ASPECT = 2.2f

    /**
     * [w] x [h] pixels of the field, row by row, as opaque ARGB ints. [stops] has four colors.
     * [aspect] is the long side over the short, in the portrait composition (the default, the
     * app's); a widget passes its own height over its width so the blobs stay round on a wide
     * or short tile instead of being stretched with the picture.
     */
    fun pixels(base: Int, stops: List<Int>, w: Int, h: Int, aspect: Float = ASPECT): IntArray {
        require(stops.size == 4) { "the field has four stops" }
        val out = IntArray(w * h)
        val cx = FloatArray(4) { ANCHOR_X[it] + TRAVEL * cos(PHASE_X[it]) }
        val cy = FloatArray(4) { (ANCHOR_Y[it] + TRAVEL * sin(PHASE_Y[it])) * aspect }
        val baseRgb = floatArrayOf(red(base), green(base), blue(base))
        val stopRgb = Array(4) { floatArrayOf(red(stops[it]), green(stops[it]), blue(stops[it])) }
        for (y in 0 until h) {
            val v = (y + 0.5f) / h * aspect
            for (x in 0 until w) {
                val u = (x + 0.5f) / w
                var r = baseRgb[0]
                var g = baseRgb[1]
                var b = baseRgb[2]
                for (i in 0 until 4) {
                    val dx = u - cx[i]
                    val dy = v - cy[i]
                    val k = exp(-(dx * dx + dy * dy) * 5f)
                    r += (stopRgb[i][0] - r) * k
                    g += (stopRgb[i][1] - g) * k
                    b += (stopRgb[i][2] - b) * k
                }
                out[y * w + x] = (0xFF shl 24) or (to8(r) shl 16) or (to8(g) shl 8) or to8(b)
            }
        }
        return out
    }

    private fun red(c: Int) = ((c shr 16) and 0xFF).toFloat()
    private fun green(c: Int) = ((c shr 8) and 0xFF).toFloat()
    private fun blue(c: Int) = (c and 0xFF).toFloat()
    private fun to8(v: Float) = (v + 0.5f).toInt().coerceIn(0, 255)
}
