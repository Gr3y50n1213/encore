package uk.gcartee.encore.app.wear

import android.content.Context
import android.graphics.Bitmap
import android.os.SystemClock
import android.util.Log
import com.google.android.gms.wearable.Asset
import com.google.android.gms.wearable.CapabilityClient
import com.google.android.gms.wearable.PutDataMapRequest
import com.google.android.gms.wearable.Wearable
import java.io.ByteArrayOutputStream
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.notifications.LiveUpdates
import uk.gcartee.encore.app.pebble.PebbleLink
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.playback.PlayerState
import uk.gcartee.encore.app.widgets.SnapshotPrivacy
import uk.gcartee.encore.app.widgets.SnapshotRead
import uk.gcartee.encore.app.widgets.SnapshotReader
import uk.gcartee.encore.core.EventKind
import uk.gcartee.encore.core.LyricsSync
import uk.gcartee.encore.core.Song
import uk.gcartee.encore.wearlink.LyricTiming
import uk.gcartee.encore.wearlink.MixChip
import uk.gcartee.encore.wearlink.Mixes
import uk.gcartee.encore.wearlink.MixesCodec
import uk.gcartee.encore.wearlink.NowCodec
import uk.gcartee.encore.wearlink.NowPlaying
import uk.gcartee.encore.wearlink.Paths
import uk.gcartee.encore.wearlink.Throttle
import uk.gcartee.encore.wearlink.WearGms
import uk.gcartee.encore.wearlink.awaitOrNull

/**
 * Stage W1 (docs/adr/android-0008-wear-os-app.md): the phone's side of the Wear OS link.
 * Publishes [Paths.NOW] and [Paths.MIXES] as DataItems, and only while a watch with the
 * Encore watch app is reachable (its [Paths.WATCH_CAPABILITY]); with none, it only keeps
 * the last player state, and on a phone without Google Play services it does nothing.
 *
 * - Now Playing: the song, play state, position, like, the next song's title, the lyric
 *   line being sung and a cover of at most 200 px, whenever any of them changes, at most
 *   twice a second ([Throttle]). The position alone moving on is not sent: the watch
 *   works it out from the position and its time.
 * - Mixes: up to four pinned mixes with their colours, from the widget snapshot the core
 *   already writes (CORE-API, "Widgets"), so no new core call. Guest, a locked profile and
 *   Private listening send none.
 * - `private` is the phone's "Hide listening history while locked" setting.
 * - Lyrics are read only for a song the library says has lyrics (`Song.hasLyrics`), the
 *   way the lyrics view reads them, so a watch never starts a lookup the phone wouldn't.
 *
 * Stage W2 (docs/adr/android-0009-pebble.md): the same Now Playing goes to the Encore
 * watchapp on a Pebble ([PebbleLink]), built here once for both, on the same changes and
 * within the same limit of two sends a second. The Pebble gets no cover and no mixes.
 *
 * Started by CoreHost, like WidgetSync: it runs only where the core already runs.
 */
object WearLink {
    private const val TAG = "WearLink"
    private const val MIN_INTERVAL_MS = 500L
    private const val COVER_PX = 200
    private const val COVER_QUALITY = 80
    private const val LIKED_RECHECK_MS = 5_000L

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    @Volatile private var attached = false
    @Volatile private var watchNearby = false

    /** A Wear OS watch with Encore is reachable, or the Pebble watchapp is open. */
    private fun anyWatch(): Boolean = watchNearby || PebbleLink.active

    private val nowSignal = Channel<Unit>(Channel.CONFLATED)
    private val mixesSignal = Channel<Unit>(Channel.CONFLATED)

    @Volatile private var state = PlayerState()
    @Volatile private var stateAt = 0L
    @Volatile private var lastNow: NowPlaying? = null
    @Volatile private var lastMixes: Mixes? = null
    @Volatile private var lastHadCover = false

    private class Lyrics(val songId: Long, val starts: List<Long?>, val texts: List<String>)
    @Volatile private var lyrics: Lyrics? = null
    @Volatile private var lyricsFor: Long? = null
    private var lineJob: Job? = null

    private class Liked(val songId: Long, val liked: Boolean, val at: Long)
    @Volatile private var liked: Liked? = null

    private class Cover(val songId: Long, val bytes: ByteArray?)
    @Volatile private var cover: Cover? = null

