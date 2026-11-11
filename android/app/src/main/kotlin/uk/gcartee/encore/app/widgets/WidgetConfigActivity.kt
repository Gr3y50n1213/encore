package uk.gcartee.encore.app.widgets

import android.app.Activity
import android.appwidget.AppWidgetManager
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import uk.gcartee.encore.app.R
import uk.gcartee.encore.app.ui.theme.EncoreTheme

/**
 * Board 34, item 10: a widget's Profile setting, Active profile (the default) or one
 * profile. Shown when a widget is placed (before Android 12) or reconfigured. It lists the
 * profiles the app last saw (WidgetSync saves them) and never starts the core; with no
 * list yet, it offers Active profile only.
 */
class WidgetConfigActivity : ComponentActivity() {
    private var appWidgetId = AppWidgetManager.INVALID_APPWIDGET_ID

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        appWidgetId = intent?.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
            ?: AppWidgetManager.INVALID_APPWIDGET_ID
        // Backing out leaves the widget unplaced (before Android 12) or unchanged.
        setResult(Activity.RESULT_CANCELED, resultIntent())
        // The activity is exported (Android's widget host starts it): act only for a widget
        // that is ours.
        if (!isOurWidget(appWidgetId)) {
            finish()
            return
        }
        val profileState = WidgetConfig.profileState(this)
        val profiles = profileState.profiles
        val initial = WidgetConfig.choice(this, appWidgetId)
        // Phase 9 (WP7): only a Mixes widget has a Mix setting.
        val mixes = providerIs(MixesWidgetReceiver::class.java)
        val initialMix = WidgetConfig.mixChoice(this, appWidgetId)
        val widgetsDir = SnapshotReader.widgetsDir(filesDir)
        val mixOptions = { choice: WidgetProfileChoice -> pinnedMixesOf(widgetsDir, WidgetConfig.resolve(choice, profileState)) }
        setContent {
            EncoreTheme {
                ConfigScreen(
                    profiles = profiles,
                    initial = initial,
                    showMix = mixes,
                    initialMix = initialMix,
                    mixOptions = mixOptions,
                    onSave = ::save,
                    onCancel = ::finish,
                )
            }
        }
    }

    // --- Phase 9 (WP7): one screen serves all three widgets; the provider tells them apart. ---
    private fun providerIs(receiver: Class<*>): Boolean {
        val info = runCatching { AppWidgetManager.getInstance(this).getAppWidgetInfo(appWidgetId) }.getOrNull()
        return info?.provider?.className == receiver.name
    }

    /** The pinned mixes in the profile's file, for the Mix list: none for a PIN, Guest, a missing file. */
    private fun pinnedMixesOf(widgetsDir: java.io.File, profileId: String?): List<SnapshotMix> {
        if (profileId == null) return emptyList()
        val read = SnapshotReader.read(widgetsDir, profileId) as? SnapshotRead.Ok ?: return emptyList()
        return if (read.snapshot.privacy == SnapshotPrivacy.NORMAL) read.snapshot.pinnedMixes else emptyList()
    }
    // --- End Phase 9 (WP7). ---

    private fun isOurWidget(id: Int): Boolean {
        if (id == AppWidgetManager.INVALID_APPWIDGET_ID) return false
        val info = runCatching { AppWidgetManager.getInstance(this).getAppWidgetInfo(id) }.getOrNull() ?: return false
        return info.provider?.packageName == packageName
    }

    private fun save(choice: WidgetProfileChoice, mix: WidgetMixChoice) {
        WidgetConfig.setChoice(this, appWidgetId, choice)
        // Phase 9 (WP7): the Mix setting is kept for a Mixes widget only.
        val widget: GlanceAppWidget = when {
            providerIs(MixesWidgetReceiver::class.java) -> {
                WidgetConfig.setMixChoice(this, appWidgetId, mix)
                MixesWidget()
            }
            providerIs(RecentWidgetReceiver::class.java) -> RecentWidget()
            else -> NowPlayingWidget()
        }
        WidgetRedraws.bump()
        val context = applicationContext
        val id = appWidgetId
        // Outlives this screen: the widget redraws after it has closed.
        CoroutineScope(SupervisorJob() + Dispatchers.Default).launch {
            runCatching {
                val glanceId = GlanceAppWidgetManager(context).getGlanceIdBy(id)
                widget.update(context, glanceId)
            }
        }
        setResult(Activity.RESULT_OK, resultIntent())
        finish()
    }

    private fun resultIntent() = Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId)
}

