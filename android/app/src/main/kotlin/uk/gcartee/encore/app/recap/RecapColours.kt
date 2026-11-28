package uk.gcartee.encore.app.recap

import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import uk.gcartee.encore.tokens.EncoreTokens.Recap.Variation

/** Colour arithmetic on opaque ARGB ints, plain Kotlin so the contrast rules are tested. */
object RecapColour {
    /** Board 40, item 9: every pack passes a 4.5:1 text contrast check. */
    const val MIN_CONTRAST: Double = Variation.textContrastMin

    const val WHITE: Int = 0xFFFFFFFF.toInt()

    fun hex(s: String?): Int? {
        val t = s?.trim()?.removePrefix("#") ?: return null
        if (t.length != 6) return null
        val v = t.toLongOrNull(16) ?: return null
        return (0xFF000000L or v).toInt()
    }

    fun red(c: Int) = (c shr 16) and 0xFF
    fun green(c: Int) = (c shr 8) and 0xFF
    fun blue(c: Int) = c and 0xFF
    fun rgb(r: Int, g: Int, b: Int): Int = (0xFF shl 24) or (r.coerceIn(0, 255) shl 16) or (g.coerceIn(0, 255) shl 8) or b.coerceIn(0, 255)

    private fun lin(v: Int): Double {
        val s = v / 255.0
        return if (s <= 0.03928) s / 12.92 else ((s + 0.055) / 1.055).pow(2.4)
    }

    /** WCAG relative luminance. */
    fun luminance(c: Int): Double = 0.2126 * lin(red(c)) + 0.7152 * lin(green(c)) + 0.0722 * lin(blue(c))

    /** WCAG contrast ratio, 1 to 21. */
    fun contrast(a: Int, b: Int): Double {
        val la = luminance(a)
        val lb = luminance(b)
        return (max(la, lb) + 0.05) / (min(la, lb) + 0.05)
    }

    /** Hue (0 to 360), saturation and lightness (0 to 1). */
    fun hsl(c: Int): FloatArray {
        val r = red(c) / 255f
        val g = green(c) / 255f
        val b = blue(c) / 255f
        val mx = max(r, max(g, b))
        val mn = min(r, min(g, b))
        val l = (mx + mn) / 2f
        if (mx == mn) return floatArrayOf(0f, 0f, l)
        val d = mx - mn
        val s = if (l > 0.5f) d / (2f - mx - mn) else d / (mx + mn)
        val h = when (mx) {
            r -> ((g - b) / d + (if (g < b) 6f else 0f))
            g -> ((b - r) / d + 2f)
            else -> ((r - g) / d + 4f)
        } * 60f
        return floatArrayOf(h, s, l)
    }

    fun fromHsl(h: Float, s: Float, l: Float): Int {
        val ss = s.coerceIn(0f, 1f)
        val ll = l.coerceIn(0f, 1f)
        val c = (1f - abs(2f * ll - 1f)) * ss
        val hp = ((h % 360f) + 360f) % 360f / 60f
        val x = c * (1f - abs(hp % 2f - 1f))
        val (r1, g1, b1) = when {
            hp < 1f -> Triple(c, x, 0f)
            hp < 2f -> Triple(x, c, 0f)
            hp < 3f -> Triple(0f, c, x)
            hp < 4f -> Triple(0f, x, c)
            hp < 5f -> Triple(x, 0f, c)
            else -> Triple(c, 0f, x)
        }
        val m = ll - c / 2f
        return rgb(((r1 + m) * 255f + 0.5f).toInt(), ((g1 + m) * 255f + 0.5f).toInt(), ((b1 + m) * 255f + 0.5f).toInt())
    }

    fun lightness(c: Int): Float = hsl(c)[2]

    fun saturation(c: Int): Float = hsl(c)[1]

    fun withLightness(c: Int, l: Float): Int = hsl(c).let { fromHsl(it[0], it[1], l) }

    fun withSaturation(c: Int, s: Float): Int = hsl(c).let { fromHsl(it[0], s, it[2]) }

    /** [a] towards [b] by [t] (0 is a, 1 is b), per channel. */
    fun mix(a: Int, b: Int, t: Float): Int = rgb(
        (red(a) + (red(b) - red(a)) * t + 0.5f).toInt(),
        (green(a) + (green(b) - green(a)) * t + 0.5f).toInt(),
        (blue(a) + (blue(b) - blue(a)) * t + 0.5f).toInt(),
    )

    /**
     * [c] darkened (or lightened) in small steps of lightness until [ok] holds, keeping its
     * hue; null when it would have to pass [limit] to get there.
     */
    fun fit(c: Int, darker: Boolean, limit: Float, ok: (Int) -> Boolean): Int? {
        val h = hsl(c)
        var l = h[2]
        for (step in 0..100) {
            val x = fromHsl(h[0], h[1], l)
            if (ok(x)) return x
            l += if (darker) -0.01f else 0.01f
            if (darker && l < limit) return null
            if (!darker && l > limit) return null
        }
        return null
    }

