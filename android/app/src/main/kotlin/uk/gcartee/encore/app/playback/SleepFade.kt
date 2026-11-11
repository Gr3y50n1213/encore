package uk.gcartee.encore.app.playback

import kotlin.math.abs
import uk.gcartee.encore.core.SleepFadeAction
import uk.gcartee.encore.core.SleepFadePlan

/**
 * Phase 9: the sleep timer's fade-out (CORE-API "Sleep timer"; board 31, "Fading"). The
 * core says what to do each second ([update] with `sleep_fade_plan`); this works out the
 * volume in between ([level], called about 16 times a second) and when to stop
 * ([timeIsUp]). The player applies the level to its own volume and puts it back after.
 *
 * - **FadeNow** fades by the clock: from the level now to silence over `fade_ms`. A skip
 *   doesn't stop it; the next song carries on from where the volume was.
 * - **StopAtSongEnd** fades by the song's position: full until `fade_from_ms`, silent at
 *   the song's end. The player stops at that end ([stopsAtSongEnd]) instead of moving on.
 * - **KeepPlaying** while fading means the timer moved (Add 15 min) or went: the fade
 *   stops and the level is back to full. A pause or a handoff calls [clear] for the same.
 * - End of song and End of album never fade: the core always says KeepPlaying for them.
 *
 * A fade only goes down: switching from one kind to the other starts from the level
 * reached, never from full. The clock is injected so the rule is unit-tested.
 */
class SleepFade(private val now: () -> Long) {
    enum class Phase { IDLE, TIMED, SONG_END }

    var phase: Phase = Phase.IDLE
        private set

    /** The level the current stretch of fading starts from. */
    private var startLevel = 1f
    /** TIMED: when it started and when it reaches silence, on the injected clock. */
    private var startAt = 0L
    private var endAt = 0L
    /** SONG_END: where in the song it starts going down, and the song's end. */
    private var fromMs = 0L
    private var toMs = 0L
    /** The last level handed out. */
    private var last = 1f

    val fading: Boolean get() = phase != Phase.IDLE

    /** A song that ends while fading is where the music stops (report it Finished, then end the timer). */
    val stopsAtSongEnd: Boolean get() = fading

    /** The core's plan for this moment, with the current song's position. */
    fun update(plan: SleepFadePlan, positionMs: Long) {
        when (plan.action) {
            SleepFadeAction.KEEP_PLAYING -> if (fading) clear()
            SleepFadeAction.FADE_NOW -> {
                val t = now()
                val end = t + plan.fadeMs.coerceAtLeast(0L)
                // Each second the core gives what is left: the same end. Start a new stretch
                // only on the way in, or when the end came sooner.
                if (phase != Phase.TIMED || end < endAt - SLACK_MS) {
                    startLevel = if (fading) last else 1f
                    startAt = t
                    endAt = end
                    phase = Phase.TIMED
                }
            }
            SleepFadeAction.STOP_AT_SONG_END -> {
                val to = plan.fadeFromMs + plan.fadeMs.coerceAtLeast(0L)
                // Another song (a skip) has another end: go on from the level reached.
                if (phase != Phase.SONG_END || abs(to - toMs) > SLACK_MS) {
                    startLevel = if (fading) last else 1f
                    fromMs = maxOf(plan.fadeFromMs, positionMs)
                    toMs = to
                    phase = Phase.SONG_END
                }
            }
        }
    }

    /** The volume to apply now, 0 to 1, as a share of the volume before the fade. */
    fun level(positionMs: Long): Float {
        val raw = when (phase) {
            Phase.IDLE -> return 1f.also { last = 1f }
            Phase.TIMED -> {
                val span = endAt - startAt
                if (span <= 0L) 0f else startLevel * ((endAt - now()).toFloat() / span.toFloat())
            }
            Phase.SONG_END -> when {
                positionMs <= fromMs -> startLevel
                toMs <= fromMs -> 0f
                else -> startLevel * ((toMs - positionMs).toFloat() / (toMs - fromMs).toFloat())
            }
        }.coerceIn(0f, 1f)
        last = minOf(last, raw)
        return last
    }

    /**
     * Silence reached by the clock (FadeNow): pause and end the timer now. For a song-end
     * fade only once the song is well past its expected end (its length was wrong).
     */
    fun timeIsUp(positionMs: Long): Boolean = when (phase) {
        Phase.IDLE -> false
        Phase.TIMED -> now() >= endAt
        Phase.SONG_END -> positionMs >= toMs + SONG_END_GRACE_MS
    }

    /** Pause, handoff, cancel, Add 15 min, or the end done: no fade, full level. */
    fun clear() {
        phase = Phase.IDLE
        startLevel = 1f
        last = 1f
    }

    companion object {
        /** Jitter between the core's answers a second apart. */
        const val SLACK_MS = 250L
        /** How far past its expected end a song may run before the fade stops it anyway. */
        const val SONG_END_GRACE_MS = 3_000L
    }
}
