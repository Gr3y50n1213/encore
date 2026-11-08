package uk.gcartee.encore.app.ui.home

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.carousel.HorizontalMultiBrowseCarousel
import androidx.compose.material3.carousel.rememberCarouselState
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.layout
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.animation.core.animateFloat
import androidx.compose.ui.graphics.graphicsLayer
import uk.gcartee.encore.core.SongSort
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.graphics.shapes.RoundedPolygon
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.ui.components.Artwork
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.TILE_ART_DP
import uk.gcartee.encore.app.ui.components.TILE_RADIUS_DP
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.components.toPath
import uk.gcartee.encore.app.ui.library.EmptyState
import uk.gcartee.encore.app.ui.library.Mosaic
import uk.gcartee.encore.app.ui.library.MosaicFill
import uk.gcartee.encore.app.ui.library.SectionTitle
import uk.gcartee.encore.app.ui.library.SyncRow
import uk.gcartee.encore.app.ui.library.madeWhen
import uk.gcartee.encore.app.ui.nav.Route
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.Home
import uk.gcartee.encore.core.MixState
import uk.gcartee.encore.core.MixSummary
import uk.gcartee.encore.core.Song
import uk.gcartee.encore.core.StartedFrom
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/** Home's data, shared by Home and Mixes; yesterday's set shows while a new one builds. */
class HomeData(private val host: CoreHost) {
    var home by mutableStateOf<Home?>(null)
        private set
    var loading by mutableStateOf(false)
        private set
    var problem by mutableStateOf<String?>(null)
        private set

    suspend fun load(regenerate: Boolean = false) {
        if (loading) return
        loading = true
        val r = runCatching { if (regenerate) host.core.regenerateMixes() else host.core.home() }
        r.onSuccess { home = it; problem = null }.onFailure { problem = it.userMessage() }
        loading = false
    }
}

/** A Material shape as a Compose shape (board 23: generated mix covers only, never real art). */
class PolygonShape(private val polygon: RoundedPolygon) : Shape {
    override fun createOutline(size: Size, layoutDirection: LayoutDirection, density: Density): Outline =
        Outline.Generic(polygon.toPath(size))
}

/** Board 23, item 2: one shape per mix, picked from its id so it stays the same. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
fun shapeFor(mixId: String): Shape {
    val set = listOf(MaterialShapes.Cookie9Sided, MaterialShapes.Square, MaterialShapes.Arch, MaterialShapes.Clover4Leaf)
    return PolygonShape(set[Math.floorMod(mixId.hashCode(), set.size)])
}

/** Board 05. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun HomeScreen(host: CoreHost, data: HomeData, onProfile: () -> Unit, reselect: Int, contentPadding: PaddingValues) {
    val type = LocalEncoreType.current
    val actions = LocalAppActions.current
    val sync by host.sync.state.collectAsState()
    val scope = rememberCoroutineScope()
    val list = rememberLazyListState()
    ScrollToTopOn(reselect, list)
    LaunchedEffect(sync.generation) { data.load() }
    val home = data.home
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        // Whole screen: pull down refreshes the library (and Home after it).
        PullToRefreshBox(isRefreshing = sync.running, onRefresh = { host.sync.syncNow() }) {
            LazyColumn(state = list, contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp)) {
                // A greeting for the time of day, the way a listening app says hello.
                item { PageTitleRow(greeting(), host, onProfile) }
                item { SyncRow(sync) }
                if (home == null) {
                    item {
                        Box(Modifier.fillMaxWidth().height(240.dp), contentAlignment = Alignment.Center) {
                            if (data.problem != null) Text(data.problem.orEmpty(), style = type.body, color = MaterialTheme.colorScheme.error)
                            else LoadingIndicator()
                        }
                    }
                    return@LazyColumn
                }
                // The last album played, big, with Play; then more of what was played
                // lately as quick tiles, two to a row.
                val recent = (home.recentlyPlayed + home.recentlyAdded + home.mostPlayed).filter { it.albumId != null }.distinctBy { it.albumId }
                recent.firstOrNull()?.let { s -> item { JumpBackIn(s) } }
                val picks = recent.drop(1).take(QUICK_PICKS)
                if (picks.size >= 2) item { QuickPicks(picks) }
                // Item 2: Made for you, with every shelf state (CORE-API "Mix states").
                item { SectionTitle("Made for you") }
                item { MadeForYou(home, data, onOpenMix = { actions.open(Route.Mix(it)) }, onRefresh = { scope.launch { data.load(regenerate = true) } }) }
                // Items 3 and 4: tiles open, never play; Rediscover holds Not interested.
                shelf("Recently played", home.recentlyPlayed)
                shelf("Rediscover", home.rediscover, tileDp = LARGE_TILE_DP)
                shelf("Deep cuts", home.deepCuts)
                shelf("Recently added", home.recentlyAdded)
                shelf("Most played", home.mostPlayed)
                if (home.recentlyPlayed.isEmpty() && home.recentlyAdded.isEmpty() && home.mostPlayed.isEmpty()) {
                    item { EmptyState("Play something to train your mixes", "Your library is in the Library tab.") }
                }
            }
        }
    }
}

/** The title row with the profile button (board 05, item 1): a dot when changes wait to sync. */
@Composable
fun PageTitleRow(title: String, host: CoreHost, onProfile: () -> Unit, trailing: (@Composable () -> Unit)? = null) {
    val type = LocalEncoreType.current
    Row(
        Modifier.fillMaxWidth().statusBarsPadding().padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = (Grid.base * 2).dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, style = type.pageTitle, color = MaterialTheme.colorScheme.onSurface, modifier = Modifier.weight(1f).semantics { heading() })
        trailing?.invoke()
        ProfileButton(host, onProfile)
    }
}

