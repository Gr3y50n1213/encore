package uk.gcartee.encore.app.ui.player

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uk.gcartee.encore.app.core.CoreHost
import uk.gcartee.encore.app.core.userMessage
import uk.gcartee.encore.app.playback.RemoteControl
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.components.LocalAppActions
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.core.EventKind
import uk.gcartee.encore.core.OutputDevice
import uk.gcartee.encore.core.RemoteCommandKind
import uk.gcartee.encore.core.RemoteQueue
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/**
 * Board 12, "Remote control": while another device plays, Now Playing shows a capsule,
 * "Playing on Living Room Mac", with that device's volume. A tap on the capsule moves the
 * music back here.
 */
@Composable
fun RemoteCapsule(device: OutputDevice, content: Color, modifier: Modifier = Modifier) {
    val type = LocalEncoreType.current
    val actions = LocalAppActions.current
    Column(modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        Surface(
            onClick = { RemoteControl.playHere { actions.toaster.show(it.userMessage()) } },
            shape = CircleShape,
            color = content.copy(alpha = 0.16f),
            contentColor = content,
            modifier = Modifier.semantics { contentDescription = "Playing on ${device.name}. Tap to play on this device." },
        ) {
            Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.devices(), contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(Grid.base.dp))
                Text("Playing on ${device.name}", style = type.buttonLabelSelected, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Spacer(Modifier.width(Grid.base.dp))
                Text("· Play here", style = type.caption, color = content.copy(alpha = 0.75f))
            }
        }
        // That device's volume: the slider moves at once, the change goes when let go.
        var level by remember(device.deviceId) { mutableFloatStateOf(device.volume.toFloat()) }
        var dragging by remember { mutableStateOf(false) }
        LaunchedEffect(device.volume) { if (!dragging) level = device.volume.toFloat() }
        Row(Modifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.speaker(), contentDescription = null, tint = content.copy(alpha = 0.8f), modifier = Modifier.size(18.dp))
            Slider(
                value = level,
                onValueChange = { dragging = true; level = it },
                onValueChangeFinished = {
                    dragging = false
                    RemoteControl.send(RemoteCommandKind.SET_VOLUME, level = level.toDouble())
                },
                colors = SliderDefaults.colors(thumbColor = content, activeTrackColor = content, inactiveTrackColor = content.copy(alpha = 0.24f)),
                modifier = Modifier.weight(1f).padding(horizontal = Grid.base.dp).semantics { contentDescription = "Volume on ${device.name}" },
            )
        }
    }
}

/** The controlled device's queue (board 12: queue edits act there): now, Up Next, then Autoplay. */
@Composable
fun RemoteQueuePanel(host: CoreHost, device: OutputDevice, content: Color, modifier: Modifier = Modifier) {
    val type = LocalEncoreType.current
    var queue by remember { mutableStateOf<RemoteQueue?>(null) }
    var version by remember { mutableStateOf(0) }
    LaunchedEffect(device.deviceId, version) {
        queue = withContext(Dispatchers.IO) { runCatching { host.core.remoteQueue(device.deviceId) }.getOrNull() }
    }
    // A new song there, or a queue change, reads it again.
    LaunchedEffect(device.deviceId) {
        host.events.collect { e -> if (e.kind == EventKind.REMOTE_STATE) version++ }
    }
    val q = queue
    if (q == null) {
        Text("Reading the queue on ${device.name}…", style = type.body, color = content.copy(alpha = 0.8f), modifier = modifier.padding(Grid.base.dp))
        return
    }
    LazyColumn(modifier, verticalArrangement = Arrangement.spacedBy(2.dp)) {
        q.current?.let { c ->
            item { Text("Now playing on ${device.name}", style = type.groupLabel, color = content.copy(alpha = 0.8f), modifier = Modifier.padding(vertical = Grid.base.dp)) }
            item { RemoteRow(c.song.title, c.song.artist, content, onRemove = null) }
        }
        if (q.upNext.isNotEmpty()) {
            item { Text("Up Next", style = type.groupLabel, color = content.copy(alpha = 0.8f), modifier = Modifier.padding(vertical = Grid.base.dp)) }
            items(q.upNext, key = { "u${it.uid}" }) { e ->
                RemoteRow(e.song.title, e.song.artist, content) {
                    RemoteControl.send(RemoteCommandKind.REMOVE, uid = e.uid)
                    version++
                }
            }
        }
        if (q.autoplay.isNotEmpty()) {
            item { Text("Autoplay", style = type.groupLabel, color = content.copy(alpha = 0.8f), modifier = Modifier.padding(vertical = Grid.base.dp)) }
            items(q.autoplay, key = { "a${it.uid}" }) { e -> RemoteRow(e.song.title, e.song.artist, content, onRemove = null) }
        }
        if (q.current == null && q.upNext.isEmpty()) {
            item { Text("Nothing is queued on ${device.name}.", style = type.body, color = content.copy(alpha = 0.8f)) }
        }
    }
}

@Composable
private fun RemoteRow(title: String, artist: String, content: Color, onRemove: (() -> Unit)?) {
    val type = LocalEncoreType.current
    Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(title, style = type.songRowTitle, color = content, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(artist, style = type.body, color = content.copy(alpha = 0.75f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (onRemove != null) {
            IconButton(onClick = onRemove) { Icon(Icons.close(), contentDescription = "Remove $title from the queue there", tint = content.copy(alpha = 0.8f)) }
        }
    }
}
