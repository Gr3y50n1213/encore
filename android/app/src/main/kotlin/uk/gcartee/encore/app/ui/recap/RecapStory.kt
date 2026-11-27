package uk.gcartee.encore.app.ui.recap

import android.content.Context
import android.content.ContextWrapper
import android.graphics.Bitmap
import android.os.SystemClock
import android.view.accessibility.AccessibilityManager
import androidx.activity.compose.BackHandler
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.withFrameMillis
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.layer.drawLayer
import androidx.compose.ui.graphics.rememberGraphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.PointerInputChange
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import java.time.ZoneId
import kotlin.math.abs
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.recap.BeatData
import uk.gcartee.encore.app.recap.PackId
import uk.gcartee.encore.app.recap.Recap
import uk.gcartee.encore.app.recap.RecapColour
import uk.gcartee.encore.app.recap.RecapFormat
import uk.gcartee.encore.app.recap.RecapKind
import uk.gcartee.encore.app.recap.RecapProgress
import uk.gcartee.encore.app.recap.RecapRules
import uk.gcartee.encore.app.recap.RecapStore
import uk.gcartee.encore.app.recap.RecapText
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LocalToaster
import uk.gcartee.encore.app.ui.components.SystemBarIconsFor
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.onboarding.reduceMotion
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.tokens.EncoreTokens.Breakpoints
import uk.gcartee.encore.tokens.EncoreTokens.Motion.Timing
import uk.gcartee.encore.tokens.EncoreTokens.Recap.Motion as RecapMotion

/** Opens the year recap's story for a year (EncoreRoot provides it). */
val LocalOpenRecap = staticCompositionLocalOf<(Int) -> Unit> { {} }

/**
 * Board 39: the year's story, full screen. Loads the recap off the main thread, then plays
 * it card by card; a recap with nothing to show says so plainly.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun RecapStory(host: CoreHost, year: Int, onClose: () -> Unit) {
    var result by remember(year) { mutableStateOf<Result<Recap>?>(null) }
    LaunchedEffect(year) { result = RecapStore.load(host, year) }
    BackHandler(onBack = onClose)
    val r = result
    val recap = r?.getOrNull()
    when {
        r == null -> Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface).blockTouches(), contentAlignment = Alignment.Center) {
            LoadingIndicator(Modifier.semantics { contentDescription = "Making your $year" })
        }
        recap == null -> Plain("Your $year couldn't be made", r.exceptionOrNull()?.userMessage() ?: "Try again later.", onClose)
        recap.kind == RecapKind.NONE || recap.story.isEmpty() -> Plain(
            "Your $year builds as you listen",
            if (recap.noneReason == "off") "Recap is off in Settings, so nothing is counted." else "Play a few more songs and your recap appears here.",
            onClose,
        )
        else -> Story(host, recap, onClose)
    }
}

/** No story to show: a plain page with the reason and Close. */
@Composable
private fun Plain(title: String, line: String, onClose: () -> Unit) {
    val type = LocalEncoreType.current
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surface).blockTouches(), contentAlignment = Alignment.Center) {
        Column(Modifier.padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(title, style = type.sheetTitle, color = MaterialTheme.colorScheme.onSurface, textAlign = TextAlign.Center, modifier = Modifier.semantics { heading() })
            Spacer(Modifier.height(8.dp))
            Text(line, style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
            Spacer(Modifier.height(24.dp))
            Button(onClick = onClose, modifier = Modifier.heightIn(min = 48.dp)) { Text("Close") }
        }
    }
}

/** The story covers the app: touches never reach the page underneath. */
internal fun Modifier.blockTouches(): Modifier = pointerInput(Unit) {
    awaitEachGesture { awaitFirstDown(requireUnconsumed = false) }
}

