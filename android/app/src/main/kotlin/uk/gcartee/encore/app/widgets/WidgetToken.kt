package uk.gcartee.encore.app.widgets

import android.content.Context
import java.security.MessageDigest
import java.security.SecureRandom

/**
 * The per-install secret our own widget and tile intents carry (the plan's Security row).
 * The playback service is exported for Android Auto, so any app could start it; it acts on
 * a private action (start a mix) only when the intent carries this secret. Made once with
 * SecureRandom, kept in the app's private preferences (backups are off), and never logged.
 * Other apps can't read it: it travels inside our immutable PendingIntents, whose extras
 * only the creating app can see.
 */
object WidgetToken {
    private const val PREFS = "widget-token"
    private const val KEY = "token"
    private const val BYTES = 32

    @Synchronized
    fun get(context: Context): String {
        val prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        prefs.getString(KEY, null)?.takeIf { it.isNotEmpty() }?.let { return it }
        val bytes = ByteArray(BYTES).also { SecureRandom().nextBytes(it) }
        val token = bytes.joinToString("") { "%02x".format(it) }
        prefs.edit().putString(KEY, token).commit()
        return token
    }

    fun matches(context: Context, presented: String?): Boolean = matches(get(context), presented)

    /** Pure, constant time. */
    fun matches(expected: String, presented: String?): Boolean =
        presented != null && expected.isNotEmpty() &&
            MessageDigest.isEqual(expected.toByteArray(Charsets.UTF_8), presented.toByteArray(Charsets.UTF_8))
}
