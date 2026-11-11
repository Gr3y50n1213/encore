package uk.gcartee.encore.app.widgets

/** A Mixes widget's Mix setting (board 34): follow Next up, or lead with one pinned mix. */
sealed interface WidgetMixChoice {
    data object NextUp : WidgetMixChoice
    data class Pinned(val mixId: String) : WidgetMixChoice
}

/** One tile. A null [mixId] is Next up: the core picks the first unplayed mix of today's set. */
data class MixTile(val mixId: String?, val title: String, val cover: String?)

sealed interface MixesView {
    data class Card(val kind: CardKind, val tap: WidgetTarget) : MixesView

    data class Tiles(
        val tiles: List<MixTile>,
        /** Over 24 h old: dimmed, and says Open Encore to refresh. */
        val stale: Boolean,
        /** Where a tap goes instead of starting the mix: null plays it; the switcher or the app otherwise. */
        val tilesOpen: WidgetTarget?,
    ) : MixesView
}

sealed interface RecentView {
    data class Card(val kind: CardKind, val tap: WidgetTarget) : RecentView

    data class Albums(
        /** The profile the file belongs to: the app checks it is the one open before opening an album. */
        val profileId: String,
        val albums: List<SnapshotAlbum>,
        val stale: Boolean,
        /** Null opens the album; the switcher or the app otherwise. */
        val albumsOpen: WidgetTarget?,
    ) : RecentView
}

/**
 * Pure rules for the Mixes and Recently played widgets (board 34, items 3 and 4). They
 * reuse the Now Playing widget's state rules: the same cards for a PIN, Guest, a removed
 * profile and a newer file, the same 24 h dim, the same "never another profile" and "never
 * switch profiles silently" rules. JVM-tested.
 */
object CollectionStates {
    const val MIX_TILES = 4
    const val RECENT_ALBUMS = 8

    private sealed interface Gate {
        data class Stop(val kind: CardKind, val tap: WidgetTarget) : Gate
        data class Go(val snapshot: Snapshot) : Gate
    }

    private fun gate(read: SnapshotRead?): Gate = when (read) {
        null -> Gate.Stop(CardKind.OPEN_APP, WidgetTarget.APP)
        SnapshotRead.Missing -> Gate.Stop(CardKind.REMOVED, WidgetTarget.PROFILES)
        SnapshotRead.Unreadable -> Gate.Stop(CardKind.OPEN_APP, WidgetTarget.APP)
        is SnapshotRead.Ok -> when (read.snapshot.privacy) {
            SnapshotPrivacy.LOCKED -> Gate.Stop(CardKind.LOCKED, WidgetTarget.APP)
            SnapshotPrivacy.GUEST -> Gate.Stop(CardKind.OPEN_APP, WidgetTarget.APP)
            SnapshotPrivacy.NORMAL -> Gate.Go(read.snapshot)
        }
    }

    private fun isStale(s: Snapshot, nowUtcMs: Long) = nowUtcMs - s.writtenAtUtcMs > NowPlayingStates.STALE_MS

    /**
     * The tiles, at most four. Next up leads (and follows the daily set); a pinned choice
     * leads instead while that mix is still pinned, then Next up, then the other pinned
     * mixes. A mix is never shown twice.
     */
    fun mixTiles(s: Snapshot, choice: WidgetMixChoice): List<MixTile> {
        val next = s.nextUp?.let { MixTile(null, it.title, it.cover) }
        val nextId = s.nextUp?.mixId
        val pinned = s.pinnedMixes
            .filter { it.mixId != nextId }
            .distinctBy { it.mixId }
            .map { MixTile(it.mixId, it.title, it.cover) }
        val chosen = (choice as? WidgetMixChoice.Pinned)?.let { c -> pinned.firstOrNull { it.mixId == c.mixId } }
        val ordered = if (chosen != null) {
            listOf(chosen) + listOfNotNull(next) + pinned.filter { it !== chosen }
        } else {
            listOfNotNull(next) + pinned
        }
        return ordered.take(MIX_TILES)
    }

    fun mixes(read: SnapshotRead?, choice: WidgetMixChoice, canControl: Boolean, nowUtcMs: Long): MixesView {
        val s = when (val g = gate(read)) {
            is Gate.Stop -> return MixesView.Card(g.kind, g.tap)
            is Gate.Go -> g.snapshot
        }
        if (!s.reachable) return MixesView.Card(CardKind.UNREACHABLE, WidgetTarget.APP)
        val tiles = mixTiles(s, choice)
        if (tiles.isEmpty()) return MixesView.Card(CardKind.NOTHING_YET, WidgetTarget.APP)
        val stale = isStale(s, nowUtcMs)
        return MixesView.Tiles(tiles, stale, tilesOpen(stale, canControl))
    }

    fun recent(read: SnapshotRead?, canControl: Boolean, nowUtcMs: Long): RecentView {
        val s = when (val g = gate(read)) {
            is Gate.Stop -> return RecentView.Card(g.kind, g.tap)
            is Gate.Go -> g.snapshot
        }
        val albums = s.recentAlbums.distinctBy { it.albumId }.take(RECENT_ALBUMS)
        if (albums.isEmpty()) return RecentView.Card(CardKind.NOTHING_YET, WidgetTarget.APP)
        val stale = isStale(s, nowUtcMs)
        return RecentView.Albums(s.profileId, albums, stale, tilesOpen(stale, canControl))
    }

    private fun tilesOpen(stale: Boolean, canControl: Boolean): WidgetTarget? = when {
        stale -> WidgetTarget.APP
        !canControl -> WidgetTarget.PROFILES
        else -> null
    }
}

/**
 * The request a Recently played cover sends the app (MainActivity puts it in
 * `host.openRequest`, EncoreRoot acts on it): `album:<albumId>:<profileId>`. Profile IDs
 * are safe names, so the second colon always ends the album ID.
 */
object AlbumOpenRequest {
    private const val PREFIX = "album:"

    data class Parsed(val albumId: Long, val profileId: String)

    fun encode(albumId: Long, profileId: String): String = "$PREFIX$albumId:$profileId"

    fun parse(request: String?): Parsed? {
        if (request == null || !request.startsWith(PREFIX)) return null
        val parts = request.removePrefix(PREFIX).split(':', limit = 2)
        if (parts.size != 2) return null
        val albumId = parts[0].toLongOrNull() ?: return null
        if (!SnapshotReader.isSafeName(parts[1])) return null
        return Parsed(albumId, parts[1])
    }
}
