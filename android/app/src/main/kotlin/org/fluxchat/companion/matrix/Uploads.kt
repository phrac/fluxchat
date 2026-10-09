package org.fluxchat.companion.matrix

import android.content.Context
import android.graphics.BitmapFactory
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.provider.OpenableColumns
import java.io.File
import java.util.UUID

/** A file picked to send: what the composer shows, and where to read it from. */
data class Attachment(val uri: Uri, val name: String, val mime: String, val size: Long?) {
    val isImage: Boolean get() = mime.startsWith("image/")
    val isVideo: Boolean get() = mime.startsWith("video/")
}

/** Picked files, ready for the SDK's upload (which reads a path). */
object Uploads {
    /** A picked file's name, type and size, as its provider reports them. */
    fun describe(context: Context, uri: Uri): Attachment {
        var name: String? = null
        var size: Long? = null
        runCatching {
            context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    c.getColumnIndex(OpenableColumns.DISPLAY_NAME).takeIf { it >= 0 && !c.isNull(it) }?.let { name = c.getString(it) }
                    c.getColumnIndex(OpenableColumns.SIZE).takeIf { it >= 0 && !c.isNull(it) }?.let { size = c.getLong(it) }
                }
            }
        }
        val mime = context.contentResolver.getType(uri) ?: "application/octet-stream"
        return Attachment(uri, name ?: uri.lastPathSegment?.substringAfterLast('/') ?: "file", mime, size)
    }

    /**
     * A copy of the file in the cache, under its own name (the upload names
     * the file after it). The caller deletes [File.getParentFile] when done.
     */
    fun copy(context: Context, a: Attachment): File {
        val dir = File(context.cacheDir, "uploads/${UUID.randomUUID()}").apply { mkdirs() }
        val file = File(dir, safeName(a.name))
        context.contentResolver.openInputStream(a.uri)?.use { input -> file.outputStream().use { input.copyTo(it) } }
            ?: error("Couldn't read ${a.name}")
        return file
    }

    /** A file name that stays inside its directory. */
    fun safeName(name: String): String =
        name.replace('/', '_').replace('\\', '_').trimStart('.').take(120).ifBlank { "file" }

    /** Width and height of an image file (null if it can't be read). */
    fun imageSize(file: File): Pair<Int, Int>? {
        val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.path, o)
        return if (o.outWidth > 0 && o.outHeight > 0) o.outWidth to o.outHeight else null
    }

    data class VideoMeta(val width: Int?, val height: Int?, val durationMs: Long?)

    fun videoMeta(file: File): VideoMeta = runCatching {
        val r = MediaMetadataRetriever()
        try {
            r.setDataSource(file.path)
            fun n(key: Int) = r.extractMetadata(key)?.toLongOrNull()
            val rotated = n(MediaMetadataRetriever.METADATA_KEY_VIDEO_ROTATION)?.let { it % 180 != 0L } ?: false
            val w = n(MediaMetadataRetriever.METADATA_KEY_VIDEO_WIDTH)?.toInt()
            val h = n(MediaMetadataRetriever.METADATA_KEY_VIDEO_HEIGHT)?.toInt()
            VideoMeta(if (rotated) h else w, if (rotated) w else h, n(MediaMetadataRetriever.METADATA_KEY_DURATION))
        } finally {
            r.release()
        }
    }.getOrDefault(VideoMeta(null, null, null))
}
