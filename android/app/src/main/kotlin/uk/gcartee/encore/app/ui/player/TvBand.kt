package uk.gcartee.encore.app.ui.player

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
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
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import uk.gcartee.encore.app.playback.tv.ActiveTv
import uk.gcartee.encore.app.playback.tv.TvOutput
import uk.gcartee.encore.app.ui.components.Icons
import uk.gcartee.encore.app.ui.theme.LocalEncoreType
import uk.gcartee.encore.tokens.EncoreTokens.Grid

/**
 * Mock-up 50, screen 2: "Playing on <TV>" over Now Playing while a TV has the music. A tap
 * opens Play on, to bring the music back to the phone or move it to another TV. Drawn on the
 * album's field like the other controls there.
 */
@Composable
fun TvBand(tv: ActiveTv, content: Color, onOpen: () -> Unit, modifier: Modifier = Modifier) {
    val type = LocalEncoreType.current
    Surface(
        onClick = onOpen,
        shape = RoundedCornerShape(BAND_RADIUS_DP.dp),
        color = content.copy(alpha = 0.12f),
        contentColor = content,
        modifier = modifier
            .fillMaxWidth()
            // TalkBack reads the band's own words, "Playing on <TV>", as a button.
            .heightIn(min = 48.dp),
    ) {
        Row(Modifier.padding(horizontal = 14.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.tv(), contentDescription = null, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width((Grid.base * 1.25f).dp))
            Text(
                buildAnnotatedString {
                    append("Playing on ")
                    withStyle(SpanStyle(fontWeight = FontWeight.SemiBold)) { append(tv.name) }
                },
                style = type.body,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/**
 * Mock-up 50, screen 2: the TV's own volume. The slider moves at once and the TV follows when
 * it is let go; the phone's volume keys move it too (the session is a remote one).
 */
@Composable
fun TvVolume(tv: ActiveTv, content: Color, modifier: Modifier = Modifier) {
    val type = LocalEncoreType.current
    var level by remember(tv.id) { mutableFloatStateOf(tv.volume) }
    var dragging by remember { mutableStateOf(false) }
    LaunchedEffect(tv.volume) { if (!dragging) level = tv.volume }
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Icon(Icons.tv(), contentDescription = null, tint = content.copy(alpha = 0.8f), modifier = Modifier.size(18.dp))
        Slider(
            value = level,
            enabled = tv.canSetVolume,
            onValueChange = {
                dragging = true
                level = it
            },
            onValueChangeFinished = {
                dragging = false
                TvOutput.setVolume(level)
            },
            colors = SliderDefaults.colors(thumbColor = content, activeTrackColor = content, inactiveTrackColor = content.copy(alpha = 0.24f)),
            modifier = Modifier
                .weight(1f)
                .padding(horizontal = Grid.base.dp)
                .semantics { contentDescription = "Volume on ${tv.name}" },
        )
        Text("TV volume", style = type.caption, color = content.copy(alpha = 0.8f))
    }
}

private const val BAND_RADIUS_DP = 18
