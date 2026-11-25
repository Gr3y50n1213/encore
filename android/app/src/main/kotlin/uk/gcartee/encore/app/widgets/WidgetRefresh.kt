package uk.gcartee.encore.app.widgets

import android.annotation.SuppressLint
import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context
import android.os.Bundle
import android.util.Log
import androidx.glance.ExperimentalGlanceApi
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.compose
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

/**
 * Redraws every placed widget, each one by the widget class its own receiver is for.
 *
 * `GlanceAppWidget.updateAll` looks the placed widgets up by class, and on the S20 FE (12 November
 * 2026) the Mixes and Recently played widgets began to draw Now Playing's content once the app
 * had run it: a session for the wrong class had started for them, and a running session is not
 * replaced. Pairing the ids the system lists for each receiver with that receiver's own widget
 * leaves no way to mix them up. Touches no core: it only asks Glance to draw again.
 *
 * Each redraw is one drawing ([draw]), never a running Glance session. With the app open and
 * playing, running sessions sent all three widgets to the home screen again and again, about
 * forty times a second, from the main thread: each send made a dozen tap intents, one system
 * call each, and the lyrics got about eleven frames a second (S20 FE, 25 November 2026).
 */
object WidgetRefresh {
    const val TAG = "EncoreWidgets"

    /** Where receivers draw; a drawing outlives the broadcast that asked for it (goAsync). */
    internal val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    /** Each widget receiver with the widget it draws. */
    internal fun kinds(): List<Pair<Class<out GlanceAppWidgetReceiver>, GlanceAppWidget>> = listOf(
        NowPlayingWidgetReceiver::class.java to NowPlayingWidget(),
        MixesWidgetReceiver::class.java to MixesWidget(),
        RecentWidgetReceiver::class.java to RecentWidget(),
    )

    /** The app widget ids placed for [receiver]. */
    internal fun placed(context: Context, receiver: Class<out GlanceAppWidgetReceiver>): List<Int> =
        AppWidgetManager.getInstance(context).getAppWidgetIds(ComponentName(context, receiver)).toList()

    suspend fun updateAll(context: Context) {
        for ((receiver, widget) in kinds()) {
            for (id in placed(context, receiver)) {
                // Logged so a phone's logcat shows which widget class each id was drawn by.
                Log.i(TAG, "redraw ${receiver.simpleName.removeSuffix("Receiver")} for appWidgetId $id")
                runCatching { draw(context, widget, id) }.onFailure { Log.w(TAG, "couldn't draw appWidgetId $id", it) }
            }
        }
    }

    /**
     * Draws [widget] once, at every size its size mode asks for, and sends that to the home
     * screen. The composition ends with the drawing, so nothing keeps redrawing it after.
     */
    @OptIn(ExperimentalGlanceApi::class)
    suspend fun draw(context: Context, widget: GlanceAppWidget, appWidgetId: Int, options: Bundle? = null) {
        val manager = AppWidgetManager.getInstance(context)
        val id = GlanceAppWidgetManager(context).getGlanceIdBy(appWidgetId)
        val views = widget.compose(context, id, options ?: manager.getAppWidgetOptions(appWidgetId))
        manager.updateAppWidget(appWidgetId, views)
    }
}

/**
 * A widget receiver that draws its widgets once for each update and resize ([WidgetRefresh.draw])
 * instead of starting Glance sessions, which keep running and redrawing (see [WidgetRefresh]).
 * Its widgets' taps are all intents, so no session is needed to answer them.
 */
abstract class DrawnOnceWidgetReceiver : GlanceAppWidgetReceiver() {
    // Not super: Glance's own onUpdate and onAppWidgetOptionsChanged start the sessions this replaces.
    @SuppressLint("MissingSuperCall")
    override fun onUpdate(context: Context, appWidgetManager: AppWidgetManager, appWidgetIds: IntArray) {
        // Also the 3-hour update: each drawing reads the file again, so the 24 h dim and the
        // "playing" check use the time now.
        drawLater(context, appWidgetIds, null)
    }

    @SuppressLint("MissingSuperCall")
    override fun onAppWidgetOptionsChanged(context: Context, appWidgetManager: AppWidgetManager, appWidgetId: Int, newOptions: Bundle) {
        drawLater(context, intArrayOf(appWidgetId), newOptions)
    }

    private fun drawLater(context: Context, ids: IntArray, options: Bundle?) {
        val app = context.applicationContext
        val widget = glanceAppWidget
        val pending = goAsync()
        WidgetRefresh.scope.launch {
            try {
                for (id in ids) {
                    runCatching { WidgetRefresh.draw(app, widget, id, options) }
                        .onFailure { android.util.Log.w(WidgetRefresh.TAG, "couldn't draw appWidgetId $id", it) }
                }
            } finally {
                pending.finish()
            }
        }
    }
}
