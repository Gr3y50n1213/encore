package uk.gcartee.encore.app

import android.app.Application
import uk.gcartee.encore.core.EncoreTls

class EncoreApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // Before the first https connection: the core checks certificates with
        // Android's own verifier, which needs a Context (request core-0001).
        EncoreTls.initialize(applicationContext)
    }
}
