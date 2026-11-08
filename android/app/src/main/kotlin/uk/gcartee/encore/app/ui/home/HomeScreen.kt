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
                item { PageTitleRow("Home", host, onProfile) }
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
                // Item 2: Made for you, with every shelf state (CORE-API "Mix states").
                item { SectionTitle("Made for you") }
                item { MadeForYou(home, data, onOpenMix = { actions.open(Route.Mix(it)) }, onRefresh = { scope.launch { data.load(regenerate = true) } }) }
                // Items 3 and 4: tiles open, never play; Rediscover holds Not interested.
                // Board 05: Recently played is a row of small tiles, Rediscover larger.
                shelf("Recently played", home.recentlyPlayed, tileDp = SMALL_TILE_DP)
                shelf("Rediscover", home.rediscover)
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

/** Board 05, item 1 and board 14: the avatar (initials until an image), a dot while changes wait to sync. */
@Composable
fun ProfileButton(host: CoreHost, onProfile: () -> Unit) {
    val type = LocalEncoreType.current
    val profile = remember { runCatching { host.core.profiles().firstOrNull { it.active } }.getOrNull() }
    val pending = remember { runCatching { host.core.pendingEdits() }.getOrDefault(0u) > 0u }
    Box {
        FilledIconButton(
            onClick = onProfile,
            colors = IconButtonDefaults.filledIconButtonColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHighest, contentColor = MaterialTheme.colorScheme.onSurface),
            modifier = Modifier.size(48.dp).semantics { contentDescription = "Profiles and settings" },
        ) {
            Text(profile?.initials ?: "You", style = type.buttonLabelSelected)
        }
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
            Notice("Encore is still learning", "${shelf.playsSoFar} of ${shelf.playsNeeded} plays. Mixes appear as you listen.")
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
private fun Notice(title: String, line: String) {
    val type = LocalEncoreType.current
    Surface(color = MaterialTheme.colorScheme.surfaceContainer, shape = androidx.compose.foundation.shape.RoundedCornerShape(28.dp), modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp)) {
        Column(Modifier.padding((Grid.base * 2).dp)) {
            Text(title, style = type.sectionHeader)
            Spacer(Modifier.height(4.dp))
            Text(line, style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
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
        // Item 3: the item menu; Rediscover's includes Not interested.
        DropdownMenu(menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text("Play") }, onClick = {
                menu = false
                song.albumId?.let { id ->
                    val ids = runCatching { actions.host.core.album(id).songs.map { it.id } }.getOrDefault(emptyList())
                    actions.play(ids, 0, StartedFrom.ALBUM, id.toString(), song.albumTitle)
                }
            })
            DropdownMenuItem(text = { Text("Play next") }, onClick = { menu = false; actions.playNext(listOf(song)) })
            DropdownMenuItem(text = { Text("Not interested", color = MaterialTheme.colorScheme.error) }, onClick = { menu = false; actions.notInterested(song) })
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
 * away from the card's tap), pinned ones first.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MixesScreen(host: CoreHost, data: HomeData, onProfile: () -> Unit, contentPadding: PaddingValues) {
    val type = LocalEncoreType.current
    val actions = LocalAppActions.current
    LaunchedEffect(Unit) { if (data.home == null) data.load() }
    val shelf = data.home?.madeForYou
    val grid = rememberLazyGridState()
    Column(Modifier.fillMaxSize()) {
        PageTitleRow("Mixes", host, onProfile)
        when {
            shelf == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicator() }
            shelf.mixes.isEmpty() -> MixesLearning(host, shelf.playsSoFar.toInt(), shelf.playsNeeded.toInt(), contentPadding)
            else -> {
                // Item 1: when and where they were made.
                val count = shelf.mixes.size
                Text(
                    "${if (count == 1) "One new mix" else "$count new mixes"} today, made on ${if (shelf.fromCompanion) "your server" else "this device"}",
                    style = type.caption,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = (Grid.base * 2).dp),
                )
                LazyVerticalGrid(
                    columns = GridCells.Adaptive(160.dp),
                    state = grid,
                    contentPadding = PaddingValues(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = (Grid.base * 2).dp, bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp),
                    horizontalArrangement = Arrangement.spacedBy((Grid.base * 2).dp),
                    verticalArrangement = Arrangement.spacedBy((Grid.base * 3).dp),
                ) {
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
    }
}

/**
 * Boards 15, 23 and 38, before there are any mixes: the card says Encore is still learning
 * with the plays so far on a progress line and a Play something button (board 15, "Not
 * enough history"); under it the four mix shapes wait as tonal placeholders, two turning
 * slowly and two breathing, so the tab looks like it is working on them. Reduce Motion
 * keeps them still.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun MixesLearning(host: CoreHost, plays: Int, needed: Int, contentPadding: PaddingValues) {
    val type = LocalEncoreType.current
    val actions = LocalAppActions.current
    val scheme = MaterialTheme.colorScheme
    val still = uk.gcartee.encore.app.ui.onboarding.reduceMotion()
    val motion = androidx.compose.animation.core.rememberInfiniteTransition(label = "learning")
    val turn by motion.animateFloat(
        0f, 360f,
        androidx.compose.animation.core.infiniteRepeatable(androidx.compose.animation.core.tween(LEARNING_TURN_MS, easing = androidx.compose.animation.core.LinearEasing)),
        label = "turn",
    )
    val breathe by motion.animateFloat(
        LEARNING_BREATHE_MIN, 1f,
        androidx.compose.animation.core.infiniteRepeatable(
            androidx.compose.animation.core.tween(LEARNING_BREATHE_MS, easing = androidx.compose.animation.core.FastOutSlowInEasing),
            androidx.compose.animation.core.RepeatMode.Reverse,
        ),
        label = "breathe",
    )
    val shapes = remember { listOf(MaterialShapes.Cookie9Sided, MaterialShapes.Square, MaterialShapes.Arch, MaterialShapes.Clover4Leaf).map { PolygonShape(it) } }
    val tones = listOf(scheme.primaryContainer, scheme.tertiaryContainer, scheme.secondaryContainer, scheme.surfaceContainerHighest)
    LazyVerticalGrid(
        columns = GridCells.Adaptive(160.dp),
        contentPadding = PaddingValues(start = (Grid.base * 2).dp, end = (Grid.base * 2).dp, top = Grid.base.dp, bottom = contentPadding.calculateBottomPadding() + (Grid.base * 2).dp),
        horizontalArrangement = Arrangement.spacedBy((Grid.base * 2).dp),
        verticalArrangement = Arrangement.spacedBy((Grid.base * 3).dp),
    ) {
        item(span = { androidx.compose.foundation.lazy.grid.GridItemSpan(maxLineSpan) }) {
            Surface(color = scheme.surfaceContainer, shape = androidx.compose.foundation.shape.RoundedCornerShape(LEARNING_CARD_RADIUS_DP.dp)) {
                Column(Modifier.fillMaxWidth().padding((Grid.base * 2.5f).dp)) {
                    Text("Encore is still learning", style = type.sectionHeader)
                    Spacer(Modifier.height(4.dp))
                    Text("Play a few more songs and your mixes will start. They're made on this phone, from what you play.", style = type.body, color = scheme.onSurfaceVariant)
                    Spacer(Modifier.height((Grid.base * 2).dp))
                    androidx.compose.material3.LinearWavyProgressIndicator(
                        progress = { if (needed > 0) (plays.toFloat() / needed).coerceIn(0f, 1f) else 0f },
                        modifier = Modifier.fillMaxWidth(),
                    )
                    Spacer(Modifier.height((Grid.base * 1.5f).dp))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("$plays of $needed plays", style = type.caption, color = scheme.onSurfaceVariant, modifier = Modifier.weight(1f))
                        androidx.compose.material3.Button(onClick = {
                            val ids = runCatching { host.core.songs(SongSort.RECENTLY_ADDED, 0u, PLAY_SOMETHING_SONGS.toUInt()).map { it.id } }.getOrDefault(emptyList())
                            if (ids.isNotEmpty()) actions.play(ids, ids.indices.random(), StartedFrom.SONGS, null, "Your library", shuffle = true)
                        }) {
                            Icon(Icons.shuffle(), contentDescription = null, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.width(Grid.base.dp))
                            Text("Play something")
                        }
                    }
                }
            }
        }
        items(shapes.size) { i ->
            Column(Modifier.semantics { contentDescription = "A mix being made" }) {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .aspectRatio(1f)
                        .graphicsLayer {
                            if (!still) {
                                // Cookie and clover turn, opposite ways; squircle and arch breathe.
                                if (i % 3 == 0) rotationZ = if (i == 0) turn else -turn
                                else {
                                    scaleX = breathe
                                    scaleY = breathe
                                }
                            }
                        }
                        .clip(shapes[i])
                        .background(tones[i]),
                )
                Spacer(Modifier.height(Grid.base.dp))
                // Where the title and line will go.
                Box(Modifier.fillMaxWidth(0.7f).height(14.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(7.dp)).background(scheme.surfaceContainerHighest))
                Spacer(Modifier.height(6.dp))
                Box(Modifier.fillMaxWidth(0.45f).height(10.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(5.dp)).background(scheme.surfaceContainerHigh))
            }
        }
    }
}

private const val LEARNING_TURN_MS = 24_000
private const val LEARNING_BREATHE_MS = 2_400
private const val LEARNING_BREATHE_MIN = 0.94f
private const val LEARNING_CARD_RADIUS_DP = 24
private const val PLAY_SOMETHING_SONGS = 300

// Board 05's sizes, measured from the Android render.
private const val CAROUSEL_ITEM_DP = 220
// Board 05: the large tile is wider than tall (about 220 by 176 dp), its labels below.
private const val CAROUSEL_HEIGHT_DP = 222
private const val CAROUSEL_LABEL_INSET_DP = 4
private const val SHELF_TILE_DP = 120
private const val SMALL_TILE_DP = 96
private const val RING_DP = 6
