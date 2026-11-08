package uk.gcartee.encore.app.ui.components

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

private fun Context.activity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.activity()
    else -> null
}

/**
 * Edge to edge, the status and navigation bar icons sit over whatever the screen draws.
 * Over an album's dark field they must be light, over a light page dark; the previous
 * style comes back when the screen leaves (board 08 and 09 draw the field to the top).
 */
@Composable
fun SystemBarIconsFor(background: Color) {
    val view = LocalView.current
    val themeDark = isSystemInDarkTheme()
    val lightIcons = background.luminance() < 0.5f
    DisposableEffect(lightIcons) {
        val window = view.context.activity()?.window
        val controller = window?.let { WindowCompat.getInsetsController(it, view) }
        val before = controller?.isAppearanceLightStatusBars
        controller?.isAppearanceLightStatusBars = !lightIcons
        onDispose {
            controller?.isAppearanceLightStatusBars = before ?: !themeDark
        }
    }
}

/**
 * The navigation bar as each device's own navigation expects it (Android's edge-to-edge
 * guidance): with gesture navigation the bar is fully clear and the app draws to the
 * bottom edge, the handle over it; with three-button navigation the system's translucent
 * scrim stays so the buttons read over anything. A tablet's taskbar is the system's own
 * and stays as it is. Gesture navigation has no tappable inset at the bottom, buttons do.
 */
@Composable
fun NavigationBarForNavigationMode() {
    val view = LocalView.current
    val density = androidx.compose.ui.platform.LocalDensity.current
    val tappable = androidx.compose.foundation.layout.WindowInsets.tappableElement
    val buttons = tappable.getBottom(density) > 0 ||
        tappable.getLeft(density, androidx.compose.ui.unit.LayoutDirection.Ltr) > 0 ||
        tappable.getRight(density, androidx.compose.ui.unit.LayoutDirection.Ltr) > 0
    androidx.compose.runtime.SideEffect {
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.Q) {
            view.context.activity()?.window?.isNavigationBarContrastEnforced = buttons
        }
    }
}