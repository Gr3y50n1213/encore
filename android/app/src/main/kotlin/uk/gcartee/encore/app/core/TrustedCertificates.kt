package uk.gcartee.encore.app.core

import android.content.Context
import java.security.KeyStore
import java.security.MessageDigest
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import javax.net.ssl.HostnameVerifier
import javax.net.ssl.HttpsURLConnection
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLSession
import javax.net.ssl.TrustManagerFactory
import javax.net.ssl.X509TrustManager

/**
 * Board 21, item 3: "A self-signed certificate asks once to trust its fingerprint." The
 * core keeps that pin for its own requests; ExoPlayer and the artwork loader make their
 * own connections to the same server, so the app keeps the same fingerprints and lets
 * a connection through when the system trusts it **or** its leaf certificate is one the
 * user chose to trust. Installed once, for every HttpsURLConnection in the app.
 */
class TrustedCertificates private constructor(context: Context) {
    private val prefs = context.getSharedPreferences("trusted-certificates", Context.MODE_PRIVATE)

    fun trust(sha256Hex: String) {
        val set = pins() + sha256Hex.normalized()
        prefs.edit().putStringSet(KEY, set).apply()
    }

    fun pins(): Set<String> = prefs.getStringSet(KEY, emptySet()).orEmpty()

    private fun isPinned(cert: X509Certificate): Boolean = sha256(cert) in pins()

    private fun install() {
        val factory = TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm())
        factory.init(null as KeyStore?)
        val system = factory.trustManagers.filterIsInstance<X509TrustManager>().first()
        val trustManager = object : X509TrustManager {
            override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) =
                system.checkClientTrusted(chain, authType)

            override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {
                try {
                    system.checkServerTrusted(chain, authType)
                } catch (e: CertificateException) {
                    if (chain.isEmpty() || !isPinned(chain[0])) throw e
                }
            }

            override fun getAcceptedIssuers(): Array<X509Certificate> = system.acceptedIssuers
        }
        val ssl = SSLContext.getInstance("TLS").apply { init(null, arrayOf(trustManager), null) }
        HttpsURLConnection.setDefaultSSLSocketFactory(ssl.socketFactory)
        val defaultVerifier = HttpsURLConnection.getDefaultHostnameVerifier()
        HttpsURLConnection.setDefaultHostnameVerifier(object : HostnameVerifier {
            override fun verify(hostname: String, session: SSLSession): Boolean {
                if (defaultVerifier.verify(hostname, session)) return true
                val leaf = runCatching { session.peerCertificates.firstOrNull() as? X509Certificate }.getOrNull()
                return leaf != null && isPinned(leaf)
            }
        })
    }

    companion object {
        private const val KEY = "sha256"
        @Volatile private var instance: TrustedCertificates? = null

        fun get(context: Context): TrustedCertificates =
            instance ?: synchronized(this) {
                instance ?: TrustedCertificates(context.applicationContext).also {
                    it.install()
                    instance = it
                }
            }

        fun sha256(cert: X509Certificate): String =
            MessageDigest.getInstance("SHA-256").digest(cert.encoded).joinToString("") { "%02x".format(it) }

        /** The core shows fingerprints with or without colons, in either case. */
        fun String.normalized(): String = filter { it.isLetterOrDigit() }.lowercase()
    }
}
