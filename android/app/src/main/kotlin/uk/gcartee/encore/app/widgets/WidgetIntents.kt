package uk.gcartee.encore.app.widgets

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.view.KeyEvent
import uk.gcartee.encore.app.MainActivity
import uk.gcartee.encore.app.playback.PlaybackService

/**
 * The intents behind widget taps. Transport buttons go straight to the playback service as
 * media buttons (Media3's MediaSessionService.onStartCommand handles ACTION_MEDIA_BUTTON
 * and its playback resumption), so they work with the app closed and never open it.
 * Starting a mix is a private action that carries [WidgetToken]. Neither ever names a
 * profile: the service plays the profile it has open (or the last active one).
 */
object WidgetIntents {
    const val ACTION_PLAY_MIX = "uk.gcartee.encore.app.action.PLAY_MIX"
    const val EXTRA_TOKEN = "uk.gcartee.encore.app.extra.WIDGET_TOKEN"

    /** The mix to start; absent means Next up. */
    const val EXTRA_MIX_ID = "uk.gcartee.encore.app.extra.MIX_ID"

    /** Marks a start from a widget or the tile (the service's foreground guard). */
    const val EXTRA_FROM_WIDGET = "uk.gcartee.encore.app.extra.FROM_WIDGET"

    const val KEY_PLAY_PAUSE = KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE
    const val KEY_NEXT = KeyEvent.KEYCODE_MEDIA_NEXT
    const val KEY_PREVIOUS = KeyEvent.KEYCODE_MEDIA_PREVIOUS

    private const val SCHEME = "encore-widget"

    /**
     * A media button for the playback service. Media3 acts on the key's ACTION_DOWN.
     * Glance gives each button's PendingIntent its own data URI, so the three never
     * replace one another; Media3 finds no session by that URI and uses ours.
     */
    fun mediaButton(context: Context, keyCode: Int): Intent =
        Intent(Intent.ACTION_MEDIA_BUTTON)
            .setComponent(ComponentName(context, PlaybackService::class.java))
            .putExtra(Intent.EXTRA_KEY_EVENT, KeyEvent(KeyEvent.ACTION_DOWN, keyCode))
            .putExtra(EXTRA_FROM_WIDGET, true)

    /** Start [mixId], or Next up when null (CORE-API `play_mix`). */
    fun playMix(context: Context, mixId: String?): Intent =
        Intent(ACTION_PLAY_MIX)
            .setComponent(ComponentName(context, PlaybackService::class.java))
            // One PendingIntent per mix: the data differs, the extras alone wouldn't.
            .setData(Uri.Builder().scheme(SCHEME).authority("play-mix").appendPath(mixId ?: "next-up").build())
            .putExtra(EXTRA_TOKEN, WidgetToken.get(context))
            .putExtra(EXTRA_FROM_WIDGET, true)
            .apply { if (mixId != null) putExtra(EXTRA_MIX_ID, mixId) }

    /**
     * Phase 9 (WP7): opens [albumId] in the app. [profileId] is the widget's profile: the
     * app opens the album only when that profile is the one open, and only when the album
     * is in its library (MainActivity, EncoreRoot). It is not a secret and changes nothing
     * by itself.
     */
    fun openAlbum(context: Context, albumId: Long, profileId: String): Intent =
        Intent(context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            // One PendingIntent per album: the extras alone wouldn't tell them apart.
            .setData(Uri.Builder().scheme(SCHEME).authority("album").appendPath(albumId.toString()).build())
            .putExtra(MainActivity.EXTRA_OPEN, MainActivity.OPEN_ALBUM)
            .putExtra(MainActivity.EXTRA_ALBUM_ID, albumId)
            .putExtra(MainActivity.EXTRA_PROFILE_ID, profileId)

    /** Opens the app on Now Playing, the profile switcher, or where it was. */
    fun open(context: Context, target: WidgetTarget): Intent =
        Intent(context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            .apply {
                when (target) {
                    WidgetTarget.NOW_PLAYING -> putExtra(MainActivity.EXTRA_OPEN, MainActivity.OPEN_NOW_PLAYING)
                    WidgetTarget.PROFILES -> putExtra(MainActivity.EXTRA_OPEN, MainActivity.OPEN_PROFILES)
                    WidgetTarget.APP -> Unit
                }
            }
}
