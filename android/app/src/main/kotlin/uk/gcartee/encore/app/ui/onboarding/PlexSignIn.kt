package uk.gcartee.encore.app.ui.onboarding

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.PlexServerChoice
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/** Where the Plex sign-in is (mock-up 48, board 21 amended): the code, then the server. */
internal sealed interface PlexStep {
    data object Starting : PlexStep
    data class Code(val code: String, val authUrl: String, val expiresAtMs: Long, val copied: Boolean = false) : PlexStep
    data object Expired : PlexStep
    data class Choose(
        val servers: List<PlexServerChoice>,
        val selected: String?,
        val signingIn: Boolean = false,
        val unreachable: Set<String> = emptySet(),
    ) : PlexStep
}

/** Mock-up 48, screen 1: what Plex chosen shows instead of the user name and password. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun PlexStart(starting: Boolean, onStart: () -> Unit) {
    val type = LocalEncoreType.current
    Text(
        "Plex signs you in on its own website. You'll get a short code to enter at plex.tv/link, on this device or any other. Then pick your server.",
        style = type.body,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Spacer(Modifier.height((Grid.base * 3).dp))
    Button(
        onClick = onStart,
        enabled = !starting,
        shapes = ButtonDefaults.shapes(),
        modifier = Modifier.fillMaxWidth().height(BUTTON_DP.dp),
    ) {
        if (starting) {
            LoadingIndicator(Modifier.size(LOADING_DP.dp), color = MaterialTheme.colorScheme.onPrimary)
        } else {
            Text("Sign in with Plex", style = type.sectionHeader.copy(fontSize = 16.sp))
        }
    }
    Spacer(Modifier.height((Grid.base * 2).dp))
    Row(verticalAlignment = Alignment.Top) {
        Icon(Icons.lock(), contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(16.dp).padding(top = 2.dp))
        Spacer(Modifier.size(Grid.base.dp))
        Text(
            "Encore never sees your Plex password. It contacts plex.tv only to sign in and to find your server's address.",
            style = type.caption,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** Mock-up 48, screen 2: the code to enter at plex.tv/link, and the wait. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun PlexCode(step: PlexStep.Code, now: Long, onOpen: () -> Unit, onCopy: () -> Unit, onCancel: () -> Unit) {
    val type = LocalEncoreType.current
    val left = ((step.expiresAtMs - now) / 1000).coerceAtLeast(0)
    Surface(shape = RoundedCornerShape(28.dp), color = MaterialTheme.colorScheme.surfaceContainerHigh, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(vertical = (Grid.base * 3).dp, horizontal = (Grid.base * 2).dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Text("Your code", style = type.caption, color = MaterialTheme.colorScheme.onSurfaceVariant)
            // Spaced so it reads off easily onto the other device.
            Text(step.code, style = type.displayTitle.copy(letterSpacing = 8.sp), color = MaterialTheme.colorScheme.onSurface, modifier = Modifier.padding(vertical = Grid.base.dp))
            Text("plex.tv/link", style = type.sectionHeader, color = MaterialTheme.colorScheme.onSurface)
        }
    }
    Spacer(Modifier.height((Grid.base * 2).dp))
    Surface(shape = RoundedCornerShape(16.dp), color = MaterialTheme.colorScheme.secondaryContainer, modifier = Modifier.fillMaxWidth()) {
        Row(Modifier.padding(horizontal = (Grid.base * 2).dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            LoadingIndicator(Modifier.size(24.dp), color = MaterialTheme.colorScheme.onSecondaryContainer)
            Spacer(Modifier.size(12.dp))
            Text("Waiting for you to sign in", style = type.body, color = MaterialTheme.colorScheme.onSecondaryContainer, modifier = Modifier.weight(1f))
            Text("%d:%02d".format(left / 60, left % 60), style = type.caption, color = MaterialTheme.colorScheme.onSecondaryContainer)
        }
    }
    Spacer(Modifier.height((Grid.base * 3).dp))
    Button(onClick = onOpen, shapes = ButtonDefaults.shapes(), modifier = Modifier.fillMaxWidth().height(BUTTON_DP.dp)) {
        Icon(Icons.openInNew(), contentDescription = null, modifier = Modifier.size(20.dp))
        Spacer(Modifier.size(Grid.base.dp))
        Text("Open Plex sign-in here", style = type.sectionHeader.copy(fontSize = 16.sp))
    }
    Spacer(Modifier.height(Grid.base.dp))
    FilledTonalButton(onClick = onCopy, shapes = ButtonDefaults.shapes(), modifier = Modifier.fillMaxWidth().height(BUTTON_DP.dp)) {
        Icon(if (step.copied) Icons.check() else Icons.copy(), contentDescription = null, modifier = Modifier.size(20.dp))
        Spacer(Modifier.size(Grid.base.dp))
        Text(if (step.copied) "Code copied" else "Copy code", style = type.sectionHeader.copy(fontSize = 16.sp))
    }
    TextButton(onClick = onCancel, modifier = Modifier.fillMaxWidth()) { Text("Cancel") }
}

/** An expired code: start again. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun PlexExpired(onAgain: () -> Unit, onCancel: () -> Unit) {
    val type = LocalEncoreType.current
    Text("The code ran out before it was used.", style = type.body, color = MaterialTheme.colorScheme.onSurfaceVariant)
    Spacer(Modifier.height((Grid.base * 3).dp))
    Button(onClick = onAgain, shapes = ButtonDefaults.shapes(), modifier = Modifier.fillMaxWidth().height(BUTTON_DP.dp)) {
        Text("Get a new code", style = type.sectionHeader.copy(fontSize = 16.sp))
    }
    TextButton(onClick = onCancel, modifier = Modifier.fillMaxWidth()) { Text("Cancel") }
}

/** Mock-up 48, screen 3: more than one server on the account. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun PlexChoose(step: PlexStep.Choose, onSelect: (String) -> Unit, onContinue: () -> Unit) {
    val type = LocalEncoreType.current
    Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
        step.servers.forEachIndexed { i, s ->
            val on = s.serverId == step.selected
            val down = s.serverId in step.unreachable
            val big = 20.dp
            val small = 8.dp
            val shape = RoundedCornerShape(
                topStart = if (i == 0) big else small, topEnd = if (i == 0) big else small,
                bottomStart = if (i == step.servers.lastIndex) big else small, bottomEnd = if (i == step.servers.lastIndex) big else small,
            )
            Surface(
                shape = shape,
                color = if (on) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh,
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = 68.dp)
                    .alpha(if (down) 0.55f else 1f)
                    .selectable(selected = on, enabled = !step.signingIn, role = Role.RadioButton) { onSelect(s.serverId) },
            ) {
                Row(Modifier.padding(horizontal = 14.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    val ring = if (on) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline
                    Box(Modifier.size(20.dp).border(if (on) 6.dp else 2.dp, ring, CircleShape))
                    Spacer(Modifier.size(12.dp))
                    Column(Modifier.weight(1f)) {
                        val fg = if (on) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurface
                        Text(s.name, style = type.body.copy(fontSize = 15.sp), color = fg)
                        val whose = if (s.owned) "Your server" else "Shared with you"
                        Text(
                            if (down) "$whose · Not reachable right now" else "$whose · Plex ${s.version}",
                            style = type.caption,
                            color = fg.copy(alpha = 0.8f),
                        )
                    }
                }
            }
        }
    }
    Spacer(Modifier.height((Grid.base * 3).dp))
    Button(
        onClick = onContinue,
        enabled = step.selected != null && !step.signingIn,
        shapes = ButtonDefaults.shapes(),
        modifier = Modifier.fillMaxWidth().height(BUTTON_DP.dp),
    ) {
        if (step.signingIn) {
            LoadingIndicator(Modifier.size(LOADING_DP.dp), color = MaterialTheme.colorScheme.onPrimary)
        } else {
            Text(if (step.selected in step.unreachable) "Try again" else "Continue", style = type.sectionHeader.copy(fontSize = 16.sp))
        }
    }
}

/** The plain sentence for a Plex error, in the live region under the steps. */
@Composable
internal fun PlexError(message: String) {
    Text(
        message,
        style = LocalEncoreType.current.caption,
        color = MaterialTheme.colorScheme.error,
        textAlign = TextAlign.Start,
        modifier = Modifier.padding(top = Grid.base.dp).semantics { liveRegion = LiveRegionMode.Assertive },
    )
}

private const val BUTTON_DP = 56
private const val LOADING_DP = 28
