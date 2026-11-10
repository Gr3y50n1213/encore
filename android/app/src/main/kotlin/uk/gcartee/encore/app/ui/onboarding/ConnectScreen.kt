package uk.gcartee.encore.app.ui.onboarding

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.keyframes
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ToggleButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.material3.ButtonDefaults
import androidx.compose.foundation.background
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.sp
import androidx.compose.ui.autofill.ContentType
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentType
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.TrustedCertificates
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.ui.components.Haptic
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.rememberHaptics
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.app.ui.theme.tokenColor
import uk.gcartee.encore.core.EncoreException
import uk.gcartee.encore.core.ServerCheck
import uk.gcartee.encore.core.SignedIn
import uk.gcartee.encore.tokens.EncoreTokens.Color.Accents.Fern
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Motion.BlockedActionShake
import uk.gcartee.encore.tokens.EncoreTokens.TouchTarget

private sealed interface Check {
    data object Idle : Check
    data object Checking : Check
    data class Found(val server: ServerCheck) : Check
    data class Untrusted(val address: String, val sha256: String) : Check
    data class Failed(val message: String) : Check
}

private data class QuickConnect(val code: String, val expiresAtMs: Long)

/**
 * Board 21, Connect your server. The steps counter counts the steps that exist: board
 * 21 draws four, but "Choose what to include" needs a core call that is not there yet
 * and the companion step arrives in Phase 4.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ConnectScreen(host: CoreHost, step: Int, steps: Int, onBack: () -> Unit, onSignedIn: (SignedIn) -> Unit) {
    val type = LocalEncoreType.current
    val haptics = rememberHaptics()
    val scope = rememberCoroutineScope()
    val focus = LocalFocusManager.current
    val dark = isSystemInDarkTheme()

    // Item 1: Back keeps what was typed. The password is never saved, not even in state.
    var address by rememberSaveable { mutableStateOf("") }
    var username by rememberSaveable { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var showPassword by remember { mutableStateOf(false) }
    var check by remember { mutableStateOf<Check>(Check.Idle) }
    var signingIn by remember { mutableStateOf(false) }
    var signInError by remember { mutableStateOf<String?>(null) }
    var quick by remember { mutableStateOf<QuickConnect?>(null) }
    var quickJob by remember { mutableStateOf<Job?>(null) }
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    val shake = remember { Animatable(0f) }

    // Item 3: checks once the user stops typing for 600 ms.
    LaunchedEffect(address) {
        val typed = address.trim()
        if (typed.isEmpty()) {
            check = Check.Idle
            return@LaunchedEffect
        }
        delay(CHECK_AFTER_MS)
        check = Check.Checking
        check = runCatching { host.core.checkAddress(typed) }.fold(
            onSuccess = { Check.Found(it) },
            onFailure = { e ->
                if (e is EncoreException.CertificateUntrusted) Check.Untrusted(typed, e.sha256) else Check.Failed(e.userMessage())
            },
        )
    }
    LaunchedEffect(quick) {
        while (quick != null) {
            now = System.currentTimeMillis()
            delay(1_000)
        }
    }

    val found = (check as? Check.Found)?.server

    suspend fun wrongPassword(message: String) {
        haptics.perform(Haptic.Reject)
        password = ""
        signInError = message
        val amp = BlockedActionShake.amplitude.toFloat()
        val step = BlockedActionShake.durationMs / (BlockedActionShake.cycles * 2)
        shake.animateTo(0f, keyframes {
            durationMillis = BlockedActionShake.durationMs
            for (i in 0 until BlockedActionShake.cycles) {
                amp at step * (2 * i)
                -amp at step * (2 * i + 1)
            }
        })
    }

    fun signIn() {
        val server = found ?: return
        if (signingIn || username.isBlank()) return
        signingIn = true
        signInError = null
        focus.clearFocus()
        scope.launch {
            runCatching { host.core.signIn(server.baseUrl, username.trim(), password) }
                .onSuccess {
                    password = ""
                    haptics.perform(Haptic.Confirm)
                    onSignedIn(it)
                }
                .onFailure { e ->
                    if (e is EncoreException.WrongCredentials) wrongPassword(e.userMessage())
                    else {
                        haptics.perform(Haptic.Reject)
                        signInError = e.userMessage()
                    }
                }
            signingIn = false
        }
    }

    fun startQuickConnect() {
        val server = found ?: return
        signInError = null
        quickJob = scope.launch {
            runCatching { host.core.quickConnectStart(server.baseUrl) }
                .onSuccess { start ->
                    quick = QuickConnect(start.code, System.currentTimeMillis() + start.expiresInSeconds.toLong() * 1000)
                    runCatching { host.core.quickConnectWait() }
                        .onSuccess {
                            quick = null
                            haptics.perform(Haptic.Confirm)
                            onSignedIn(it)
                        }
                        .onFailure { e ->
                            quick = null
                            if (e !is EncoreException.QuickConnectCancelled) signInError = e.userMessage()
                        }
                }
                .onFailure { e ->
                    haptics.perform(Haptic.Reject)
                    signInError = e.userMessage()
                }
        }
    }

    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        Column(
            Modifier
                .fillMaxSize()
                .safeDrawingPadding()
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = (Grid.base * 3).dp, vertical = (Grid.base * 2).dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Column(Modifier.widthIn(max = 560.dp).fillMaxWidth()) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    FilledTonalIconButton(
                        onClick = onBack,
                        shapes = androidx.compose.material3.IconButtonDefaults.shapes(RoundedCornerShape(12.dp), RoundedCornerShape(50)),
                        modifier = Modifier.size((TouchTarget.android).dp),
                    ) { Icon(Icons.back(), contentDescription = "Back") }
                    Spacer(Modifier.weight(1f))
                    // Item 1: where you are in setup, as a tonal pill.
                    Surface(shape = RoundedCornerShape(50), color = MaterialTheme.colorScheme.secondaryContainer) {
                        Text("Step $step of $steps", style = type.buttonLabelSelected, color = MaterialTheme.colorScheme.onSecondaryContainer, modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp))
                    }
                }
                Spacer(Modifier.height((Grid.base * 3).dp))
                // A small expressive mark for the step: the server in a cookie shape.
                Box(
                    Modifier.size(HERO_DP.dp).clip(uk.gcartee.encore.app.ui.home.PolygonShape(androidx.compose.material3.MaterialShapes.Cookie9Sided)).background(MaterialTheme.colorScheme.primaryContainer),
                    contentAlignment = Alignment.Center,
                ) { Icon(Icons.servers(), contentDescription = null, tint = MaterialTheme.colorScheme.onPrimaryContainer, modifier = Modifier.size(32.dp)) }
                Spacer(Modifier.height((Grid.base * 2).dp))
                Text("Connect your server", style = type.onboardingTitle, color = MaterialTheme.colorScheme.onSurface)
                Spacer(Modifier.height(Grid.base.dp))
                Text(
                    "Your music stays on your server. This app only plays it.",
                    style = type.body.copy(fontSize = 15.sp, lineHeight = 21.sp),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height((Grid.base * 3).dp))

                // Item 2: server type, a connected button group. Only Jellyfin works today.
                uk.gcartee.encore.app.ui.library.ChoiceGroup(
                    options = listOf(0 to "Jellyfin", 1 to "Plex (soon)", 2 to "Other"),
                    selected = 0,
                    onSelect = { },
                    enabled = { it == 0 },
                )
                Text(
                    "Other servers are added through adapters as they are built.",
                    style = type.caption,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = Grid.base.dp, start = 4.dp),
                )
                Spacer(Modifier.height((Grid.base * 3).dp))

                // Item 3: the address and what was found there.
                OutlinedTextField(
                    value = address,
                    onValueChange = { address = it },
                    label = { Text("Server address") },
                    placeholder = { Text("https://music.example.home") },
                    leadingIcon = { Icon(Icons.servers(), contentDescription = null) },
                    shape = RoundedCornerShape(FIELD_RADIUS_DP.dp),
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Next, autoCorrectEnabled = false),
                    keyboardActions = KeyboardActions(onNext = { focus.moveFocus(FocusDirection.Down) }),
                    modifier = Modifier.fillMaxWidth().testTag("address"),
                )
                Box(Modifier.padding(top = Grid.base.dp, start = 4.dp).semantics { liveRegion = LiveRegionMode.Polite }) {
                    when (val c = check) {
                        Check.Idle -> Unit
                        Check.Checking -> Text("Checking…", style = type.caption, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        is Check.Found -> Text(
                            if (c.server.plainHttpOnLocalNetwork) {
                                "Found Jellyfin ${c.server.version} on your home network. The connection is not encrypted."
                            } else {
                                "Found Jellyfin ${c.server.version}. Connection is secure."
                            },
                            style = type.caption,
                            color = if (c.server.plainHttpOnLocalNetwork) MaterialTheme.colorScheme.onSurfaceVariant
                            else tokenColor(if (dark) Fern.Dark.primary else Fern.Light.primary),
                        )
                        is Check.Untrusted -> Text("This server's certificate is not trusted yet.", style = type.caption, color = MaterialTheme.colorScheme.error)
                        is Check.Failed -> Text(c.message, style = type.caption, color = MaterialTheme.colorScheme.error)
                    }
                }
                Spacer(Modifier.height((Grid.base * 2).dp))

                // Item 4: credentials. Autofill works; nothing but the token is stored.
                Column(Modifier.graphicsLayer { translationX = shake.value * density }) {
                    OutlinedTextField(
                        value = username,
                        onValueChange = { username = it },
                        label = { Text("Username") },
                        leadingIcon = { Icon(Icons.artist(), contentDescription = null) },
                        shape = RoundedCornerShape(FIELD_RADIUS_DP.dp),
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next, autoCorrectEnabled = false),
                        keyboardActions = KeyboardActions(onNext = { focus.moveFocus(FocusDirection.Down) }),
                        modifier = Modifier.fillMaxWidth().semantics { contentType = ContentType.Username },
                    )
                    Spacer(Modifier.height((Grid.base * 2).dp))
                    OutlinedTextField(
                        value = password,
                        onValueChange = { password = it },
                        label = { Text("Password") },
                        leadingIcon = { Icon(Icons.lock(), contentDescription = null) },
                        shape = RoundedCornerShape(FIELD_RADIUS_DP.dp),
                        singleLine = true,
                        visualTransformation = if (showPassword) VisualTransformation.None else PasswordVisualTransformation(),
                        trailingIcon = {
                            IconButton(onClick = { showPassword = !showPassword }, shapes = androidx.compose.material3.IconButtonDefaults.shapes()) {
                                Icon(
                                    if (showPassword) Icons.visibilityOff() else Icons.visibility(),
                                    contentDescription = if (showPassword) "Hide password" else "Show password",
                                )
                            }
                        },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
                        keyboardActions = KeyboardActions(onDone = { signIn() }),
                        modifier = Modifier.fillMaxWidth().semantics { contentType = ContentType.Password },
                    )
                }
                signInError?.let {
                    Text(
                        it,
                        style = type.caption,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = Grid.base.dp).semantics { liveRegion = LiveRegionMode.Assertive },
                    )
                }
                Spacer(Modifier.height((Grid.base * 2).dp))

                // Item 5: a spinner inside the button, disabled while it works (no double submit).
                Button(
                    onClick = ::signIn,
                    enabled = found != null && username.isNotBlank() && !signingIn && quick == null,
                    shapes = ButtonDefaults.shapes(),
                    modifier = Modifier.fillMaxWidth().height(BUTTON_DP.dp),
                ) {
                    if (signingIn) {
                        androidx.compose.material3.LoadingIndicator(Modifier.size(LOADING_DP.dp), color = MaterialTheme.colorScheme.onPrimary)
                    } else {
                        Text("Sign in", style = type.sectionHeader.copy(fontSize = 16.sp))
                    }
                }

                Row(Modifier.padding(vertical = (Grid.base * 2).dp), verticalAlignment = Alignment.CenterVertically) {
                    HorizontalDivider(Modifier.weight(1f))
                    Text("or", style = type.caption, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = Grid.base.dp))
                    HorizontalDivider(Modifier.weight(1f))
                }

                // Item 6: Quick Connect, a 6-character code approved on another device.
                val q = quick
                if (q == null) {
                    FilledTonalButton(
                        onClick = ::startQuickConnect,
                        enabled = found != null && !signingIn,
                        shapes = ButtonDefaults.shapes(),
                        modifier = Modifier.fillMaxWidth().height(BUTTON_DP.dp).testTag("quick-connect"),
                    ) {
                        Icon(Icons.devices(), contentDescription = null, modifier = Modifier.size(20.dp))
                        Spacer(Modifier.size(Grid.base.dp))
                        Text("Use Quick Connect", style = type.sectionHeader.copy(fontSize = 16.sp))
                    }
                } else {
                    val left = ((q.expiresAtMs - now) / 1000).coerceAtLeast(0)
                    Surface(shape = RoundedCornerShape(28.dp), color = MaterialTheme.colorScheme.secondaryContainer, modifier = Modifier.fillMaxWidth()) {
                        Column(Modifier.padding((Grid.base * 3).dp), horizontalAlignment = Alignment.CenterHorizontally) {
                            androidx.compose.material3.LoadingIndicator(Modifier.size(40.dp), color = MaterialTheme.colorScheme.onSecondaryContainer)
                            Spacer(Modifier.height(Grid.base.dp))
                            // Spaced so it reads off easily onto the other device.
                            Text(q.code, style = type.displayTitle.copy(letterSpacing = 6.sp), color = MaterialTheme.colorScheme.onSecondaryContainer)
                            Spacer(Modifier.height(Grid.base.dp))
                            Text(
                                "Waiting for approval · %d:%02d".format(left / 60, left % 60),
                                style = type.body,
                                color = MaterialTheme.colorScheme.onSecondaryContainer,
                            )
                            Text(
                                "Approve this code from Quick Connect on a device already signed in to Jellyfin.",
                                style = type.caption,
                                color = MaterialTheme.colorScheme.onSecondaryContainer,
                                modifier = Modifier.padding(top = Grid.base.dp),
                            )
                            TextButton(onClick = {
                                host.core.quickConnectCancel()
                                quickJob?.cancel()
                                quick = null
                            }) { Text("Cancel") }
                        }
                    }
                }
                // Item 7, the demo library, waits for openly licensed tracks to bundle
                // (docs/android/REVIEW-PHASE2.md); no button that does nothing is shown.
            }
        }
    }

    (check as? Check.Untrusted)?.let { u ->
        AlertDialog(
            onDismissRequest = { check = Check.Failed("This server's certificate is not trusted.") },
            title = { Text("Trust this certificate?") },
            text = {
                Column {
                    Text("The server uses a certificate this phone does not trust, such as a self-signed one. Only trust it if this fingerprint matches the one your server shows.", style = type.body)
                    Spacer(Modifier.height(Grid.base.dp))
                    Text(u.sha256.chunked(2).joinToString(":").uppercase(), style = type.caption)
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    host.core.trustCertificate(u.address, u.sha256)
                    TrustedCertificates.get(host.context).trust(u.sha256)
                    // Check again; the pin is saved with the server on sign-in.
                    val again = address
                    check = Check.Checking
                    scope.launch {
                        check = runCatching { host.core.checkAddress(again.trim()) }.fold(
                            onSuccess = { Check.Found(it) },
                            onFailure = { Check.Failed(it.userMessage()) },
                        )
                    }
                }) { Text("Trust") }
            },
            dismissButton = {
                TextButton(onClick = { check = Check.Failed("This server's certificate is not trusted.") }) { Text("Don't trust") }
            },
        )
    }
}

private const val CHECK_AFTER_MS = 600L
private const val BUTTON_DP = 56
private const val SPINNER_DP = 20
private const val LOADING_DP = 28
private const val HERO_DP = 72
private const val FIELD_RADIUS_DP = 16