/** Where the story is: the card, its progress, and what is pausing it. */
@Stable
private class StoryController(val count: Int, start: Int) {
    var index by mutableIntStateOf(start.coerceIn(0, (count - 1).coerceAtLeast(0)))
        private set
    var shownAt by mutableLongStateOf(SystemClock.uptimeMillis())
        private set
    /** The current card's time on screen, 0 to 1 (drawn by the segments, read in draw only). */
    val progress = mutableFloatStateOf(0f)
    /** A finger is down: paused (board 39, item 6, HOLD). */
    var held by mutableStateOf(false)
    /** Held a while: the controls are hidden too. */
    var hidden by mutableStateOf(false)
    /** Space on a keyboard, or the pause button. */
    var userPaused by mutableStateOf(false)
    /** How far a swipe down has pulled the card. */
    var dragY by mutableFloatStateOf(0f)

    fun go(to: Int) {
        progress.floatValue = 0f
        shownAt = SystemClock.uptimeMillis()
        index = to.coerceIn(0, (count - 1).coerceAtLeast(0))
    }

    fun next(): Boolean {
        if (index >= count - 1) return false
        go(index + 1)
        return true
    }

    /** Back a card; on the first card, it starts again. */
    fun prev() = go(index - 1)
}

/** A card being made into a picture for sharing. */
private data class Capture(val index: Int, val hideNames: Boolean)

/** The export's size: a 9:16 still (board 39, item 8). */
private const val EXPORT_W_PX = 1080
private const val EXPORT_H_PX = 1920

/** Held this long, the controls hide (board 39, item 6: hold pauses and hides the controls). */
private const val HOLD_HIDES_MS = 250L

