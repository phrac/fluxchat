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
import org.fluxchat.companion.Diag

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

    @Volatile private var opened: HistoryFile? = null

    private fun file(context: Context): HistoryFile = opened ?: synchronized(this) {
        opened ?: HistoryFile(File(context.filesDir, "runs.bin"), ::seal, ::open) { msg, e -> Diag.error(msg, e) }.also { opened = it }
    }

    fun load(context: Context): ByteArray? = file(context).load()

    fun save(context: Context, plain: ByteArray) {
        file(context).save(plain)
    }

    fun clear(context: Context) = file(context).clear()

    private fun seal(plain: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, key(create = true) ?: error("no keystore key"))
        return cipher.iv + cipher.doFinal(plain)
    }

    private fun open(bytes: ByteArray): ByteArray {
        val iv = bytes.copyOfRange(0, IV_BYTES)
        val cipher = Cipher.getInstance(TRANSFORM)
        // Never creates a key on the way in (like [Vault.load]): one made here
        // would replace the key the file was written with, and a keystore that
        // only hiccuped would have cost the history for good.
        val key = key(create = false) ?: throw IllegalStateException("the phone's keystore has no key for the run history")
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, iv))
        return cipher.doFinal(bytes.copyOfRange(IV_BYTES, bytes.size))
    }

    private fun key(create: Boolean): SecretKey? {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getEntry(ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        if (!create) return null
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

/**
 * The history file and what happens when it can't be read — apart from the
 * Keystore ([seal] / [open]) so the rules can be tested.
 *
 * A file that won't open is NOT "no history": read that way, the first run
 * recorded afterwards saved a new, nearly empty history over it, and one
 * keystore hiccup at start cost everything recorded so far. So after a
 * failed load nothing is saved for the rest of the process (this session's
 * runs stay in memory only) and the file is tried again at the next start.
 *
 * Only when it fails [MAX_FAILED_LOADS] starts in a row is it given up on —
 * a key that is really gone (a restore onto a new phone) would otherwise
 * mean no history is ever saved again. Even then it is moved aside
 * (`.unreadable`), not deleted.
 */
internal class HistoryFile(
    private val file: File,
    private val seal: (ByteArray) -> ByteArray,
    private val open: (ByteArray) -> ByteArray,
    private val report: (String, Throwable?) -> Unit,
) {
    private val aside = File(file.path + ".unreadable")
    /** How many starts in a row the file wouldn't open. */
    private val failed = File(file.path + ".failed")
    private var unreadable = false
    private var skipNoted = false
    private var counted = false

    @Synchronized fun load(): ByteArray? {
        unreadable = false
        skipNoted = false
        if (!file.exists()) return null
        return try {
            open(file.readBytes()).also { failed.delete() }
        } catch (e: Exception) {
            // Counted once per process: "Try again" on a failed start reads
            // the file again, and three taps are not three starts.
            val before = runCatching { failed.readText().trim().toInt() }.getOrDefault(0)
            val failures = if (counted) before.coerceAtLeast(1) else before + 1
            counted = true
            if (failures >= MAX_FAILED_LOADS) {
                aside.delete()
                val moved = file.renameTo(aside)
                failed.delete()
                // Not moved: still there, so still not to be written over.
                unreadable = !moved
                report(
                    "Reading the run history failed $failures starts in a row: " +
                        if (moved) "set aside as ${aside.name}, starting a new one" else "couldn't set it aside either, still not saving over it",
                    e,
                )
            } else {
                unreadable = true
                runCatching { failed.writeText(failures.toString()) }.onFailure { report("Counting the failed run history reads", it) }
                report("Reading the run history (kept as it is; runs recorded until the next start aren't saved, attempt $failures of $MAX_FAILED_LOADS)", e)
            }
            null
        }
    }

    /** False when nothing was written. */
    @Synchronized fun save(plain: ByteArray): Boolean {
        if (unreadable) {
            if (!skipNoted) {
                skipNoted = true
                report("Not saving the run history: the saved one couldn't be read at start and is being kept", null)
            }
            return false
        }
        return try {
            val out = seal(plain)
            val tmp = File(file.path + ".tmp")
            tmp.writeBytes(out)
            if (!tmp.renameTo(file)) {
                tmp.copyTo(file, overwrite = true)
                tmp.delete()
            }
            true
        } catch (e: Exception) {
            report("Saving the run history", e)
            false
        }
    }

    /** Signing out: the history goes, the set-aside copy with it. */
    @Synchronized fun clear() {
        file.delete()
        aside.delete()
        failed.delete()
        unreadable = false
        skipNoted = false
    }

    companion object {
        const val MAX_FAILED_LOADS = 3
    }
}
