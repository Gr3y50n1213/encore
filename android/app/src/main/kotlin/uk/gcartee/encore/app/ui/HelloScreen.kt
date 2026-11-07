package uk.gcartee.encore.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uk.gcartee.encore.core.appName
import uk.gcartee.encore.core.coreVersion
import uk.gcartee.encore.core.hello
import uk.gcartee.encore.tokens.EncoreTokens.Grid
import uk.gcartee.encore.tokens.EncoreTokens.Typography.Scale.Compact

/** What the Phase 1 screen shows; every string comes from the Rust core. */
data class CoreGreeting(val title: String, val greeting: String, val version: String) {
    companion object {
        fun fromCore() = CoreGreeting(
            title = appName(),
            greeting = hello("Android"),
            version = coreVersion(),
        )
    }
}

private fun role(size: Double, weight: Int, lineHeight: Double) =
    TextStyle(fontSize = size.sp, fontWeight = FontWeight(weight), lineHeight = lineHeight.sp)

/**
 * Phase 1's screen (docs/ROADMAP.md: "a screen whose text comes from the Rust core").
 * The title is the core's product name and the line under it is the core's greeting.
 */
@Composable
fun HelloScreen(greeting: CoreGreeting = remember { CoreGreeting.fromCore() }) {
    Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        Column(
            modifier = Modifier
                .safeDrawingPadding()
                .padding(horizontal = (Grid.base * 2).dp, vertical = (Grid.base * 3).dp),
            verticalArrangement = Arrangement.spacedBy(Grid.base.dp),
        ) {
            Text(
                text = greeting.title,
                style = with(Compact.PageTitle) { role(size, weight, lineHeight) },
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier
                    .testTag("title")
                    .semantics { heading() },
            )
            Text(
                text = greeting.greeting,
                style = with(Compact.Body) { role(size, weight, lineHeight) },
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.testTag("greeting"),
            )
            Text(
                text = "Core ${greeting.version}",
                style = with(Compact.Caption) { role(size, weight, lineHeight) },
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.testTag("version"),
            )
        }
    }
}
