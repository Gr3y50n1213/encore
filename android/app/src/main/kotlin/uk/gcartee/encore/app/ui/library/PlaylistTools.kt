package uk.gcartee.encore.app.ui.library

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.ui.components.ActionSheet
import uk.gcartee.encore.app.ui.components.AddToPlaylistSheet
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.components.LocalDownloads
import uk.gcartee.encore.app.ui.components.SheetAction
import uk.gcartee.encore.app.ui.components.formatLength
import uk.gcartee.encore.core.PlaylistSummary
import uk.gcartee.encore.core.Song
import uk.gcartee.encore.core.StartedFrom

/*
 * Mock-up 47, board 3 (owner, 24 November 2026): "you should be able to press and hold a
 * playlist to give you an option", with the rest of what a playlist can do. The same rename
 * and delete dialogs serve the playlist page.
 */

/** Rename, from the playlist page or its press-and-hold menu. The new name reaches every device. */
@Composable
fun RenamePlaylistDialog(host: CoreHost, playlistId: Long, name: String, onDone: (renamed: Boolean) -> Unit) {
    val actions = LocalAppActions.current
    var typed by remember { mutableStateOf(name) }
    fun save() {
        val wanted = typed.trim()
        if (wanted.isEmpty() || wanted == name) {
            onDone(false)
            return
        }
        runCatching { host.core.renamePlaylist(playlistId, wanted) }
            .onSuccess { r ->
                onDone(true)
                actions.toaster.show("Renamed to $wanted") { r.editId?.let { runCatching { host.core.undo(it) } }; host.remoteChanges.value++ }
            }
            .onFailure {
                onDone(false)
                actions.toaster.show(it.userMessage())
            }
    }
    AlertDialog(
        onDismissRequest = { onDone(false) },
        title = { Text("Rename playlist") },
        text = {
            OutlinedTextField(
                typed,
                { typed = it },
                singleLine = true,
                keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Done),
                keyboardActions = androidx.compose.foundation.text.KeyboardActions(onDone = { save() }),
            )
        },
        confirmButton = { TextButton(onClick = { save() }, enabled = typed.isNotBlank()) { Text("Rename") } },
        dismissButton = { TextButton(onClick = { onDone(false) }) { Text("Cancel") } },
    )
}

/** Delete, with no Undo, so it asks first. The songs stay in the library. */
@Composable
fun DeletePlaylistDialog(host: CoreHost, playlistId: Long, name: String, onDismiss: () -> Unit, onDeleted: () -> Unit) {
    val actions = LocalAppActions.current
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Delete “$name”?") },
        text = { Text("It's removed from your server and every device. The songs stay in your library.") },
        confirmButton = {
            TextButton(onClick = {
                onDismiss()
                runCatching { host.core.deletePlaylist(playlistId) }
                    .onSuccess {
                        actions.forgetPlaylist(playlistId)
                        actions.toaster.show("Deleted $name")
                        host.remoteChanges.value++
                        onDeleted()
                    }
                    .onFailure { actions.toaster.show(it.userMessage()) }
            }) { Text("Delete", color = MaterialTheme.colorScheme.error) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

/**
 * A playlist's press-and-hold menu (Library, Search): Play, Shuffle and Play next as the
 * quick tiles; then Add to queue, Add all to another playlist, Download, Rename; Delete in red.
 */
@Composable
fun PlaylistMenu(host: CoreHost, playlist: PlaylistSummary, onDismiss: () -> Unit) {
    val actions = LocalAppActions.current
    var renaming by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf(false) }
    var addAll by remember { mutableStateOf<List<Song>?>(null) }
    var sheet by remember { mutableStateOf(true) }
    val songs by produceState<List<Song>?>(null, playlist.id) {
        value = withContext(Dispatchers.IO) {
            runCatching { host.core.playlist(playlist.id).entries.mapNotNull { it.song } }.getOrDefault(emptyList())
        }
    }
    val download = LocalDownloads.current.playlist(playlist.id)
    val target = uk.gcartee.encore.app.core.playlistTarget(playlist.id)
    val n = playlist.songCount.toInt()
    if (sheet) {
        val list = songs.orEmpty()
        val ids = list.map { it.id }
        val empty = list.isEmpty()
        val play = Icons.play()
        val shuffle = Icons.shuffle()
        val next = Icons.playNext()
        val queue = Icons.queue()
        val add = Icons.playlistAdd()
        val downloadIcon = if (download == null) Icons.download() else Icons.delete()
        val edit = Icons.edit()
        val delete = Icons.delete()
        ActionSheet(
            // The sheet closes first and its action runs straight after; the menu ends once
            // nothing it opened is still showing (the effect below).
            onDismiss = { sheet = false },
            header = {
                SheetHeader(playlist.name, listOf(if (n == 1) "1 song" else "$n songs", formatLength(playlist.durationMs)).joinToString(" · ")) {
                    Box(Modifier.size(56.dp).clip(RoundedCornerShape(14.dp))) { MosaicFill(ids.take(4), Modifier.fillMaxSize()) }
                }
            },
            quick = if (empty) emptyList() else listOf(
                SheetAction("Play", play) { actions.play(ids, 0, StartedFrom.PLAYLIST, playlist.id.toString(), playlist.name) },
                SheetAction("Shuffle", shuffle) { actions.play(ids, ids.indices.random(), StartedFrom.PLAYLIST, playlist.id.toString(), playlist.name, shuffle = true) },
                SheetAction("Play next", next) { actions.playNext(list) },
            ),
            groups = listOf(
                listOfNotNull(
                    if (empty) null else SheetAction("Add to queue", queue) { actions.addToQueue(list) },
                    if (empty) null else SheetAction("Add all to another playlist", add) { addAll = list },
                    if (empty) null else SheetAction(if (download == null) "Download" else "Remove download", downloadIcon) {
                        if (download == null) actions.download(target, playlist.name) else actions.removeDownload(target, playlist.name)
                    },
                    SheetAction("Rename", edit) { renaming = true },
                ),
                listOf(SheetAction("Delete playlist", delete, destructive = true) { deleting = true }),
            ),
        )
    }
    androidx.compose.runtime.LaunchedEffect(sheet, renaming, deleting, addAll) {
        if (!sheet && !renaming && !deleting && addAll == null) onDismiss()
    }
    if (renaming) RenamePlaylistDialog(host, playlist.id, playlist.name) { renaming = false }
    if (deleting) DeletePlaylistDialog(host, playlist.id, playlist.name, onDismiss = { deleting = false }, onDeleted = {})
    addAll?.let { list -> AddToPlaylistSheet(list) { addAll = null } }
}
