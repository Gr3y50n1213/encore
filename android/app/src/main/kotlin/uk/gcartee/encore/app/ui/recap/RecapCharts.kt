package uk.gcartee.encore.app.ui.recap

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin
import kotlin.math.sqrt
import uk.gcartee.encore.app.recap.PackId
import uk.gcartee.encore.app.recap.RecapFormat

/*
 * Board 40, item 4: the data is the artwork. Each pack draws the person's own numbers its
 * own way: Editorial ruled bars and an hour grid, Tide a swell and a tide ring, Bloom pills
 * and petals, Glass bubbles sized by plays and a lens ring. [enter] (0 to 1) eases the shapes
 * in; Reduce Motion passes 1. Charts are drawn only: every value is in the card's text
 * version for screen readers.
 */

/** Months (or days) as bars: [highlight] gets the accent; [letters] sit under them. */
@Composable
fun BarsChart(
    theme: RecapTheme,
    ground: Ground,
    values: List<Long>,
    letters: List<String>,
    enter: Float,
    height: Dp,
    modifier: Modifier = Modifier,
    highlight: Int? = null,
    colour: Color = ground.fg,
    accent: Color = ground.accentShape,
    showValues: Boolean = false,
) {
    val max = (values.maxOrNull() ?: 0L).coerceAtLeast(1L).toFloat()
    val type = theme.type
    Column(modifier.fillMaxWidth()) {
        if (showValues) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                values.forEach { v ->
                    Text(RecapFormat.count(v), style = type.row.copy(fontSize = 9.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center), color = ground.fg, modifier = Modifier.weight(1f))
                }
            }
        }
        Canvas(Modifier.fillMaxWidth().height(height)) {
            val n = values.size.coerceAtLeast(1)
            val gap = (if (showValues) 6 else 4).dp.toPx()
            val bw = (size.width - gap * (n - 1)) / n
            when (theme.pack) {
                PackId.TIDE -> drawSwell(values, max, enter, accent, ground.fg)
                PackId.GLASS -> values.forEachIndexed { i, v ->
                    val cx = i * (bw + gap) + bw / 2f
                    val r = (sqrt(v / max) * min(bw, size.height) * 0.62f).coerceAtLeast(1.dp.toPx()) * enter
                    val cy = size.height - r
                    val c = if (i == highlight) accent else colour
                    drawCircle(Brush.radialGradient(listOf(c.copy(alpha = 0.55f), c.copy(alpha = 0.12f)), center = Offset(cx - r * 0.3f, cy - r * 0.3f), radius = r * 1.4f), radius = r, center = Offset(cx, cy))
                    drawCircle(c.copy(alpha = 0.6f), radius = r, center = Offset(cx, cy), style = Stroke(1.dp.toPx()))
                }
                else -> values.forEachIndexed { i, v ->
                    val bh = size.height * (v / max) * enter
                    val x = i * (bw + gap)
                    val c = when {
                        i == highlight -> accent
                        theme.pack == PackId.BLOOM -> ground.shapes.getOrElse(i % ground.shapes.size.coerceAtLeast(1)) { accent }
                        else -> colour
                    }
                    if (theme.pack == PackId.BLOOM) {
                        drawRoundRect(c, Offset(x, size.height - bh), Size(bw, bh), CornerRadius(bw / 2f, bw / 2f))
                    } else {
                        drawRect(c, Offset(x, size.height - bh), Size(bw, bh))
                    }
                }
            }
            if (theme.pack == PackId.EDITORIAL) drawLine(ground.fg, Offset(0f, size.height), Offset(size.width, size.height), 1.dp.toPx())
        }
        if (letters.isNotEmpty()) {
            Row(Modifier.fillMaxWidth().padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(if (showValues) 6.dp else 4.dp)) {
                letters.forEach { l ->
                    Text(l, style = type.row.copy(fontSize = 7.5.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center), color = ground.fg.copy(alpha = 0.7f), modifier = Modifier.weight(1f))
                }
            }
        }
    }
}

