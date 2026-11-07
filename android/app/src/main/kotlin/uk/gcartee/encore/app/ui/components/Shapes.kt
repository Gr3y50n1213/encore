package uk.gcartee.encore.app.ui.components

import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Path
import androidx.graphics.shapes.Morph
import androidx.graphics.shapes.RoundedPolygon

/**
 * A Material shape (or a morph between two) as a Compose path scaled to [size]. The
 * shapes come from the library's MaterialShapes (board 24: cookie, sunny, clover, arch,
 * squircle); only the drawing is done here.
 */
fun Morph.toPath(progress: Float, size: Size, path: Path = Path()): Path {
    path.reset()
    var first = true
    asCubics(progress).forEach { c ->
        if (first) {
            path.moveTo(c.anchor0X * size.width, c.anchor0Y * size.height)
            first = false
        }
        path.cubicTo(
            c.control0X * size.width, c.control0Y * size.height,
            c.control1X * size.width, c.control1Y * size.height,
            c.anchor1X * size.width, c.anchor1Y * size.height,
        )
    }
    path.close()
    return path
}

fun RoundedPolygon.toPath(size: Size): Path = Morph(this, this).toPath(0f, size)
