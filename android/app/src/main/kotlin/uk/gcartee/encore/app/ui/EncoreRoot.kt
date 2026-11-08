package uk.gcartee.encore.app.ui

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
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
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.playback.PlayerState
import uk.gcartee.encore.app.ui.components.AppActions
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.LocalToaster
import uk.gcartee.encore.app.ui.components.Toaster
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.home.HomeData
import uk.gcartee.encore.app.ui.home.HomeScreen
import uk.gcartee.encore.app.ui.home.MixesScreen
import uk.gcartee.encore.app.ui.library.AlbumScreen
import uk.gcartee.encore.app.ui.library.ArtistScreen
import uk.gcartee.encore.app.ui.library.LibraryScreen
import uk.gcartee.encore.app.ui.library.MixScreen
import uk.gcartee.encore.app.ui.library.PlaylistScreen
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.nav.Tab
import uk.gcartee.encore.app.ui.nav.rememberNavigator
import uk.gcartee.encore.app.ui.onboarding.ConnectScreen
import uk.gcartee.encore.app.ui.onboarding.WelcomeScreen
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.app.ui.player.ContinueBanner
import uk.gcartee.encore.app.ui.player.MiniPlayer
import uk.gcartee.encore.app.ui.player.NowPlayingScreen
import uk.gcartee.encore.app.ui.player.OutputSheet
import uk.gcartee.encore.app.ui.profile.ProfileSheet
import uk.gcartee.encore.app.ui.search.SearchScreen
import uk.gcartee.encore.app.ui.settings.SettingsScreen
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.EventKind
import uk.gcartee.encore.core.OutputDevice
import uk.gcartee.encore.tokens.EncoreTokens.Breakpoints
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Motion.Timing

/** First launch: Welcome, then Connect; later launches go straight to the library. */
@Composable
fun EncoreRoot(host: CoreHost) {
    val open by host.profileOpen.collectAsState()
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val toaster = remember { Toaster(snackbar, scope) }
    CompositionLocalProvider(LocalToaster provides toaster) {
        if (open) Shell(host, toaster) else Onboarding(host)
    }
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
            1 -> WelcomeScreen(onGetStarted = { step = 2 }, onExistingProfile = { step = 2 })
            else -> ConnectScreen(host, step = 2, steps = 2, onBack = { step = 1 }, onSignedIn = { signed ->
                host.signedIn(signed)
                host.sync.syncNow()
            })
        }
    }
}