/** A pull this far down closes the story. */
private const val CLOSE_PULL_DP = 96

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Story(host: CoreHost, recap: Recap, onClose: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val toaster = LocalToaster.current
    val haptics = rememberHaptics()
    val theme = remember(recap) { RecapTheme.of(recap) }
    val beats = remember(recap) { recap.story }
    val grounds = remember(recap) { beats.map(theme::ground) }
    val profile = remember { runCatching { host.core.currentProfile() }.getOrNull().orEmpty() }
    val profileName = remember { runCatching { host.core.profiles().firstOrNull { it.active }?.displayName }.getOrNull().orEmpty() }
    val progressStore = remember(profile) { RecapProgress(context, profile) }
    val controller = remember(recap) { StoryController(beats.size, progressStore.resumeIndex(recap.year, beats.size)) }
    val resumedAt = remember(recap) { controller.index }
    val talkBack = rememberTalkBack()
    val still = reduceMotion() || talkBack
    val resumed = rememberResumed()
    val saver = remember { powerSaving(context) }
    val ambient = rememberAmbientClock(enabled = !still && resumed && !saver && theme.pack != PackId.EDITORIAL)
    var detail by rememberSaveable { mutableStateOf(false) }
    var shareSheet by remember { mutableStateOf(false) }
    var hideNames by rememberSaveable { mutableStateOf(false) }
    var capture by remember { mutableStateOf<Capture?>(null) }
    var saving by remember { mutableStateOf(false) }
    var saved by remember { mutableStateOf(false) }
    val names = remember(recap) { RecapText.names(recap) }
    val counted = remember(recap) { RecapFormat.countedThrough(recap, ZoneId.systemDefault()) }
    val storySeconds = remember(beats) { beats.sumOf { RecapRules.cardMillis(it.seconds) } / 1000L }
    val introFirst = beats.firstOrNull()?.data is BeatData.Intro

    // Board 41: the plays seen, for the final run's "more plays since you last looked".
    LaunchedEffect(recap) { progressStore.setSeenPlays(recap.year, recap.totals.plays) }
    // Resume where it was left (30 days); the last card marks the story finished.
    LaunchedEffect(controller.index) {
        progressStore.save(recap.year, controller.index)
        if (controller.index == beats.lastIndex) progressStore.markFinished(recap.year)
    }

    // Auto-advance by each beat's seconds; off with TalkBack (board 39, item 11), while held,
    // paused, in the background, or with a sheet or the detail page over the story.
    val auto = !talkBack && !detail && !shareSheet && capture == null && resumed && !controller.held && !controller.userPaused
    LaunchedEffect(controller.index, auto) {
        if (!auto) return@LaunchedEffect
        val total = RecapRules.cardMillis(beats[controller.index].seconds).toFloat()
        var last = withFrameMillis { it }
        while (controller.progress.floatValue < 1f) {
            val now = withFrameMillis { it }
            controller.progress.floatValue = (controller.progress.floatValue + (now - last) / total).coerceAtMost(1f)
            last = now
        }
        controller.next()
    }
    // Board 39, item 4: the rest of a top 5 shows after 2 s; still frames show it at once.
    var reveal by remember(controller.index) { mutableStateOf(still) }
    LaunchedEffect(controller.index, still) {
        if (!still) delay(RecapRules.REVEAL_REST_MS)
        reveal = true
    }

    fun close() {
        progressStore.save(recap.year, controller.index)
        onClose()
    }
    val closeNow by rememberUpdatedState(::close)

    fun env(i: Int, export: Boolean = false, hide: Boolean = false) = CardEnv(
        recap = recap,
        theme = theme,
        ground = grounds[i],
        beat = beats[i],
        index = i,
        count = beats.size,
        number = if (introFirst) i else i + 1,
        still = still || export,
        reveal = export || i != controller.index || reveal,
        hideNames = hide,
        names = names,
        forExport = export,
        countedThrough = counted,
        storySeconds = storySeconds,
        profileName = profileName,
    )

    val closer = CloserActions(
        saving = saving,
        saved = saved,
        onSave = {
            val c = beats.firstNotNullOfOrNull { it.data as? BeatData.Closer }
            if (c != null && !saving && !saved) {
                saving = true
                scope.launch {
                    val res = RecapStore.saveTopSongs(host, recap, c)
                    saving = false
                    res.onSuccess { s ->
                        if (s.saved > 0) {
                            saved = true
                            haptics.perform(Haptic.Confirm)
                            toaster.show(
                                if (s.saved >= s.wanted) "Saved ${c.title} to your library"
                                else "Saved ${s.saved} of ${s.wanted} songs. The rest aren't in your library now.",
                            )
                        } else {
                            toaster.show("None of these songs are in your library now")
                        }
                    }.onFailure { toaster.show(it.userMessage()) }
                }
            }
        },
        onShare = { shareSheet = true },
        onDetail = { detail = true },
    )

    val focus = remember { FocusRequester() }
    LaunchedEffect(talkBack) { if (!talkBack) runCatching { focus.requestFocus() } }

    BoxWithConstraints(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .blockTouches()
            // Board 41: arrow keys step, Space pauses, Escape closes.
            .onPreviewKeyEvent { e ->
                if (detail || e.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                when (e.key) {
                    Key.DirectionRight -> { controller.next(); true }
                    Key.DirectionLeft -> { controller.prev(); true }
                    Key.Spacebar -> { controller.userPaused = !controller.userPaused; true }
                    Key.Escape -> { close(); true }
                    else -> false
                }
            }
            .let { if (talkBack) it else it.focusRequester(focus).focusable() },
    ) {
        val widthDp = maxWidth.value
        val heightDp = maxHeight.value
        val wide = widthDp >= Breakpoints.Medium.min
        val expanded = widthDp >= Breakpoints.Expanded.min
        val index = controller.index
        val ground = grounds[index]
        SystemBarIconsFor(if (wide) Color.Black else ground.top)

        // The share picture is drawn first, under everything, and recorded.
        capture?.let { cap ->
            CaptureCard(env(cap.index, export = true, hide = cap.hideNames), recap) { bitmap ->
                capture = null
                if (bitmap == null) {
                    toaster.show("Couldn't make the picture")
                } else {
                    scope.launch { RecapStore.share(context, bitmap, recap.year, cap.index).onFailure { toaster.show("Couldn't share the picture") } }
                }
            }
        }

        val bars = WindowInsets.systemBars.asPaddingValues()
        if (!wide) {
            // Compact: full screen, the background under the system bars (board 41).
            CardFrame(
                envFor = { i -> env(i) },
                controller = controller,
                theme = theme,
                grounds = grounds,
                beats = beats,
                recap = recap,
                widthDp = widthDp,
                insets = bars,
                still = still,
                talkBack = talkBack,
                ambient = { ambient.value },
                closer = closer,
                resumedAt = resumedAt,
                onClose = { closeNow() },
                modifier = Modifier.fillMaxSize(),
            )
        } else {
            // Medium: the 9:16 card in a soft glow; Expanded: a Details panel beside it.
            GlowBackdrop(theme)
            val top = bars.calculateTopPadding().value
            val bottom = bars.calculateBottomPadding().value
            val panel = if (expanded) DETAILS_PANEL_DP + DETAILS_GAP_DP else 0f
            var cardH = heightDp - top - bottom - 2 * FRAME_MARGIN_DP
            var cardW = cardH * 9f / 16f
            if (cardW + panel + 2 * FRAME_MARGIN_DP > widthDp) {
                cardW = widthDp - panel - 2 * FRAME_MARGIN_DP
                cardH = cardW * 16f / 9f
            }
            Row(Modifier.fillMaxSize().padding(bars), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
                CardFrame(
                    envFor = { i -> env(i) },
                    controller = controller,
                    theme = theme,
                    grounds = grounds,
                    beats = beats,
                    recap = recap,
                    widthDp = cardW,
                    insets = PaddingValues(0.dp),
                    still = still,
                    talkBack = talkBack,
                    ambient = { ambient.value },
                    closer = closer,
                    resumedAt = resumedAt,
                    onClose = { closeNow() },
                    modifier = Modifier.size(cardW.dp, cardH.dp).clip(RoundedCornerShape((CARD_RADIUS_DP * cardW / DESIGN_WIDTH).dp)),
                )
                if (expanded) {
                    Spacer(Modifier.width(DETAILS_GAP_DP.dp))
                    DetailsPanel(recap, beats[index], onShare = { shareSheet = true }, modifier = Modifier.width(DETAILS_PANEL_DP.dp).heightIn(max = cardH.dp))
                }
            }
        }
    }

    if (shareSheet) {
        ShareSheet(
            hide = hideNames,
            onHide = { hideNames = it },
            onShare = {
                shareSheet = false
                capture = Capture(controller.index, hideNames)
            },
            onDismiss = { shareSheet = false },
        )
    }
    if (detail) RecapDetailPage(recap, onClose = { detail = false })
}

private const val DETAILS_PANEL_DP = 320f
private const val DETAILS_GAP_DP = 24f
private const val FRAME_MARGIN_DP = 16f
private const val CARD_RADIUS_DP = 26f

/**
 * One card on screen: its background (with Editorial's wipe or the others' fade between
 * cards), the card, and the progress and close row on top. Taps, holds and the swipe down
 * are read here (board 39, item 6).
 */
@Composable
private fun CardFrame(
    envFor: (Int) -> CardEnv,
    controller: StoryController,
    theme: RecapTheme,
    grounds: List<Ground>,
    beats: List<uk.gcartee.encore.app.recap.Beat>,
    recap: Recap,
    widthDp: Float,
    insets: PaddingValues,
    still: Boolean,
    talkBack: Boolean,
    ambient: () -> Float,
    closer: CloserActions,
    resumedAt: Int,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val scope = rememberCoroutineScope()
    val scale = (widthDp / DESIGN_WIDTH).coerceAtLeast(0.1f)
    val onCloseNow by rememberUpdatedState(onClose)
    val lastIsCloser = beats.lastOrNull()?.data is BeatData.Closer

    // The background follows the card: wiped (Editorial) or faded in over 500 ms.
    val change = remember { Animatable(1f) }
    var from by remember { mutableStateOf<Int?>(null) }
    var shown by remember { mutableIntStateOf(controller.index) }
    LaunchedEffect(controller.index) {
        if (controller.index == shown) return@LaunchedEffect
        from = shown
        shown = controller.index
        if (still) {
            change.snapTo(1f)
        } else {
            change.snapTo(0f)
            change.animateTo(1f, tween(RecapMotion.levelEaseMs))
        }
    }

    Box(
        modifier
            .graphicsLayer {
                if (!still) {
                    val pull = controller.dragY
                    translationY = pull
                    val s = 1f - (pull / size.height.coerceAtLeast(1f)).coerceIn(0f, 0.4f) * 0.25f
                    scaleX = s
                    scaleY = s
                }
            }
            .semantics {
                customActions = listOf(
                    CustomAccessibilityAction("Next card") { controller.next() },
                    CustomAccessibilityAction("Previous card") { controller.prev(); true },
                    CustomAccessibilityAction("Close") { onCloseNow(); true },
                )
            }
            .pointerInput(controller) {
                val slop = viewConfiguration.touchSlop
                val closeAt = CLOSE_PULL_DP.dp.toPx()
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    val id = down.id
                    controller.held = true
                    var state = 0
                    var up: PointerInputChange? = null
                    var moved = Offset.Zero
                    var dragging = false
                    var lastTime = down.uptimeMillis
                    var hideArmed = true
                    while (state == 0) {
                        val e = if (hideArmed) {
                            withTimeoutOrNull((down.uptimeMillis + HOLD_HIDES_MS - lastTime).coerceAtLeast(1L)) { awaitPointerEvent() }
                        } else {
                            awaitPointerEvent()
                        }
                        if (e == null) {
                            hideArmed = false
                            if (!dragging) controller.hidden = true
                            continue
                        }
                        val c = e.changes.firstOrNull { it.id == id }
                        if (c == null) {
                            state = 2
                        } else if (!c.pressed) {
                            up = c
                            state = 1
                        } else {
                            lastTime = c.uptimeMillis
                            moved = c.position - down.position
                            if (!dragging && moved.y > slop && moved.y > abs(moved.x)) dragging = true
                            if (dragging) {
                                controller.dragY = moved.y.coerceAtLeast(0f)
                                c.consume()
                            }
                        }
                    }
                    controller.held = false
                    controller.hidden = false
                    if (dragging) {
                        if (controller.dragY > closeAt) {
                            onCloseNow()
                        } else {
                            val startY = controller.dragY
                            scope.launch { animate(startY, 0f) { v, _ -> controller.dragY = v } }
                        }
                        return@awaitEachGesture
                    }
                    val u = up ?: return@awaitEachGesture
                    val quick = u.uptimeMillis - down.uptimeMillis <= Timing.tapMaxMs
                    if (!quick || u.isConsumed || moved.getDistance() > slop) return@awaitEachGesture
                    if (!RecapRules.tapCounts(SystemClock.uptimeMillis() - controller.shownAt)) return@awaitEachGesture
                    when (RecapRules.tapZone(down.position.x, size.width.toFloat())) {
                        RecapRules.Tap.BACK -> controller.prev()
                        RecapRules.Tap.FORWARD -> if (!controller.next() && !lastIsCloser) onCloseNow()
                    }
                }
            },
    ) {
        Canvas(Modifier.fillMaxSize()) {
            val toBeat = beats[shown]
            val fromIndex = from
            drawGroundChange(
                theme,
                fromIndex?.let { grounds[it] },
                fromIndex?.let { beats[it] },
                grounds[shown],
                toBeat,
                change.value,
                ambient(),
            )
        }
        val designInsets = PaddingValues(
            top = (insets.calculateTopPadding().value / scale).dp,
            bottom = (insets.calculateBottomPadding().value / scale).dp,
        )
        DesignScale(widthDp) {
            Crossfade(targetState = controller.index, animationSpec = tween(if (still) 0 else CARD_FADE_MS), label = "card") { i ->
                val env = envFor(i)
                BeatCard(env, designInsets, if (env.beat.data is BeatData.Closer) closer else null)
            }
            Chrome(
                theme = theme,
                ground = grounds[controller.index],
                recap = recap,
                controller = controller,
                insets = designInsets,
                talkBack = talkBack,
                resumedAt = resumedAt,
                onClose = onClose,
            )
        }
    }
}

