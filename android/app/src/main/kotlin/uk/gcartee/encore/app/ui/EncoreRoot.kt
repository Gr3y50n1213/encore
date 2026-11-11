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
import androidx.compose.ui.platform.testTag
import uk.gcartee.encore.app.ui.components.topBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.width
import uk.gcartee.encore.app.MainActivity
import androidx.compose.ui.graphics.luminance
import androidx.compose.foundation.layout.statusBars
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
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
import uk.gcartee.encore.app.ui.player.ART_PAUSED_SCALE
import uk.gcartee.encore.app.ui.player.MINI_ART_RADIUS_DP
import kotlinx.coroutines.launch
import androidx.compose.ui.input.pointer.pointerInput
import uk.gcartee.encore.app.ui.player.COLLAPSE_MIN_WAY_DP
import uk.gcartee.encore.app.ui.player.collapseCloses
import uk.gcartee.encore.app.ui.player.detectCollapseDrag
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.boundsInRoot
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
import uk.gcartee.encore.tokens.EncoreTokens.Radius
import uk.gcartee.encore.tokens.EncoreTokens.Color.Accents.Amber
import uk.gcartee.encore.tokens.EncoreTokens.Motion.Timing

/** First launch: Welcome, then Connect; later launches go straight to the library. */
@Composable
fun EncoreRoot(host: CoreHost) {
    // Clear behind gestures, the system's scrim behind buttons, on every screen.
    uk.gcartee.encore.app.ui.components.NavigationBarForNavigationMode()
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
    // Where the mini player is and how big the screen is, for the morph between them.
    var miniBar by remember { mutableStateOf<androidx.compose.ui.geometry.Rect?>(null) }
    var miniArt by remember { mutableStateOf<androidx.compose.ui.geometry.Rect?>(null) }
    // Where Now Playing's artwork is: the big cover, or the small one over Lyrics and the
    // Queue. The big one is kept for the next opening, which always starts on it.
    var nowArt by remember { mutableStateOf<ArtSpot?>(null) }
    var bigArt by remember { mutableStateOf<ArtSpot?>(null) }
    // The mini player and navigation bar's height: pages pad by it, toasts sit above it.
    var miniHeight by remember { mutableIntStateOf(0) }
    var screen by remember { mutableStateOf(androidx.compose.ui.unit.IntSize.Zero) }
    // 1 is Now Playing open, 0 is closed into the bar; a swipe down drives it directly.
    val expand = remember { androidx.compose.animation.core.Animatable(0f) }
    // The bar's bounce once it has taken Now Playing in.
    val pulse = remember { androidx.compose.animation.core.Animatable(0f) }
    var releasing by remember { mutableStateOf(false) }
    var scrubHeld by remember { mutableStateOf(false) }
    val morphScope = rememberCoroutineScope()
    val density = LocalDensity.current
    var profiles by remember { mutableStateOf(false) }
    // The Play on sheet is open, in the colours of the Now Playing page that opened it.
    var output by remember { mutableStateOf<uk.gcartee.encore.app.ui.player.FieldColours?>(null) }
    // The sleep timer sheet is open (Now Playing's More, Settings, the notification); the
    // tint is the album's colours when More opened it, none otherwise.
    var sleepSheet by remember { mutableStateOf(false) }
    var sleepTint by remember { mutableStateOf<uk.gcartee.encore.app.ui.player.FieldColours?>(null) }
    var addingAccount by rememberSaveable { mutableStateOf(false) }
    var continueFrom by remember { mutableStateOf<OutputDevice?>(null) }
    var switches by remember { mutableIntStateOf(0) }
    val homeData = remember(switches) { HomeData(host) }
    val still = reduceMotion()
    val playingSongId = player.entry?.song?.id
    val widthDp = LocalConfiguration.current.screenWidthDp
    val wide = widthDp >= Breakpoints.railFrom
    val expanded = widthDp >= Breakpoints.expandedRailFrom
    // The owner (8 November 2026): on a portrait tablet (about 880 dp) the 240 dp labelled
    // rail beside the list and the page squeezed the page. The labelled rail waits for a
    // Large window (1200 dp, a landscape tablet); below that the 80 dp rail serves.
    val railLabelled = widthDp >= LABELLED_RAIL_FROM_DP
    // Phase 8: the settled network (board 20: in after 2 s, out 2 s after it returns)
    // and what is downloaded, for every screen.
    val offline by host.connectivity.offline.collectAsState()
    // The offline pill floats over the page; pages move their content down by its height.
    var bannerPx by remember { mutableIntStateOf(0) }
    val bannerTop by androidx.compose.animation.core.animateDpAsState(
        if (offline) with(LocalDensity.current) { bannerPx.toDp() } else 0.dp,
        tween(BANNER_MS),
        label = "banner",
    )
    val downloadView by host.downloads.view.collectAsState()
    val downloadProgress = host.downloads.progress.collectAsState()
    val actions = remember(nav) {
        AppActions(host, toaster, context, open = { route -> nowPlaying = false; nav.push(route) }, openNowPlaying = { nowPlaying = true }, openProfiles = { profiles = true })
    }
    actions.onRefused = { haptics.perform(Haptic.Reject) }
    // The download notification opens Settings › Downloads.
    val openRequest by host.openRequest.collectAsState()
    // Settings and the sleep timer's notification open the sleep timer sheet.
    LaunchedEffect(openRequest) {
        if (openRequest == MainActivity.OPEN_SLEEP_TIMER) {
            sleepTint = null
            sleepSheet = true
            host.openRequest.value = null
        }
    }
    // Phase 9: the Now Playing widget opens the player from its cover or text, and the
    // profile switcher when it belongs to another profile or one with a PIN.
    LaunchedEffect(openRequest) {
        when (openRequest) {
            MainActivity.OPEN_NOW_PLAYING -> {
                profiles = false
                nowPlaying = true
                host.openRequest.value = null
            }
            MainActivity.OPEN_PROFILES -> {
                nowPlaying = false
                profiles = true
                host.openRequest.value = null
            }
        }
    }
    LaunchedEffect(openRequest) {
        if (openRequest == MainActivity.OPEN_DOWNLOADS) {
            nowPlaying = false
            if (nav.top != Route.Settings("downloads")) nav.push(Route.Settings("downloads"))
            host.openRequest.value = null
        }
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

    CompositionLocalProvider(
        LocalAppActions provides actions,
        uk.gcartee.encore.app.ui.components.LocalDownloads provides downloadView,
        uk.gcartee.encore.app.ui.components.LocalDownloadProgress provides downloadProgress,
        uk.gcartee.encore.app.ui.components.LocalTopBanner provides bannerTop,
    ) {
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
                if (railLabelled) {
                    // Board 29, item 1: the expanded rail, 240 dp, labels beside the icons, on Large windows.
                    ExpandedRail(host, nav.tab, onTab = { nav.select(it) }, onOpen = actions.open, onProfile = { profiles = true })
                } else if (wide) {
                    // Board 29: an 80 dp rail with labels under the icons from 600 dp.
                    NavigationRail(Modifier.statusBarsPadding().testTag("nav-rail")) {
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
                    // Board 08: the page runs behind the mini player, which floats over it, so a
                    // tinted album page reaches the bar. Pages pad their lists by its height.
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
                                Row(Modifier.fillMaxSize().topBarsPadding().padding(PANE_GAP_DP.dp)) {
                                    Surface(shape = androidx.compose.foundation.shape.RoundedCornerShape(PANE_RADIUS_DP.dp), color = MaterialTheme.colorScheme.surfaceContainerLow, modifier = Modifier.width(LIST_PANE_DP.dp).fillMaxSize()) {
                                        LibraryScreen(host, actions.open, playingSongId, inner)
                                    }
                                    androidx.compose.foundation.layout.Spacer(Modifier.width(PANE_GAP_DP.dp))
                                    Surface(shape = androidx.compose.foundation.shape.RoundedCornerShape(PANE_RADIUS_DP.dp), modifier = Modifier.weight(1f).fillMaxSize()) {
                                        CompositionLocalProvider(uk.gcartee.encore.app.ui.components.LocalInPane provides true) {
                                            Page(route, host, playingSongId, nav::back, inner, onProfiles = { profiles = true }, onAddServer = { addingAccount = true })
                                        }
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
                            MiniPlayer(
                                host,
                                player,
                                onExpand = { nowPlaying = true },
                                modifier = Modifier
                                    .onGloballyPositioned { miniBar = it.boundsInRoot() }
                                    // The bar takes Now Playing in: it swells a little as the
                                    // artwork arrives and settles with a small bounce after.
                                    .graphicsLayer {
                                        val s = 1f + pulse.value
                                        scaleX = s
                                        scaleY = s
                                    }
                                    .padding(vertical = Grid.base.dp)
                                    .let { if (wide) it.navigationBarsPadding() else it },
                                bar = wide,
                                onArtBounds = { if (expand.value == 0f && pulse.value == 0f && it.usable()) miniArt = it },
                            )
                            if (!wide) {
                                // Material 3 Expressive is round: the bar's top corners are 28 dp
                                // and it runs under the clear gesture area to the screen's edge.
                                NavigationBar(
                                    containerColor = MaterialTheme.colorScheme.surfaceContainer,
                                    modifier = Modifier.clip(androidx.compose.foundation.shape.RoundedCornerShape(topStart = NAV_RADIUS_DP.dp, topEnd = NAV_RADIUS_DP.dp)).testTag("nav-bar"),
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
                        // Board 20, item 1, the owner's "not too intrusive but notable": a slim
                        // amber pill that floats under the status bar. The page keeps drawing
                        // behind it (an album's colour, the status bar's icons) and moves its
                        // content down by the pill's height (LocalTopBanner), so it covers nothing.
                        OfflineBanner(offline, Modifier.align(Alignment.TopCenter), onHeight = { bannerPx = it })
                    }
                }
            }
            ContinueBanner(host, continueFrom, onDone = { continueFrom = null })
            // Board 09 and 42, as the owner asked: one movement, not a shrink then a fade. A
            // swipe down drives it directly: the screen closes in around the artwork, which
            // zooms into the mini player's own cover, centred on it by the time it is its size;
            // the page behind comes up from under a dimming scrim; once the bar has taken it in
            // it eases up a little and floats back. Opening runs the same path backwards, with
            // no swell. Reduce Motion cross-fades.
            val visible = nowPlaying && player.entry != null
            val shown = visible || expand.value > 0f
            var everShown by remember { mutableStateOf(false) }
            LaunchedEffect(shown) {
                if (shown) {
                    everShown = true
                    // Opened again mid-bounce: the bar settles at once rather than staying swollen.
                    pulse.snapTo(0f)
                } else {
                    // Closed: the next opening starts from the big cover, whatever panel
                    // was showing when it closed.
                    nowArt = bigArt
                }
                if (!shown && everShown && !still) {
                    // A soft gulp once the artwork is in: up a little, then floating back.
                    pulse.animateTo(MINI_SWELL, tween(PULSE_UP_MS, easing = androidx.compose.animation.core.FastOutSlowInEasing))
                    pulse.animateTo(0f, androidx.compose.animation.core.spring(dampingRatio = PULSE_DAMPING, stiffness = androidx.compose.animation.core.Spring.StiffnessLow))
                }
            }
            LaunchedEffect(visible) {
                if (!visible && releasing) return@LaunchedEffect
                val target = if (visible) 1f else 0f
                when {
                    still -> expand.animateTo(target, tween(Timing.reduceMotionCrossfadeMs))
                    visible -> expand.animateTo(target, androidx.compose.animation.core.spring(dampingRatio = OPEN_DAMPING, stiffness = androidx.compose.animation.core.Spring.StiffnessMediumLow))
                    else -> expand.animateTo(target, androidx.compose.animation.core.spring(dampingRatio = 1f, stiffness = androidx.compose.animation.core.Spring.StiffnessMediumLow))
                }
            }
            fun travel(): Float {
                val h = screen.height.toFloat().coerceAtLeast(1f)
                val from = nowArt?.rect
                val to = miniArt
                // Never a hair trigger: in landscape the cover sits close to the bar.
                val least = maxOf(h * MIN_TRAVEL, with(density) { COLLAPSE_MIN_WAY_DP.dp.toPx() }).coerceAtMost(h)
                return if (from != null && to != null) (to.center.y - from.center.y).coerceAtLeast(least) else h * DEFAULT_TRAVEL
            }
            fun release(velocity: Float) {
                val way = travel()
                val perSecond = -velocity / way
                val closes = with(density) {
                    collapseCloses(
                        draggedDp = ((1f - expand.value) * way).toDp().value,
                        wayDp = way.toDp().value,
                        velocityDpPerS = velocity.toDp().value,
                    )
                }
                if (closes) {
                    releasing = true
                    nowPlaying = false
                    morphScope.launch {
                        // Finally: a tap on the bar mid-close cancels this, and a flag left
                        // set would stop the next close.
                        try {
                            expand.animateTo(0f, androidx.compose.animation.core.spring(dampingRatio = 1f, stiffness = androidx.compose.animation.core.Spring.StiffnessMediumLow), initialVelocity = perSecond)
                        } finally {
                            releasing = false
                        }
                    }
                } else {
                    morphScope.launch { expand.animateTo(1f, androidx.compose.animation.core.spring(dampingRatio = 0.8f, stiffness = androidx.compose.animation.core.Spring.StiffnessMedium), initialVelocity = perSecond) }
                }
            }
            if (shown && !still) {
                // The page behind, dimmed while Now Playing covers it, clears as it goes.
                Box(Modifier.fillMaxSize().background(androidx.compose.ui.graphics.Color.Black.copy(alpha = SCRIM * expand.value.coerceIn(0f, 1f))))
            }
            if (shown) Box(
                Modifier
                    .fillMaxSize()
                    .onSizeChanged { screen = it }
                    // Outside the layer below, so the finger is read in screen terms however
                    // small the screen has become.
                    .pointerInput(scrubHeld) {
                        if (scrubHeld) return@pointerInput
                        // Deliberate and mostly downward, and never over something that
                        // takes the finger first (CollapseGesture.kt).
                        detectCollapseDrag(
                            onDrag = { dy ->
                                val next = (expand.value - dy / travel()).coerceIn(0f, 1f)
                                if (next != expand.value) morphScope.launch { expand.snapTo(next) }
                            },
                            onRelease = { release(it) },
                        )
                    }
                    .graphicsLayer {
                        val p = expand.value
                        if (still) {
                            alpha = p.coerceIn(0f, 1f)
                        } else if (screen.width > 0 && screen.height > 0) {
                            val g = p.coerceIn(0f, 1f)
                            val w = screen.width.toFloat()
                            val h = screen.height.toFloat()
                            val spot = nowArt
                            val laid = spot?.rect ?: androidx.compose.ui.geometry.Rect(w * 0.06f, h * 0.14f, w * 0.94f, h * 0.14f + w * 0.88f)
                            // The artwork as drawn: the big cover sits at 88 % while paused.
                            val art = if (spot?.shrinks != false && !player.isPlaying) laid.scaledAround(laid.center, ART_PAUSED_SCALE) else laid
                            val thumb = miniArt ?: androidx.compose.ui.geometry.Rect(w * 0.05f, h * 0.86f, w * 0.05f + 48.dp.toPx(), h * 0.86f + 48.dp.toPx())
                            val ratio = thumb.width / art.width
                            val k = if (ratio.isFinite() && ratio > 0f) ratio else 1f
                            // A zoom when the artwork is much bigger than the cover: the size
                            // changes geometrically and the position follows it, so the art is
                            // centred on the cover by the time it is the cover's size. Between
                            // two covers of about one size (Lyrics, the Queue), a straight move.
                            // The owner (8 November 2026): the zoom's formula divided by zero
                            // there, and Now Playing stayed invisible behind its grey scrim.
                            val zoom = k < ZOOM_BELOW
                            val s = if (zoom) Math.pow(k.toDouble(), (1f - g).toDouble()).toFloat() else mix(k, 1f, g)
                            val u = if (zoom) ((s - k) / (1f - k)).coerceIn(0f, 1f) else g
                            val cx = mix(thumb.center.x, art.center.x, u)
                            val cy = mix(thumb.center.y, art.center.y, u)
                            transformOrigin = androidx.compose.ui.graphics.TransformOrigin(0f, 0f)
                            scaleX = s
                            scaleY = s
                            translationX = cx - art.center.x * s
                            translationY = cy - art.center.y * s
                            if (g < 0.999f) {
                                // The visible part closes in from the whole screen to the art,
                                // its corners from a card's to the bar cover's, in the layer's
                                // own coordinates (divided by the scale to land right).
                                val region = androidx.compose.ui.geometry.Rect(
                                    mix(art.left, 0f, u), mix(art.top, 0f, u), mix(art.right, w, u), mix(art.bottom, h, u),
                                )
                                val r = mix(MINI_ART_RADIUS_DP.dp.toPx() / k, CARD_RADIUS_DP.dp.toPx() / s, u)
                                shape = RegionShape(region, r)
                                clip = true
                            }
                            // Only at the very end, already on the cover, does it hand over.
                            alpha = (u / MORPH_FADE_UNTIL).coerceIn(0f, 1f)
                        }
                    },
            ) {
                NowPlayingScreen(
                    host,
                    player,
                    onCollapse = { nowPlaying = false },
                    onOpen = { route ->
                        nowPlaying = false
                        nav.push(route)
                    },
                    onOutput = { output = it },
                    onSleepTimer = { sleepTint = it; sleepSheet = true },
                    onArtBounds = { rect, shrinks ->
                        if (expand.value >= 0.999f && rect.usable()) {
                            val spot = ArtSpot(rect, shrinks)
                            nowArt = spot
                            if (shrinks) bigArt = spot
                        }
                    },
                    onScrubHeld = { scrubHeld = it },
                )
            }
            // Toasts above everything, so one from Now Playing is seen there too: over the
            // page they sit above the mini player; over Now Playing, under its top bar. They
            // never cover controls (board 20) and take the app's own colours.
            SnackbarHost(
                toaster.host,
                Modifier
                    .align(if (visible) Alignment.TopCenter else Alignment.BottomCenter)
                    .let { if (visible) it.statusBarsPadding().padding(top = TOAST_UNDER_TOP_BAR_DP.dp) else it.padding(bottom = with(density) { miniHeight.toDp() } + Grid.base.dp) }
                    .widthIn(max = TOAST_MAX_DP.dp)
                    .semantics { liveRegion = LiveRegionMode.Polite },
            ) { data ->
                androidx.compose.material3.Snackbar(
                    data,
                    shape = androidx.compose.foundation.shape.RoundedCornerShape(Radius.androidCard.dp),
                    containerColor = MaterialTheme.colorScheme.secondaryContainer,
                    contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                    actionColor = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(horizontal = (Grid.base * 2).dp),
                )
            }
        }
        if (profiles) ProfileSheet(host, playing = player.isPlaying, onDismiss = { profiles = false }, onSwitched = { nav.reset(); switches++ }, onAddAccount = { addingAccount = true }, showSettings = nav.top !is Route.Settings)
        val primer by host.notificationPrimer.collectAsState()
        val primerForSleep by uk.gcartee.encore.app.ui.player.PrimerCopy.sleepTimer.collectAsState()
        if (sleepSheet) {
            val close = { sleepSheet = false }
            val tint = sleepTint
            if (tint != null) uk.gcartee.encore.app.ui.player.FieldSheetTheme(tint) { uk.gcartee.encore.app.ui.player.SleepTimerSheet(host, onDismiss = close) }
            else uk.gcartee.encore.app.ui.player.SleepTimerSheet(host, onDismiss = close)
        }
        if (primer) NotificationPrimer(forSleepTimer = primerForSleep, onDone = { host.notificationPrimer.value = false })
        output?.let { tint ->
            uk.gcartee.encore.app.ui.player.FieldSheetTheme(tint) { OutputSheet(host, playingHere = player.isPlaying, onDismiss = { output = null }) }
        }
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
        Modifier.width(EXPANDED_RAIL_DP.dp).statusBarsPadding().testTag("nav-rail"),
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
/**
 * Board 20, item 1 (States.html: an amber 16 dp banner, "Offline. Showing downloaded
 * music"): the owner asked for it "not too intrusive but notable". A slim pill in the
 * Amber accent's container colours, light and dark, under the status bar; it slides
 * in after 2 s without a network and out 2 s after it returns (Connectivity), and is
 * announced politely to screen readers.
 */
@Composable
private fun OfflineBanner(visible: Boolean, modifier: Modifier = Modifier, onHeight: (Int) -> Unit) {
    val dark = MaterialTheme.colorScheme.surface.luminance() < 0.5f
    val container = uk.gcartee.encore.app.ui.theme.tokenColor(if (dark) Amber.Dark.primaryContainer else Amber.Light.primaryContainer)
    val content = uk.gcartee.encore.app.ui.theme.tokenColor(if (dark) Amber.Dark.onPrimaryContainer else Amber.Light.onPrimaryContainer)
    Box(modifier.fillMaxWidth().statusBarsPadding(), contentAlignment = Alignment.TopCenter) {
    androidx.compose.animation.AnimatedVisibility(
        visible,
        enter = androidx.compose.animation.slideInVertically(tween(BANNER_MS)) { -it } + fadeIn(tween(BANNER_MS)),
        exit = androidx.compose.animation.slideOutVertically(tween(BANNER_MS)) { -it } + fadeOut(tween(BANNER_MS)),
    ) {
        Box(
            Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = 6.dp).onSizeChanged { onHeight(it.height) },
            contentAlignment = Alignment.Center,
        ) {
            Surface(
                shape = androidx.compose.foundation.shape.RoundedCornerShape(BANNER_RADIUS_DP.dp),
                color = container,
                contentColor = content,
                // A soft lift so it reads over an album's colours as well as a plain page.
                shadowElevation = BANNER_LIFT_DP.dp,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
            ) {
                Row(Modifier.padding(horizontal = 14.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.offline(), contentDescription = null, modifier = Modifier.size(18.dp))
                    androidx.compose.foundation.layout.Spacer(Modifier.width(8.dp))
                    Text("Offline · Showing downloaded music", style = LocalEncoreType.current.body.copy(fontSize = 14.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Medium), maxLines = 1)
                }
            }
        }
    }
    }
}

/**
 * Board 25: before Android's own question, say what notifications are for, once, when
 * the first download starts. Not now asks nothing (the one system prompt is kept for
 * later); Settings › Downloads then shows a quiet "Turn on".
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class, androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun NotificationPrimer(forSleepTimer: Boolean, onDone: () -> Unit) {
    val type = LocalEncoreType.current
    val launcher = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.RequestPermission()) { onDone() }
    androidx.compose.material3.ModalBottomSheet(onDismissRequest = onDone, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding().padding(horizontal = (Grid.base * 3).dp, vertical = Grid.base.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(
                Modifier.size(PRIMER_MARK_DP.dp).clip(uk.gcartee.encore.app.ui.home.PolygonShape(androidx.compose.material3.MaterialShapes.Cookie9Sided)).background(MaterialTheme.colorScheme.primaryContainer),
                contentAlignment = Alignment.Center,
            ) { Icon(if (forSleepTimer) Icons.sleepTimer() else Icons.download(), contentDescription = null, tint = MaterialTheme.colorScheme.onPrimaryContainer, modifier = Modifier.size(28.dp)) }
            androidx.compose.foundation.layout.Spacer(Modifier.size((Grid.base * 2).dp))
            Text(if (forSleepTimer) "Show the sleep timer?" else "Show download progress?", style = type.sheetTitle, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            androidx.compose.foundation.layout.Spacer(Modifier.size(Grid.base.dp))
            Text(
                if (forSleepTimer) {
                    "See time left and add 15 minutes from your notifications. The timer works if you say no."
                } else {
                    "Allow notifications so downloads can show progress and tell you when they finish. Playback keeps working if you say no."
                },
                style = type.body,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
            androidx.compose.foundation.layout.Spacer(Modifier.size((Grid.base * 3).dp))
            androidx.compose.material3.Button(
                onClick = {
                    if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) launcher.launch(android.Manifest.permission.POST_NOTIFICATIONS) else onDone()
                },
                shapes = androidx.compose.material3.ButtonDefaults.shapes(),
                modifier = Modifier.fillMaxWidth().height(PRIMER_BUTTON_DP.dp),
            ) { Text("Allow", style = type.sectionHeader) }
            androidx.compose.material3.TextButton(onClick = onDone, modifier = Modifier.fillMaxWidth().height(PRIMER_BUTTON_DP.dp)) { Text("Not now", style = type.sectionHeader) }
            androidx.compose.foundation.layout.Spacer(Modifier.size((Grid.base * 2).dp))
        }
    }
}

private const val BANNER_MS = 280
private const val BANNER_LIFT_DP = 3
private const val BANNER_RADIUS_DP = 16
private const val PRIMER_MARK_DP = 64
private const val PRIMER_BUTTON_DP = 52

// Board 29's panes.
private const val LIST_PANE_DP = 340
private const val LABELLED_RAIL_FROM_DP = 1200
private const val PANE_GAP_DP = 12
private const val PANE_RADIUS_DP = 28
private val EXPANDED_RAIL_DP = Breakpoints.expandedRailWidth

private const val OPEN_FADE_MS = 200
private const val CLOSE_MS = 280
private const val NAV_RADIUS_DP = 28

private fun mix(start: Float, stop: Float, fraction: Float): Float = start + (stop - start) * fraction
private const val OPEN_DAMPING = 0.85f
private const val MORPH_FADE_UNTIL = 0.04f
/** Below this cover-to-artwork ratio the morph zooms; above it, it moves. */
private const val ZOOM_BELOW = 0.8f
private const val TOAST_UNDER_TOP_BAR_DP = 64
private const val TOAST_MAX_DP = 560
private const val CARD_RADIUS_DP = 32
private const val SCRIM = 0.32f
private const val MINI_SWELL = 0.04f
private const val PULSE_DAMPING = 0.7f
private const val PULSE_UP_MS = 140
private const val MIN_TRAVEL = 0.3f
private const val DEFAULT_TRAVEL = 0.6f


private fun androidx.compose.ui.geometry.Rect.scaledAround(c: androidx.compose.ui.geometry.Offset, s: Float) =
    androidx.compose.ui.geometry.Rect(c.x + (left - c.x) * s, c.y + (top - c.y) * s, c.x + (right - c.x) * s, c.y + (bottom - c.y) * s)

/** A rounded rectangle at [region] within the layer: the part of Now Playing still seen. */
private class RegionShape(val region: androidx.compose.ui.geometry.Rect, val radius: Float) : androidx.compose.ui.graphics.Shape {
    override fun createOutline(size: androidx.compose.ui.geometry.Size, layoutDirection: androidx.compose.ui.unit.LayoutDirection, density: androidx.compose.ui.unit.Density): androidx.compose.ui.graphics.Outline =
        androidx.compose.ui.graphics.Outline.Rounded(androidx.compose.ui.geometry.RoundRect(region, androidx.compose.ui.geometry.CornerRadius(radius)))
}

/** Now Playing's artwork on screen; [shrinks]: the big cover, drawn at 88 % while paused. */
private class ArtSpot(val rect: androidx.compose.ui.geometry.Rect, val shrinks: Boolean)

/** A rect worth morphing to: finite, and not empty. */
private fun androidx.compose.ui.geometry.Rect.usable(): Boolean =
    left.isFinite() && top.isFinite() && right.isFinite() && bottom.isFinite() && width > 1f && height > 1f
