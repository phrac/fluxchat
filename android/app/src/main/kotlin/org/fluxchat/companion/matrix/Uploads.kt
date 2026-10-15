package org.fluxchat.companion.matrix

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.provider.OpenableColumns
import androidx.exifinterface.media.ExifInterface
import java.io.File
import java.io.FileOutputStream
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
     * A fresh staging file for a system camera app to write into (via the
     * FileProvider). The caller deletes [File.getParentFile] when done.
     */
    fun stagingFile(context: Context, name: String): File {
        val dir = File(context.cacheDir, "uploads/${UUID.randomUUID()}").apply { mkdirs() }
        return File(dir, safeName(name))
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
        if (a.mime.equals("image/jpeg", true)) normalizeImage(file)
        return file
    }

    /** A file name that stays inside its directory. */
    fun safeName(name: String): String =
        name.replace('/', '_').replace('\\', '_').trimStart('.').take(120).ifBlank { "file" }

    /**
     * Bake the JPEG's EXIF orientation into its pixels and drop the tag.
     * Camera apps write sensor-oriented pixels plus an orientation hint;
     * viewers that honor it show the shot upright, but the agents (and any
     * plain image reader) saw sideways photos. Re-encoding keeps the byte
     * size similar; a failed rewrite leaves the original bytes in place.
     */
    fun normalizeImage(file: File): File = runCatching {
        val jpeg = file.name.endsWith(".jpg", true) || file.name.endsWith(".jpeg", true)
        if (!jpeg) return@runCatching file
        val exif = ExifInterface(file.path)
        val deg = exif.rotationDegrees
        if (deg == 0) return@runCatching file
        val bmp = BitmapFactory.decodeFile(file.path) ?: return@runCatching file
        val m = Matrix().apply { postRotate(deg.toFloat()) }
        val rotated = Bitmap.createBitmap(bmp, 0, 0, bmp.width, bmp.height, m, true)
        FileOutputStream(file).use { rotated.compress(Bitmap.CompressFormat.JPEG, 95, it) }
        if (rotated != bmp) rotated.recycle()
        bmp.recycle()
        runCatching {
            val after = ExifInterface(file.path)
            after.setAttribute(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL.toString())
            after.saveAttributes()
        }
        file
    }.getOrDefault(file)

    /** Width and height as an EXIF-aware viewer sees them (null if unreadable). */
    fun imageSize(file: File): Pair<Int, Int>? {
        val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.path, o)
        if (o.outWidth <= 0 || o.outHeight <= 0) return null
        val sideways = runCatching { ExifInterface(file.path).rotationDegrees % 180 != 0 }.getOrDefault(false)
        return if (sideways) o.outHeight to o.outWidth else o.outWidth to o.outHeight
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