@Composable
private fun Shell(host: CoreHost, toaster: Toaster) {
    val context = LocalContext.current
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val nav = rememberNavigator()
    val player by PlaybackHub.state.collectAsState(PlayerState())
    var nowPlaying by rememberSaveable { mutableStateOf(false) }
    var profiles by remember { mutableStateOf(false) }
    var output by remember { mutableStateOf(false) }
    var addingAccount by rememberSaveable { mutableStateOf(false) }
    var continueFrom by remember { mutableStateOf<OutputDevice?>(null) }
    var switches by remember { mutableIntStateOf(0) }
    val homeData = remember(switches) { HomeData(host) }
    val still = reduceMotion()
    val playingSongId = player.entry?.song?.id
    val widthDp = LocalConfiguration.current.screenWidthDp
    val wide = widthDp >= Breakpoints.railFrom
    val expanded = widthDp >= Breakpoints.expandedRailFrom
    val offline = rememberOffline(context)
    val actions = remember(nav) {
        AppActions(host, toaster, context, open = { route -> nowPlaying = false; nav.push(route) }, openNowPlaying = { nowPlaying = true }, openProfiles = { profiles = true })
    }

    LaunchedEffect(Unit) {
        PlaybackHub.connect(context)
        // Content comes from the mirror first; a delta sync follows (board 05).
        host.sync.syncNow()
    }
    LaunchedEffect(Unit) {
        host.events.collect { e ->
            when (e.kind) {
                // Board 12: Continue only while this device isn't playing; the core decides when.
                EventKind.CONTINUE_AVAILABLE -> if (!player.isPlaying) continueFrom = e.device
                EventKind.HANDOFF_FAILED -> {
                    haptics.perform(Haptic.Reject)
                    toaster.show(e.handoff?.reason ?: "Couldn't move the music")
                }
                EventKind.HANDOFF_INCOMING -> toaster.show("Playing here from ${e.handoff?.deviceName.orEmpty()}")
                else -> Unit
            }
        }
    }
    BackHandler(enabled = nowPlaying) { nowPlaying = false }
    BackHandler(enabled = !nowPlaying && nav.canGoBack) { nav.back() }

    CompositionLocalProvider(LocalAppActions provides actions) {
        if (addingAccount) {
            // Board 14, item 4: the onboarding flow; the new profile opens when sign-in succeeds.
            BackHandler { addingAccount = false }
            ConnectScreen(host, step = 1, steps = 1, onBack = { addingAccount = false }, onSignedIn = { signed ->
                host.signedIn(signed)
                runCatching { host.core.queue() }.onSuccess(host::profileSwitched)
                host.sync.syncNow()
                addingAccount = false
                nav.reset()
                switches++
            })
            return@CompositionLocalProvider
        }
        Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface)) {
            Row(Modifier.fillMaxSize()) {
                if (expanded) {
                    // Board 29, item 1: from 840 dp the expanded rail, 240 dp, labels beside the icons.
                    ExpandedRail(host, nav.tab, onTab = { nav.select(it) }, onOpen = actions.open, onProfile = { profiles = true })
                } else if (wide) {
                    // Board 29: an 80 dp rail with labels under the icons from 600 dp.
                    NavigationRail(Modifier.statusBarsPadding()) {
                        Tab.entries.forEach { tab ->
                            val selected = nav.tab == tab
                            NavigationRailItem(
                                selected = selected,
                                onClick = { nav.select(tab) },
                                icon = { TabIcon(tab, selected) },
                                label = { Text(tab.name, style = if (selected) type.navLabelSelected else type.navLabel) },
                            )
                        }
                    }
                }
                Column(Modifier.weight(1f).fillMaxSize()) {
                    if (offline) OfflineBanner()
                    // Board 08: the page runs behind the mini player, which floats over it, so a
                    // tinted album page reaches the bar. Pages pad their lists by its height.
                    var miniHeight by remember { mutableIntStateOf(0) }
                    Box(Modifier.weight(1f)) {
                        val inner = PaddingValues(bottom = with(LocalDensity.current) { miniHeight.toDp() })
                        val top = nav.top
                        AnimatedContent(
                            targetState = nav.tab to top,
                            transitionSpec = {
                                if (still) fadeIn(tween(Timing.reduceMotionCrossfadeMs)) togetherWith fadeOut(tween(Timing.reduceMotionCrossfadeMs))
                                else fadeIn(tween(220)) togetherWith fadeOut(tween(160))
                            },
                            label = "page",
                        ) { (tab, route) ->
                            // Board 29, item 2: on expanded windows Library is a 340 dp list with the
                            // chosen album, artist or playlist open beside it, two panes 12 dp apart.
                            if (expanded && tab == Tab.Library && route != null && route !is Route.Settings) {
                                Row(Modifier.fillMaxSize().statusBarsPadding().padding(PANE_GAP_DP.dp)) {
                                    Surface(shape = androidx.compose.foundation.shape.RoundedCornerShape(PANE_RADIUS_DP.dp), color = MaterialTheme.colorScheme.surfaceContainerLow, modifier = Modifier.width(LIST_PANE_DP.dp).fillMaxSize()) {
                                        LibraryScreen(host, actions.open, playingSongId, inner)
                                    }
                                    androidx.compose.foundation.layout.Spacer(Modifier.width(PANE_GAP_DP.dp))
                                    Surface(shape = androidx.compose.foundation.shape.RoundedCornerShape(PANE_RADIUS_DP.dp), modifier = Modifier.weight(1f).fillMaxSize()) {
                                        Page(route, host, playingSongId, nav::back, inner, onProfiles = { profiles = true }, onAddServer = { addingAccount = true })
                                    }
                                }
                                return@AnimatedContent
                            }
                            when (route) {
                                is Route.Album -> AlbumScreen(host, route.id, playingSongId, onBack = { nav.back() }, contentPadding = inner)
                                is Route.Artist -> ArtistScreen(host, route.id, playingSongId, onBack = { nav.back() }, contentPadding = inner)
                                is Route.Playlist -> PlaylistScreen(host, route.id, playingSongId, onBack = { nav.back() }, contentPadding = inner)
                                is Route.Mix -> MixScreen(host, route.id, playingSongId, onBack = { nav.back() }, contentPadding = inner)
                                is Route.Settings -> SettingsScreen(host, route.page, onBack = { nav.back() }, onProfiles = { profiles = true }, onAddServer = { addingAccount = true }, contentPadding = inner)
                                null -> when (tab) {
                                    Tab.Home -> HomeScreen(host, homeData, onProfile = { profiles = true }, reselect = nav.reselect, contentPadding = inner)
                                    Tab.Mixes -> MixesScreen(host, homeData, onProfile = { profiles = true }, contentPadding = inner)
                                    Tab.Library -> LibraryScreen(host, actions.open, playingSongId, inner)
                                    Tab.Search -> SearchScreen(host, playingSongId, actions.open, inner, onLibrary = { nav.select(Tab.Library) })
                                }
                            }
                        }
                        // The mini player and, on phones, the navigation bar float over the
                        // page together; pages pad their lists by the pair's height.
                        Column(Modifier.align(Alignment.BottomCenter).fillMaxWidth().onSizeChanged { miniHeight = it.height }) {
                            MiniPlayer(host, player, onExpand = { nowPlaying = true }, modifier = Modifier.padding(vertical = Grid.base.dp).let { if (wide) it.navigationBarsPadding() else it }, bar = wide)
                            if (!wide) {
                                // Material 3 Expressive is round: the bar's top corners are 28 dp
                                // and it runs under the clear gesture area to the screen's edge.
                                NavigationBar(
                                    containerColor = MaterialTheme.colorScheme.surfaceContainer,
                                    modifier = Modifier.clip(androidx.compose.foundation.shape.RoundedCornerShape(topStart = NAV_RADIUS_DP.dp, topEnd = NAV_RADIUS_DP.dp)),
                                ) {
                                    Tab.entries.forEach { tab ->
                                        val selected = nav.tab == tab
                                        NavigationBarItem(
                                            selected = selected,
                                            onClick = { nav.select(tab) },
                                            icon = { TabIcon(tab, selected) },
                                            label = { Text(tab.name, style = if (selected) type.navLabelSelected else type.navLabel) },
                                        )
                                    }
                                }
                            }
                        }
                        // Toasts sit above the mini player and never cover controls (board 20).
                        SnackbarHost(toaster.host, Modifier.align(Alignment.BottomCenter).padding(bottom = inner.calculateBottomPadding() + Grid.base.dp).semantics { liveRegion = LiveRegionMode.Polite })
                    }
                }
            }
            ContinueBanner(host, continueFrom, onDone = { continueFrom = null })
            AnimatedVisibility(
                visible = nowPlaying && player.entry != null,
                // Up on the expressive spring; down on one smooth curve, from wherever a
                // swipe left it (NowPlayingScreen carries a swipe down itself).
                enter = if (still) fadeIn(tween(Timing.reduceMotionCrossfadeMs)) else slideInVertically(androidx.compose.animation.core.spring(dampingRatio = 0.9f, stiffness = androidx.compose.animation.core.Spring.StiffnessMediumLow)) { it } + fadeIn(tween(OPEN_FADE_MS)),
                exit = if (still) fadeOut(tween(Timing.reduceMotionCrossfadeMs)) else slideOutVertically(tween(CLOSE_MS, easing = androidx.compose.animation.core.FastOutSlowInEasing)) { it } + fadeOut(tween(CLOSE_MS)),
            ) {
                NowPlayingScreen(
                    host,
                    player,
                    onCollapse = { nowPlaying = false },
                    onOpen = { route ->
                        nowPlaying = false
                        nav.push(route)
                    },
                    onOutput = { output = true },
                )
            }
        }
        if (profiles) ProfileSheet(host, playing = player.isPlaying, onDismiss = { profiles = false }, onSwitched = { nav.reset(); switches++ }, onAddAccount = { addingAccount = true }, showSettings = nav.top !is Route.Settings)
        if (output) OutputSheet(host, playingHere = player.isPlaying, onDismiss = { output = false })
    }
}