private const val CARD_FADE_MS = 300

/** The progress segments, "Your 2026", and Close (board 39). Hidden while held. */
@Composable
private fun Chrome(
    theme: RecapTheme,
    ground: Ground,
    recap: Recap,
    controller: StoryController,
    insets: PaddingValues,
    talkBack: Boolean,
    resumedAt: Int,
    onClose: () -> Unit,
    export: Boolean = false,
) {
    val alpha by animateFloatAsState(if (controller.hidden) 0f else 1f, tween(200), label = "chrome")
    val fg = ground.fg
    val count = controller.count
    val type = theme.type
    Column(Modifier.fillMaxWidth().padding(insets).padding(start = 14.dp, end = 8.dp, top = 12.dp).graphicsLayer { this.alpha = alpha }) {
        if (!export) {
            Canvas(Modifier.fillMaxWidth().padding(end = 6.dp).height(3.dp).clearAndSetSemantics { contentDescription = "Card ${controller.index + 1} of $count" }) {
                val gap = 3.dp.toPx()
                val w = (size.width - gap * (count - 1)) / count.coerceAtLeast(1)
                val r = CornerRadius(2.dp.toPx(), 2.dp.toPx())
                for (i in 0 until count) {
                    val x = i * (w + gap)
                    drawRoundRect(fg.copy(alpha = 0.33f), Offset(x, 0f), Size(w, size.height), r)
                    val f = when {
                        i < controller.index -> 1f
                        i == controller.index -> controller.progress.floatValue
                        else -> 0f
                    }
                    if (f > 0f) drawRoundRect(fg, Offset(x, 0f), Size(w * f, size.height), r)
                }
            }
        }
        Row(Modifier.fillMaxWidth().heightIn(min = 32.dp), verticalAlignment = Alignment.CenterVertically) {
            val title = "Your ${recap.year}" + if (recap.soFar) " · So far" else ""
            Text(if (type.labelUpper) title.uppercase() else title, style = type.label, color = fg, modifier = Modifier.weight(1f))
            if (!export) {
                if (resumedAt > 0 && controller.index == resumedAt) {
                    ChromeButton(fg, "Start over", onClick = { controller.go(0) }) { Text("Start over", style = type.label, color = fg) }
                }
                if (talkBack) {
                    // With TalkBack nothing advances by itself: these step through the cards.
                    ChromeButton(fg, "Previous card", onClick = { controller.prev() }) { Icon(Icons.previous(), null, tint = fg, modifier = Modifier.size(14.dp)) }
                    ChromeButton(fg, "Next card", onClick = { controller.next() }) { Icon(Icons.next(), null, tint = fg, modifier = Modifier.size(14.dp)) }
                }
                // Song snippets (board 39, item 5) come later, so there is no Sound button yet.
                ChromeButton(fg, "Close your ${recap.year}", onClick = onClose) { Icon(Icons.close(), null, tint = fg, modifier = Modifier.size(14.dp)) }
            }
        }
    }
}

