package uk.gcartee.encore.app

import androidx.compose.runtime.saveable.SaverScope
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uk.gcartee.encore.app.ui.nav.Navigator
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.nav.Tab

class NavigatorTest {
    private fun nav() = Navigator(Tab.Home, emptyMap())

    @Test
    fun eachTabKeepsItsOwnStack() {
        val n = nav()
        n.select(Tab.Library)
        n.push(Route.Album(1))
        n.select(Tab.Search)
        n.push(Route.Artist(2))
        n.select(Tab.Library)
        assertEquals(Route.Album(1), n.top)
        n.select(Tab.Search)
        assertEquals(Route.Artist(2), n.top)
    }

    @Test
    fun tappingTheActiveTabReturnsToItsRootThenScrollsToTop() {
        val n = nav()
        n.select(Tab.Library)
        n.push(Route.Album(1))
        n.push(Route.Artist(2))
        n.select(Tab.Library) // board 05, item 6: back to the tab's root
        assertNull(n.top)
        val before = n.reselect
        n.select(Tab.Library) // at the root: scroll to top
        assertEquals(before + 1, n.reselect)
    }

    @Test
    fun backPopsThenGoesHomeThenStops() {
        val n = nav()
        n.select(Tab.Library)
        n.push(Route.Album(1))
        assertTrue(n.back())
        assertEquals(Tab.Library, n.tab)
        assertTrue(n.back())
        assertEquals(Tab.Home, n.tab)
        assertFalse(n.back())
    }

    @Test
    fun theStacksSurviveARecreation() {
        val n = nav()
        n.select(Tab.Library)
        n.push(Route.Album(7))
        n.push(Route.Playlist(9))
        val saved = with(Navigator.saver) { SaverScope { true }.save(n) }!!
        val back = Navigator.saver.restore(saved)!!
        assertEquals(Tab.Library, back.tab)
        assertEquals(listOf(Route.Album(7), Route.Playlist(9)), back.stack())
    }

    @Test
    fun routesRoundTripAndBadOnesAreDropped() {
        listOf(Route.Album(1), Route.Artist(-3), Route.Playlist(42)).forEach {
            assertEquals(it, Route.decode(it.encode()))
        }
        assertNull(Route.decode("album:x"))
        assertNull(Route.decode("song:1"))
        assertNull(Route.decode("nonsense"))
    }
}
