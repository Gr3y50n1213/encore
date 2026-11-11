package uk.gcartee.encore.app.widgets

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/**
 * Bumped whenever a widget should read its file again. A Glance session that is already
 * running doesn't call provideGlance again on update(), so the widgets collect this and
 * reload. Holds no data.
 */
object WidgetRedraws {
    private val _stamp = MutableStateFlow(0L)
    val stamp: StateFlow<Long> = _stamp.asStateFlow()

    fun bump() {
        _stamp.update { it + 1 }
    }
}
