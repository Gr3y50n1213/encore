package uk.gcartee.encore.app.widgets

import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Context
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver

/**
 * Redraws every placed widget, each one by the widget class its own receiver is for.
 *
 * `GlanceAppWidget.updateAll` looks the placed widgets up by class, and on the S20 FE (12 November
 * 2026) the Mixes and Recently played widgets began to draw Now Playing's content once the app
 * had run it: a session for the wrong class had started for them, and a running session is not
 * replaced. Pairing the ids the system lists for each receiver with that receiver's own widget
 * leaves no way to mix them up. Touches no core: it only asks Glance to draw again.
 */
object WidgetRefresh {
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
        val glance = GlanceAppWidgetManager(context)
        for ((receiver, widget) in kinds()) {
            for (id in placed(context, receiver)) {
                runCatching { widget.update(context, glance.getGlanceIdBy(id)) }
            }
        }
    }
}
