package uk.gcartee.encore.app.ui.library

import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.snapshots.SnapshotStateList
import kotlinx.coroutines.flow.distinctUntilChanged

/**
 * Pages from the on-device mirror. Browse calls are synchronous and take milliseconds
 * (CORE-API), so a page is read on the main thread as the list nears its end; a big
 * library never loads all at once.
 */
class Pager<T>(private val pageSize: Int, private val read: (offset: Int, limit: Int) -> List<T>) {
    val items: SnapshotStateList<T> = mutableStateListOf()
    private var done = false

    fun reload() {
        items.clear()
        done = false
        more()
    }

    fun more() {
        if (done) return
        val page = runCatching { read(items.size, pageSize) }.getOrDefault(emptyList())
        items.addAll(page)
        if (page.size < pageSize) done = true
    }
}

@Composable
fun <T> rememberPager(vararg keys: Any?, pageSize: Int = 200, read: (Int, Int) -> List<T>): Pager<T> {
    val pager = remember(*keys) { Pager(pageSize, read) }
    LaunchedEffect(pager) { pager.reload() }
    return pager
}

/** Reads the next page when the last visible item is within a page of the end. */
@Composable
fun LoadMore(state: LazyListState, pager: Pager<*>) {
    LaunchedEffect(state, pager) {
        snapshotFlow { state.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0 }
            .distinctUntilChanged()
            .collect { last -> if (last >= pager.items.size - LOOKAHEAD) pager.more() }
    }
}

@Composable
fun LoadMore(state: LazyGridState, pager: Pager<*>) {
    LaunchedEffect(state, pager) {
        snapshotFlow { state.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0 }
            .distinctUntilChanged()
            .collect { last -> if (last >= pager.items.size - LOOKAHEAD) pager.more() }
    }
}

private const val LOOKAHEAD = 40
