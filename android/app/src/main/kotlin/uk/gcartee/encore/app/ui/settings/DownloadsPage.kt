package uk.gcartee.encore.app.ui.settings

import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.StatFs
import android.provider.Settings
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.downloads.DownloadScheduler
import uk.gcartee.encore.app.playback.RollingCache
import uk.gcartee.encore.app.ui.components.ActionSheet
import uk.gcartee.encore.app.ui.components.Artwork
import uk.gcartee.encore.app.ui.components.ConfirmRemoveDownload
import uk.gcartee.encore.app.ui.components.DownloadBadge
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.LocalDownloads
import uk.gcartee.encore.app.ui.components.SheetAction
import uk.gcartee.encore.app.ui.components.SwipeAction
import uk.gcartee.encore.app.ui.components.SwipeRow
import uk.gcartee.encore.app.ui.components.downloadLine
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.ArtworkOf
import uk.gcartee.encore.core.DownloadKind
import uk.gcartee.encore.core.DownloadSettings
import uk.gcartee.encore.core.DownloadStop
import uk.gcartee.encore.core.DownloadedItem
import uk.gcartee.encore.core.StreamQuality
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/** 12.4 GB, 820 MB, 4 KB (decimal, as the system's storage page counts). */
fun formatBytes(bytes: Long): String = when {
    bytes >= 1_000_000_000 -> "%.1f GB".format(bytes / 1e9)
    bytes >= 1_000_000 -> "${bytes / 1_000_000} MB"
    bytes >= 1_000 -> "${bytes / 1_000} KB"
    else -> "$bytes B"
}

private const val GB = 1_000_000_000uL
private const val MB = 1_000_000uL

