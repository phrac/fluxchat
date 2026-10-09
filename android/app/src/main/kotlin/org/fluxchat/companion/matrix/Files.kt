package org.fluxchat.companion.matrix

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.fluxchat.companion.agents.FileRef
import org.matrix.rustcomponents.sdk.MediaSource
import java.io.File
import java.security.MessageDigest

/** Attachments in messages: downloaded (decrypted) through the SDK, then opened or saved. */
object Files {
    /**
     * The file on disk, downloading it the first time: `cache/files/<hash of
     * its source>/<its name>`, so other apps see the real name.
     */
    suspend fun fetch(context: Context, f: FileRef): File = withContext(Dispatchers.IO) {
        val dir = File(context.cacheDir, "files/${hash(f.source)}")
        val dest = File(dir, Uploads.safeName(f.name))
        if (dest.exists() && dest.length() > 0) return@withContext dest
        val client = Matrix.client ?: error("Not signed in")
        val tmp = File(context.cacheDir, "media-tmp").apply { mkdirs() }
        val handle = MediaSource.fromJson(f.source).use { src ->
            client.getMediaFile(src, f.name, f.mime ?: "application/octet-stream", true, tmp.path)
        }
        try {
            dir.mkdirs()
            val part = File(dir, dest.name + ".part")
            File(handle.path()).inputStream().use { input -> part.outputStream().use { input.copyTo(it) } }
            part.renameTo(dest)
        } finally {
            handle.destroy()
        }
        dest
    }

    /** Hand the file to an app that opens its type. False when none can. */
    fun open(context: Context, file: File, mime: String?): Boolean {
        val uri = FileProvider.getUriForFile(context, context.packageName + ".files", file)
        val view = Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime ?: context.contentResolver.getType(uri) ?: "*/*")
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        return try {
            context.startActivity(Intent.createChooser(view, file.name).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            true
        } catch (_: ActivityNotFoundException) {
            false
        }
    }

    /** Copy the file to where the user chose to save it. */
    suspend fun saveTo(context: Context, file: File, target: Uri) = withContext(Dispatchers.IO) {
        context.contentResolver.openOutputStream(target)?.use { out -> file.inputStream().use { it.copyTo(out) } }
            ?: error("Couldn't write there")
    }

    private fun hash(s: String): String =
        MessageDigest.getInstance("SHA-256").digest(s.toByteArray()).take(12).joinToString("") { "%02x".format(it) }
}