    /** [c] darkened until white text reads on it. */
    fun darkForWhite(c: Int, limit: Float = 0.04f): Int? = fit(c, darker = true, limit = limit) { contrast(it, WHITE) >= MIN_CONTRAST }

    /** [c] lightened until [ink] text reads on it. */
    fun lightForInk(c: Int, ink: Int, limit: Float = 0.9f): Int? = fit(c, darker = false, limit = limit) { contrast(it, ink) >= MIN_CONTRAST }
}

/** The four launch packs (board 40). An unknown pack draws Editorial with a note. */
enum class PackId(val id: String) {
    EDITORIAL("editorial"),
    TIDE("tide"),
    BLOOM("bloom"),
    GLASS("glass");

    companion object {
        fun of(id: String?): PackId? = entries.firstOrNull { it.id == id }
    }
}

/**
 * A card's ground (board 39 draws them): Editorial's spot colour, paper, a second paper and
 * ink; the other packs read them as moods (Tide: sunset, sea, sea, night).
 */
enum class GroundKind { SPOT, PAPER, PAPER2, INK }

/** One card's colours: its background (top to bottom), text, accent text and accent shapes. */
data class GroundColours(
    val kind: GroundKind,
    val top: Int,
    val bottom: Int,
    val fg: Int,
    val accentText: Int,
    val accentShape: Int,
    /** The pack's chart and cover colours for this card, most prominent first. */
    val shapes: List<Int>,
)

/**
 * Board 40, "How the palette was made": each pack takes the year's colour clusters it
 * needs, shades them for its ground and checks text contrast against them, falling back to
 * its own ink and paper when a colour cannot pass.
 */
class PackColours(val pack: PackId, palette: RecapPalette?) {
    /** The year's colours, lead first ("#rrggbb" in the document). */
    val year: List<Int> = palette?.colours?.mapNotNull(RecapColour::hex).orEmpty()

    // Editorial: paper, ink and one spot colour.
    val paper = 0xFFF3EEE4.toInt()
    val paper2 = 0xFFE7DDCB.toInt()
    val ink = 0xFF15110E.toInt()
    val spot: Int = year.asSequence()
        .filter { RecapColour.saturation(it) >= SPOT_MIN_SATURATION }
        .mapNotNull { c -> RecapColour.lightForInk(c, ink, SPOT_MAX_LIGHTNESS)?.takeIf { RecapColour.contrast(it, paper) >= SPOT_FROM_PAPER } }
        .firstOrNull() ?: DEFAULT_SPOT

    // Tide: a sky, four waves and deep water, each dark enough for white text.
    private val tideSource: List<Int> = (year + TIDE_DEFAULTS).distinct()
    val skyTop: Int = RecapColour.darkForWhite(tideSource[0]) ?: 0xFFB8361F.toInt()
    val skyBottom: Int = RecapColour.darkForWhite(tideSource.drop(1).firstOrNull { RecapColour.saturation(it) > 0.2f } ?: TIDE_DEFAULTS[1]) ?: 0xFF9E4A12.toInt()
    val waves: List<Int> = (0 until 4).map { k ->
        val c = tideSource[(k + 2) % tideSource.size]
        val base = RecapColour.darkForWhite(c) ?: TIDE_DEFAULTS[k + 2]
        // Each wave a little deeper than the one above it.
        if (k == 0) base else RecapColour.withLightness(base, (RecapColour.lightness(base) * (1f - 0.12f * k)).coerceAtLeast(0.06f))
    }
    val deep: Int = RecapColour.withLightness(waves.minByOrNull { RecapColour.lightness(it) } ?: 0xFF0B2E4E.toInt(), DEEP_LIGHTNESS)
    val nightTop: Int = RecapColour.withLightness(RecapColour.withSaturation(deep, 0.45f), NIGHT_TOP_LIGHTNESS)
    val nightBottom: Int = RecapColour.withLightness(deep, NIGHT_BOTTOM_LIGHTNESS)
    val seaTop: Int = waves[0]
    val seaBottom: Int = deep

    /** Tide's light accent for night and sea: the brightest year colour that reads on both. */
    val gold: Int = (year + 0xFFF2C14E.toInt()).sortedByDescending { RecapColour.luminance(it) }
        .firstNotNullOfOrNull { c -> RecapColour.fit(c, darker = false, limit = 0.92f) { x -> listOf(nightTop, seaTop).all { RecapColour.contrast(x, it) >= RecapColour.MIN_CONTRAST } } }
        ?: RecapColour.WHITE

    // Bloom: four saturated colour fields on cream, light enough for dark ink text.
    val cream = 0xFFFFF0D2.toInt()
    val bloomInk = 0xFF1E0F0A.toInt()
    val fields: List<Int> = (year.filter { RecapColour.saturation(it) >= 0.25f } + BLOOM_DEFAULTS).distinct().take(4).map { c ->
        val vivid = RecapColour.withSaturation(c, maxOf(RecapColour.saturation(c), 0.75f))
        RecapColour.lightForInk(vivid, bloomInk) ?: 0xFFFF9F1C.toInt()
    }

