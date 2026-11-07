package uk.gcartee.encore.app.ui.components

import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.Stable
import androidx.compose.runtime.staticCompositionLocalOf
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uk.gcartee.encore.tokens.EncoreTokens.Motion.Timing

/**
 * The toasts of boards 11, 13 and 42: one line, with Undo for 5 s
 * (`motion.timing.undoToastMs`) when the action can be undone. Announced to TalkBack by
 * the snackbar itself and never traps focus (board 20).
 */
@Stable
class Toaster(val host: SnackbarHostState, private val scope: CoroutineScope) {
    private var job: Job? = null

    fun show(message: String, undo: (() -> Unit)? = null) {
        job?.cancel()
        host.currentSnackbarData?.dismiss()
        job = scope.launch {
            val timeout = launch {
                delay(Timing.undoToastMs.toLong())
                host.currentSnackbarData?.dismiss()
            }
            val result = host.showSnackbar(
                message = message,
                actionLabel = if (undo != null) "Undo" else null,
                duration = SnackbarDuration.Indefinite,
            )
            timeout.cancel()
            if (result == SnackbarResult.ActionPerformed) undo?.invoke()
        }
    }
}

val LocalToaster = staticCompositionLocalOf<Toaster> { error("No Toaster provided") }
