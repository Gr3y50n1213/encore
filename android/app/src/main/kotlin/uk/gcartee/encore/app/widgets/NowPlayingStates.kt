package uk.gcartee.encore.app.widgets

/** Where a tap on the widget's body goes. */
enum class WidgetTarget { NOW_PLAYING, APP, PROFILES }

/** The one large button: round when it plays, a rounded square while playing. */
enum class MainButton { PLAY, PAUSE, START_NEXT_UP }

/** Board 34's state cards: always words, never an error icon. */
enum class CardKind {
    /** "Nothing yet / Mixes build as you listen" with Play. */
    NOTHING_YET,

    /** No song yet but mixes exist: the Next up mix's name with Play. */
    MIX_READY,

    /** "Saved mixes / Server not reachable". */
    UNREACHABLE,

    /** "Open Encore / Profile is locked". */
    LOCKED,

    /** Guest, a newer file, no profile yet: the neutral "Open Encore". */
    OPEN_APP,

    /** "Profile removed / Choose a profile". */
    REMOVED,

    /** On the lock screen with "Hide listening history while locked": Play and the mix name only. */
    LOCK_SCREEN_IDLE,
}

sealed interface NowPlayingView {
    val tap: WidgetTarget

    /**
     * Where the buttons go instead of playing: null when they go straight to the playback
     * service; [WidgetTarget.PROFILES] for another profile or a PIN; [WidgetTarget.APP] when
     * the snapshot is stale.
     */
    val buttonsOpen: WidgetTarget?

    data class Card(
        val kind: CardKind,
        override val tap: WidgetTarget,
        val button: MainButton? = null,
        val mixName: String? = null,
        override val buttonsOpen: WidgetTarget? = null,
    ) : NowPlayingView

    data class Song(
        val title: String,
        val artist: String,
        val album: String?,
        val cover: String?,
        /** The playing state shown: a "playing" snapshot whose song must have ended reads as paused. */
        val playing: Boolean,
        /** The song is the current one (else the last played, shown with Play). */
        val current: Boolean,
        /** The album's colors; null until the core has made them (the widget then wears the theme's). */
        val palette: SnapshotPalette?,
        /**
         * Which of the three level-bar drawings to show while it plays. It changes with every
         * song, pause and resume (the file's position time moves with each), so the bars look
         * alive whenever the widget redraws. A widget can't animate.
         */
        val barsVariant: Int,
        /** Over 24 h old: dimmed, and says Open Encore to refresh. */
        val stale: Boolean,
        val button: MainButton,
        override val tap: WidgetTarget,
        override val buttonsOpen: WidgetTarget?,
    ) : NowPlayingView
}

/**
 * Pure rules for the Now Playing widget (board 34, item 2; CORE-API "Widgets"; the plan's
 * decisions table). JVM-tested.
 */
object NowPlayingStates {
    const val STALE_MS = 24L * 60 * 60 * 1000

    /**
     * @param read the profile's file; null when no profile resolved (nothing signed in yet).
     * @param canControl the buttons may play directly (WidgetConfig.canControl).
     * @param onLockScreen the host says this widget is on the keyguard.
     * @param hideHistoryWhileLocked the "Hide listening history while locked" setting.
     */
    fun decide(
        read: SnapshotRead?,
        canControl: Boolean,
        nowUtcMs: Long,
        onLockScreen: Boolean,
        hideHistoryWhileLocked: Boolean,
    ): NowPlayingView {
        val s = when (read) {
            null -> return NowPlayingView.Card(CardKind.OPEN_APP, WidgetTarget.APP)
            SnapshotRead.Missing -> return NowPlayingView.Card(CardKind.REMOVED, WidgetTarget.PROFILES)
            SnapshotRead.Unreadable -> return NowPlayingView.Card(CardKind.OPEN_APP, WidgetTarget.APP)
            is SnapshotRead.Ok -> read.snapshot
        }
        when (s.privacy) {
            SnapshotPrivacy.LOCKED -> return NowPlayingView.Card(CardKind.LOCKED, WidgetTarget.APP)
            SnapshotPrivacy.GUEST -> return NowPlayingView.Card(CardKind.OPEN_APP, WidgetTarget.APP)
            SnapshotPrivacy.NORMAL -> Unit
        }
        val stale = isStale(s, nowUtcMs)
        val buttonsOpen = when {
            stale -> WidgetTarget.APP
            !canControl -> WidgetTarget.PROFILES
            else -> null
        }
        val bodyTap = when {
            stale -> WidgetTarget.APP
            !canControl -> WidgetTarget.PROFILES
            else -> WidgetTarget.NOW_PLAYING
        }
        val song = s.song
        val playing = song != null && song.nowPlaying && s.isPlaying && !hasSurelyEnded(s, song, nowUtcMs)

        // The lock screen never shows what was played: only Play and the mix name, unless
        // something is playing now (the system's own player shows that anyway).
        if (onLockScreen && hideHistoryWhileLocked && !playing) {
            return NowPlayingView.Card(
                CardKind.LOCK_SCREEN_IDLE,
                tap = WidgetTarget.APP,
                button = if (song != null) MainButton.PLAY else MainButton.START_NEXT_UP,
                mixName = s.nextUp?.title?.takeIf { it.isNotBlank() },
                buttonsOpen = buttonsOpen,
            )
        }
        if (!s.reachable && !playing) {
            return NowPlayingView.Card(CardKind.UNREACHABLE, WidgetTarget.APP)
        }
        if (song == null) {
            val nothing = s.pinnedMixes.isEmpty() && s.nextUp == null && s.recentAlbums.isEmpty()
            return if (nothing) {
                NowPlayingView.Card(CardKind.NOTHING_YET, WidgetTarget.APP, MainButton.START_NEXT_UP, buttonsOpen = buttonsOpen)
            } else {
                NowPlayingView.Card(
                    CardKind.MIX_READY,
                    WidgetTarget.APP,
                    MainButton.START_NEXT_UP,
                    mixName = (s.nextUp ?: s.pinnedMixes.firstOrNull())?.title,
                    buttonsOpen = buttonsOpen,
                )
            }
        }
        return NowPlayingView.Song(
            title = song.title,
            artist = song.artist,
            album = song.albumTitle,
            cover = song.cover,
            playing = playing,
            current = song.nowPlaying,
            palette = song.palette,
            barsVariant = Math.floorMod(((song.songId * 31L) + s.positionAtUtcMs / 1000L).toInt(), BARS_VARIANTS),
            stale = stale,
            button = if (playing) MainButton.PAUSE else MainButton.PLAY,
            tap = bodyTap,
            buttonsOpen = buttonsOpen,
        )
    }

    /**
     * A "playing" file whose song would have finished by now (written at, plus what was
     * left of the song) reads as paused: the app may have been closed since.
     */
    fun hasSurelyEnded(s: Snapshot, song: SnapshotSong, nowUtcMs: Long): Boolean {
        val remaining = (song.durationMs - s.positionMs).coerceAtLeast(0L)
        return s.writtenAtUtcMs + remaining < nowUtcMs
    }

    /** The file is over a day old: the widget dims and says Open Encore to refresh. */
    fun isStale(s: Snapshot, nowUtcMs: Long): Boolean = nowUtcMs - s.writtenAtUtcMs > STALE_MS

    /** The level-bar drawings (`widget_bars_a`, `_b`, `_c`). */
    const val BARS_VARIANTS = 3
}
