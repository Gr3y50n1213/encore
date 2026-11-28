package uk.gcartee.encore.wear

import android.content.Context
import android.util.Log
import com.google.android.gms.wearable.CapabilityClient
import com.google.android.gms.wearable.Wearable
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import uk.gcartee.encore.wearlink.Paths
import uk.gcartee.encore.wearlink.WearCommand
import uk.gcartee.encore.wearlink.WearGms
import uk.gcartee.encore.wearlink.awaitOrNull
import uk.gcartee.encore.wearlink.toBytes

/**
 * Sends the watch's commands to the phone ([Paths.CMD]), one at a time and in order, to the
 * reachable node with the Encore phone app ([Paths.PHONE_CAPABILITY]), else to the node the
 * last Now Playing came from.
 */
object PhoneLink {
    private const val TAG = "PhoneLink"

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO.limitedParallelism(1))

    fun send(context: Context, command: WearCommand) {
        val app = context.applicationContext
        if (!WearGms.available(app)) return
        scope.launch {
            val node = phone(app)
            if (node == null) {
                Log.w(TAG, "No phone to send ${command.wire} to")
                return@launch
            }
            if (Wearable.getMessageClient(app).sendMessage(node, Paths.CMD, command.toBytes()).awaitOrNull() == null) {
                Log.w(TAG, "Couldn't send ${command.wire}")
            }
        }
    }

    private suspend fun phone(context: Context): String? {
        val info = Wearable.getCapabilityClient(context)
            .getCapability(Paths.PHONE_CAPABILITY, CapabilityClient.FILTER_REACHABLE)
            .awaitOrNull()
        val nodes = info?.nodes.orEmpty()
        return (nodes.firstOrNull { it.isNearby } ?: nodes.firstOrNull())?.id ?: WatchStore.phoneNode(context)
    }
}