/** A pushed page, for the detail pane. */
@Composable
private fun Page(route: Route, host: CoreHost, playingSongId: Long?, onBack: () -> Unit, inner: PaddingValues, onProfiles: () -> Unit, onAddServer: () -> Unit) {
    when (route) {
        is Route.Album -> AlbumScreen(host, route.id, playingSongId, onBack = onBack, contentPadding = inner)
        is Route.Artist -> ArtistScreen(host, route.id, playingSongId, onBack = onBack, contentPadding = inner)
        is Route.Playlist -> PlaylistScreen(host, route.id, playingSongId, onBack = onBack, contentPadding = inner)
        is Route.Mix -> MixScreen(host, route.id, playingSongId, onBack = onBack, contentPadding = inner)
        is Route.Settings -> SettingsScreen(host, route.page, onBack = onBack, onProfiles = onProfiles, onAddServer = onAddServer, contentPadding = inner)
    }
}

/**
 * Board 29, item 1: the expanded rail: the four destinations as full-round tonal pills,
 * then pinned mixes and playlists, and the profile at the foot.
 */
@Composable
private fun ExpandedRail(host: CoreHost, tab: Tab, onTab: (Tab) -> Unit, onOpen: (Route) -> Unit, onProfile: () -> Unit) {
    val type = LocalEncoreType.current
    val playlists = remember { runCatching { host.core.playlists(0u, 8u) }.getOrDefault(emptyList()) }
    val profile = remember { runCatching { host.core.profiles().firstOrNull { it.active } }.getOrNull() }
    androidx.compose.material3.PermanentDrawerSheet(
        Modifier.width(EXPANDED_RAIL_DP.dp).statusBarsPadding(),
        drawerContainerColor = MaterialTheme.colorScheme.surface,
    ) {
        androidx.compose.foundation.lazy.LazyColumn(Modifier.weight(1f).padding(horizontal = 12.dp)) {
            items(Tab.entries.size) { i ->
                val t = Tab.entries[i]
                androidx.compose.material3.NavigationDrawerItem(
                    label = { Text(t.name, style = if (t == tab) type.navLabelSelected else type.listRowTitle) },
                    icon = { TabIcon(t, t == tab) },
                    selected = t == tab,
                    onClick = { onTab(t) },
                )
            }
            if (playlists.isNotEmpty()) {
                item { Text("Playlists", style = type.groupLabel, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 16.dp, top = 24.dp, bottom = 8.dp)) }
                items(playlists.size) { i ->
                    val p = playlists[i]
                    androidx.compose.material3.NavigationDrawerItem(
                        label = { Text(p.name, style = type.listRowTitle, maxLines = 1) },
                        selected = false,
                        onClick = { onTab(Tab.Library); onOpen(Route.Playlist(p.id)) },
                    )
                }
            }
        }
        androidx.compose.material3.NavigationDrawerItem(
            label = { Text(profile?.displayName ?: "Profile", style = type.listRowTitle) },
            icon = { Icon(Icons.artist(), contentDescription = null) },
            selected = false,
            onClick = onProfile,
            modifier = Modifier.padding(12.dp),
        )
    }
}

