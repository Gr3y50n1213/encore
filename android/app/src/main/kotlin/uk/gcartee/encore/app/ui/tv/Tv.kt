package uk.gcartee.encore.app.ui.tv

import android.app.UiModeManager
import android.content.Context
import android.content.res.Configuration
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.LocalRippleThemeConfiguration
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RippleThemeConfiguration
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.io.File
import uk.gcartee.encore.app.R

/*
 * Encore TV (the projector build, mock-up 45, approved by the owner on 23 November 2026). The
 * "tv" flavor is the same app with a TV launcher entry; everything that differs on a TV asks
 * [isTv] or [LocalTv], so the phone app is unchanged.
 */

/** The TV build, or any device Android calls a television. */
fun isTv(context: Context): Boolean {
    if (context.resources.getBoolean(R.bool.tv_build)) return true
    val ui = context.getSystemService(UiModeManager::class.java)
    return ui?.currentModeType == Configuration.UI_MODE_TYPE_TELEVISION
}

/** True on a TV: wide layouts, the remote's focus ring, still backgrounds, no sound analysis. */
val LocalTv = staticCompositionLocalOf { false }

/**
 * The remote's focus: Material 3's own focus ring (two strokes, so it shows on light and dark
 * covers alike) on everything that ripples, instead of the faint tint a phone needs for a
 * keyboard. Nothing changes on a phone.
 */
@Composable
fun TvFocus(content: @Composable () -> Unit) {
    if (!LocalTv.current) {
        content()
        return
    }
    CompositionLocalProvider(
        LocalRippleThemeConfiguration provides RippleThemeConfiguration(
            RippleThemeConfiguration.Focus.InsetRing(0.dp, RING_OUTER_DP.dp, RING_OUTER_DP.dp, RING_INNER_DP.dp),
        ),
        content = content,
    )
}

private const val RING_OUTER_DP = 3
private const val RING_INNER_DP = 2

// --- The crash report. A projector can't be reached for its logs, so on a TV a crash is
// written to a file and shown, a few lines, the next time the app opens. ---

private const val CRASH_FILE = "last-crash.txt"

/** From Application.onCreate, on a TV only. Android's own handler still runs after. */
fun recordCrashes(context: Context) {
    val app = context.applicationContext
    val previous = Thread.getDefaultUncaughtExceptionHandler()
    Thread.setDefaultUncaughtExceptionHandler { thread, e ->
        runCatching {
            val trace = e.stackTraceToString().lineSequence().take(CRASH_LINES).joinToString("\n")
            File(app.filesDir, CRASH_FILE).writeText("${java.util.Date()}\nThread: ${thread.name}\n$trace")
        }
        previous?.uncaughtException(thread, e)
    }
}

/** The last crash, once: shown at launch, then deleted. */
@Composable
fun CrashReport() {
    val context = LocalContext.current
    var text by remember {
        mutableStateOf(
            runCatching {
                val f = File(context.filesDir, CRASH_FILE)
                f.takeIf { it.isFile }?.readText()?.also { f.delete() }
            }.getOrNull(),
        )
    }
    val t = text ?: return
    AlertDialog(
        onDismissRequest = { text = null },
        title = { Text("Encore closed unexpectedly last time") },
        text = {
            Column(Modifier.heightIn(max = 320.dp).verticalScroll(rememberScrollState())) {
                Text("Read these lines out, or take a photo, so it can be fixed:", style = MaterialTheme.typography.bodyMedium)
                Text(t, fontFamily = FontFamily.Monospace, fontSize = 11.sp, lineHeight = 14.sp)
            }
        },
        confirmButton = { TextButton(onClick = { text = null }) { Text("OK") } },
    )
}

private const val CRASH_LINES = 24

/** The rolling playback cache on a TV: the projector has 8 GB in all, shared with the system. */
const val TV_CACHE_LIMIT_BYTES = 300_000_000L