/**
 * Board 17, item 3: Downloads. What they take next to the cache and the free space,
 * the transfer under way or why it waits, the quality, Wi-Fi only, the limit,
 * auto-remove, Liked songs and Smart downloads, the rolling cache, then every
 * downloaded item (swipe to remove) and Remove all, apart and in red.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun DownloadsPage(host: CoreHost, onBack: () -> Unit, contentPadding: PaddingValues) {
    val context = LocalContext.current
    val actions = LocalAppActions.current
    val haptics = rememberHaptics()
    val type = LocalEncoreType.current
    val view = LocalDownloads.current
    val progress by host.downloads.progress.collectAsState()
    val overview = remember(view, progress?.songsDone) { runCatching { host.core.downloadsOverview() }.getOrNull() }
    var settings by remember { mutableStateOf(runCatching { host.core.downloadSettings() }.getOrNull()) }
    var limitSheet by remember { mutableStateOf(false) }
    var cacheSheet by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf<DownloadedItem?>(null) }
    var removeAll by remember { mutableStateOf(false) }
    val s = settings
    fun change(next: DownloadSettings) {
        val before = settings
        runCatching { host.core.setDownloadSettings(next) }
            .onSuccess {
                settings = next
                if (before?.wifiOnly != next.wifiOnly) DownloadScheduler.start(context, byUser = true)
                if (before?.smartDownloads != next.smartDownloads) DownloadScheduler.smart(context, next.smartDownloads)
                if (before?.likedSongs != next.likedSongs && next.likedSongs) DownloadScheduler.start(context, byUser = true)
                if (before?.cacheLimitBytes != next.cacheLimitBytes) {
                    RollingCache.setLimit(context, next.cacheLimitBytes.toLong())
                    if (next.cacheLimitBytes == 0uL) RollingCache.clear(context)
                }
                host.downloads.refreshNow()
            }
            .onFailure { actions.toaster.show(it.userMessage()) }
    }
    Page("Downloads", onBack, contentPadding, subtitle = "On this device") {
        item { StorageCard(context, overview?.usedBytes?.toLong() ?: 0L) }
        // The transfer, or why it waits.
        item {
            val p = progress
            val stop = overview?.stop
            val queued = overview?.queued ?: 0u
            when {
                p != null -> StatusCard(
                    title = "Downloading ${p.itemTitle}",
                    line = "${p.itemSaved} of ${p.itemTotal} songs · ${p.songTitle}",
                    progress = (p.itemSaved.toFloat() + (p.bytesTotal?.takeIf { it > 0uL }?.let { p.bytesDone.toFloat() / it.toFloat() } ?: 0f)) / p.itemTotal.coerceAtLeast(1u).toFloat(),
                )
                queued > 0u && stop != null -> StatusCard(title = waitTitle(stop), line = waitLine(stop, queued), progress = null)
                queued > 0u -> StatusCard(title = "Waiting to download", line = if (queued == 1u) "1 song" else "$queued songs", progress = null)
                else -> Unit
            }
        }
        if ((overview?.failed ?: 0u) > 0u) {
            item {
                val n = overview?.failed ?: 0u
                Surface(
                    color = MaterialTheme.colorScheme.errorContainer,
                    contentColor = MaterialTheme.colorScheme.onErrorContainer,
                    shape = RoundedCornerShape(OUTER.dp),
                    modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
                ) {
                    Row(Modifier.padding((Grid.base * 2).dp), verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.error(), contentDescription = null)
                        Spacer(Modifier.width((Grid.base * 1.5f).dp))
                        Text(if (n == 1u) "1 song couldn't download" else "$n songs couldn't download", style = rowTitle(), modifier = Modifier.weight(1f))
                        FilledTonalButton(onClick = { actions.retryDownloads() }) { Text("Try again") }
                    }
                }
            }
        }
        if (s != null) {
            item { SectionLabel("Quality") }
            item {
                RadioRows(
                    listOf(
                        Triple(StreamQuality.ORIGINAL, "Original", "The file as it is: lossless, gapless albums seamless"),
                        Triple(StreamQuality.HIGH, "High", "320 kbps, about 2.4 MB a minute"),
                        Triple(StreamQuality.NORMAL, "Normal", "160 kbps, about 1.2 MB a minute"),
                    ),
                    s.quality,
                    { q -> change(s.copy(quality = q)) },
                )
            }
            item { Body("A new quality applies to songs downloaded from now on.") }
            group("When and how much", listOf(
                Row0("Download only on Wi-Fi", "Saves your mobile data", { Icons.wifi() }, trailing = {
                    CheckSwitch(s.wifiOnly, onCheckedChange = { on ->
                        haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
                        change(s.copy(wifiOnly = on))
                    })
                }),
                Row0("Storage limit", "At the limit, new songs wait; nothing is removed", { Icons.storage() }, right = s.limitBytes?.let { formatBytes(it.toLong()) } ?: "No limit") { limitSheet = true },
                Row0("Remove unplayed downloads", "Albums and playlists not played for 30 days", { Icons.history() }, trailing = {
                    CheckSwitch(s.removeUnplayed, onCheckedChange = { on ->
                        haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
                        change(s.copy(removeUnplayed = on))
                    })
                }),
            ))
            group("Automatic", listOf(
                Row0("Download liked songs", "Songs you like download by themselves", { Icons.favorite(false) }, trailing = {
                    CheckSwitch(s.likedSongs, onCheckedChange = { on ->
                        haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
                        change(s.copy(likedSongs = on))
                    })
                }),
                Row0("Smart downloads", "Your pinned mixes, up to 2 GB, overnight on Wi-Fi while charging", { Icons.spark() }, trailing = {
                    CheckSwitch(s.smartDownloads, onCheckedChange = { on ->
                        haptics.perform(if (on) Haptic.ToggleOn else Haptic.ToggleOff)
                        change(s.copy(smartDownloads = on))
                    })
                }),
            ))
            group("Streaming cache", listOf(
                Row0(
                    "Keep recently played",
                    "So they start at once and play offline",
                    { Icons.history() },
                    right = if (s.cacheLimitBytes == 0uL) "Off" else formatBytes(s.cacheLimitBytes.toLong()),
                ) { cacheSheet = true },
            ))
            // Board 25: notifications were declined; say what they're for and where to turn them on.
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                ContextCompat.checkSelfPermission(context, android.Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED
            ) {
                group(null, listOf(
                    Row0("Show download progress", "Notifications are off for Encore", { Icons.notifications() }, right = "Turn on") {
                        runCatching {
                            context.startActivity(
                                Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName),
                            )
                        }
                    },
                ))
            }
        }
        val items = overview?.items.orEmpty().filter { !it.auto || it.total > 0u }
        if (items.isNotEmpty()) {
            item { SectionLabel("Downloaded") }
            items(items, key = { "${it.target.kind}-${it.target.id}-${it.target.mixId}" }) { item ->
                val deleteIcon = Icons.delete()
                val remove = remember(item.target) { SwipeAction("Remove", destructive = true, icon = { deleteIcon }) { removing = item } }
                Box(Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = 1.dp).clip(RoundedCornerShape(INNER.dp))) {
                    SwipeRow(toEnd = null, toStart = if (item.auto) null else remove, rowColor = MaterialTheme.colorScheme.surfaceContainer) {
                        ItemRow(item)
                    }
                }
            }
            item { Spacer(Modifier.height((Grid.base * 2).dp)) }
            group(null, listOf(
                Row0("Remove all downloads", "Everything saved on this device for you", { Icons.delete() }, destructive = true) { removeAll = true },
            ))
        }
    }
    if (limitSheet && s != null) {
        val options = listOf<ULong?>(null) + listOf(2uL, 5uL, 10uL, 20uL, 50uL).map { it * GB }
        ChoiceSheet(
            "Storage limit",
            "At the limit, new songs wait. Nothing is removed.",
            options.map { (it?.let { b -> formatBytes(b.toLong()) } ?: "No limit") to (it == s.limitBytes) },
            onDismiss = { limitSheet = false },
        ) { i -> change(s.copy(limitBytes = options[i])) }
    }
    if (cacheSheet && s != null) {
        val options = listOf(0uL, 500uL * MB, 1uL * GB, 2uL * GB, 5uL * GB)
        ChoiceSheet(
            "Keep recently played",
            "Songs you stream are kept, so they start at once and play offline. The oldest go first.",
            options.map { (if (it == 0uL) "Off" else formatBytes(it.toLong())) to (it == s.cacheLimitBytes) },
            onDismiss = { cacheSheet = false },
        ) { i -> change(s.copy(cacheLimitBytes = options[i])) }
    }
    removing?.let { item ->
        ConfirmRemoveDownload(item.title, onConfirm = { actions.removeDownload(item.target, item.title) }, onDismiss = { removing = null })
    }
    if (removeAll) {
        ActionSheet(
            onDismiss = { removeAll = false },
            header = {
                Column {
                    Text("Remove all downloads?", style = type.sectionHeader)
                    Text("Every song saved on this device for you goes. They stay in your library and play when you're online.", style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            },
            groups = listOf(
                listOf(SheetAction("Keep them", Icons.downloadDone()) {}),
                listOf(SheetAction("Remove all downloads", Icons.delete(), destructive = true) {
                    runCatching { host.core.removeAllDownloads() }
                        .onSuccess {
                            DownloadScheduler.cancel(context)
                            host.downloads.refreshNow()
                            actions.toaster.show("Removed all downloads")
                        }
                        .onFailure { actions.toaster.show(it.userMessage()) }
                }),
            ),
        )
    }
}

/**
 * How much the device holds: downloads, the cache, everything else, and free, as one
 * segmented bar with a legend, so the numbers read at a glance (board 17).
 */