@Composable
private fun TabIcon(tab: Tab, selected: Boolean) {
    when (tab) {
        Tab.Home -> Icon(Icons.home(selected), contentDescription = null)
        Tab.Mixes -> Icon(Icons.mixes(), contentDescription = null)
        Tab.Library -> Icon(Icons.library(selected), contentDescription = null)
        Tab.Search -> Icon(Icons.search(), contentDescription = null)
    }
}

/**
 * Board 20, item 1: offline after 2 s without a network, back 2 s after it returns, so a
 * short drop doesn't flash a banner.
 */
@Composable
private fun rememberOffline(context: Context): Boolean {
    var online by remember { mutableStateOf(true) }
    var shown by remember { mutableStateOf(false) }
    DisposableEffect(context) {
        val cm = context.getSystemService(ConnectivityManager::class.java)
        fun check() = cm?.getNetworkCapabilities(cm.activeNetwork)?.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) == true
        online = check()
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) { online = true }
            override fun onLost(network: Network) { online = check() }
        }
        runCatching { cm?.registerDefaultNetworkCallback(callback) }
        onDispose { runCatching { cm?.unregisterNetworkCallback(callback) } }
    }
    LaunchedEffect(online) {
        delay(OFFLINE_DELAY_MS)
        shown = !online
    }
    return shown
}

@Composable
private fun OfflineBanner() {
    Surface(color = MaterialTheme.colorScheme.secondaryContainer, modifier = Modifier.fillMaxWidth().semantics { liveRegion = LiveRegionMode.Polite }) {
        Row(Modifier.statusBarsPadding().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.offline(), contentDescription = null)
            Text(
                "You're offline. Your library still browses, and the current song plays from its buffer.",
                style = LocalEncoreType.current.body,
                modifier = Modifier.padding(start = Grid.base.dp),
            )
        }
    }
}

private const val OFFLINE_DELAY_MS = 2_000L

// Board 29's panes.
private const val LIST_PANE_DP = 340
private const val PANE_GAP_DP = 12
private const val PANE_RADIUS_DP = 28
private val EXPANDED_RAIL_DP = Breakpoints.expandedRailWidth

private const val OPEN_FADE_MS = 200
private const val CLOSE_MS = 280
private const val NAV_RADIUS_DP = 28
