package uk.gcartee.encore.app.ui.settings

import android.net.Uri
import android.provider.DocumentsContract
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.LocalFolderInfo

/**
 * Mock-up 49, screens 2 and 3 (approved 26 November 2026): the folders Encore plays
 * music from, adding one with Android's own folder picker, the look through them, and
 * whether new songs are looked for by themselves.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun DeviceMusicPage(host: CoreHost, onBack: () -> Unit, contentPadding: PaddingValues) {
    val music = host.localMusic
    val changes by music.changes.collectAsState()
    val look by music.look.collectAsState()
    val folders = remember(changes, look == null) { music.folders() }
    var auto by remember(changes) { mutableStateOf(music.autoLook()) }
    var removing by remember { mutableStateOf<LocalFolderInfo?>(null) }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree: Uri? ->
        tree?.let(music::addFolder)
    }
    val type = LocalEncoreType.current
    val error = MaterialTheme.colorScheme.error
    val mutedIcon = MaterialTheme.colorScheme.onSurfaceVariant
    val device = deviceNoun()

    Page("Music on this $device", onBack, contentPadding, subtitle = "Encore plays the songs in the folders you choose.") {
        look?.let { l -> item { LookCard(l) } }
        group(
            null,
            folders.map { f ->
                val looking = look?.folderId == f.folderId
                Row0(
                    f.name,
                    when {
                        looking -> "Reading…"
                        !f.available -> "Not available right now"
                        else -> "${device.replaceFirstChar(Char::uppercase)} storage · ${songs(f.songCount.toInt())}"
                    },
                    icon = { Icons.folder() },
                    valueColor = if (!f.available && !looking) error else null,
                    trailing = {
                        IconButton(onClick = { removing = f }, modifier = Modifier.semantics { contentDescription = "Stop using ${f.name}" }) {
                            Icon(Icons.close(), contentDescription = null, tint = mutedIcon)
                        }
                    },
                )
            },
        )
        item {
            FilledTonalButton(
                onClick = { pick.launch(MUSIC_FOLDER) },
                modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = (Grid.base * 1.5f).dp).height(56.dp),
            ) {
                Icon(Icons.add(), contentDescription = null, modifier = Modifier.size(20.dp))
                Spacer(Modifier.width(Grid.base.dp))
                Text("Add a folder", style = type.sectionHeader.copy(fontSize = 16.sp))
            }
        }
        if (folders.isNotEmpty()) {
            group(
                "Options",
                listOf(
                    Row0(
                        "Look for new songs by itself",
                        "When Encore opens",
                        trailing = { CheckSwitch(auto) { on -> auto = on; music.setAutoLook(on) } },
                    ) { auto = !auto; music.setAutoLook(auto) },
                    Row0("Look again now", if (look != null) "Looking…" else null) { if (look == null) music.lookThroughAll() },
                ),
            )
        }
        item {
            Row(Modifier.padding(horizontal = (Grid.base * 3).dp, vertical = (Grid.base * 2).dp), verticalAlignment = Alignment.Top) {
                Icon(Icons.lock(), contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(18.dp).padding(top = 2.dp))
                Spacer(Modifier.width(Grid.base.dp))
                Text(
                    "Encore reads only these folders, and never changes or deletes a file. Nothing from them leaves this $device.",
                    style = rowValue(),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }

    removing?.let { f ->
        AlertDialog(
            onDismissRequest = { removing = null },
            title = { Text("Stop using ${f.name}?") },
            text = { Text("Its songs leave Encore. The files stay on your $device.") },
            confirmButton = {
                TextButton(onClick = { music.removeFolder(f); removing = null }) { Text("Stop using") }
            },
            dismissButton = { TextButton(onClick = { removing = null }) { Text("Cancel") } },
        )
    }
}

/** Screen 3: the look in progress, with the app's wavy progress bar. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun LookCard(l: uk.gcartee.encore.app.core.LocalLook) {
    val type = LocalEncoreType.current
    val scheme = MaterialTheme.colorScheme
    // A rough time left, from how fast this look has gone so far.
    val started = remember(l.folderId) { System.currentTimeMillis() }
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(l.done) { now = System.currentTimeMillis() }
    val left = if (l.done in 5 until l.total) ((now - started) / l.done.toDouble() * (l.total - l.done) / 60_000.0) else null
    Surface(
        color = scheme.surfaceContainer,
        shape = RoundedCornerShape(28.dp),
        modifier = Modifier.fillMaxWidth().padding(horizontal = (Grid.base * 2).dp, vertical = Grid.base.dp),
    ) {
        Column(Modifier.padding(horizontal = (Grid.base * 2.5f).dp, vertical = (Grid.base * 3).dp), verticalArrangement = Arrangement.spacedBy(Grid.base.dp)) {
            Row(verticalAlignment = Alignment.Bottom) {
                Text("%,d".format(l.done), style = type.pageTitle.copy(fontSize = 40.sp, lineHeight = 44.sp), color = scheme.onSurface)
                Text(" of ${"%,d".format(l.total)} files", style = rowValue(), color = scheme.onSurfaceVariant, modifier = Modifier.padding(start = 6.dp, bottom = 6.dp))
            }
            LinearWavyProgressIndicator(
                progress = { if (l.total > 0) l.done.toFloat() / l.total else 0f },
                modifier = Modifier.fillMaxWidth().padding(vertical = Grid.base.dp),
            )
            Row {
                Text("Finding your music", style = type.caption, color = scheme.onSurfaceVariant, modifier = Modifier.weight(1f))
                left?.let {
                    Text(if (it < 1.5) "about 1 min" else "about ${it.toInt() + 1} min", style = type.caption, color = scheme.onSurfaceVariant)
                }
            }
        }
    }
    Body("You can keep using Encore. New songs appear in Library as they're found.")
}

/** "phone", or "tablet" on a large screen, as Play on says "This tablet". */
@Composable
internal fun deviceNoun(): String =
    if (androidx.compose.ui.platform.LocalConfiguration.current.smallestScreenWidthDp >= TABLET_DP) "tablet" else "phone"

private const val TABLET_DP = 600

private fun songs(n: Int) = if (n == 1) "1 song" else "%,d songs".format(n)

/** Where Android's folder picker starts: the phone's Music folder. */
private val MUSIC_FOLDER: Uri = DocumentsContract.buildDocumentUri("com.android.externalstorage.documents", "primary:Music")