@Composable
private fun ChromeButton(fg: Color, label: String, onClick: () -> Unit, content: @Composable () -> Unit) {
    Box(
        Modifier
            .heightIn(min = 34.dp)
            .padding(horizontal = 2.dp)
            .clip(RoundedCornerShape(17.dp))
            .clickable(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = label }
            .padding(horizontal = 10.dp),
        contentAlignment = Alignment.Center,
    ) { content() }
}

/** Medium and Expanded windows: a soft glow made from the palette behind the card. */
@Composable
private fun GlowBackdrop(theme: RecapTheme) {
    val lead = Color(theme.colours.year.firstOrNull()?.let { RecapColour.darkForWhite(it) } ?: theme.colours.spot)
    Canvas(Modifier.fillMaxSize()) {
        drawRect(Color(0xFF0D0B10))
        drawCircle(
            Brush.radialGradient(listOf(lead.copy(alpha = 0.6f), lead.copy(alpha = 0f)), center = center, radius = size.maxDimension * 0.6f),
            radius = size.maxDimension * 0.6f,
            center = center,
        )
    }
}

/** Expanded: the full top 5, Show the math and Share beside the card (board 41). */
@Composable
private fun DetailsPanel(recap: Recap, beat: uk.gcartee.encore.app.recap.Beat, onShare: () -> Unit, modifier: Modifier = Modifier) {
    val type = LocalEncoreType.current
    val white = Color.White
    val lines: List<String> = when (val d = beat.data) {
        is BeatData.TopSong -> d.top.map { "${it.rank}  ${it.title}  ${RecapFormat.count(it.plays)}" }
        is BeatData.TopArtist -> d.top.map { "${it.rank}  ${it.name}  ${RecapFormat.count(it.plays)}" }
        is BeatData.TopAlbum -> d.top.map { "${it.rank}  ${it.name}  ${RecapFormat.count(it.plays)}" }
        is BeatData.YourSound -> d.top.map { "${it.rank}  ${it.name}  ${RecapFormat.percent(it.permille)}%" }
        is BeatData.Personality -> d.metrics.map { "${it.label}  ${RecapFormat.metricValue(it)}" }
        is BeatData.SongOfEachSeason -> d.seasons.map { "${RecapFormat.seasonName(it.id)}  ${it.song.title}" }
        else -> emptyList()
    }
    Column(
        modifier
            .clip(RoundedCornerShape(24.dp))
            .background(white.copy(alpha = 0.1f))
            .border(1.dp, white.copy(alpha = 0.2f), RoundedCornerShape(24.dp))
            .verticalScroll(rememberScrollState())
            .padding(20.dp),
    ) {
        Text("Details", style = type.sectionHeader, color = white, modifier = Modifier.semantics { heading() })
        Spacer(Modifier.height(8.dp))
        if (lines.isEmpty()) Text(beat.text, style = type.body.copy(fontSize = 15.sp), color = white)
        lines.forEach { Text(it, style = type.body.copy(fontSize = 15.sp), color = white, modifier = Modifier.padding(vertical = 2.dp)) }
        Spacer(Modifier.height(20.dp))
        Text("Show the math", style = type.sectionHeader, color = white, modifier = Modifier.semantics { heading() })
        Spacer(Modifier.height(8.dp))
        beat.math.counts.forEach { c -> Text("${c.label}: ${RecapFormat.mathValue(c)}", style = type.body.copy(fontSize = 15.sp), color = white) }
        if (beat.math.rule.isNotBlank()) Text(beat.math.rule, style = type.body.copy(fontSize = 14.sp), color = white.copy(alpha = 0.85f), modifier = Modifier.padding(top = 6.dp))
        Spacer(Modifier.height(16.dp))
        TextButton(onClick = onShare, modifier = Modifier.heightIn(min = 48.dp)) { Text("Share this card", color = white, style = type.body.copy(fontWeight = FontWeight.SemiBold)) }
    }
}

