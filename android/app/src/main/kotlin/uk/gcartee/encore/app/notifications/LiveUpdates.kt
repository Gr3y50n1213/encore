package uk.gcartee.encore.app.notifications

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.os.Build
import android.provider.Settings
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat

/**
 * Phase 9: Live Updates (Android 16 QPR1, API 36.1 and later). A task the person started
 * (a download now, the sleep timer later) can ask Android to promote its ongoing
 * notification: pinned at the top, on the lock screen, and a short chip in the status bar.
 * Everywhere else the same notification stays a plain ongoing one.
 *
 * Rules kept here so every caller behaves the same:
 * - The toggles below control promotion only. The notification itself is always posted.
 * - Only work the person started may be promoted, never automatic or Smart downloads.
 * - With "Hide listening history while locked" on (the default), the lock screen shows a
 *   neutral line, not what the person listens to.
 */
object LiveUpdates {
    private const val PREFS = "live-updates"
    private const val KEY_SLEEP = "sleep-timer"
    private const val KEY_DOWNLOADS = "downloads"
    private const val KEY_HIDE_LOCKED = "hide-history-locked"

    /** Chip text must stay under this many characters to fit the status bar. */
    const val CHIP_MAX_CHARS = 6

    private const val PROMOTION_SETTINGS_ACTION = "android.settings.APP_NOTIFICATION_PROMOTION_SETTINGS"

    /** Which card a promotion belongs to, for its toggle. */
    enum class Card { SLEEP_TIMER, DOWNLOADS }

    /**
     * Pure rule for the API gate: Android 16 QPR1 is SDK_INT 36 with SDK_INT_FULL at
     * [Build.VERSION_CODES_FULL.BAKLAVA_1]. [fullSdk] is only read when [sdk] is 36 or more.
     */
    fun apiAllowsPromotion(sdk: Int, fullSdk: Int, minFull: Int): Boolean = sdk >= 36 && fullSdk >= minFull

    /** Pure rule: promote only on a new enough Android, for the person's own task, with the card's toggle on. */
    fun shouldPromote(apiAllows: Boolean, userInitiated: Boolean, toggleOn: Boolean): Boolean =
        apiAllows && userInitiated && toggleOn

    /** Short chip text for a fraction done, like "62%". Always [CHIP_MAX_CHARS] characters or fewer. */
    fun chipText(percent: Int): String = "${percent.coerceIn(0, 100)}%"

    /** This device runs Android 16 QPR1 or later. */
    @android.annotation.SuppressLint("NewApi")
    fun apiAllows(): Boolean =
        Build.VERSION.SDK_INT >= 36 &&
            apiAllowsPromotion(Build.VERSION.SDK_INT, Build.VERSION.SDK_INT_FULL, Build.VERSION_CODES_FULL.BAKLAVA_1)

    /** Live Updates can show right now: the API is new enough and the person hasn't switched them off for the app in Settings. */
    fun canPromote(context: Context): Boolean =
        apiAllows() && NotificationManagerCompat.from(context).canPostPromotedNotifications()

    fun isOn(context: Context, card: Card): Boolean =
        prefs(context).getBoolean(key(card), true)

    fun setOn(context: Context, card: Card, on: Boolean) {
        prefs(context).edit().putBoolean(key(card), on).apply()
    }

    /** "Hide listening history while locked", on by default. */
    fun hideHistoryWhileLocked(context: Context): Boolean =
        prefs(context).getBoolean(KEY_HIDE_LOCKED, true)

    fun setHideHistoryWhileLocked(context: Context, on: Boolean) {
        prefs(context).edit().putBoolean(KEY_HIDE_LOCKED, on).apply()
    }

    /**
     * Asks Android to promote [builder]'s notification, with [chip] in the status bar, when
     * the API allows it, [card]'s toggle is on and the person started the task. Returns
     * whether promotion was requested. The caller keeps the notification ongoing, titled,
     * not colorized and without custom views, which Android requires.
     */
    fun applyPromotion(
        context: Context,
        builder: NotificationCompat.Builder,
        card: Card,
        userInitiated: Boolean,
        chip: String,
    ): Boolean {
        if (!shouldPromote(apiAllows(), userInitiated, isOn(context, card))) return false
        builder.setRequestPromotedOngoing(true).setShortCriticalText(chip.take(CHIP_MAX_CHARS))
        return true
    }

    /**
     * The intent for the app's Live Updates settings page, or the app's notification
     * settings where that page doesn't exist.
     */
    fun settingsIntent(context: Context): Intent =
        Intent(PROMOTION_SETTINGS_ACTION).putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)

    private fun fallbackIntent(context: Context): Intent =
        Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)

    /** The "Open settings" button: the Live Updates page, else the app's notification settings. */
    fun openSettings(context: Context) {
        val flags = Intent.FLAG_ACTIVITY_NEW_TASK
        try {
            context.startActivity(settingsIntent(context).addFlags(flags))
        } catch (e: ActivityNotFoundException) {
            runCatching { context.startActivity(fallbackIntent(context).addFlags(flags)) }
        }
    }

    private fun key(card: Card) = if (card == Card.SLEEP_TIMER) KEY_SLEEP else KEY_DOWNLOADS

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
}
