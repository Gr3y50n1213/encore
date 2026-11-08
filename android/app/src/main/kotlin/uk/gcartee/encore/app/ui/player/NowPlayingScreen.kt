package uk.gcartee.encore.app.ui.player

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.basicMarquee
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FilledTonalIconToggleButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.playback.PlayerState
import uk.gcartee.encore.app.ui.components.Artwork
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.rememberPop
import uk.gcartee.encore.app.ui.components.fadingEdges
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.MenuPlace
import uk.gcartee.encore.app.ui.components.SongMenu
import uk.gcartee.encore.app.ui.components.formatDuration
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.core.ListeningMode
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Motion.Timing

/** Board 09, item 8: the view shown in place of the artwork. */
enum class PlayerPanel { Artwork, Lyrics, Queue }

/**
 * Board 09, Android: the album's moving field; collapse and More over it; the artwork
 * (swipe to skip, double tap to like, hold for the menu, 88 % while paused); title and
 * artist; like; the wavy scrub bar; the transport row; and Lyrics, Output and Queue as one
 * connected group that swaps the artwork for that view in place. A swipe down follows
 * the finger and collapses past 30 % of the height or a fast fling, never while the
 * scrub bar is held. Landscape phones put the artwork left and the rest right.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class, ExperimentalFoundationApi::class)
@Composable
fun NowPlayingScreen(
    host: CoreHost,
    state: PlayerState,
    onCollapse: () -> Unit,
    onOpen: (Route) -> Unit,
    onOutput: () -> Unit,
    // Where the artwork is on screen, for the morph into the mini player (EncoreRoot), and
    // whether it is the big cover (which shrinks while paused) or the small one over a panel.
    onArtBounds: (androidx.compose.ui.geometry.Rect, Boolean) -> Unit = { _, _ -> },
    // The scrub bar is held: the swipe down to collapse (EncoreRoot) must not start.
    onScrubHeld: (Boolean) -> Unit = {},
) {
    val entry = state.entry ?: return
    val song = entry.song
    val context = LocalContext.current
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val actions = LocalAppActions.current
    val still = reduceMotion()
    val scope = rememberCoroutineScope()
    val colours = rememberFieldColours(host, song.albumId)
    uk.gcartee.encore.app.ui.components.SystemBarIconsFor(colours.base)
    val content = colours.text
    var panel by rememberSaveable { mutableStateOf(PlayerPanel.Artwork) }
    var liked by remember(song.id) { mutableStateOf(actions.isLiked(song)) }
    var scrubbing by remember { mutableStateOf<Long?>(null) }
    // The position on frame time, read where it is drawn (the bar, the lyrics).
    val smooth = rememberSmoothPosition(state.positionMs, state.isPlaying, state.durationMs)
    val position: () -> Long = { scrubbing ?: smooth.value }
    var scrubHeld by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var moreMenu by remember { mutableStateOf(false) }
    var heart by remember { mutableStateOf(false) }
    var endOfQueue by remember { mutableStateOf(false) }
    val playingFrom by host.playingFrom.collectAsState()
    val privateOn by host.privateListening.collectAsState()
    val remoteChanges by host.remoteChanges.collectAsState()
    LaunchedEffect(remoteChanges, song.id) { liked = actions.isLiked(song) }
    LaunchedEffect(heart) {
        if (heart) {
            kotlinx.coroutines.delay(HEART_MS)
            heart = false
        }
    }

    fun toggleLike(on: Boolean) {
        if (actions.setLiked(song, on)) {
            liked = on
            haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
        } else {
            haptics.perform(Haptic.Reject)
        }
    }

    // Item 4: More's menu, the same wherever its button is (top bar, a panel's song row, a
    // turned phone's controls): the song's actions plus Private listening.
    val moreSheet: @Composable () -> Unit = {
        SongMenu(
            song,
            expanded = moreMenu,
            onDismiss = { moreMenu = false },
            place = MenuPlace(
                chosenBy = entry.chosenBy,
                extra = listOf(
                    uk.gcartee.encore.app.ui.components.MenuExtra(if (privateOn) "Turn off Private listening" else "Private listening", destructive = false) {
                        val on = !privateOn
                        runCatching { host.core.setListeningMode(if (on) ListeningMode.PRIVATE else ListeningMode.NORMAL) }
                            .onSuccess {
                                host.privateListening.value = on
                                actions.toaster.show(if (on) "Private listening is on. Nothing is recorded." else "Private listening is off")
                            }
                    },
                ),
            ),
        )
    }

    // Item 1: the swipe down is EncoreRoot's: it drives the morph into the mini player.
    BoxWithConstraints(
        Modifier.fillMaxSize(),
    ) {
        // Landscape (a turned phone, a tablet): artwork left, the rest right.
        val landscape = maxWidth > maxHeight
        // Read here, in the BoxWithConstraints scope itself.
        val shortWindow = maxHeight < LANDSCAPE_TALL_DP.dp
        // A phone on its side: nothing scrolls on the controls side, so the top bar's two
        // buttons move into that column and the spacing tightens.
        val dense = landscape && shortWindow
        AlbumField(colours, Modifier.fillMaxSize())
        Column(Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = (Grid.base * 3).dp)) {
            // Item 1: collapse, "Playing from", More.
            // Board 10: with Lyrics or the Queue open the header is the compact song row below.
            if ((panel == PlayerPanel.Artwork || landscape) && !dense) Row(Modifier.fillMaxWidth().padding(vertical = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
                FilledTonalIconButton(
                    onClick = onCollapse,
                    colors = IconButtonDefaults.filledTonalIconButtonColors(containerColor = content.copy(alpha = 0.16f), contentColor = content),
                ) { Icon(Icons.collapse(), contentDescription = "Close Now Playing") }
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    // Phase 8: offline, a small chip says why other songs are passed over.
                    if (!uk.gcartee.encore.app.ui.components.LocalDownloads.current.online) {
                        uk.gcartee.encore.app.ui.components.OfflineChip(content, Modifier.padding(bottom = 2.dp))
                    }
                    if (privateOn) Text("Private listening", style = type.caption, color = content.copy(alpha = 0.8f))
                    playingFrom?.let {
                        Text("Playing from", style = type.caption, color = content.copy(alpha = 0.72f))
                        Text(it, style = type.buttonLabelSelected, color = content, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
                Box {
                    MoreButton(moreMenu, content) { moreMenu = true }
                    // Item 4: the artwork's menu plus Private listening (Sleep Timer is Phase 9).
                    moreSheet()
                }
            }
            val artArea: @Composable (Modifier) -> Unit = { m ->
                AnimatedContent(
                    targetState = panel,
                    transitionSpec = { fadeIn(tween(if (still) Timing.reduceMotionCrossfadeMs else 250)) togetherWith fadeOut(tween(if (still) Timing.reduceMotionCrossfadeMs else 200)) },
                    label = "panel",
                    modifier = m,
                ) { p ->
                    when (p) {
                        PlayerPanel.Artwork -> ArtworkArea(host, state, liked, heart, endOfQueue, onArtBounds = { onArtBounds(it, true) }, onLike = {
                            // Double tap likes with a heart pop.
                            if (!liked) toggleLike(true) else haptics.perform(Haptic.Confirm)
                            heart = true
                        }, onMenu = { menu = true }, onEnd = { endOfQueue = it })
                        PlayerPanel.Lyrics -> LyricsPanel(host, song, position, content, onSeek = { PlaybackHub.seekAndPlay(context, it) }, modifier = Modifier.fillMaxSize().fadingEdges())
                        PlayerPanel.Queue -> QueuePanel(host, content, onPlayer = { panel = PlayerPanel.Artwork }, modifier = Modifier.fillMaxSize().fadingEdges(), onColour = colours.base)
                    }
                }
                Box { SongMenu(song, expanded = menu, onDismiss = { menu = false }, place = MenuPlace(chosenBy = entry.chosenBy)) }
            }
            val controls: @Composable (Boolean, TransportSize) -> Unit = { compact, transport ->
                Column {
                    if (dense) {
                        // Collapse and More, at the top of the controls instead of a top bar.
                        Row(Modifier.fillMaxWidth().padding(bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                            FilledTonalIconButton(
                                onClick = onCollapse,
                                colors = IconButtonDefaults.filledTonalIconButtonColors(containerColor = content.copy(alpha = 0.16f), contentColor = content),
                            ) { Icon(Icons.collapse(), contentDescription = "Close Now Playing") }
                            Spacer(Modifier.weight(1f))
                            Box {
                                MoreButton(moreMenu, content) { moreMenu = true }
                                moreSheet()
                            }
                        }
                    }
                    if (!compact) Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            // Item 3: title opens the album, artist the artist, both collapsing first.
                            Text(
                                song.title,
                                style = if (dense) type.nowPlayingTitle.copy(fontSize = DENSE_TITLE_SP.sp, lineHeight = (DENSE_TITLE_SP + 4).sp) else type.nowPlayingTitle,
                                color = content,
                                maxLines = 1,
                                modifier = Modifier
                                    .semantics { heading() }
                                    .let { if (still) it else it.basicMarquee(initialDelayMillis = MARQUEE_DELAY_MS) }
                                    .clickable(enabled = song.albumId != null) {
                                        onCollapse()
                                        song.albumId?.let { onOpen(Route.Album(it)) }
                                    },
                            )
                            Text(
                                listOfNotNull(song.artist, song.albumTitle).joinToString(" · "),
                                style = type.nowPlayingSubtitle,
                                color = colours.secondary,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                modifier = Modifier.clickable {
                                    onCollapse()
                                    actions.goToArtist(song)
                                },
                            )
                        }
                        // Item 4: like; also the "love" signal. Board 35: off it is round and
                        // tonal, on it turns squarer and fills, and the heart pops 1 -> 1.25 -> 1.
                        val heart = rememberPop(liked)
                        FilledTonalIconToggleButton(
                            checked = liked,
                            onCheckedChange = ::toggleLike,
                            shapes = IconButtonDefaults.toggleableShapes(
                                shape = androidx.compose.foundation.shape.RoundedCornerShape(50),
                                pressedShape = androidx.compose.foundation.shape.RoundedCornerShape(14.dp),
                                checkedShape = androidx.compose.foundation.shape.RoundedCornerShape(18.dp),
                            ),
                            colors = IconButtonDefaults.filledTonalIconToggleButtonColors(
                                containerColor = content.copy(alpha = 0.16f),
                                contentColor = content,
                                checkedContainerColor = content,
                                checkedContentColor = colours.base,
                            ),
                            modifier = Modifier.size(if (dense) 48.dp else 56.dp).semantics { stateDescription = if (liked) "Liked" else "Not liked" },
                        ) { Icon(Icons.favorite(liked), contentDescription = "Like", modifier = Modifier.graphicsLayer { scaleX = heart; scaleY = heart }) }
                    }
                    if (endOfQueue) Text("End of queue", style = type.caption, color = content.copy(alpha = 0.8f))
                    Spacer(Modifier.height(Grid.base.dp))
                    // Item 5.
                    ScrubBar(
                        smooth = { smooth.value },
                        positionMs = state.positionMs,
                        durationMs = state.durationMs,
                        playing = state.isPlaying,
                        colour = content,
                        onSeek = { PlaybackHub.seekTo(context, it) },
                        onScrub = { scrubbing = it },
                        onHeld = { scrubHeld = it; onScrubHeld(it) },
                        buffering = state.buffering,
                    )
                    var remaining by rememberSaveable { mutableStateOf(false) }
                    val pos = scrubbing ?: state.positionMs
                    if (!compact) Row(Modifier.fillMaxWidth()) {
                        Text(formatDuration(pos), style = type.caption, color = colours.secondary)
                        Spacer(Modifier.weight(1f))
                        Text(
                            if (remaining) "-" + formatDuration(state.durationMs - pos) else formatDuration(state.durationMs),
                            style = type.caption,
                            color = colours.secondary,
                            modifier = Modifier.clickable { remaining = !remaining },
                        )
                    }
                    Spacer(Modifier.height(if (dense) Grid.base.dp else (Grid.base * 2).dp))
                    // Item 6: three separate buttons 8 dp apart, centred.
                    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                        TransportRow(
                            playing = state.isPlaying,
                            loading = state.buffering,
                            onPrevious = { PlaybackHub.previous(context) },
                            onToggle = { PlaybackHub.togglePlay(context) },
                            onNext = { PlaybackHub.next(context) },
                            onScan = { forward, active -> if (active) PlaybackHub.startScan(context, forward) else PlaybackHub.stopScan(context) },
                            container = content,
                            onContainer = colours.base,
                            size = transport,
                        )
                    }
                    Spacer(Modifier.height(if (dense) (Grid.base * 1.5f).dp else (Grid.base * 3).dp))
                    // Item 8: Lyrics, Output and Queue, one connected group.
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
                        listOf(PlayerPanel.Lyrics to "Lyrics", null to "Output", PlayerPanel.Queue to "Queue").forEachIndexed { i, (p, label) ->
                            val checked = p != null && panel == p
                            ToggleButton(
                                checked = checked,
                                onCheckedChange = {
                                    haptics.perform(Haptic.Tick)
                                    if (p == null) onOutput() else panel = if (panel == p) PlayerPanel.Artwork else p
                                },
                                shapes = when (i) {
                                    0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                                    2 -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                                    else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                                },
                                colors = ToggleButtonDefaults.colors(
                                    containerColor = content.copy(alpha = 0.12f),
                                    contentColor = content,
                                    checkedContainerColor = content.copy(alpha = 0.3f),
                                    checkedContentColor = content,
                                ),
                                modifier = Modifier.weight(1f),
                            ) {
                                Icon(
                                    when (i) {
                                        0 -> Icons.lyrics()
                                        1 -> Icons.output()
                                        else -> Icons.queue()
                                    },
                                    contentDescription = null,
                                    modifier = Modifier.size(18.dp),
                                )
                                Spacer(Modifier.size(6.dp))
                                Text(label, style = type.buttonLabel)
                            }
                        }
                    }
                    Spacer(Modifier.height(if (dense) 0.dp else (Grid.base * 2).dp))
                }
            }
            if (landscape) {
                // The controls side is fixed: it fits (the medium transport on a short window),
                // so only the artwork's swipe, the lyrics and the queue move.
                Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy((Grid.base * 3).dp)) {
                    artArea(Modifier.weight(1f).fillMaxSize().padding(vertical = if (dense) Grid.base.dp else 0.dp))
                    Box(Modifier.weight(1f).fillMaxSize(), contentAlignment = Alignment.Center) {
                        controls(false, if (shortWindow) TransportSize.Medium else TransportSize.Large)
                    }
                }
            } else if (panel != PlayerPanel.Artwork) {
                // Board 10, item 1: art and title stay pinned so the song is always known; a
                // tap returns to the artwork. Item 8: the controls, compact.
                Row(Modifier.fillMaxWidth().padding(vertical = Grid.base.dp), verticalAlignment = Alignment.CenterVertically) {
                    Row(
                        Modifier.weight(1f).clickable(onClickLabel = "Show artwork") { panel = PlayerPanel.Artwork },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Artwork(ArtworkOf.SONG, song.id, COMPACT_ART_DP.dp, Modifier.size(COMPACT_ART_DP.dp).onGloballyPositioned { onArtBounds(it.boundsInRoot(), false) }, cornerRadius = 12.dp)
                        Spacer(Modifier.size((Grid.base * 1.5f).dp))
                        Column {
                            Text(song.title, style = type.songRowTitle.copy(fontSize = type.nowPlayingSubtitle.fontSize), color = content, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(song.artist, style = type.body, color = colours.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                    }
                    Box {
                        MoreButton(moreMenu, content) { moreMenu = true }
                        moreSheet()
                    }
                }
                // Lyrics and the Queue sit straight on the field, fading at their edges.
                Box(Modifier.weight(1f).fillMaxWidth().fadingEdges()) {
                    when (panel) {
                        PlayerPanel.Lyrics -> LyricsPanel(host, song, position, content, onSeek = { PlaybackHub.seekAndPlay(context, it) }, modifier = Modifier.fillMaxSize())
                        else -> QueuePanel(host, content, onPlayer = { panel = PlayerPanel.Artwork }, modifier = Modifier.fillMaxSize(), onColour = colours.base)
                    }
                }
                controls(true, TransportSize.Medium)
            } else {
                artArea(Modifier.weight(1f).fillMaxWidth())
                controls(false, TransportSize.Large)
            }
        }
    }
}

/** Item 2: the artwork, which swipes to skip (board 42), double-taps to like, holds for the menu. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ArtworkArea(
    host: CoreHost,
    state: PlayerState,
    liked: Boolean,
    heart: Boolean,
    endOfQueue: Boolean,
    onLike: () -> Unit,
    onMenu: () -> Unit,
    onEnd: (Boolean) -> Unit,
    onArtBounds: (androidx.compose.ui.geometry.Rect) -> Unit = {},
) {
    val context = LocalContext.current
    val haptics = rememberHaptics()
    val still = reduceMotion()
    val scale by animateFloatAsState(if (state.isPlaying || still) 1f else PAUSED_SCALE, androidx.compose.animation.core.spring(dampingRatio = ART_DAMPING, stiffness = androidx.compose.animation.core.Spring.StiffnessMediumLow), label = "art")
    BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        // As big as the smaller side of the space (landscape is height-bound), never more
        // than the board's 420 dp, so it never runs under the header or off the screen.
        val side = minOf(maxWidth, maxHeight, ART_MAX_DP.dp)
        SkipSwipe(
            host,
            current = state.entry,
            onNext = { PlaybackHub.swipeNext(context) },
            onPrevious = { PlaybackHub.swipePrevious(context) },
            onEndOfQueue = onEnd,
            modifier = Modifier.size(side),
        ) { entry, slot ->
            val id = entry?.song?.id ?: return@SkipSwipe
            Box(
                Modifier
                    .size(side)
                    .let { m -> if (slot == Slot.Current) m.onGloballyPositioned { onArtBounds(it.boundsInRoot()) } else m }
                    .graphicsLayer {
                        val s = if (slot == Slot.Current) scale else 1f
                        scaleX = s
                        scaleY = s
                        // Clipped here too, so a swipe's scale and fade keep the corners.
                        shape = androidx.compose.foundation.shape.RoundedCornerShape(ART_RADIUS_DP.dp)
                        clip = true
                    }
                    .combinedClickable(
                        // The owner: a tap on the cover plays or pauses, like the button; the
                        // cover shrinks to 88 % paused and grows back playing (item 2).
                        onClick = {
                            haptics.perform(Haptic.Transport)
                            PlaybackHub.togglePlay(context)
                        },
                        onClickLabel = if (state.isPlaying) "Pause" else "Play",
                        onDoubleClick = onLike,
                        onLongClick = {
                            haptics.perform(Haptic.LongPress)
                            onMenu()
                        },
                        onLongClickLabel = "More options",
                    ),
            ) {
                Artwork(ArtworkOf.SONG, id, side, Modifier.fillMaxSize(), cornerRadius = ART_RADIUS_DP.dp)
            }
        }
        androidx.compose.animation.AnimatedVisibility(heart, enter = scaleIn(spring()) + fadeIn(), exit = scaleOut() + fadeOut()) {
            Icon(Icons.favorite(true), contentDescription = null, tint = Color.White, modifier = Modifier.size(96.dp))
        }
    }
}

private const val PAUSED_SCALE = 0.88f
/** The paused artwork's scale, for the morph into the mini player (EncoreRoot). */
internal const val ART_PAUSED_SCALE = PAUSED_SCALE
private const val ART_DAMPING = 0.7f
private const val COMPACT_ART_DP = 52

