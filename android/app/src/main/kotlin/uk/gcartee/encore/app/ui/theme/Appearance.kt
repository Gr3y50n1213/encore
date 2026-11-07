package uk.gcartee.encore.app.ui.theme

import android.content.Context
import android.os.Build
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

enum class ThemeMode(val label: String) { System("Automatic"), Light("Light"), Dark("Dark") }

/** The eight profile accents of design/tokens.json `color.accents`, plus the wallpaper's colours. */
val ACCENTS = listOf("Iris", "Cobalt", "Lagoon", "Fern", "Amber", "Coral", "Rose", "Graphite")
const val WALLPAPER = "Wallpaper"

data class AppearanceChoice(val mode: ThemeMode = ThemeMode.System, val accent: String = "Iris")

/**
 * Board 17, Appearance: automatic, light or dark, and the accent, kept per profile
 * (board 14: each profile has its own settings). Changes apply at once.
 */
object Appearance {
    private val _choice = MutableStateFlow(AppearanceChoice())
    val choice: StateFlow<AppearanceChoice> = _choice.asStateFlow()
    private var prefsName = "appearance"

    fun load(context: Context, profileId: String?) {
        prefsName = "appearance-${profileId.orEmpty()}"
        val p = context.getSharedPreferences(prefsName, Context.MODE_PRIVATE)
        _choice.value = AppearanceChoice(
            mode = runCatching { ThemeMode.valueOf(p.getString("mode", null)!!) }.getOrDefault(ThemeMode.System),
            accent = p.getString("accent", null)?.takeIf { it in ACCENTS || it == WALLPAPER } ?: "Iris",
        )
    }

    fun set(context: Context, choice: AppearanceChoice) {
        _choice.value = choice
        context.getSharedPreferences(prefsName, Context.MODE_PRIVATE).edit()
            .putString("mode", choice.mode.name)
            .putString("accent", choice.accent)
            .apply()
    }
}

/** Reads one accent's roles from the generated constants (`EncoreTokens.Color.Accents.<Name>.<Light|Dark>`). */
private fun roles(accent: String, dark: Boolean): (String) -> androidx.compose.ui.graphics.Color {
    val cls = Class.forName("uk.gcartee.encore.tokens.EncoreTokens\$Color\$Accents\$$accent\$${if (dark) "Dark" else "Light"}")
    val instance = cls.getField("INSTANCE").get(null)
    return { role ->
        val hex = runCatching { cls.getField(role).get(null) as String }.getOrElse { cls.getMethod("get" + role.replaceFirstChar(Char::uppercase)).invoke(instance) as String }
        tokenColor(hex)
    }
}

fun accentScheme(context: Context, accent: String, dark: Boolean): ColorScheme {
    if (accent == WALLPAPER && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        return if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
    }
    if (accent == "Iris" || accent == WALLPAPER) return if (dark) irisDarkScheme() else irisLightScheme()
    val r = runCatching { roles(accent, dark) }.getOrElse { return if (dark) irisDarkScheme() else irisLightScheme() }
    val s = if (dark) darkColorScheme() else lightColorScheme()
    return s.copy(
        primary = r("primary"), onPrimary = r("onPrimary"), primaryContainer = r("primaryContainer"), onPrimaryContainer = r("onPrimaryContainer"),
        inversePrimary = r("inversePrimary"), secondary = r("secondary"), onSecondary = r("onSecondary"),
        secondaryContainer = r("secondaryContainer"), onSecondaryContainer = r("onSecondaryContainer"),
        tertiary = r("tertiary"), onTertiary = r("onTertiary"), tertiaryContainer = r("tertiaryContainer"), onTertiaryContainer = r("onTertiaryContainer"),
        background = r("surface"), onBackground = r("onSurface"), surface = r("surface"), onSurface = r("onSurface"),
        surfaceVariant = r("surfaceContainerHighest"), onSurfaceVariant = r("onSurfaceVariant"), surfaceTint = r("primary"),
        inverseSurface = r("inverseSurface"), inverseOnSurface = r("inverseOnSurface"),
        error = r("error"), onError = r("onError"), errorContainer = r("errorContainer"), onErrorContainer = r("onErrorContainer"),
        outline = r("outline"), outlineVariant = r("outlineVariant"), scrim = r("scrim"),
        surfaceBright = r("surfaceBright"), surfaceContainer = r("surfaceContainer"), surfaceContainerHigh = r("surfaceContainerHigh"),
        surfaceContainerHighest = r("surfaceContainerHighest"), surfaceContainerLow = r("surfaceContainerLow"),
        surfaceContainerLowest = r("surfaceContainerLowest"), surfaceDim = r("surfaceDim"),
    )
}
