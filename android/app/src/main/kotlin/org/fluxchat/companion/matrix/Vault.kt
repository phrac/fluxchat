package org.fluxchat.companion.matrix

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import org.json.JSONObject
import org.matrix.rustcomponents.sdk.Session
import org.matrix.rustcomponents.sdk.SlidingSyncVersion
import java.io.File
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The session (access token) and the SDK store's passphrase, encrypted with
 * a key that never leaves the Android Keystore — like the desktop's vault,
 * which keeps them in the OS credential store.
 */
class Vault(context: Context) {
    private val file = File(context.filesDir, "vault.bin")

    data class Saved(val session: Session, val storePassphrase: String)

    fun load(): Saved? {
        if (!file.exists()) return null
        return runCatching {
            val bytes = file.readBytes()
            val iv = bytes.copyOfRange(0, IV_BYTES)
            val cipher = Cipher.getInstance(TRANSFORM)
            cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, iv))
            val o = JSONObject(String(cipher.doFinal(bytes.copyOfRange(IV_BYTES, bytes.size)), Charsets.UTF_8))
            val s = o.getJSONObject("session")
            Saved(
                Session(
                    accessToken = s.getString("access_token"),
                    refreshToken = s.optString("refresh_token").ifEmpty { null },
                    userId = s.getString("user_id"),
                    deviceId = s.getString("device_id"),
                    homeserverUrl = s.getString("homeserver_url"),
                    oauthData = s.optString("oauth_data").ifEmpty { null },
                    slidingSyncVersion = SlidingSyncVersion.valueOf(s.optString("sliding_sync", "NATIVE")),
                ),
                o.getString("store_passphrase"),
            )
        }.getOrNull()
    }

    fun save(session: Session, storePassphrase: String) {
        val s = JSONObject()
            .put("access_token", session.accessToken)
            .put("refresh_token", session.refreshToken ?: "")
            .put("user_id", session.userId)
            .put("device_id", session.deviceId)
            .put("homeserver_url", session.homeserverUrl)
            .put("oauth_data", session.oauthData ?: "")
            .put("sliding_sync", session.slidingSyncVersion.name)
        val plain = JSONObject().put("session", s).put("store_passphrase", storePassphrase).toString().toByteArray(Charsets.UTF_8)
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, key())
        val out = cipher.iv + cipher.doFinal(plain)
        val tmp = File(file.path + ".tmp")
        tmp.writeBytes(out)
        tmp.renameTo(file)
    }

    fun clear() {
        file.delete()
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

    companion object {
        private const val ALIAS = "fluxchat_vault"
        private const val TRANSFORM = "AES/GCM/NoPadding"
        private const val IV_BYTES = 12

        fun newPassphrase(): String {
            val b = ByteArray(32)
            SecureRandom().nextBytes(b)
            return android.util.Base64.encodeToString(b, android.util.Base64.NO_WRAP)
        }
    }
}
