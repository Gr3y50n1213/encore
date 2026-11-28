package uk.gcartee.encore.wear

import com.google.android.gms.wearable.DataEvent
import com.google.android.gms.wearable.DataEventBuffer
import com.google.android.gms.wearable.WearableListenerService

/**
 * The phone's DataItems ([uk.gcartee.encore.wearlink.Paths]): Google Play services binds
 * this when one changes, on a background thread, and checks that the caller is itself.
 */
class WatchDataService : WearableListenerService() {
    override fun onDataChanged(events: DataEventBuffer) {
        for (event in events) {
            when (event.type) {
                DataEvent.TYPE_CHANGED -> WatchStore.ingest(this, event.dataItem)
                DataEvent.TYPE_DELETED -> WatchStore.forget(this, event.dataItem.uri.path)
            }
        }
    }
}