/**
 * Board 05, item 1 and board 14: the avatar, the media server's picture or the initials,
 * with a dot while changes wait to sync. It presses in a little on a spring.
 */
@Composable
fun ProfileButton(host: CoreHost, onProfile: () -> Unit, size: androidx.compose.ui.unit.Dp = 48.dp) {
    val profile = remember { runCatching { host.core.profiles().firstOrNull { it.active } }.getOrNull() }
    val pending = remember { runCatching { host.core.pendingEdits() }.getOrDefault(0u) > 0u }
    val press = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val pressed by press.collectIsPressedAsState()
    val scale by androidx.compose.animation.core.animateFloatAsState(if (pressed) 0.9f else 1f, androidx.compose.animation.core.spring(dampingRatio = 0.6f), label = "press")
    Box(
        Modifier
            .size(size)
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            }
            .clip(CircleShape)
            .clickable(interactionSource = press, indication = androidx.compose.material3.ripple(), onClick = onProfile)
            .semantics { contentDescription = "Profiles and settings" },
    ) {
        uk.gcartee.encore.app.ui.components.ProfileAvatar(profile?.initials ?: "You", profile?.avatarUrl, size)
        if (pending) Box(Modifier.align(Alignment.TopEnd).size(10.dp).background(MaterialTheme.colorScheme.primary, CircleShape))
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun MadeForYou(home: Home, data: HomeData, onOpenMix: (String) -> Unit, onRefresh: () -> Unit) {
    val type = LocalEncoreType.current
    val shelf = home.madeForYou
    when (shelf.state) {
        MixState.NOT_ENOUGH_PLAYS -> if (shelf.mixes.isEmpty()) {
            Notice("Encore is learning your taste", "${shelf.playsSoFar} of ${shelf.playsNeeded} plays. Mixes appear as you listen.", learning = true)
            return
        }
        MixState.LEARNING_OFF -> {
            Notice("Learning is off", "Turn on Learn from my listening in Privacy and data for mixes.")
            return
        }
        MixState.PRIVATE -> {
            Notice("Private listening", "Mixes come back when Private listening or Guest ends.")
            return
        }
        else -> Unit
    }
    if (shelf.state == MixState.OUT_OF_DATE) {
        Row(Modifier.padding(horizontal = (Grid.base * 2).dp), verticalAlignment = Alignment.CenterVertically) {
            Text(madeWhen(shelf.madeUtcMs), style = type.caption, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
            FilledTonalButton(onClick = onRefresh, enabled = !data.loading) { Text("Refresh") }
        }
    }
    // Board 05, item 2: Android's first tile is wide and shrinks as it scrolls off: the
    // Material 3 Expressive multi-browse carousel, snapping one tile at a time. Rounded
    // tiles here; the Material shapes are the Mixes tab's (board 23).
    val state = rememberCarouselState { shelf.mixes.size }
    HorizontalMultiBrowseCarousel(
        state = state,
        preferredItemWidth = CAROUSEL_ITEM_DP.dp,
        itemSpacing = Grid.base.dp,
        contentPadding = PaddingValues(horizontal = (Grid.base * 2).dp),
        modifier = Modifier.fillMaxWidth().height(CAROUSEL_HEIGHT_DP.dp),
    ) { i ->
        val m = shelf.mixes[i]
        Column(Modifier.fillMaxSize().combinedClickable(onClick = { onOpenMix(m.mixId) })) {
            MosaicFill(m.coverSongIds, Modifier.weight(1f).fillMaxWidth().maskClip(MaterialTheme.shapes.extraLarge))
            Spacer(Modifier.height(6.dp))
            // Inset a little: the carousel clips each item to its mask, which can sit a hair
            // inside the item, and the first letter must never be cut.
            Text(m.title, style = type.tileTitle, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = CAROUSEL_LABEL_INSET_DP.dp))
            Text(m.subtitle, style = type.tileSubtitle, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = CAROUSEL_LABEL_INSET_DP.dp))
        }
    }
}