/** Tide's swell: the months as one smooth body of water. */
private fun DrawScope.drawSwell(values: List<Long>, max: Float, enter: Float, colour: Color, line: Color) {
    if (values.isEmpty()) return
    val n = values.size
    val step = if (n > 1) size.width / (n - 1) else size.width
    val pts = values.mapIndexed { i, v -> Offset(i * step, size.height - size.height * 0.9f * (v / max) * enter) }
    val top = smoothPath(pts)
    val fill = smoothPath(pts)
    fill.lineTo(size.width, size.height)
    fill.lineTo(0f, size.height)
    fill.close()
    drawPath(fill, Brush.verticalGradient(listOf(colour, colour.copy(alpha = 0.35f))))
    drawPath(top, line.copy(alpha = 0.8f), style = Stroke(1.5.dp.toPx()))
}

/** A smooth line through [pts] (midpoint quadratic curves). */
private fun smoothPath(pts: List<Offset>): Path {
    val p = Path()
    if (pts.isEmpty()) return p
    p.moveTo(pts[0].x, pts[0].y)
    for (i in 1 until pts.size) {
        val a = pts[i - 1]
        val b = pts[i]
        val mid = Offset((a.x + b.x) / 2f, (a.y + b.y) / 2f)
        p.quadraticTo(a.x, a.y, mid.x, mid.y)
    }
    val last = pts.last()
    p.lineTo(last.x, last.y)
    return p
}

/**
 * The listening clock, 168 hours of the week (Monday 00:00 first). Editorial: a 7 by 24
 * grid; Tide: a tide ring; Bloom: 24 petals; Glass: a lens ring of 24 ticks.
 */
