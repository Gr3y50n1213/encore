package uk.gcartee.encore.app.ui.nav

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue

/** The four destinations of the Android bar (board 05, item 6): Search is the fourth. */
enum class Tab { Home, Mixes, Library, Search }

/** A page pushed inside a tab. Plain data so the stacks survive rotation and folds. */
sealed interface Route {
    data class Album(val id: Long) : Route
    data class Artist(val id: Long) : Route
    data class Playlist(val id: Long) : Route
    data class Mix(val id: String) : Route
    /** Settings and its pages (board 17): "settings", "servers", "privacy", "licences", "playback", "suggestions", "appearance". */
    data class Settings(val page: String = "settings") : Route

    fun encode(): String = when (this) {
        is Album -> "album:$id"
        is Artist -> "artist:$id"
        is Playlist -> "playlist:$id"
        is Mix -> "mix:$id"
        is Settings -> "settings:$page"
    }

    companion object {
        fun decode(s: String): Route? {
            val (kind, id) = s.split(':', limit = 2).takeIf { it.size == 2 } ?: return null
            when (kind) {
                "mix" -> return id.takeIf { it.isNotEmpty() }?.let(::Mix)
                "settings" -> return id.takeIf { it.isNotEmpty() }?.let(::Settings)
            }
            val n = id.toLongOrNull() ?: return null
            return when (kind) {
                "album" -> Album(n)
                "artist" -> Artist(n)
                "playlist" -> Playlist(n)
                else -> null
            }
        }
    }
}

/**
 * One back stack per tab, kept across layout changes (CLAUDE.md: keep state across
 * resizes, folds, rotations). Tapping the active tab again returns to its root
 * (board 05, item 6); the screen scrolls to top on the first tap.
 */
@Stable
class Navigator(initialTab: Tab, stacks: Map<Tab, List<Route>>) {
    var tab by mutableStateOf(initialTab)
        private set
    private val stacks = mutableStateMapOf<Tab, List<Route>>().apply { putAll(stacks) }
    /** Bumped when the active tab is tapped again, so its list scrolls to top. */
    var reselect by mutableStateOf(0)
        private set

    fun stack(t: Tab = tab): List<Route> = stacks[t].orEmpty()
    val top: Route? get() = stack().lastOrNull()
    val canGoBack: Boolean get() = stack().isNotEmpty() || tab != Tab.Home

    fun select(t: Tab) {
        if (t == tab) {
            if (stack().isEmpty()) reselect++ else stacks[t] = emptyList()
        } else {
            tab = t
        }
    }

    fun push(route: Route) {
        if (stack().lastOrNull() == route) return
        stacks[tab] = stack() + route
    }

    /** A profile switch resets to Home (board 14, item 2). */
    fun reset() {
        Tab.entries.forEach { stacks[it] = emptyList() }
        tab = Tab.Home
    }

    /** Back: pop this tab's page, else go to Home; false when there is nowhere to go. */
    fun back(): Boolean {
        val s = stack()
        return when {
            s.isNotEmpty() -> {
                stacks[tab] = s.dropLast(1)
                true
            }
            tab != Tab.Home -> {
                tab = Tab.Home
                true
            }
            else -> false
        }
    }

    companion object {
        val saver: Saver<Navigator, List<String>> = Saver(
            save = { nav ->
                listOf(nav.tab.name) + Tab.entries.map { t -> t.name + "=" + nav.stack(t).joinToString("\u0001") { it.encode() } }
            },
            restore = { saved ->
                val tab = Tab.valueOf(saved.first())
                val stacks = saved.drop(1).associate { line ->
                    val (t, routes) = line.split('=', limit = 2)
                    Tab.valueOf(t) to routes.split('\u0001').filter { it.isNotEmpty() }.mapNotNull(Route::decode)
                }
                Navigator(tab, stacks)
            },
        )
    }
}

@Composable
fun rememberNavigator(): Navigator = rememberSaveable(saver = Navigator.saver) { Navigator(Tab.Home, emptyMap()) }