/**
 * More, on the field: round at rest, it squeezes when pressed and stays a squircle while
 * its sheet is open (board 35's toggle rule), so it is clear what opened the sheet.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun MoreButton(open: Boolean, content: Color, onOpen: () -> Unit) {
    androidx.compose.material3.FilledTonalIconToggleButton(
        checked = open,
        onCheckedChange = { onOpen() },
        shapes = IconButtonDefaults.toggleableShapes(
            shape = androidx.compose.foundation.shape.RoundedCornerShape(50),
            pressedShape = androidx.compose.foundation.shape.RoundedCornerShape(12.dp),
            checkedShape = androidx.compose.foundation.shape.RoundedCornerShape(16.dp),
        ),
        colors = IconButtonDefaults.filledTonalIconToggleButtonColors(
            containerColor = content.copy(alpha = 0.16f),
            contentColor = content,
            checkedContainerColor = content.copy(alpha = 0.30f),
            checkedContentColor = content,
        ),
        modifier = Modifier.testTag("now-playing-more"),
    ) { Icon(Icons.more(), contentDescription = "More") }
}
private const val ART_MAX_DP = 420
private const val LANDSCAPE_TALL_DP = 520
private const val DENSE_TITLE_SP = 22
private const val ART_RADIUS_DP = 28
private const val MARQUEE_DELAY_MS = 1_500
private const val HEART_MS = 700L
