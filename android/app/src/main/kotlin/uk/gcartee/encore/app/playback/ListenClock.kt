package uk.gcartee.encore.app.playback

/**
 * Counts the time a song was actually heard (`listened_ms`, plan section 5): wall time
 * while playing, never seeks. One per play; `reset` when the next song starts.
 * The clock is injected so the rule is unit-tested.
 */
class ListenClock(private val now: () -> Long) {
    private var listened = 0L
    private var playingSince: Long? = null

    val listenedMs: Long
        get() = listened + (playingSince?.let { now() - it } ?: 0L)

    fun playing(isPlaying: Boolean) {
        val since = playingSince
        if (isPlaying && since == null) {
            playingSince = now()
        } else if (!isPlaying && since != null) {
            listened += now() - since
            playingSince = null
        }
    }

    /** Starts a new play; keeps running if the player is playing. */
    fun reset(carriedMs: Long = 0L) {
        val wasPlaying = playingSince != null
        listened = carriedMs
        playingSince = if (wasPlaying) now() else null
    }
}
