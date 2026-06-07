package com.sigmundgranaas.turbo.expressive.core.auth

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Hardens the long-lived refresh token at rest: AES-256/GCM with a key held in the
 * AndroidKeyStore (never leaves secure hardware where available), no extra deps.
 * Ciphertext is `base64(iv ‖ ct)`. [decryptOrNull] returns null for legacy plaintext
 * or any tamper/loss, so the caller simply treats it as signed-out.
 */
internal object KeystoreCipher {
    private const val KEYSTORE = "AndroidKeyStore"
    private const val KEY_ALIAS = "turbo_token_key"
    private const val TRANSFORMATION = "AES/GCM/NoPadding"
    private const val GCM_TAG_BITS = 128
    private const val GCM_IV_BYTES = 12

    fun encrypt(plain: String): String {
        val cipher = Cipher.getInstance(TRANSFORMATION).apply { init(Cipher.ENCRYPT_MODE, key()) }
        val ct = cipher.doFinal(plain.toByteArray(Charsets.UTF_8))
        return Base64.encodeToString(cipher.iv + ct, Base64.NO_WRAP)
    }

    fun decryptOrNull(stored: String): String? = runCatching {
        val data = Base64.decode(stored, Base64.NO_WRAP)
        val iv = data.copyOfRange(0, GCM_IV_BYTES)
        val ct = data.copyOfRange(GCM_IV_BYTES, data.size)
        val cipher = Cipher.getInstance(TRANSFORMATION).apply {
            init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(GCM_TAG_BITS, iv))
        }
        String(cipher.doFinal(ct), Charsets.UTF_8)
    }.getOrNull()

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (ks.getEntry(KEY_ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE).apply {
            init(
                KeyGenParameterSpec.Builder(
                    KEY_ALIAS,
                    KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
                )
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .setKeySize(256)
                    .build(),
            )
        }.generateKey()
    }
}