@Composable
private fun StorageCard(context: Context, downloads: Long) {
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    val stat = remember { runCatching { StatFs(context.filesDir.absolutePath) }.getOrNull() }
    val total = stat?.totalBytes ?: 0L
    val free = stat?.availableBytes ?: 0L
    val cache = remember(downloads) { RollingCache.usedBytes(context) }
    val other = (total - free - downloads - cache).coerceAtLeast(0L)
    Surface(
        color = scheme.surfaceContainer,
        shape = RoundedCornerShape(OUTER.dp),
        modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
    ) {
        Column(Modifier.padding((Grid.base * 2.5f).dp)) {
            Text("Using ${formatBytes(downloads)}", style = type.pageTitle.copy(fontSize = 28.sp, lineHeight = 34.sp), color = scheme.onSurface)
            Text("for downloads", style = rowValue(), color = scheme.onSurfaceVariant)
            Spacer(Modifier.height((Grid.base * 2).dp))
            if (total > 0) {
                Row(
                    Modifier
                        .fillMaxWidth()
                        .height(BAR_DP.dp)
                        .clip(RoundedCornerShape(50))
                        .semantics { contentDescription = "Downloads ${formatBytes(downloads)}, cache ${formatBytes(cache)}, free ${formatBytes(free)}" },
                    horizontalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    fun w(n: Long) = (n.toFloat() / total).coerceAtLeast(if (n > 0) MIN_SLICE else 0f)
                    if (downloads > 0) Box(Modifier.weight(w(downloads)).height(BAR_DP.dp).background(scheme.primary))
                    if (cache > 0) Box(Modifier.weight(w(cache)).height(BAR_DP.dp).background(scheme.tertiary))
                    if (other > 0) Box(Modifier.weight(w(other)).height(BAR_DP.dp).background(scheme.outlineVariant))
                    if (free > 0) Box(Modifier.weight(w(free)).height(BAR_DP.dp).background(scheme.surfaceContainerHighest))
                }
                Spacer(Modifier.height((Grid.base * 1.5f).dp))
                Row(horizontalArrangement = Arrangement.spacedBy((Grid.base * 2).dp)) {
                    Legend(scheme.primary, "Downloads", formatBytes(downloads))
                    Legend(scheme.tertiary, "Cache", formatBytes(cache))
                    Legend(scheme.surfaceContainerHighest, "Free", formatBytes(free))
                }
            }
        }
    }
}

@Composable
private fun Legend(colour: Color, label: String, value: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(10.dp).background(colour, CircleShape))
        Spacer(Modifier.width(6.dp))
        Column {
            Text(label, style = rowValue().copy(fontSize = 13.sp), color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(value, style = rowTitle().copy(fontSize = 14.sp), color = MaterialTheme.colorScheme.onSurface)
        }
    }
}

