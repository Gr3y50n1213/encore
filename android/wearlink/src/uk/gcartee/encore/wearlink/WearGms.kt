package uk.gcartee.encore.wearlink

import android.content.Context
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailability
import com.google.android.gms.tasks.Task
import com.google.android.gms.wearable.DataMap
import kotlin.coroutines.resume
import kotlinx.coroutines.suspendCancellableCoroutine

/**
 * The Google Play services side of [WearProtocol.kt]'s encoding, shared by the phone and the
 * watch. Not covered by the JVM tests (DataMap needs Android); it only copies flat values.
 */
object WearGms {
    /** False on a device without Google Play services: then nothing here is ever called. */
    fun available(context: Context): Boolean =
        runCatching { GoogleApiAvailability.getInstance().isGooglePlayServicesAvailable(context) == ConnectionResult.SUCCESS }
            .getOrDefault(false)

    /** Puts [values] ([NowCodec.encode], [MixesCodec.encode]) into [map]. */
    fun put(map: DataMap, values: Map<String, Any>) {
        for ((key, value) in values) {
            when (value) {
                is String -> map.putString(key, value)
                is Long -> map.putLong(key, value)
                is Int -> map.putLong(key, value.toLong())
                is Boolean -> map.putBoolean(key, value)
                else -> error("unsupported value for $key")
            }
        }
    }

    fun values(map: DataMap): WireValues = object : WireValues {
        override fun string(key: String): String? = if (map.containsKey(key)) map.getString(key) else null
        override fun long(key: String): Long? = if (map.containsKey(key)) map.getLong(key) else null
        override fun boolean(key: String): Boolean? = if (map.containsKey(key)) map.getBoolean(key) else null
    }
}

/** A Play services task's result, or null when it failed (no watch, no Play services). */
suspend fun <T> Task<T>.awaitOrNull(): T? = suspendCancellableCoroutine { cont ->
    addOnCompleteListener { task ->
        cont.resume(if (task.isSuccessful) task.result else null)
    }
}
