package uk.gcartee.encore.app

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.ui.player.collapseCloses
import uk.gcartee.encore.app.ui.player.isCollapseDirection

/**
 * The swipe down that closes Now Playing. The owner (9 November 2026) found it too
 * sensitive, worst in landscape; a tap could close it. Ways in dp: about 480 on a phone
 * in portrait, 200 (the floor) on a phone in landscape.
 */
class CollapseGestureTest {
    @Test
    fun aQuickTapThatDriftsNeverCloses() {
        // Fast, but the cover has barely moved: this closed it before (900 dp/s, no distance).
        assertFalse(collapseCloses(draggedDp = 6f, wayDp = PORTRAIT, velocityDpPerS = 2500f))
        assertFalse(collapseCloses(draggedDp = 20f, wayDp = LANDSCAPE, velocityDpPerS = 1800f))
    }

    @Test
    fun aRealFlickCloses() {
        assertTrue(collapseCloses(draggedDp = 60f, wayDp = PORTRAIT, velocityDpPerS = 1500f))
        assertTrue(collapseCloses(draggedDp = 45f, wayDp = LANDSCAPE, velocityDpPerS = 1300f))
    }

    @Test
    fun aSlowDragClosesPastThirtyPercentOfTheWay() {
        assertFalse(collapseCloses(draggedDp = 130f, wayDp = PORTRAIT, velocityDpPerS = 200f))
        assertTrue(collapseCloses(draggedDp = 150f, wayDp = PORTRAIT, velocityDpPerS = 200f))
    }

    @Test
    fun landscapeNeedsARealDragToo() {
        // Before: 18 % of a short way, about 22 dp.
        assertFalse(collapseCloses(draggedDp = 30f, wayDp = LANDSCAPE, velocityDpPerS = 300f))
        assertFalse(collapseCloses(draggedDp = 70f, wayDp = LANDSCAPE, velocityDpPerS = 0f))
        assertTrue(collapseCloses(draggedDp = 85f, wayDp = LANDSCAPE, velocityDpPerS = 0f))
    }

    @Test
    fun aFlickBackUpKeepsItOpen() {
        assertFalse(collapseCloses(draggedDp = 300f, wayDp = PORTRAIT, velocityDpPerS = -1500f))
    }

    @Test
    fun aShortWayCanStillBeClosedByDragging() {
        // Never more than 60 % of the way, even when 80 dp is most of it.
        assertTrue(collapseCloses(draggedDp = 70f, wayDp = 110f, velocityDpPerS = 0f))
    }

    @Test
    fun onlyAMostlyDownwardMoveStartsIt() {
        assertTrue(isCollapseDirection(dx = 0f, dy = 16f))
        assertTrue(isCollapseDirection(dx = -6f, dy = 15f))
        // Diagonal: left to the artwork's sideways skip, or to nothing.
        assertFalse(isCollapseDirection(dx = 12f, dy = 12f))
        assertFalse(isCollapseDirection(dx = 16f, dy = 2f))
        // Up never closes.
        assertFalse(isCollapseDirection(dx = 0f, dy = -16f))
    }

    private companion object {
        const val PORTRAIT = 480f
        const val LANDSCAPE = 200f
    }
}
