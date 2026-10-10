package org.fluxchat.companion.matrix

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The run history on disk, encrypted with a key that never leaves the
 * Android Keystore — the same shape as [Vault] (which holds only the
 * session, so this keeps its own key). Like the desktop's encrypted
 * `agent_run_history`.
 */
object HistoryStore {
    private const val ALIAS = "fluxchat_history"
    private const val TRANSFORM = "AES/GCM/NoPadding"
    private const val IV_BYTES = 12

    private fun file(context: Context) = File(context.filesDir, "runs.bin")

    fun load(context: Context): ByteArray? {
        val f = file(context)
        if (!f.exists()) return null
        return runCatching {
            val bytes = f.readBytes()
            val iv = bytes.copyOfRange(0, IV_BYTES)
            val cipher = Cipher.getInstance(TRANSFORM)
            cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, iv))
            cipher.doFinal(bytes.copyOfRange(IV_BYTES, bytes.size))
        }.getOrNull()
    }

    fun save(context: Context, plain: ByteArray) {
        runCatching {
            val cipher = Cipher.getInstance(TRANSFORM)
            cipher.init(Cipher.ENCRYPT_MODE, key())
            val out = cipher.iv + cipher.doFinal(plain)
            val tmp = File(file(context).path + ".tmp")
            tmp.writeBytes(out)
            tmp.renameTo(file(context))
        }
    }

    fun clear(context: Context) {
        file(context).delete()
    }

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getEntry(ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return gen.generateKey()
    }
}