@Composable
fun HourChart(theme: RecapTheme, ground: Ground, grid: List<Long>, peakWeekday: Int, peakHour: Int, enter: Float, modifier: Modifier = Modifier) {
    if (theme.pack == PackId.EDITORIAL) {
        Column(modifier.fillMaxWidth()) {
            Canvas(Modifier.fillMaxWidth().height(84.dp)) {
                val max = (grid.maxOrNull() ?: 0L).coerceAtLeast(1L).toFloat()
                val gapX = 1.1.dp.toPx()
                val gapY = 1.dp.toPx()
                val cw = (size.width - gapX * 23) / 24f
                val ch = (size.height - gapY * 6) / 7f
                for (d in 0 until 7) for (h in 0 until 24) {
                    val v = grid.getOrElse(d * 24 + h) { 0L }
                    val tl = Offset(h * (cw + gapX), d * (ch + gapY))
                    if (d == peakWeekday && h == peakHour) {
                        drawRect(ground.accentShape, tl, Size(cw, ch), alpha = enter)
                        drawRect(ground.accentShape, Offset(tl.x - 2.dp.toPx(), tl.y - 2.dp.toPx()), Size(cw + 4.dp.toPx(), ch + 4.dp.toPx()), style = Stroke(1.dp.toPx()), alpha = enter)
                    } else {
                        drawRect(ground.fg, tl, Size(cw, ch), alpha = (0.12f + 0.84f * v / max) * enter)
                    }
                }
            }
            Row(Modifier.fillMaxWidth().padding(top = 6.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                listOf(0, 6, 12, 18, 24).forEach { h ->
                    Text(RecapFormat.hourShort(h), style = theme.type.row.copy(fontSize = 7.5.sp), color = ground.fg.copy(alpha = 0.7f))
                }
            }
        }
        return
    }
    val byHour = (0 until 24).map { h -> (0 until 7).sumOf { d -> grid.getOrElse(d * 24 + h) { 0L } } }
    val max = (byHour.maxOrNull() ?: 0L).coerceAtLeast(1L).toFloat()
    Canvas(modifier) {
        val c = Offset(size.width / 2f, size.height / 2f)
        val outer = min(size.width, size.height) / 2f
        // Midnight at the top, as on a clock.
        fun angle(h: Int) = (h / 24f) * 2f * PI.toFloat() - PI.toFloat() / 2f
        when (theme.pack) {
            PackId.TIDE -> {
                drawCircle(Color.White.copy(alpha = 0.18f), radius = outer * 0.92f, center = c, style = Stroke(1.dp.toPx()))
                val inner = outer * 0.25f
                val pts = (0 until 24).map { h ->
                    val r = inner + (outer * 0.88f - inner) * (byHour[h] / max) * enter
                    Offset(c.x + r * cos(angle(h)), c.y + r * sin(angle(h)))
                }
                val ring = smoothPath(pts + pts.first())
                ring.close()
                drawPath(ring, Brush.verticalGradient(listOf(ground.accentShape, ground.shapes.firstOrNull() ?: ground.accentShape)))
                val pr = inner + (outer * 0.88f - inner) * (byHour[peakHour] / max) * enter
                drawCircle(Color.White, radius = 4.dp.toPx(), center = Offset(c.x + pr * cos(angle(peakHour)), c.y + pr * sin(angle(peakHour))))
            }
            PackId.BLOOM -> {
                val colours = ground.shapes.ifEmpty { listOf(ground.accentShape) }
                for (h in 0 until 24) {
                    // The petals open one after another.
                    val open = ((enter * 30f - h) / 6f).coerceIn(0f, 1f)
                    val len = outer * (0.25f + 0.75f * byHour[h] / max) * open
                    if (len <= 0f) continue
                    rotate(degrees = h * 15f, pivot = c) {
                        drawOval(colours[h % colours.size], topLeft = Offset(c.x - outer * 0.07f, c.y - len), size = Size(outer * 0.14f, len))
                    }
                }
                drawCircle(Color.White, radius = outer * 0.13f, center = c)
            }
            else -> {
                val r0 = outer * 0.58f
                for (h in 0 until 24) {
                    val len = outer * 0.38f * (0.2f + 0.8f * byHour[h] / max) * enter
                    val a = angle(h)
                    val from = Offset(c.x + r0 * cos(a), c.y + r0 * sin(a))
                    val to = Offset(c.x + (r0 + len) * cos(a), c.y + (r0 + len) * sin(a))
                    val col = if (h == peakHour) ground.accentShape else Color.White.copy(alpha = 0.35f + 0.65f * byHour[h] / max)
                    drawLine(col, from, to, 7.dp.toPx(), cap = StrokeCap.Round)
                }
            }
        }
    }
}

/**
 * The personality's share: a ring (Editorial, Tide, Glass) or Bloom's living blob, filled
 * to [fraction].
 */
@Composable
fun ValueRing(theme: RecapTheme, ground: Ground, fraction: Float, enter: Float, modifier: Modifier = Modifier) {
    Canvas(modifier) {
        val c = Offset(size.width / 2f, size.height / 2f)
        val r = min(size.width, size.height) / 2f
        when (theme.pack) {
            PackId.BLOOM -> {
                val rnd = java.util.Random(7L)
                val lobes = 8
                val pts = (0 until lobes).map { i ->
                    val a = i * 2f * PI.toFloat() / lobes
                    val rr = r * (0.86f + 0.14f * rnd.nextFloat())
                    Offset(c.x + rr * cos(a), c.y + rr * sin(a))
                }
                val blob = smoothPath(pts + pts.first())
                blob.close()
                val colours = ground.shapes.ifEmpty { listOf(ground.accentShape) }
                drawPath(blob, Brush.linearGradient(listOf(colours[0], colours[(1).coerceAtMost(colours.size - 1)])), alpha = 0.35f + 0.65f * enter)
            }
            else -> {
                val stroke = r * 0.25f
                val box = Size(2 * (r - stroke / 2f), 2 * (r - stroke / 2f))
                val tl = Offset(c.x - box.width / 2f, c.y - box.height / 2f)
                drawArc(ground.fg.copy(alpha = 0.15f), 0f, 360f, false, tl, box, style = Stroke(stroke))
                drawArc(ground.accentShape, -90f, 360f * fraction.coerceIn(0f, 1f) * enter, false, tl, box, style = Stroke(stroke))
            }
        }
    }
}

/** Your sound: the top genres' monthly shares stacked as soft layers ("a flowing stream"). */
@Composable
fun StreamChart(months: List<List<Long>>, colours: List<Color>, enter: Float, modifier: Modifier = Modifier) {
    Canvas(modifier) {
        if (months.isEmpty()) return@Canvas
        val layers = months.maxOf { it.size }
        if (layers == 0) return@Canvas
        val n = months.size
        val step = if (n > 1) size.width / (n - 1) else size.width
        val totals = months.map { row -> row.sum().coerceAtLeast(1L).toFloat() }
        // Bottom-up: the first genre at the bottom, each layer stacked on the last.
        var below = List(n) { size.height }
        for (k in 0 until layers) {
            val above = months.mapIndexed { i, row ->
                below[i] - size.height * (row.getOrElse(k) { 0L } / totals[i]) * enter
            }
            val topPts = above.mapIndexed { i, y -> Offset(i * step, y) }
            val bottomPts = below.mapIndexed { i, y -> Offset(i * step, y) }.reversed()
            val p = smoothPath(topPts)
            p.lineTo(bottomPts.first().x, bottomPts.first().y)
            for (pt in bottomPts.drop(1)) p.lineTo(pt.x, pt.y)
            p.close()
            drawPath(p, colours[k % colours.size.coerceAtLeast(1)])
            below = above
        }
    }
}

/** The mood arc: energy (and brightness) by month; months without enough data are skipped. */
@Composable
fun LineChart(energy: List<Long?>, valence: List<Long?>, colour: Color, second: Color, enter: Float, modifier: Modifier = Modifier) {
    Canvas(modifier) {
        val all = (energy + valence).filterNotNull()
        if (all.isEmpty()) return@Canvas
        val lo = (all.min() - 50L).coerceAtLeast(0L).toFloat()
        val hi = (all.max() + 50L).coerceAtMost(1000L).toFloat()
        val span = max(hi - lo, 1f)
        val n = max(energy.size, valence.size)
        val step = if (n > 1) size.width / (n - 1) else size.width
        fun line(values: List<Long?>, c: Color, width: Float) {
            var run = mutableListOf<Offset>()
            fun flush() {
                if (run.size >= 2) drawPath(smoothPath(run), c, style = Stroke(width, cap = StrokeCap.Round))
                else if (run.size == 1) drawCircle(c, width, run[0])
                run = mutableListOf()
            }
            values.forEachIndexed { i, v ->
                if (v == null) flush() else {
                    val y = size.height - size.height * ((v - lo) / span)
                    run.add(Offset(i * step, size.height + (y - size.height) * enter))
                }
            }
            flush()
        }
        line(valence, second, 2.dp.toPx())
        line(energy, colour, 3.dp.toPx())
    }
}

/** A ledger of [count] marks (days of a streak, albums played through), up to [cap]. */
@Composable
fun DotLedger(count: Long, colour: Color, round: Boolean, enter: Float, modifier: Modifier = Modifier, cell: Dp = 10.dp, cap: Int = 120) {
    Canvas(modifier) {
        val c = cell.toPx()
        val gap = c * 0.35f
        val perRow = ((size.width + gap) / (c + gap)).toInt().coerceAtLeast(1)
        val shown = (count.coerceAtMost(cap.toLong()) * enter).toInt()
        for (i in 0 until shown) {
            val x = (i % perRow) * (c + gap)
            val y = (i / perRow) * (c + gap)
            if (y + c > size.height + 0.5f) break
            if (round) drawCircle(colour, c / 2f, Offset(x + c / 2f, y + c / 2f)) else drawRect(colour, Offset(x, y), Size(c, c))
        }
    }
}