    // Glass: four glows on near-black, dark enough for white text under a frosted lens.
    val glassBase = 0xFF120E1A.toInt()
    val glows: List<Int> = (year.filter { RecapColour.saturation(it) >= 0.2f } + GLASS_DEFAULTS).distinct().take(4).map { c ->
        RecapColour.fit(c, darker = true, limit = 0.03f) { x ->
            RecapColour.contrast(RecapColour.mix(x, RecapColour.WHITE, LENS_WHITE), RecapColour.WHITE) >= RecapColour.MIN_CONTRAST
        } ?: 0xFF0E3B5C.toInt()
    }

    /** The colours of one card of [kind] in this pack. */
    fun ground(kind: GroundKind): GroundColours = when (pack) {
        PackId.EDITORIAL -> when (kind) {
            GroundKind.SPOT -> GroundColours(kind, spot, spot, ink, ink, ink, listOf(ink, paper, paper2))
            GroundKind.PAPER -> GroundColours(kind, paper, paper, ink, ink, spot, listOf(ink, spot, paper2))
            GroundKind.PAPER2 -> GroundColours(kind, paper2, paper2, ink, ink, spot, listOf(ink, spot, paper))
            GroundKind.INK -> GroundColours(kind, ink, ink, paper, spot, spot, listOf(paper, spot, paper2))
        }
        PackId.TIDE -> when (kind) {
            GroundKind.SPOT -> GroundColours(kind, skyTop, skyBottom, RecapColour.WHITE, RecapColour.WHITE, gold, waves)
            GroundKind.INK -> GroundColours(kind, nightTop, nightBottom, RecapColour.WHITE, gold, gold, waves)
            else -> GroundColours(kind, seaTop, seaBottom, RecapColour.WHITE, gold, gold, waves)
        }
        PackId.BLOOM -> {
            val turn = when (kind) {
                GroundKind.SPOT -> 0
                GroundKind.PAPER -> 1
                GroundKind.PAPER2 -> 2
                GroundKind.INK -> 3
            }
            val rotated = fields.indices.map { fields[(it + turn) % fields.size] }
            GroundColours(kind, cream, cream, bloomInk, bloomInk, rotated[0], rotated)
        }
        PackId.GLASS -> {
            val turn = kind.ordinal
            val rotated = glows.indices.map { glows[(it + turn) % glows.size] }
            GroundColours(kind, glassBase, glassBase, RecapColour.WHITE, RecapColour.WHITE, rotated[0], rotated)
        }
    }

    companion object {
        private const val SPOT_MIN_SATURATION = 0.25f
        private const val SPOT_MAX_LIGHTNESS = 0.8f
        /** The spot colour still reads as a colour beside paper, not as more paper. */
        private const val SPOT_FROM_PAPER = 1.3
        private const val DEEP_LIGHTNESS = 0.1f
        private const val NIGHT_TOP_LIGHTNESS = 0.2f
        private const val NIGHT_BOTTOM_LIGHTNESS = 0.07f
        /** Glass's frosted lens: 14 % white over the glow (board 40). */
        const val LENS_WHITE = 0.14f
        val DEFAULT_SPOT = 0xFFE8452C.toInt()
        private val TIDE_DEFAULTS = listOf(0xFFE8452C, 0xFFF7A23B, 0xFF2FA7A0, 0xFF1F6F8B, 0xFF145079, 0xFF0B2E4E).map { it.toInt() }
        private val BLOOM_DEFAULTS = listOf(0xFFFF9F1C, 0xFFF0442B, 0xFFFF5C9A, 0xFF6A4CFF).map { it.toInt() }
        private val GLASS_DEFAULTS = listOf(0xFFE8452C, 0xFF0E3B5C, 0xFF1F3D2B, 0xFFF2C14E).map { it.toInt() }

        /**
         * Board 39 draws each beat on its own ground; the beat's layout turns it, so the same
         * pack looks different from one year and one person to the next.
         */
        fun groundFor(beatId: String, layout: Int): GroundKind {
            val base = when (beatId) {
                "intro", "topArtist", "obsessionWeek", "longestStreak", "mostSungLine" -> GroundKind.SPOT
                "topSong", "listeningClock", "comeback", "mostReplayedDay" -> GroundKind.INK
                "yourSound", "songOfEachSeason" -> GroundKind.PAPER2
                else -> GroundKind.PAPER
            }
            if (layout <= 0 || beatId == "closer") return base
            val cycle = listOf(GroundKind.SPOT, GroundKind.PAPER, GroundKind.INK, GroundKind.PAPER2)
            return cycle[(cycle.indexOf(base) + layout) % cycle.size]
        }
    }
}
