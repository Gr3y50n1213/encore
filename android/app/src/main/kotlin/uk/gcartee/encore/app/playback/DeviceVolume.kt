package uk.gcartee.encore.app.playback

import android.content.Context
import android.media.AudioManager
import kotlin.math.roundToInt

/** The device's own media volume, 0 to 1 (remote control sets and reads it). */
interface DeviceVolume {
    fun level(): Double
    fun set(level: Double)
}

/** The music stream's volume through AudioManager, as the volume keys move it. */
class AudioManagerVolume(context: Context) : DeviceVolume {
    private val audio = context.getSystemService(AudioManager::class.java)

    override fun level(): Double {
        val max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC).coerceAtLeast(1)
        return audio.getStreamVolume(AudioManager.STREAM_MUSIC).toDouble() / max
    }

    override fun set(level: Double) {
        val max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC)
        val steps = (level.coerceIn(0.0, 1.0) * max).roundToInt().coerceIn(0, max)
        // No system slider pops up on the device being controlled: it is set from elsewhere.
        runCatching { audio.setStreamVolume(AudioManager.STREAM_MUSIC, steps, 0) }
    }
}
