package uk.gcartee.encore.app.core

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Tokens from the core (`SignedIn.token`, `CredentialChanged`), kept under the core's
 * credential key and encrypted with an AES key that lives in the Android Keystore and
 * never leaves it (CLAUDE.md: secrets only in the keystore; CORE-API "Tokens never live
 * in the core"). Never logged. Passwords are never stored at all.
 */
class TokenStore(context: Context) {
    private val prefs = context.getSharedPreferences("tokens", Context.MODE_PRIVATE)

    fun save(credentialKey: String, token: String) {
        if (token.isEmpty()) {
            delete(credentialKey)
            return
        }
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, key())
        val sealed = cipher.iv + cipher.doFinal(token.toByteArray(Charsets.UTF_8))
        prefs.edit().putString(credentialKey, Base64.encodeToString(sealed, Base64.NO_WRAP)).apply()
    }

    /** Null when there is none, or when it can no longer be opened (the key was reset). */
    fun read(credentialKey: String): String? {
        val stored = prefs.getString(credentialKey, null) ?: return null
        return try {
            val sealed = Base64.decode(stored, Base64.NO_WRAP)
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(TAG_BITS, sealed, 0, IV_BYTES))
            String(cipher.doFinal(sealed, IV_BYTES, sealed.size - IV_BYTES), Charsets.UTF_8)
        } catch (_: Exception) {
            null
        }
    }

    fun delete(credentialKey: String) {
        prefs.edit().remove(credentialKey).apply()
    }

    private fun key(): SecretKey {
        val store = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    private companion object {
        const val KEYSTORE = "AndroidKeyStore"
        const val ALIAS = "encore-tokens"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val IV_BYTES = 12
        const val TAG_BITS = 128
    }
}