@Composable
private fun ConfigScreen(
    profiles: List<KnownProfile>,
    initial: WidgetProfileChoice,
    showMix: Boolean,
    initialMix: WidgetMixChoice,
    mixOptions: (WidgetProfileChoice) -> List<SnapshotMix>,
    onSave: (WidgetProfileChoice, WidgetMixChoice) -> Unit,
    onCancel: () -> Unit,
) {
    // "" is Active profile. A fixed profile that is gone stays chosen until another is picked.
    var selected by rememberSaveable { mutableStateOf((initial as? WidgetProfileChoice.Fixed)?.profileId.orEmpty()) }
    // Phase 9 (WP7): "" is Next up.
    var selectedMix by rememberSaveable { mutableStateOf((initialMix as? WidgetMixChoice.Pinned)?.mixId.orEmpty()) }
    Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .windowInsetsPadding(WindowInsets.safeDrawing)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 24.dp),
        ) {
            Text(stringResource(R.string.widget_config_title), style = MaterialTheme.typography.headlineSmall)
            Text(
                stringResource(R.string.widget_config_explainer),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 8.dp, bottom = 16.dp),
            )
            Column(modifier = Modifier.selectableGroup()) {
                ChoiceRow(stringResource(R.string.widget_config_active), null, selected.isEmpty()) { selected = "" }
                profiles.forEach { p ->
                    ChoiceRow(
                        p.name,
                        if (p.locked) stringResource(R.string.widget_config_locked) else null,
                        selected == p.id,
                    ) { selected = p.id }
                }
            }
            // Phase 9 (WP7): a Mixes widget also picks its mix: Next up, or a pinned mix of the
            // profile chosen above (the list follows that choice).
            if (showMix) {
                val choice = if (selected.isEmpty()) WidgetProfileChoice.Active else WidgetProfileChoice.Fixed(selected)
                val options = mixOptions(choice)
                Text(
                    stringResource(R.string.widget_config_mix_title),
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(top = 24.dp, bottom = 8.dp),
                )
                Column(modifier = Modifier.selectableGroup()) {
                    ChoiceRow(stringResource(R.string.widget_config_mix_next_up), null, selectedMix.isEmpty()) { selectedMix = "" }
                    options.forEach { m ->
                        ChoiceRow(m.title.ifBlank { m.mixId }, null, selectedMix == m.mixId) { selectedMix = m.mixId }
                    }
                }
                if (options.isEmpty()) {
                    Text(
                        stringResource(R.string.widget_config_mix_none),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            Row(
                modifier = Modifier.fillMaxWidth().padding(top = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
            ) {
                TextButton(onClick = onCancel) { Text(stringResource(R.string.widget_config_cancel)) }
                Button(onClick = {
                    onSave(
                        if (selected.isEmpty()) WidgetProfileChoice.Active else WidgetProfileChoice.Fixed(selected),
                        if (selectedMix.isEmpty()) WidgetMixChoice.NextUp else WidgetMixChoice.Pinned(selectedMix),
                    )
                }) { Text(stringResource(R.string.widget_config_save)) }
            }
        }
    }
}

@Composable
private fun ChoiceRow(label: String, detail: String?, selected: Boolean, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .selectable(selected = selected, onClick = onClick, role = Role.RadioButton),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(selected = selected, onClick = null)
        Column(modifier = Modifier.padding(start = 12.dp)) {
            Text(label, style = MaterialTheme.typography.bodyLarge)
            if (detail != null) {
                Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}
