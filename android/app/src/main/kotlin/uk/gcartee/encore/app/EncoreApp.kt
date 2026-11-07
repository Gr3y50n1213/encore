package uk.gcartee.encore.app

import android.app.Application
import uk.gcartee.encore.app.core.TrustedCertificates
import uk.gcartee.encore.core.EncoreTls

class EncoreApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // Before the first https connection: the core checks certificates with
        // Android's own verifier, which needs a Context (request core-0001).
        EncoreTls.initialize(applicationContext)
        // The player and the artwork loader trust the same fingerprints the user trusted
        // for the core (board 21, item 3).
        TrustedCertificates.get(this)
    }
}
