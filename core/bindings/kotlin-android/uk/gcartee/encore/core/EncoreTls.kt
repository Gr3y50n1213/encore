// Hand-written by the core track (not generated). Android only.
//
// Call EncoreTls.initialize(applicationContext) once when the app starts, before the first
// https connection. It hands Android's certificate checking to the core
// (docs/requests/core-0001-android-tls-init.md). Until it runs, https connections fail
// with "Android certificate checks are not set up"; plain http on the local network
// still works. Calling it again is harmless.
//
// The app build also needs the Kotlin component of rustls-platform-verifier; see that
// request for the Gradle setup.

package uk.gcartee.encore.core

import android.content.Context

object EncoreTls {
    init {
        System.loadLibrary("encore_core")
    }

    @JvmStatic
    external fun initialize(context: Context)
}