/**
 * Draws [env]'s card at 1080 by 1920 pixels under the story, records it into a layer and
 * hands the picture over once it has been drawn (board 39, item 8: made on the device).
 */
@Composable
private fun CaptureCard(env: CardEnv, recap: Recap, onDone: (Bitmap?) -> Unit) {
    val layer = rememberGraphicsLayer()
    val density = LocalDensity.current
    val w: Dp = with(density) { EXPORT_W_PX.toDp() }
    val h: Dp = with(density) { EXPORT_H_PX.toDp() }
    val done by rememberUpdatedState(onDone)
    Box(
        Modifier
            .requiredSize(w, h)
            .clearAndSetSemantics { }
            .drawWithContent {
                val card = this
                layer.record { card.drawContent() }
                drawLayer(layer)
            },
    ) {
        Canvas(Modifier.fillMaxSize()) { drawGround(env.theme, env.ground, env.beat.shapeSeed, tideLevel(env.beat), 0f) }
        DesignScale(w.value) {
            BeatCard(env, PaddingValues(0.dp), closer = null)
            val still = remember(env.index) { StoryController(env.count, env.index) }
            Chrome(env.theme, env.ground, recap, still, PaddingValues(0.dp), talkBack = false, resumedAt = 0, onClose = {}, export = true)
        }
    }
    LaunchedEffect(env.index, env.hideNames) {
        // Two frames: one to lay the card out and draw it, one to be sure it was recorded.
        withFrameNanos { }
        withFrameNanos { }
        delay(RecapMotion.countUpMs.toLong() / 10)
        val bitmap = runCatching { layer.toImageBitmap().asAndroidBitmap() }.getOrNull()
        done(bitmap)
    }
}

