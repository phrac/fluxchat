package org.fluxchat.companion.matrix

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.fluxchat.companion.agents.FileRef
import org.fluxchat.companion.agents.Media
import org.matrix.rustcomponents.sdk.MediaSource
import java.io.File
import java.security.MessageDigest

/** Attachments in messages: downloaded (decrypted) through the SDK, then opened, shared or saved. */
object Files {
    /**
     * One download per file at a time. Open and Save of the same file (or a
     * second tap after scrolling back to it) both wrote `<name>.part`, and
     * whichever finished first renamed the other's half-written copy.
     */
    private val fetching = HashMap<String, Mutex>()

    private fun lock(dir: File): Mutex = synchronized(fetching) { fetching.getOrPut(dir.path) { Mutex() } }

    /**
     * The file on disk, downloading it the first time: `cache/files/<hash of
     * its source>/<its name>`, so other apps see the real name.
     */
    suspend fun fetch(context: Context, f: FileRef): File = withContext(Dispatchers.IO) {
        val dir = File(context.cacheDir, "files/${hash(f.source)}")
        lock(dir).withLock {
            val dest = File(dir, Uploads.safeName(f.name))
            if (dest.exists() && dest.length() > 0) return@withLock dest
            val client = Matrix.client ?: error("Not signed in")
            val tmp = File(context.cacheDir, "media-tmp").apply { mkdirs() }
            val handle = MediaSource.fromJson(f.source).use { src ->
                client.getMediaFile(src, f.name, f.mime ?: "application/octet-stream", true, tmp.path)
            }
            try {
                dir.mkdirs()
                val part = File(dir, dest.name + ".part")
                File(handle.path()).inputStream().use { input -> part.outputStream().use { input.copyTo(it) } }
                // Unchecked, a failed rename handed back a file that isn't there.
                if (!part.renameTo(dest)) error("Couldn't store ${dest.name}")
            } finally {
                handle.destroy()
            }
            dest
        }
    }

    /**
     * A message image as a file, to save or share. An image in a message has
     * no file name of its own, so it is named for what its bytes are
     * (`image-1a2b3c4d.jpg`), next to where attachments go.
     */
    suspend fun fetchImage(context: Context, media: Media): File = withContext(Dispatchers.IO) {
        val id = hash(media.source)
        val dir = File(context.cacheDir, "files/$id")
        lock(dir).withLock {
            dir.listFiles()
                ?.firstOrNull { it.name.startsWith(IMAGE_NAME) && !it.name.endsWith(".part") && it.length() > 0 }
                ?.let { return@withLock it }
            val client = Matrix.client ?: error("Not signed in")
            val bytes = MediaSource.fromJson(media.source).use { client.getMediaContent(it) }
            dir.mkdirs()
            val dest = File(dir, "$IMAGE_NAME${id.take(8)}.${imageType(bytes)?.first ?: "img"}")
            val part = File(dir, dest.name + ".part")
            part.writeBytes(bytes)
            if (!part.renameTo(dest)) error("Couldn't store ${dest.name}")
            dest
        }
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

    /** Offer the file to other apps through the system's share sheet. False when there is none. */
    fun share(context: Context, file: File, mime: String?): Boolean {
        val uri = FileProvider.getUriForFile(context, context.packageName + ".files", file)
        val send = Intent(Intent.ACTION_SEND)
            .setType(mime ?: context.contentResolver.getType(uri) ?: "*/*")
            .putExtra(Intent.EXTRA_STREAM, uri)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        // The read grant reaches the app picked in the sheet through the clip
        // data (the extra alone doesn't carry it), which is also its preview.
        send.clipData = ClipData.newRawUri(file.name, uri)
        return try {
            context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
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

    private const val IMAGE_NAME = "image-"
}

/** An image's file extension and type from its first bytes; null when they aren't a format we know. */
internal fun imageType(bytes: ByteArray): Pair<String, String>? {
    fun at(offset: Int, magic: String) =
        bytes.size >= offset + magic.length && magic.indices.all { bytes[offset + it] == magic[it].code.toByte() }
    return when {
        bytes.size >= 3 && bytes[0] == 0xFF.toByte() && bytes[1] == 0xD8.toByte() && bytes[2] == 0xFF.toByte() -> "jpg" to "image/jpeg"
        at(1, "PNG") && bytes[0] == 0x89.toByte() -> "png" to "image/png"
        at(0, "GIF8") -> "gif" to "image/gif"
        at(0, "RIFF") && at(8, "WEBP") -> "webp" to "image/webp"
        at(4, "ftypavif") -> "avif" to "image/avif"
        at(4, "ftypheic") || at(4, "ftypheix") || at(4, "ftypmif1") -> "heic" to "image/heic"
        at(0, "BM") -> "bmp" to "image/bmp"
        else -> null
    }
}

/** The type of an image file [Files.fetchImage] named, from its extension. */
internal fun imageMime(name: String): String = when (name.substringAfterLast('.', "").lowercase()) {
    "jpg", "jpeg" -> "image/jpeg"
    "png" -> "image/png"
    "gif" -> "image/gif"
    "webp" -> "image/webp"
    "avif" -> "image/avif"
    "heic" -> "image/heic"
    "bmp" -> "image/bmp"
    else -> "application/octet-stream"
}