@Composable
private fun Notice(title: String, line: String, learning: Boolean = false) {
    val type = LocalEncoreType.current
    Surface(color = MaterialTheme.colorScheme.surfaceContainer, shape = androidx.compose.foundation.shape.RoundedCornerShape(28.dp), modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp)) {
        Row(Modifier.padding((Grid.base * 2).dp), verticalAlignment = Alignment.CenterVertically) {
            if (learning) {
                LearningOrb(Modifier.size(NOTICE_ORB_DP.dp))
                Spacer(Modifier.width((Grid.base * 2).dp))
            }
            Column(Modifier.weight(1f)) {
                Text(title, style = type.sectionHeader)
                Spacer(Modifier.height(4.dp))
                Text(line, style = type.body.copy(fontSize = 14.sp, lineHeight = 20.sp), color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** "Good morning" and so on, by the phone's clock. */
private fun greeting(): String = when (java.time.LocalTime.now().hour) {
    in 5..11 -> "Good morning"
    in 12..16 -> "Good afternoon"
    in 17..21 -> "Good evening"
    else -> "Good night"
}

/**
 * The album played last, big: the whole square cover beside its name, on the album's own
 * colour (board 04's palette, leading tone fading to the dark base), with Play. The card
 * opens the album; nothing is cropped.
 */
@Composable
private fun JumpBackIn(song: Song) {
    val actions = LocalAppActions.current
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val albumId = song.albumId ?: return
    val tint = uk.gcartee.encore.app.ui.library.rememberAlbumTint(actions.host, albumId)
    val press = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val pressed by press.collectIsPressedAsState()
    val scale by androidx.compose.animation.core.animateFloatAsState(if (pressed) PRESSED_SCALE else 1f, androidx.compose.animation.core.spring(dampingRatio = 0.6f), label = "press")
    Surface(
        onClick = { actions.open(Route.Album(albumId)) },
        interactionSource = press,
        shape = androidx.compose.foundation.shape.RoundedCornerShape(HERO_RADIUS_DP.dp),
        color = tint.field,
        modifier = Modifier
            .padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp)
            .fillMaxWidth()
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            },
    ) {
        Row(
            Modifier
                .background(androidx.compose.ui.graphics.Brush.linearGradient(listOf(tint.top, tint.field)))
                .padding(HERO_INSET_DP.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Artwork(uk.gcartee.encore.core.ArtworkOf.ALBUM, albumId, HERO_ART_DP.dp, Modifier.size(HERO_ART_DP.dp), cornerRadius = (HERO_RADIUS_DP - HERO_INSET_DP).dp)
            Spacer(Modifier.width((Grid.base * 2).dp))
            Column(Modifier.weight(1f)) {
                Text("Jump back in", style = type.buttonLabelSelected, color = tint.secondary)
                Spacer(Modifier.height(4.dp))
                Text(song.albumTitle ?: song.title, style = type.collectionTitle.copy(fontSize = 22.sp, lineHeight = 26.sp), color = tint.content, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(song.albumArtist.ifBlank { song.artist }, style = type.body.copy(fontSize = 15.sp), color = tint.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Spacer(Modifier.height((Grid.base * 1.5f).dp))
                FilledIconButton(
                    onClick = {
                        haptics.perform(Haptic.Soft)
                        val ids = runCatching { actions.host.core.album(albumId).songs.map { it.id } }.getOrDefault(emptyList())
                        if (ids.isNotEmpty()) actions.play(ids, 0, StartedFrom.ALBUM, albumId.toString(), song.albumTitle)
                    },
                    colors = IconButtonDefaults.filledIconButtonColors(containerColor = tint.content, contentColor = tint.field),
                    modifier = Modifier.size(HERO_PLAY_DP.dp),
                ) { Icon(Icons.play(), contentDescription = "Play ${song.albumTitle ?: song.title}", modifier = Modifier.size(26.dp)) }
            }
        }
    }
}
/** Recent albums as quick tiles, two to a row: the cover at the start, the name beside it. */
@Composable
private fun QuickPicks(songs: List<Song>) {
    val actions = LocalAppActions.current
    val type = LocalEncoreType.current
    Column(Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp), verticalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
        songs.chunked(2).forEach { row ->
            Row(horizontalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
                row.forEach { s ->
                    val albumId = s.albumId ?: return@forEach
                    Surface(
                        onClick = { actions.open(Route.Album(albumId)) },
                        shape = androidx.compose.foundation.shape.RoundedCornerShape(QUICK_RADIUS_DP.dp),
                        color = MaterialTheme.colorScheme.surfaceContainerHigh,
                        modifier = Modifier.weight(1f).height(QUICK_HEIGHT_DP.dp),
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Artwork(uk.gcartee.encore.core.ArtworkOf.ALBUM, albumId, QUICK_HEIGHT_DP.dp, Modifier.size(QUICK_HEIGHT_DP.dp), cornerRadius = 0.dp)
                            Text(
                                s.albumTitle ?: s.title,
                                style = type.tileTitle.copy(fontSize = 14.sp, lineHeight = 18.sp),
                                maxLines = 2,
                                overflow = TextOverflow.Ellipsis,
                                modifier = Modifier.weight(1f).padding(horizontal = (Grid.base * 1.5f).dp),
                            )
                        }
                    }
                }
                if (row.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun MixTile(m: MixSummary, wide: Boolean, onOpen: () -> Unit, modifier: Modifier = Modifier) {
    val type = LocalEncoreType.current
    val w = if (wide) (TILE_ART_DP * 1.25f) else TILE_ART_DP * 0.9f
    // 2xTAP does nothing, so nobody likes a playlist by accident.
    Column(modifier.width(w.dp).combinedClickable(onClick = onOpen)) {
        // Board 23: the mix's own Material shape with a tonal ring, the mosaic inside it.
        val shape = shapeFor(m.mixId)
        val ring = if (Math.floorMod(m.mixId.hashCode(), 2) == 0) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.tertiaryContainer
        Box(Modifier.fillMaxWidth().aspectRatio(1f).clip(shape).background(ring).padding(RING_DP.dp).clip(shape)) {
            MosaicFill(m.coverSongIds, Modifier.fillMaxSize())
        }
        Spacer(Modifier.height(6.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(m.title, style = type.tileTitle, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
            if (m.pinned) {
                // Item 4: a "Pinned" label.
                Spacer(Modifier.width(6.dp))
                Surface(shape = CircleShape, color = Color.Transparent, border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outline)) {
                    Text("Pinned", style = type.caption, modifier = Modifier.padding(horizontal = 6.dp, vertical = 1.dp))
                }
            }
        }
        Text(m.subtitle, style = type.tileSubtitle, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** A shelf of songs shown as their albums; a tap opens the album and never plays. */
private fun androidx.compose.foundation.lazy.LazyListScope.shelf(title: String, songs: List<Song>, tileDp: Int = SHELF_TILE_DP) {
    val albums = songs.filter { it.albumId != null }.distinctBy { it.albumId }
    if (albums.isEmpty()) return
    item { SectionTitle(title) }
    item {
        val actions = LocalAppActions.current
        LazyRow(contentPadding = PaddingValues(horizontal = (Grid.base * 2).dp), horizontalArrangement = Arrangement.spacedBy((Grid.base * 2).dp)) {
            items(albums.size) { i ->
                val s = albums[i]
                SongAlbumTile(s, tileDp, onOpen = { s.albumId?.let { actions.open(Route.Album(it)) } })
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun SongAlbumTile(song: Song, tileDp: Int, onOpen: () -> Unit) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val actions = LocalAppActions.current
    var menu by remember { mutableStateOf(false) }
    Box {
        Column(Modifier.width(tileDp.dp).combinedClickable(onClick = onOpen, onLongClick = { haptics.perform(Haptic.LongPress); menu = true })) {
            Artwork(uk.gcartee.encore.core.ArtworkOf.SONG, song.id, tileDp.dp, Modifier.fillMaxWidth(), cornerRadius = (tileDp / 7).dp)
            Spacer(Modifier.height(6.dp))
            Text(song.albumTitle ?: song.title, style = type.tileTitle, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(song.albumArtist.ifBlank { song.artist }, style = type.tileSubtitle, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
        }
        // Item 3: the item menu, the app's one action sheet; Rediscover's includes Not interested.
        if (menu) {
            val playIcon = Icons.play()
            val next = Icons.playNext()
            val block = Icons.block()
            uk.gcartee.encore.app.ui.components.ActionSheet(
                onDismiss = { menu = false },
                header = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Artwork(uk.gcartee.encore.core.ArtworkOf.SONG, song.id, 56.dp, Modifier.size(56.dp), cornerRadius = 14.dp)
                        Spacer(Modifier.width((Grid.base * 2).dp))
                        Column(Modifier.weight(1f)) {
                            Text(song.albumTitle ?: song.title, style = type.sectionHeader, maxLines = 2, overflow = TextOverflow.Ellipsis)
                            Text(song.albumArtist.ifBlank { song.artist }, style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                    }
                },
                quick = listOf(
                    uk.gcartee.encore.app.ui.components.SheetAction("Play", playIcon) {
                        song.albumId?.let { id ->
                            val ids = runCatching { actions.host.core.album(id).songs.map { it.id } }.getOrDefault(emptyList())
                            actions.play(ids, 0, StartedFrom.ALBUM, id.toString(), song.albumTitle)
                        }
                    },
                    uk.gcartee.encore.app.ui.components.SheetAction("Play next", next) { actions.playNext(listOf(song)) },
                ),
                groups = listOf(listOf(uk.gcartee.encore.app.ui.components.SheetAction("Not interested", block, destructive = true) { actions.notInterested(song) })),
            )
        }
    }
}

/** Board 05, item 6: tapping the active tab scrolls to top. */
@Composable
fun ScrollToTopOn(reselect: Int, list: LazyListState) {
    LaunchedEffect(reselect) { if (reselect > 0) list.animateScrollToItem(0) }
}

/**
 * Board 23: today's mixes in a 2-column grid (3 to 5 on wider windows), each cover a
 * mosaic in its own Material shape, a small play button at its corner (44 dp hit area,
 * away from the card's tap), pinned ones first. The title scrolls away with the grid, as
 * Home's does.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MixesScreen(host: CoreHost, data: HomeData, onProfile: () -> Unit, contentPadding: PaddingValues) {
    val type = LocalEncoreType.current
    val actions = LocalAppActions.current
    LaunchedEffect(Unit) { if (data.home == null) data.load() }
    val shelf = data.home?.madeForYou
    val grid = rememberLazyGridState()
    when {
        shelf == null -> Column(Modifier.fillMaxSize()) {
            PageTitleRow("Mixes", host, onProfile)
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicator() }
        }
        shelf.mixes.isEmpty() -> MixesLearning(host, shelf.playsSoFar.toInt(), shelf.playsNeeded.toInt(), onProfile, contentPadding)
        else -> LazyVerticalGrid(
            columns = GridCells.Adaptive(MIX_MIN_DP.dp),
            state = grid,
            contentPadding = PaddingValues(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp),
            horizontalArrangement = Arrangement.spacedBy((Grid.base * 2).dp),
            verticalArrangement = Arrangement.spacedBy((Grid.base * 3).dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            item(span = { androidx.compose.foundation.lazy.grid.GridItemSpan(maxLineSpan) }) {
                // The grid pads its sides; the title row brings its own padding, so it bleeds.
                Column(Modifier.bleed((Grid.base * 2).dp)) {
                    PageTitleRow("Mixes", host, onProfile)
                    // Item 1: when and where they were made.
                    val count = shelf.mixes.size
                    Text(
                        "${if (count == 1) "One new mix" else "$count new mixes"} today, made on ${if (shelf.fromCompanion) "your server" else "this device"}",
                        style = type.caption,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = Grid.base.dp),
                    )
                }
            }
            items(shelf.mixes.sortedByDescending { it.pinned }, key = { it.mixId }) { m ->
                Box {
                    MixTile(m, wide = false, onOpen = { actions.open(Route.Mix(m.mixId)) }, modifier = Modifier.fillMaxWidth())
                    // Item 3: play from the card.
                    FilledIconButton(
                        onClick = {
                            val ids = runCatching { host.core.mix(m.mixId).songs.map { it.song.id } }.getOrDefault(emptyList())
                            actions.play(ids, 0, StartedFrom.MIX, m.mixId, m.title)
                        },
                        modifier = Modifier.align(Alignment.TopEnd).padding(6.dp).size(44.dp),
                    ) { Icon(Icons.play(), contentDescription = "Play ${m.title}") }
                }
            }
        }
    }
}

/**
 * Boards 15, 23 and 38, before there are any mixes: the learning animation, what is
 * happening in one plain sentence, the plays so far on a progress line and Play something
 * (board 15, "Not enough history"). It scrolls, title and all, on small screens.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun MixesLearning(host: CoreHost, plays: Int, needed: Int, onProfile: () -> Unit, contentPadding: PaddingValues) {
    val type = LocalEncoreType.current
    val actions = LocalAppActions.current
    val scheme = MaterialTheme.colorScheme
    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        PageTitleRow("Mixes", host, onProfile)
        Spacer(Modifier.height((Grid.base * 3).dp))
        LearningOrb(Modifier.size(LEARNING_ORB_DP.dp))
        Spacer(Modifier.height((Grid.base * 3).dp))
        Text("Encore is learning your taste", style = type.collectionTitle.copy(fontSize = LEARNING_TITLE_SP.sp), textAlign = androidx.compose.ui.text.style.TextAlign.Center, modifier = Modifier.padding(horizontal = (Grid.base * 4).dp))
        Spacer(Modifier.height(Grid.base.dp))
        Text(
            "Keep listening and your mixes will appear here. They're made on this phone, from what you play.",
            style = type.body.copy(fontSize = 15.sp, lineHeight = 21.sp),
            color = scheme.onSurfaceVariant,
            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            modifier = Modifier.padding(horizontal = (Grid.base * 4).dp),
        )
        Spacer(Modifier.height((Grid.base * 3).dp))
        Column(Modifier.padding(horizontal = (Grid.base * 5).dp).fillMaxWidth()) {
            androidx.compose.material3.LinearWavyProgressIndicator(
                progress = { if (needed > 0) (plays.toFloat() / needed).coerceIn(0f, 1f) else 0f },
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(Grid.base.dp))
            Text("$plays of $needed plays", style = type.caption, color = scheme.onSurfaceVariant, modifier = Modifier.align(Alignment.CenterHorizontally))
        }
        Spacer(Modifier.height((Grid.base * 3).dp))
        androidx.compose.material3.Button(
            onClick = {
                val ids = runCatching { host.core.songs(SongSort.RECENTLY_ADDED, 0u, PLAY_SOMETHING_SONGS.toUInt()).map { it.id } }.getOrDefault(emptyList())
                if (ids.isNotEmpty()) actions.play(ids, ids.indices.random(), StartedFrom.SONGS, null, "Your library", shuffle = true)
            },
            modifier = Modifier.height(56.dp),
        ) {
            Icon(Icons.shuffle(), contentDescription = null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(Grid.base.dp))
            Text("Play something", style = type.sectionHeader.copy(fontSize = 16.sp))
        }
    }
}

/** Lets a full-width item ignore [horizontal] of its list's side padding on each side. */
fun Modifier.bleed(horizontal: androidx.compose.ui.unit.Dp): Modifier = this.then(
    Modifier.layout { measurable, constraints ->
        val extra = horizontal.roundToPx()
        val wide = constraints.copy(minWidth = constraints.minWidth + 2 * extra, maxWidth = constraints.maxWidth + 2 * extra)
        val placeable = measurable.measure(wide)
        layout(constraints.maxWidth, placeable.height) { placeable.place(-extra, 0) }
    },
)

// Two columns even with a large display size (the S20 FE at its zoom is about 360 dp wide).
private const val MIX_MIN_DP = 128
private const val LEARNING_ORB_DP = 240
private const val LEARNING_TITLE_SP = 26
private const val PLAY_SOMETHING_SONGS = 300
// Board 05's sizes, measured from the Android render.
private const val CAROUSEL_ITEM_DP = 220
// Board 05: the large tile is wider than tall (about 220 by 176 dp), its labels below.
private const val CAROUSEL_HEIGHT_DP = 222
private const val CAROUSEL_LABEL_INSET_DP = 4
private const val SHELF_TILE_DP = 132
private const val LARGE_TILE_DP = 160
private const val QUICK_PICKS = 6
private const val QUICK_HEIGHT_DP = 60
private const val QUICK_RADIUS_DP = 16
private const val HERO_RADIUS_DP = 28
private const val HERO_ART_DP = 132
private const val HERO_INSET_DP = 12
private const val HERO_PLAY_DP = 52
private const val PRESSED_SCALE = 0.98f
private const val NOTICE_ORB_DP = 72
private const val SMALL_TILE_DP = 96
private const val RING_DP = 6
