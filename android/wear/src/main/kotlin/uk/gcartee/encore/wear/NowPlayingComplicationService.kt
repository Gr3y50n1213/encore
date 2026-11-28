package uk.gcartee.encore.wear

import android.app.PendingIntent
import android.content.Intent
import android.graphics.drawable.Icon
import androidx.wear.watchface.complications.data.ComplicationData
import androidx.wear.watchface.complications.data.ComplicationType
import androidx.wear.watchface.complications.data.LongTextComplicationData
import androidx.wear.watchface.complications.data.MonochromaticImage
import androidx.wear.watchface.complications.data.PlainComplicationText
import androidx.wear.watchface.complications.data.ShortTextComplicationData
import androidx.wear.watchface.complications.datasource.ComplicationRequest
import androidx.wear.watchface.complications.datasource.SuspendingComplicationDataSourceService
import uk.gcartee.encore.wearlink.WatchPrivacy

/**
 * Mock-up 50, screen 5: a complication with the song playing on the phone; a tap opens Now
 * Playing. On a locked watch it says only "Encore", and with the phone's "Hide listening
 * history while locked" on it never shows the last song once it stops (WatchPrivacy).
 * Redrawn whenever the song or its play state changes (WatchStore).
 */
class NowPlayingComplicationService : SuspendingComplicationDataSourceService() {

    override suspend fun onComplicationRequest(request: ComplicationRequest): ComplicationData? {
        WatchStore.load(this)
        val now = WatchStore.now.value
        val show = WatchPrivacy.showSong(watchLocked(this), now, onWatchFace = true)
        val name = getString(R.string.app_name)
        return if (show && now != null) {
            data(request.complicationType, now.title, now.artist, "${now.title}, ${now.artist}")
        } else {
            data(request.complicationType, name, null, name)
        }
    }

    override fun getPreviewData(type: ComplicationType): ComplicationData? =
        // Mock-up 50's made-up song.
        data(type, "Slow Burn Season", "Marigold Ave", "Slow Burn Season, Marigold Ave", tap = false)

    private fun data(type: ComplicationType, text: String, title: String?, description: String, tap: Boolean = true): ComplicationData? {
        val icon = MonochromaticImage.Builder(Icon.createWithResource(this, R.drawable.sym_music_note)).build()
        val open = if (tap) {
            PendingIntent.getActivity(
                this,
                0,
                Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        } else {
            null
        }
        return when (type) {
            ComplicationType.SHORT_TEXT ->
                ShortTextComplicationData.Builder(plain(text), plain(description))
                    .setMonochromaticImage(icon)
                    .setTapAction(open)
                    .build()
            ComplicationType.LONG_TEXT ->
                LongTextComplicationData.Builder(plain(text), plain(description))
                    .apply { if (title != null) setTitle(plain(title)) }
                    .setMonochromaticImage(icon)
                    .setTapAction(open)
                    .build()
            else -> null
        }
    }

    private fun plain(s: String) = PlainComplicationText.Builder(s).build()
}
