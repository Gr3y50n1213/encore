package uk.gcartee.encore.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.graphics.Color
import uk.gcartee.encore.tokens.EncoreTokens.Color.Accents.Iris

/** A `#RRGGBB` token as an opaque colour. */
fun tokenColor(hex: String): Color {
    require(hex.length == 7 && hex[0] == '#') { "Not a #RRGGBB token: $hex" }
    return Color(0xFF000000 or hex.substring(1).toLong(16))
}

/**
 * The Iris accent's light scheme (design/tokens.json `color.accents.iris.light`), the
 * default on Android until a profile picks an accent or an album colour applies.
 */
fun irisLightScheme(): ColorScheme = with(Iris.Light) {
    lightColorScheme(
        primary = tokenColor(primary),
        onPrimary = tokenColor(onPrimary),
        primaryContainer = tokenColor(primaryContainer),
        onPrimaryContainer = tokenColor(onPrimaryContainer),
        inversePrimary = tokenColor(inversePrimary),
        secondary = tokenColor(secondary),
        onSecondary = tokenColor(onSecondary),
        secondaryContainer = tokenColor(secondaryContainer),
        onSecondaryContainer = tokenColor(onSecondaryContainer),
        tertiary = tokenColor(tertiary),
        onTertiary = tokenColor(onTertiary),
        tertiaryContainer = tokenColor(tertiaryContainer),
        onTertiaryContainer = tokenColor(onTertiaryContainer),
        background = tokenColor(surface),
        onBackground = tokenColor(onSurface),
        surface = tokenColor(surface),
        onSurface = tokenColor(onSurface),
        surfaceVariant = tokenColor(surfaceContainerHighest),
        onSurfaceVariant = tokenColor(onSurfaceVariant),
        surfaceTint = tokenColor(primary),
        inverseSurface = tokenColor(inverseSurface),
        inverseOnSurface = tokenColor(inverseOnSurface),
        error = tokenColor(error),
        onError = tokenColor(onError),
        errorContainer = tokenColor(errorContainer),
        onErrorContainer = tokenColor(onErrorContainer),
        outline = tokenColor(outline),
        outlineVariant = tokenColor(outlineVariant),
        scrim = tokenColor(scrim),
        surfaceBright = tokenColor(surfaceBright),
        surfaceContainer = tokenColor(surfaceContainer),
        surfaceContainerHigh = tokenColor(surfaceContainerHigh),
        surfaceContainerHighest = tokenColor(surfaceContainerHighest),
        surfaceContainerLow = tokenColor(surfaceContainerLow),
        surfaceContainerLowest = tokenColor(surfaceContainerLowest),
        surfaceDim = tokenColor(surfaceDim),
    )
}

/** The Iris accent's dark scheme (`color.accents.iris.dark`). */
fun irisDarkScheme(): ColorScheme = with(Iris.Dark) {
    darkColorScheme(
        primary = tokenColor(primary),
        onPrimary = tokenColor(onPrimary),
        primaryContainer = tokenColor(primaryContainer),
        onPrimaryContainer = tokenColor(onPrimaryContainer),
        inversePrimary = tokenColor(inversePrimary),
        secondary = tokenColor(secondary),
        onSecondary = tokenColor(onSecondary),
        secondaryContainer = tokenColor(secondaryContainer),
        onSecondaryContainer = tokenColor(onSecondaryContainer),
        tertiary = tokenColor(tertiary),
        onTertiary = tokenColor(onTertiary),
        tertiaryContainer = tokenColor(tertiaryContainer),
        onTertiaryContainer = tokenColor(onTertiaryContainer),
        background = tokenColor(surface),
        onBackground = tokenColor(onSurface),
        surface = tokenColor(surface),
        onSurface = tokenColor(onSurface),
        surfaceVariant = tokenColor(surfaceContainerHighest),
        onSurfaceVariant = tokenColor(onSurfaceVariant),
        surfaceTint = tokenColor(primary),
        inverseSurface = tokenColor(inverseSurface),
        inverseOnSurface = tokenColor(inverseOnSurface),
        error = tokenColor(error),
        onError = tokenColor(onError),
        errorContainer = tokenColor(errorContainer),
        onErrorContainer = tokenColor(onErrorContainer),
        outline = tokenColor(outline),
        outlineVariant = tokenColor(outlineVariant),
        scrim = tokenColor(scrim),
        surfaceBright = tokenColor(surfaceBright),
        surfaceContainer = tokenColor(surfaceContainer),
        surfaceContainerHigh = tokenColor(surfaceContainerHigh),
        surfaceContainerHighest = tokenColor(surfaceContainerHighest),
        surfaceContainerLow = tokenColor(surfaceContainerLow),
        surfaceContainerLowest = tokenColor(surfaceContainerLowest),
        surfaceDim = tokenColor(surfaceDim),
    )
}

/**
 * Material 3 Expressive with the expressive motion scheme (plan section 16: Android
 * takes its springs from `MotionScheme.expressive()`) and the token colours.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun EncoreTheme(dark: Boolean? = null, content: @Composable () -> Unit) {
    val type = typeForWindow()
    val choice by Appearance.choice.collectAsState()
    val context = LocalContext.current
    val isDark = dark ?: when (choice.mode) {
        ThemeMode.System -> isSystemInDarkTheme()
        ThemeMode.Light -> false
        ThemeMode.Dark -> true
    }
    val scheme = remember(choice.accent, isDark) { accentScheme(context, choice.accent, isDark) }
    MaterialExpressiveTheme(
        colorScheme = scheme,
        motionScheme = MotionScheme.expressive(),
        typography = materialTypography(type),
    ) {
        // Text and icons outside a Surface take the scheme's on-surface colour, never the
        // default black (dark mode showed black titles on Settings and Appearance).
        CompositionLocalProvider(
            LocalEncoreType provides type,
            androidx.compose.material3.LocalContentColor provides scheme.onSurface,
            content = content,
        )
    }
}

/** Material's own components (buttons, fields, chips, menus) set in Inter at the token sizes. */
private fun materialTypography(t: EncoreType): Typography {
    val base = Typography()
    fun TextStyle.inter() = copy(fontFamily = Inter)
    return Typography(
        displayLarge = base.displayLarge.inter(),
        displayMedium = base.displayMedium.inter(),
        displaySmall = t.displayTitle,
        headlineLarge = t.onboardingTitle,
        headlineMedium = t.pageTitle,
        headlineSmall = t.sheetTitle,
        titleLarge = t.nowPlayingTitle,
        titleMedium = t.sectionHeader,
        titleSmall = t.songRowTitle,
        bodyLarge = base.bodyLarge.inter(),
        bodyMedium = t.listRowTitle,
        bodySmall = t.body,
        labelLarge = base.labelLarge.inter(),
        labelMedium = t.buttonLabel,
        labelSmall = t.caption,
    )
}
