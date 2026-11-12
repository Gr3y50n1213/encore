package uk.gcartee.encore.app.widgets

import android.appwidget.AppWidgetManager
import android.content.Context
import android.content.pm.ApplicationInfo
import android.os.Build
import android.util.SizeF

/**
 * A readout of the sizes Android reports for each placed Encore widget, for debug builds only
 * (the widgets redesign, 11 November 2026: One UI's cells are not Google's, so the real numbers
 * come from the phones). It asks the system live, so it needs no storage and no core, and the
 * Lock screen and widgets page shows it only when the app is debuggable.
 */
object WidgetSizeLog {
    /** Whether this build may show the readout: the debug build, never a release. */
    fun enabled(context: Context): Boolean =
        (context.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0

    /** One line per placed widget: the sizes it reports and the layout bucket each would draw. */
    fun lines(context: Context): List<String> {
        val manager = AppWidgetManager.getInstance(context)
        val out = mutableListOf<String>()
        for ((receiver, _) in WidgetRefresh.kinds()) {
            val name = receiver.simpleName.removeSuffix("WidgetReceiver")
            for (id in WidgetRefresh.placed(context, receiver)) {
                val o = manager.getAppWidgetOptions(id)
                val minW = o.getInt(AppWidgetManager.OPTION_APPWIDGET_MIN_WIDTH, 0)
                val minH = o.getInt(AppWidgetManager.OPTION_APPWIDGET_MIN_HEIGHT, 0)
                val maxW = o.getInt(AppWidgetManager.OPTION_APPWIDGET_MAX_WIDTH, 0)
                val maxH = o.getInt(AppWidgetManager.OPTION_APPWIDGET_MAX_HEIGHT, 0)
                val sizes: List<SizeF> = when {
                    Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU ->
                        o.getParcelableArrayList(AppWidgetManager.OPTION_APPWIDGET_SIZES, SizeF::class.java).orEmpty()
                    Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
                        @Suppress("DEPRECATION")
                        o.getParcelableArrayList<SizeF>(AppWidgetManager.OPTION_APPWIDGET_SIZES).orEmpty()
                    }
                    else -> emptyList()
                }
                val shown = if (sizes.isEmpty()) listOf(SizeF(minW.toFloat(), minH.toFloat())) else sizes
                val buckets = shown.joinToString(", ") { s -> "${s.width.toInt()}x${s.height.toInt()} -> ${bucket(name, s.width.toInt(), s.height.toInt())}" }
                out += "$name #$id: min ${minW}x$minH, max ${maxW}x$maxH dp; $buckets"
            }
        }
        return out.ifEmpty { listOf("No Encore widget is placed.") }
    }

    private fun bucket(name: String, w: Int, h: Int): String = when (name) {
        "NowPlaying" -> NpLayout.of(w, h).let { "${it.name} ${it.w}x${it.h}" }
        "Mixes" -> CollectionSizing.of(MixesSizing.buckets, w, h).let { "${it.cols}x${it.rows} ${it.w}x${it.h}" }
        else -> CollectionSizing.of(RecentSizing.buckets, w, h).let { "${it.cols}x${it.rows} ${it.w}x${it.h}" }
    }
}