/** The transfer under way (a wavy bar), or why songs wait. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StatusCard(title: String, line: String, progress: Float?) {
    val scheme = MaterialTheme.colorScheme
    Surface(
        color = scheme.secondaryContainer,
        contentColor = scheme.onSecondaryContainer,
        shape = RoundedCornerShape(OUTER.dp),
        modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
    ) {
        Column(Modifier.padding((Grid.base * 2).dp)) {
            Text(title, style = rowTitle(), maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(line, style = rowValue(), maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 2.dp))
            if (progress != null) {
                Spacer(Modifier.height((Grid.base * 1.5f).dp))
                LinearWavyProgressIndicator(
                    progress = { progress.coerceIn(0f, 1f) },
                    color = scheme.primary,
                    trackColor = scheme.primary.copy(alpha = 0.2f),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
    }
}

private fun waitTitle(stop: DownloadStop) = when (stop) {
    DownloadStop.WAITING_FOR_WIFI -> "Waiting for Wi-Fi"
    DownloadStop.OFFLINE -> "Waiting for a connection"
    DownloadStop.SERVER_UNREACHABLE -> "Your server didn't answer"
    DownloadStop.SIGN_IN_NEEDED -> "Sign in again to keep downloading"
    DownloadStop.STORAGE_LIMIT -> "Storage limit reached"
    DownloadStop.LOW_SPACE -> "Your device is nearly full"
    else -> "Waiting to download"
}

private fun waitLine(stop: DownloadStop, queued: UInt): String {
    val n = if (queued == 1u) "1 song waits" else "$queued songs wait"
    return when (stop) {
        DownloadStop.WAITING_FOR_WIFI -> "$n. Turn off Download only on Wi-Fi to use mobile data"
        DownloadStop.STORAGE_LIMIT -> "$n. Raise the limit below, or remove some downloads"
        DownloadStop.LOW_SPACE -> "$n. Free some space on your device"
        DownloadStop.SERVER_UNREACHABLE -> "$n. It tries again by itself"
        else -> n
    }
}

/** A downloaded item in the page's list: its picture, title, state, size. */
@Composable
private fun ItemRow(item: DownloadedItem) {
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box {
            when (item.target.kind) {
                DownloadKind.ALBUM -> Artwork(ArtworkOf.ALBUM, item.target.id, ITEM_ART_DP.dp, Modifier.size(ITEM_ART_DP.dp), cornerRadius = 10.dp)
                DownloadKind.PLAYLIST -> Artwork(ArtworkOf.PLAYLIST, item.target.id, ITEM_ART_DP.dp, Modifier.size(ITEM_ART_DP.dp), cornerRadius = 10.dp)
                DownloadKind.SONG -> Artwork(ArtworkOf.SONG, item.target.id, ITEM_ART_DP.dp, Modifier.size(ITEM_ART_DP.dp), cornerRadius = 10.dp)
                else -> Box(Modifier.size(ITEM_ART_DP.dp).background(scheme.secondaryContainer, RoundedCornerShape(10.dp)), contentAlignment = Alignment.Center) {
                    Icon(
                        when (item.target.kind) {
                            DownloadKind.LIKED_SONGS -> Icons.favorite(true)
                            DownloadKind.SMART -> Icons.spark()
                            else -> Icons.mixes()
                        },
                        contentDescription = null,
                        tint = scheme.onSecondaryContainer,
                    )
                }
            }
        }
        Spacer(Modifier.width((Grid.base * 2).dp))
        Column(Modifier.weight(1f)) {
            Text(item.title, style = rowTitle(), color = scheme.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                downloadLine(item) ?: listOfNotNull(
                    item.subtitle.ifBlank { null },
                    if (item.total == 1u) "1 song" else "${item.total} songs",
                    formatBytes(item.bytes.toLong()),
                ).joinToString(" · "),
                style = rowValue(),
                color = scheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Spacer(Modifier.width(Grid.base.dp))
        DownloadBadge(item)
    }
}

/** One choice from a short list, in the app's sheet (the limit, the cache). */
@Composable
private fun ChoiceSheet(title: String, line: String, options: List<Pair<String, Boolean>>, onDismiss: () -> Unit, onPick: (Int) -> Unit) {
    val type = LocalEncoreType.current
    val check = Icons.check()
    ActionSheet(
        onDismiss = onDismiss,
        header = {
            Column {
                Text(title, style = type.sectionHeader)
                Text(line, style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        },
        groups = listOf(
            options.mapIndexed { i, (label, on) -> SheetAction(label, if (on) check else null) { onPick(i) } },
        ),
    )
}

private const val BAR_DP = 14
private const val MIN_SLICE = 0.012f
private const val ITEM_ART_DP = 48
