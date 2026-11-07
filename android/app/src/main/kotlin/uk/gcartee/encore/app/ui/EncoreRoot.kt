package uk.gcartee.encore.app.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.playback.PlayerState
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.home.HomeScreen
import uk.gcartee.encore.app.ui.home.MixesScreen
import uk.gcartee.encore.app.ui.library.AlbumScreen
import uk.gcartee.encore.app.ui.library.ArtistScreen
import uk.gcartee.encore.app.ui.library.LibraryScreen
import uk.gcartee.encore.app.ui.library.PlaylistScreen
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.nav.Tab
import uk.gcartee.encore.app.ui.nav.rememberNavigator
import uk.gcartee.encore.app.ui.onboarding.ConnectScreen
import uk.gcartee.encore.app.ui.onboarding.WelcomeScreen
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.app.ui.player.MiniPlayer
import uk.gcartee.encore.app.ui.player.NowPlayingScreen
import uk.gcartee.encore.app.ui.search.SearchScreen
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Motion.Timing

/** First launch: Welcome, then Connect; later launches go straight to the library. */
@Composable
fun EncoreRoot(host: CoreHost) {
    val open by host.profileOpen.collectAsState()
    if (open) Shell(host) else Onboarding(host)
}

@Composable
private fun Onboarding(host: CoreHost) {
    var step by rememberSaveable { mutableStateOf(1) }
    val still = reduceMotion()
    BackHandler(enabled = step > 1) { step-- }
    AnimatedContent(
        targetState = step,
        transitionSpec = {
            // Board 22, item 3: a push, shared-axis on Android; a cross-fade with Reduce Motion.
            if (still) fadeIn(tween(Timing.reduceMotionCrossfadeMs)) togetherWith fadeOut(tween(Timing.reduceMotionCrossfadeMs))
            else {
                val forward = targetState > initialState
                (slideInHorizontally { w -> if (forward) w / 4 else -w / 4 } + fadeIn()) togetherWith
                    (slideOutHorizontally { w -> if (forward) -w / 4 else w / 4 } + fadeOut())
            }
        },
        label = "onboarding",
    ) { s ->
        when (s) {
            1 -> WelcomeScreen(onGetStarted = { step = 2 }, onExistingProfile = null)
            else -> ConnectScreen(host, step = 2, steps = 2, onBack = { step = 1 }, onSignedIn = { signed ->
                host.signedIn(signed)
                host.sync.syncNow()
            })
        }
    }
}

@Composable
private fun Shell(host: CoreHost) {
    val context = LocalContext.current
    val type = LocalEncoreType.current
    val nav = rememberNavigator()
    val player by PlaybackHub.state.collectAsState(PlayerState())
    var nowPlaying by rememberSaveable { mutableStateOf(false) }
    val still = reduceMotion()
    val playingSongId = player.entry?.song?.id

    LaunchedEffect(Unit) {
        PlaybackHub.connect(context)
        // Content comes from the mirror first; a delta sync follows (board 05).
        host.sync.syncNow()
    }
    BackHandler(enabled = nowPlaying) { nowPlaying = false }
    BackHandler(enabled = !nowPlaying && nav.canGoBack) { nav.back() }

    Box(Modifier.fillMaxSize()) {
        Scaffold(
            bottomBar = {
                Column {
                    MiniPlayer(
                        player,
                        onExpand = { nowPlaying = true },
                        onToggle = { PlaybackHub.togglePlay(context) },
                        onNext = { PlaybackHub.next(context) },
                        onPrevious = { PlaybackHub.previous(context) },
                        modifier = Modifier.padding(bottom = Grid.base.dp),
                    )
                    NavigationBar {
                        Tab.entries.forEach { tab ->
                            val selected = nav.tab == tab
                            NavigationBarItem(
                                selected = selected,
                                onClick = { nav.select(tab) },
                                icon = {
                                    when (tab) {
                                        Tab.Home -> Icon(Icons.home(selected), contentDescription = null)
                                        Tab.Mixes -> Icon(Icons.mixes(), contentDescription = null)
                                        Tab.Library -> Icon(Icons.library(selected), contentDescription = null)
                                        Tab.Search -> Icon(Icons.search(), contentDescription = null)
                                    }
                                },
                                label = { Text(tab.name, style = if (selected) type.navLabelSelected else type.navLabel) },
                            )
                        }
                    }
                }
            },
            containerColor = MaterialTheme.colorScheme.surface,
        ) { padding ->
            val inner = PaddingValues(bottom = padding.calculateBottomPadding())
            val onOpen: (Route) -> Unit = { nav.push(it) }
            val top = nav.top
            AnimatedContent(targetState = nav.tab to top, label = "page") { (tab, route) ->
                when (route) {
                    is Route.Album -> AlbumScreen(host, route.id, playingSongId, onBack = { nav.back() }, onOpen = onOpen, contentPadding = inner)
                    is Route.Artist -> ArtistScreen(host, route.id, playingSongId, onBack = { nav.back() }, onOpen = onOpen, contentPadding = inner)
                    is Route.Playlist -> PlaylistScreen(host, route.id, playingSongId, onBack = { nav.back() }, contentPadding = inner)
                    null -> when (tab) {
                        Tab.Home -> HomeScreen(host, onOpen, nav.reselect, inner)
                        Tab.Mixes -> MixesScreen()
                        Tab.Library -> LibraryScreen(host, onOpen, playingSongId, inner)
                        Tab.Search -> SearchScreen(host, playingSongId, onOpen, inner)
                    }
                }
            }
        }
        AnimatedVisibility(
            visible = nowPlaying && player.entry != null,
            enter = if (still) fadeIn(tween(Timing.reduceMotionCrossfadeMs)) else slideInVertically { it } + fadeIn(),
            exit = if (still) fadeOut(tween(Timing.reduceMotionCrossfadeMs)) else slideOutVertically { it } + fadeOut(),
        ) {
            NowPlayingScreen(
                host,
                player,
                onCollapse = { nowPlaying = false },
                onOpen = { route ->
                    nowPlaying = false
                    nav.push(route)
                },
                onToggle = { PlaybackHub.togglePlay(context) },
                onPrevious = { PlaybackHub.previous(context) },
                onNext = { PlaybackHub.next(context) },
                onSeek = { PlaybackHub.seekTo(context, it) },
            )
        }
    }
}