/** Share: hide names first if wanted, then the system share sheet (board 39, item 8). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ShareSheet(hide: Boolean, onHide: (Boolean) -> Unit, onShare: () -> Unit, onDismiss: () -> Unit) {
    val type = LocalEncoreType.current
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding().padding(horizontal = 24.dp, vertical = 8.dp)) {
            Text("Share this card", style = type.sheetTitle, modifier = Modifier.semantics { heading() })
            Spacer(Modifier.height(8.dp))
            Text("A picture of the card, made on this phone. Nothing is uploaded and there's no watermark.", style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(16.dp))
            Row(
                Modifier.fillMaxWidth().heightIn(min = 56.dp).toggleable(value = hide, role = Role.Switch, onValueChange = onHide),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text("Hide song and artist names", style = type.listRowTitle)
                    Text("They show as dots on the picture", style = type.caption, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                Switch(checked = hide, onCheckedChange = null)
            }
            Spacer(Modifier.height(16.dp))
            Button(onClick = onShare, modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp)) { Text("Share") }
            Spacer(Modifier.height(16.dp))
        }
    }
}

/** TalkBack (or any touch exploration) is on: no auto-advance, the cards' text versions. */
@Composable
private fun rememberTalkBack(): Boolean {
    val context = LocalContext.current
    val am = remember(context) { context.getSystemService(AccessibilityManager::class.java) }
    var on by remember { mutableStateOf(am?.isTouchExplorationEnabled == true) }
    DisposableEffect(am) {
        val listener = AccessibilityManager.TouchExplorationStateChangeListener { on = it }
        am?.addTouchExplorationStateChangeListener(listener)
        onDispose { am?.removeTouchExplorationStateChangeListener(listener) }
    }
    return on
}

private fun Context.lifecycleOwner(): LifecycleOwner? = when (this) {
    is LifecycleOwner -> this
    is ContextWrapper -> baseContext.lifecycleOwner()
    else -> null
}

/** The app is in front: the story and its motion stop in the background. */
@Composable
private fun rememberResumed(): Boolean {
    val context = LocalContext.current
    var resumed by remember { mutableStateOf(true) }
    DisposableEffect(context) {
        val owner = context.lifecycleOwner()
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) resumed = true
            if (event == Lifecycle.Event.ON_PAUSE) resumed = false
        }
        owner?.lifecycle?.addObserver(observer)
        onDispose { owner?.lifecycle?.removeObserver(observer) }
    }
    return resumed
}
