package uk.gcartee.encore.app.core

import uk.gcartee.encore.core.EncoreException

/**
 * The sentence to show for a core error. Each case carries it as `reason`, because
 * Kotlin's own `message` reads "reason=<text>" (CORE-API, "Errors"). Anything that is not
 * a core error gets a plain fallback, never a stack trace.
 */
fun Throwable.userMessage(): String = when (this) {
    is EncoreException.InvalidAddress -> reason
    is EncoreException.InsecureAddress -> reason
    is EncoreException.Unreachable -> reason
    is EncoreException.CertificateUntrusted -> reason
    is EncoreException.NotJellyfin -> reason
    is EncoreException.UnsupportedVersion -> reason
    is EncoreException.WrongCredentials -> reason
    is EncoreException.SessionExpired -> reason
    is EncoreException.QuickConnectDisabled -> reason
    is EncoreException.QuickConnectExpired -> reason
    is EncoreException.QuickConnectCancelled -> reason
    is EncoreException.NotFound -> reason
    is EncoreException.Server -> reason
    is EncoreException.NoProfile -> reason
    is EncoreException.Storage -> reason
    is EncoreException.Internal -> reason
    is EncoreException.NotAllowed -> reason
    else -> "Something went wrong. Try again."
}

/** The details disclosure for a server error (boards 18, 20 and 21). */
fun Throwable.details(): String? = (this as? EncoreException.Server)?.let { "HTTP ${it.status}: ${it.detail}" }
