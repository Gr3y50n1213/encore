package uk.gcartee.encore.app.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.ui.components.AlbumTile
import uk.gcartee.encore.app.ui.components.TILE_ART_DP
import uk.gcartee.encore.app.ui.library.EmptyState
import uk.gcartee.encore.app.ui.library.SectionTitle
import uk.gcartee.encore.app.ui.library.SyncRow
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.AlbumSort
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/**
 * Board 05 for Phase 2: the first frame comes from the local mirror, never empty. Made
 * for you, Recently played and Rediscover come from the smart-picks engine with Phase 5;
 * until then Home holds the library shortcuts the "New profile" state describes.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(host: CoreHost, onOpen: (Route) -> Unit, reselect: Int, contentPadding: PaddingValues) {
    val type = LocalEncoreType.current
    val sync by host.sync.state.collectAsState()
    val recent = remember(sync.generation) { runCatching { host.core.albums(AlbumSort.RECENTLY_ADDED, 0u, ROW_ITEMS.toUInt()) }.getOrDefault(emptyList()) }
    val list = rememberLazyListState()
    ScrollToTopOn(reselect, list)
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        // Whole screen: pull down to refresh the library.
        PullToRefreshBox(isRefreshing = sync.running, onRefresh = { host.sync.syncNow() }) {
            LazyColumn(state = list, contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp)) {
                item {
                    Column(Modifier.statusBarsPadding().padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = (Grid.base * 2).dp)) {
                        Text("Home", style = type.pageTitle, color = MaterialTheme.colorScheme.onSurface, modifier = Modifier.semantics { heading() })
                    }
                }
                item { SyncRow(sync) }
                if (recent.isEmpty()) {
                    item { EmptyState("Your library is on its way", "Albums appear here as soon as the first sync finishes.") }
                } else {
                    item { SectionTitle("Recently added") }
                    item { AlbumRow(recent, onOpen) }
                }
            }
        }
    }
}

@Composable
private fun AlbumRow(albums: List<uk.gcartee.encore.core.AlbumSummary>, onOpen: (Route) -> Unit) {
    LazyRow(contentPadding = PaddingValues(horizontal = (Grid.base * 2).dp), horizontalArrangement = Arrangement.spacedBy((Grid.base * 2).dp)) {
        items(albums, key = { it.id }) { a ->
            AlbumTile(a, onClick = { onOpen(Route.Album(a.id)) }, modifier = Modifier.width((TILE_ART_DP * 3 / 4).dp))
        }
    }
}

/** Board 05, item 6: tapping the active tab scrolls to top. */
@Composable
fun ScrollToTopOn(reselect: Int, list: LazyListState) {
    LaunchedEffect(reselect) { if (reselect > 0) list.animateScrollToItem(0) }
}

/** Board 23 arrives with Phase 5; until then the tab says what it will hold. */
@Composable
fun MixesScreen() {
    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        Text(
            "Mixes",
            style = LocalEncoreType.current.pageTitle,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.padding(start = (Grid.base * 2).dp, top = (Grid.base * 2).dp).semantics { heading() },
        )
        EmptyState("Mixes are still learning", "Play music and your mixes start to appear here.")
    }
}

private const val ROW_ITEMS = 20