    /** From CoreHost, once the core exists. */
    fun attach(host: CoreHost) {
        if (attached) return
        attached = true
        val context = host.context.applicationContext
        val gms = WearGms.available(context)
        val pebble = PebbleLink.attach(context) { pebbleOpened(host) }
        if (!gms && !pebble) return
        if (gms) {
            val capabilities = Wearable.getCapabilityClient(context)
            scope.launch {
                val info = capabilities.getCapability(Paths.WATCH_CAPABILITY, CapabilityClient.FILTER_REACHABLE).awaitOrNull()
                watchChanged(host, info?.nodes?.isNotEmpty() == true)
            }
            runCatching {
                capabilities.addListener({ info -> watchChanged(host, info.nodes.isNotEmpty()) }, Paths.WATCH_CAPABILITY)
            }.onFailure { Log.w(TAG, "No capability listener: ${it.javaClass.simpleName}") }
        }

        scope.launch { PlaybackHub.state.collect { onState(host, it) } }
        scope.launch { host.queue.collect { if (anyWatch()) nowSignal.trySend(Unit) } }
        scope.launch { host.privateListening.collect { if (watchNearby) mixesSignal.trySend(Unit) } }
        scope.launch {
            host.events.collect { event ->
                when (event.kind) {
                    EventKind.WIDGET_SNAPSHOT_CHANGED -> {
                        // A cover saved since, or a privacy change: read the snapshot again.
                        if (cover?.bytes == null) cover = null
                        if (anyWatch()) nowSignal.trySend(Unit)
                        if (watchNearby) mixesSignal.trySend(Unit)
                    }
                    // Another device liked or unliked something.
                    EventKind.REMOTE_CHANGE -> {
                        liked = null
                        if (anyWatch()) nowSignal.trySend(Unit)
                    }
                    else -> Unit
                }
            }
        }
        scope.launch { sendLoop(nowSignal, ::anyWatch) { publishNow(host, context, gms) } }
        if (gms) scope.launch { sendLoop(mixesSignal, { watchNearby }) { publishMixes(host, context) } }
    }

    /** After the watch liked or unliked the song: read it again and tell the watch. */
    internal fun likedChanged() {
        liked = null
        if (anyWatch()) nowSignal.trySend(Unit)
    }

    /** The Pebble watchapp opened: send it everything, without restarting the position's clock. */
    private fun pebbleOpened(host: CoreHost) {
        refresh(host, state)
    }

    private fun watchChanged(host: CoreHost, nearby: Boolean) {
        val came = nearby && !watchNearby
        watchNearby = nearby
        if (!came) return
        // A watch arrived: send it everything, fresh.
        lastNow = null
        lastMixes = null
        onState(host, state)
        mixesSignal.trySend(Unit)
    }

    private fun onState(host: CoreHost, s: PlayerState) {
        state = s
        stateAt = SystemClock.elapsedRealtime()
        if (anyWatch()) refresh(host, s)
    }

    private fun refresh(host: CoreHost, s: PlayerState) {
        val song = s.entry?.song
        if (song?.id != lyricsFor) loadLyrics(host, song)
        scheduleLine(s)
        nowSignal.trySend(Unit)
    }

    private fun positionNow(): Long {
        val s = state
        val moved = if (s.isPlaying) SystemClock.elapsedRealtime() - stateAt else 0
        return (s.positionMs + moved).coerceAtLeast(0)
    }

    private fun loadLyrics(host: CoreHost, song: Song?) {
        lyricsFor = song?.id
        lyrics = null
        if (song == null || !song.hasLyrics) return
        scope.launch(Dispatchers.IO) {
            val got = runCatching { host.core.lyrics(song.id) }.getOrNull() ?: return@launch
            if (got.syncLevel == LyricsSync.UNSYNCED || got.instrumental) return@launch
            if (lyricsFor != song.id) return@launch
            lyrics = Lyrics(song.id, got.lines.map { it.startMs }, got.lines.map { it.text })
            nowSignal.trySend(Unit)
        }
    }

    /** Wakes the sender as the next line starts, so the watch's line changes on time. */
    private fun scheduleLine(s: PlayerState) {
        lineJob?.cancel()
        val l = lyrics ?: return
        if (!s.isPlaying || l.songId != s.entry?.song?.id) return
        val wait = LyricTiming.msUntilNext(l.starts, positionNow()) ?: return
        lineJob = scope.launch {
            delay(wait.coerceAtLeast(0) + 20)
            nowSignal.trySend(Unit)
        }
    }

    private suspend fun sendLoop(signal: Channel<Unit>, wanted: () -> Boolean, send: suspend () -> Boolean) {
        val throttle = Throttle(MIN_INTERVAL_MS)
        for (ignored in signal) {
            if (!wanted()) continue
            val wait = throttle.delayFor(SystemClock.elapsedRealtime())
            if (wait > 0) delay(wait)
            val sent = runCatching { send() }
                .onFailure { Log.w(TAG, "Couldn't update the watch: ${it.javaClass.simpleName}") }
                .getOrDefault(false)
            if (sent) throttle.sent(SystemClock.elapsedRealtime())
        }
    }

    /** Builds Now Playing once and gives it to each watch that is there. */
    private suspend fun publishNow(host: CoreHost, context: Context, gms: Boolean): Boolean {
        val now = buildNow(host, context)
        val toPebble = PebbleLink.send(context, now)
        val toWear = gms && watchNearby && publishWear(host, context, now)
        return toPebble || toWear
    }

