package uk.gcartee.encore.app.widgets

import android.content.Context
import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject

/** A widget's Profile setting (PLAN-UX section 20): Active profile, or one fixed profile. */
sealed interface WidgetProfileChoice {
    data object Active : WidgetProfileChoice
    data class Fixed(val profileId: String) : WidgetProfileChoice
}

/** A profile on this device, as the app last saw it (for the configuration screen). */
data class KnownProfile(val id: String, val name: String, val locked: Boolean)

/**
 * What the app last knew about profiles, kept so a widget never has to start the core:
 * [current] is the profile open now (null when none is), [lastActive] the one the
 * playback service opens when it starts with the app closed (`last_active_profile`).
 */
data class ProfileState(
    val current: String?,
    val lastActive: String?,
    val profiles: List<KnownProfile>,
)

/**
 * Per-widget settings, kept by appWidgetId in the app's private preferences. Only
 * WidgetSync (in the app's process, with the core running) writes [ProfileState]; widgets
 * and the configuration screen only read it.
 */
object WidgetConfig {
    private const val PREFS = "widget-config"
    private const val KEY_PROFILE = "profile-"

    private const val STATE_PREFS = "widget-profiles"
    private const val KEY_CURRENT = "current"
    private const val KEY_LAST_ACTIVE = "last-active"
    private const val KEY_PROFILES = "profiles"

    fun choice(context: Context, appWidgetId: Int): WidgetProfileChoice {
        val id = prefs(context).getString(KEY_PROFILE + appWidgetId, null)
        return if (id.isNullOrEmpty()) WidgetProfileChoice.Active else WidgetProfileChoice.Fixed(id)
    }

    fun setChoice(context: Context, appWidgetId: Int, choice: WidgetProfileChoice) {
        val value = when (choice) {
            WidgetProfileChoice.Active -> ""
            is WidgetProfileChoice.Fixed -> choice.profileId
        }
        prefs(context).edit().putString(KEY_PROFILE + appWidgetId, value).apply()
    }

    /** The widget was removed from the home screen. */
    fun forget(context: Context, appWidgetIds: IntArray) {
        val edit = prefs(context).edit()
        appWidgetIds.forEach { edit.remove(KEY_PROFILE + it) }
        edit.apply()
    }

    fun profileState(context: Context): ProfileState {
        val p = context.getSharedPreferences(STATE_PREFS, Context.MODE_PRIVATE)
        return ProfileState(
            current = p.getString(KEY_CURRENT, null),
            lastActive = p.getString(KEY_LAST_ACTIVE, null),
            profiles = decodeProfiles(p.getString(KEY_PROFILES, null)),
        )
    }

    /** From WidgetSync only. */
    fun saveProfileState(context: Context, state: ProfileState) {
        context.getSharedPreferences(STATE_PREFS, Context.MODE_PRIVATE).edit()
            .putString(KEY_CURRENT, state.current)
            .putString(KEY_LAST_ACTIVE, state.lastActive)
            .putString(KEY_PROFILES, encodeProfiles(state.profiles))
            .apply()
    }

    /**
     * Pure: the profile whose file a widget reads. Active is the profile open now, else
     * the last active one; a fixed profile is always itself, whatever is open, so a widget
     * set to one profile never shows another's.
     */
    fun resolve(choice: WidgetProfileChoice, state: ProfileState): String? = when (choice) {
        WidgetProfileChoice.Active -> state.current ?: state.lastActive
        is WidgetProfileChoice.Fixed -> choice.profileId
    }

    /**
     * Pure: whether the widget's buttons may go straight to the playback service. Only
     * when the service would play [profileId] without a switch (it's the last active
     * profile, and no other is open now) and it has no PIN. Otherwise a tap opens the app
     * (the plan: a tap never switches profiles silently).
     */
    fun canControl(profileId: String, state: ProfileState): Boolean =
        profileId == state.lastActive &&
            (state.current == null || state.current == profileId) &&
            state.profiles.none { it.id == profileId && it.locked }

    fun encodeProfiles(profiles: List<KnownProfile>): String {
        val array = JSONArray()
        profiles.forEach { array.put(JSONObject().put("id", it.id).put("name", it.name).put("locked", it.locked)) }
        return array.toString()
    }

    fun decodeProfiles(text: String?): List<KnownProfile> {
        if (text.isNullOrEmpty()) return emptyList()
        return try {
            val array = JSONArray(text)
            (0 until array.length()).mapNotNull { i ->
                val o = array.optJSONObject(i) ?: return@mapNotNull null
                val id = if (o.isNull("id")) null else o.optString("id")
                id?.let { KnownProfile(it, o.optString("name", ""), o.optBoolean("locked", false)) }
            }
        } catch (e: JSONException) {
            emptyList()
        }
    }

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
}