    private suspend fun publishWear(host: CoreHost, context: Context, now: NowPlaying): Boolean {
        val bytes = state.entry?.song?.let { coverBytes(host, context, it.id) }
        // A cover saved after the song started goes out too, though nothing else changed.
        if (!now.needsSendAfter(lastNow) && (bytes != null) == lastHadCover) return false
        val request = PutDataMapRequest.create(Paths.NOW)
        WearGms.put(request.dataMap, NowCodec.encode(now))
        if (bytes != null) request.dataMap.putAsset(NowCodec.COVER, Asset.createFromBytes(bytes))
        request.setUrgent()
        val put = Wearable.getDataClient(context).putDataItem(request.asPutDataRequest()).awaitOrNull()
        if (put != null) {
            lastNow = now
            lastHadCover = bytes != null
        }
        return put != null
    }

    private fun buildNow(host: CoreHost, context: Context): NowPlaying {
        val s = state
        val hidden = LiveUpdates.hideHistoryWhileLocked(context)
        val song = s.entry?.song ?: return NowPlaying.EMPTY.copy(isPrivate = hidden)
        val position = positionNow()
        val l = lyrics?.takeIf { it.songId == song.id }
        val line = l?.let { it.texts.getOrNull(LyricTiming.indexAt(it.starts, position)) }.orEmpty().trim()
        val queue = host.queue.value
        val next = (queue?.upNext?.firstOrNull() ?: queue?.autoplay?.firstOrNull())?.song?.title.orEmpty()
        return NowPlaying(
            title = song.title,
            artist = song.artist,
            album = song.albumTitle.orEmpty(),
            line = line,
            isPlaying = s.isPlaying,
            positionMs = position,
            positionAtUtcMs = System.currentTimeMillis(),
            durationMs = s.durationMs.takeIf { it > 0 } ?: song.durationMs,
            liked = isLiked(host, song.id),
            upNext = next,
            isPrivate = hidden,
        )
    }

    private fun isLiked(host: CoreHost, songId: Long): Boolean {
        val known = liked
        val at = SystemClock.elapsedRealtime()
        if (known != null && known.songId == songId && at - known.at < LIKED_RECHECK_MS) return known.liked
        val value = runCatching { host.core.areLiked(listOf(songId)).firstOrNull() == true }.getOrDefault(known?.liked == true)
        liked = Liked(songId, value, at)
        return value
    }

    /** The playing song's cover from the widget snapshot, as a JPEG of at most 200 px. */
    private fun coverBytes(host: CoreHost, context: Context, songId: Long): ByteArray? {
        cover?.takeIf { it.songId == songId }?.let { return it.bytes }
        val bytes = runCatching {
            val profile = host.core.currentProfile() ?: return@runCatching null
            val dir = SnapshotReader.widgetsDir(context.filesDir)
            val snapshot = (SnapshotReader.read(dir, profile) as? SnapshotRead.Ok)?.snapshot ?: return@runCatching null
            val song = snapshot.song?.takeIf { it.songId == songId } ?: return@runCatching null
            val file = SnapshotReader.coverFile(dir, profile, song.cover) ?: return@runCatching null
            val bitmap = SnapshotReader.loadCover(file, COVER_PX) ?: return@runCatching null
            jpeg(bitmap)
        }.getOrNull()
        cover = Cover(songId, bytes)
        return bytes
    }

    private fun jpeg(bitmap: Bitmap): ByteArray {
        val longer = maxOf(bitmap.width, bitmap.height)
        val scaled = if (longer > COVER_PX) {
            val f = COVER_PX.toFloat() / longer
            Bitmap.createScaledBitmap(bitmap, (bitmap.width * f).toInt().coerceAtLeast(1), (bitmap.height * f).toInt().coerceAtLeast(1), true)
        } else {
            bitmap
        }
        return ByteArrayOutputStream().use { out ->
            scaled.compress(Bitmap.CompressFormat.JPEG, COVER_QUALITY, out)
            out.toByteArray()
        }
    }

    private suspend fun publishMixes(host: CoreHost, context: Context): Boolean {
        val mixes = buildMixes(host, context)
        if (mixes == lastMixes) return false
        val request = PutDataMapRequest.create(Paths.MIXES)
        WearGms.put(request.dataMap, MixesCodec.encode(mixes))
        val put = Wearable.getDataClient(context).putDataItem(request.asPutDataRequest()).awaitOrNull()
        if (put != null) lastMixes = mixes
        return put != null
    }

    private fun buildMixes(host: CoreHost, context: Context): Mixes {
        val profile = runCatching { host.core.currentProfile() }.getOrNull() ?: return Mixes.EMPTY
        val read = SnapshotReader.read(SnapshotReader.widgetsDir(context.filesDir), profile)
        val snapshot = (read as? SnapshotRead.Ok)?.snapshot
        // Guest and a profile with a PIN: nothing, as on the widgets.
        if (snapshot != null && snapshot.privacy != SnapshotPrivacy.NORMAL) return Mixes.EMPTY
        // Private listening: the controls and Liked songs, but no mixes (they come from history).
        if (host.privateListening.value || snapshot == null) return Mixes(emptyList(), likedSongs = true)
        val chips = snapshot.pinnedMixes.take(MixesCodec.MAX).map { mix ->
            MixChip(mix.mixId, mix.title, mix.palette?.stops?.firstOrNull() ?: 0, mix.palette?.text ?: 0)
        }
        return Mixes(chips, likedSongs = true)
    }
}
