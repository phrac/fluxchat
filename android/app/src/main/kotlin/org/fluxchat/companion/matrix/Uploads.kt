package org.fluxchat.companion.matrix

import android.app.ActivityManager
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
import org.fluxchat.companion.Diag

/** A file picked to send: what the composer shows, and where to read it from. */
data class Attachment(val uri: Uri, val name: String, val mime: String, val size: Long?) {
    val isImage: Boolean get() = mime.startsWith("image/")
    val isVideo: Boolean get() = mime.startsWith("video/")
}

/**
 * A pending [Attachment] as the plain values a saved instance state keeps
 * (the composer's attachment survives the activity being recreated).
 */
data class SavedAttachment(val uri: String, val name: String, val mime: String, val size: Long?) {
    fun toList(): List<String> = listOf(uri, name, mime, size?.toString().orEmpty())

    companion object {
        /** Null for anything that isn't what [toList] wrote. */
        fun fromList(saved: List<String>): SavedAttachment? {
            if (saved.size != 4 || saved[0].isEmpty()) return null
            val size = if (saved[3].isEmpty()) null else saved[3].toLongOrNull() ?: return null
            return SavedAttachment(saved[0], saved[1], saved[2], size)
        }
    }
}

/** Picked files, ready for the SDK's upload (which reads a path). */
object Uploads {
    /** Staging directories nothing came back for are deleted after this long. */
    private const val STALE_STAGING_MS = 24 * 60 * 60 * 1000L
    /** Share of the memory the system can still spare that a photo's rotation may take. */
    private const val ROTATE_MEMORY_SHARE = 0.5
    /** Photos up to this many pixels (phone cameras' usual 12–16 MP) are always turned upright at full size. */
    private const val ORDINARY_PIXELS = 16_500_000L
    /** Bytes per pixel of a decoded bitmap (ARGB_8888). */
    private const val BYTES_PER_PIXEL = 4L
    /** The smallest a photo too large for the phone's memory is decoded at: 1/this. */
    private const val MAX_SAMPLE = 64
    @Volatile private var swept = false
    private val normalizing = Any()

    fun saved(a: Attachment): List<String> = SavedAttachment(a.uri.toString(), a.name, a.mime, a.size).toList()

    fun restored(saved: List<String>): Attachment? =
        SavedAttachment.fromList(saved)?.let { Attachment(Uri.parse(it.uri), it.name, it.mime, it.size) }

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
        sweepOnce(context)
        val dir = File(context.cacheDir, "uploads/${UUID.randomUUID()}").apply { mkdirs() }
        return File(dir, safeName(name))
    }

    /**
     * A camera shot's file name. With the time in it: every shot used to be
     * `photo.jpg`, and an agent that saves attachments by name overwrote the
     * one before.
     */
    fun shotName(video: Boolean, at: java.time.LocalDateTime): String {
        val stamp = at.format(java.time.format.DateTimeFormatter.ofPattern("yyyyMMdd-HHmmss", java.util.Locale.US))
        return if (video) "video-$stamp.mp4" else "photo-$stamp.jpg"
    }

    /**
     * Where under the cache directory a staged file's own directory is, from
     * the path of its FileProvider URI (`res/xml/file_paths.xml`): a camera
     * shot's `uploads/<id>/<name>`, or a shared file's
     * `files/shared/<id>/<name>`. Null for anything else — a picked file
     * belongs to another app, and a downloaded attachment (`files/<hash>/…`)
     * is the cache of something received, not ours to clear here.
     */
    fun stagedDir(pathSegments: List<String>): String? = when {
        pathSegments.any { it == "." || it == ".." || it.isEmpty() } -> null
        pathSegments.size == 3 && pathSegments[0] == "uploads" -> "uploads/${pathSegments[1]}"
        pathSegments.size == 4 && pathSegments[0] == "files" && pathSegments[1] == "shared" -> "files/shared/${pathSegments[2]}"
        else -> null
    }

    /**
     * Done with an attachment (sent, or taken off the composer): if it was
     * staged in our cache — a camera shot, a shared file — its directory
     * goes. Nothing deleted these, so every photo taken stayed on the phone
     * until sign-out.
     */
    fun release(context: Context, a: Attachment) {
        if (a.uri.authority != "${context.packageName}.files") return
        val dir = stagedDir(a.uri.pathSegments) ?: return
        val staged = File(context.cacheDir, dir)
        if (staged.exists() && !staged.deleteRecursively()) Diag.error("Clearing the staged copy of ${a.name}")
    }

    /**
     * Once per process: staging directories left behind (the app was killed
     * with a shot on the composer, or mid-upload) go after a day.
     */
    private fun sweepOnce(context: Context) {
        if (swept) return
        swept = true
        sweep(File(context.cacheDir, "uploads"), System.currentTimeMillis())
    }

    internal fun sweep(uploads: File, now: Long) {
        uploads.listFiles()?.forEach { dir ->
            if (now - dir.lastModified() > STALE_STAGING_MS && !dir.deleteRecursively()) Diag.error("Clearing an old upload in ${dir.name}")
        }
    }

    /**
     * A copy of the file in the cache, under its own name (the upload names
     * the file after it). The caller deletes [File.getParentFile] when done.
     */
    fun copy(context: Context, a: Attachment): File {
        sweepOnce(context)
        val dir = File(context.cacheDir, "uploads/${UUID.randomUUID()}").apply { mkdirs() }
        try {
            val file = File(dir, safeName(a.name))
            context.contentResolver.openInputStream(a.uri)?.use { input -> file.outputStream().use { input.copyTo(it) } }
                ?: error("Couldn't read ${a.name}")
            if (a.mime.equals("image/jpeg", true)) normalizeImage(context, file)
            return file
        } catch (e: Throwable) {
            // The caller never gets the file, so it can't clean up after it:
            // an unreadable pick (or a full disk) left its directory behind.
            dir.deleteRecursively()
            throw e
        }
    }

    /** A file name that stays inside its directory. */
    fun safeName(name: String): String =
        name.replace('/', '_').replace('\\', '_').trimStart('.').take(120).ifBlank { "file" }

    /**
     * Bake the JPEG's EXIF orientation into its pixels (the re-encoded file
     * carries no orientation tag). Camera apps write sensor-oriented pixels
     * plus an orientation hint; viewers that honor it show the shot upright,
     * but the agents (and any plain image reader) saw sideways photos.
     *
     * The upright copy is written beside the file and renamed over it, so a
     * rewrite that fails leaves the original as it was (it used to truncate
     * the original first). Rotating holds the decoded photo twice (it used
     * to be decoded at full size whatever its size); one too large for the
     * phone's memory is decoded smaller ([sampleSizeFor]) rather than sent
     * sideways. If memory runs out all the same it's tried once at half the
     * size, and failing that the original goes as it is — each of these noted
     * in Diagnostics, where it used to be silent.
     */
    fun normalizeImage(context: Context, file: File): File {
        val jpeg = file.name.endsWith(".jpg", true) || file.name.endsWith(".jpeg", true)
        if (!jpeg) return file
        // One at a time: a second pass over the same shot (the screen was
        // recreated mid-way) waits, then finds it upright already.
        synchronized(normalizing) {
            val tmp = File(file.parentFile, file.name + ".upright")
            try {
                val deg = ExifInterface(file.path).rotationDegrees
                if (deg == 0) return file
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeFile(file.path, bounds)
                if (bounds.outWidth <= 0 || bounds.outHeight <= 0) {
                    Diag.error("Turning ${file.name} upright: not a readable image, sent as it is")
                    return file
                }
                var sample = sampleSizeFor(bounds.outWidth, bounds.outHeight, spareMemory(context))
                try {
                    rotate(file, tmp, deg, sample)
                } catch (e: OutOfMemoryError) {
                    // The estimate was off (other threads allocate too): once more, at half the size.
                    sample *= 2
                    rotate(file, tmp, deg, sample)
                }
                if (sample > 1) {
                    Diag.note("${file.name} (${bounds.outWidth}×${bounds.outHeight}) was too large to turn upright at full size: sent at 1/$sample")
                }
            } catch (e: OutOfMemoryError) {
                Diag.error("Turning ${file.name} upright ran out of memory: sent as taken (agents may see it sideways)", e)
            } catch (e: Exception) {
                Diag.error("Turning ${file.name} upright: sent as taken", e)
            } finally {
                tmp.delete()
            }
            return file
        }
    }

    /**
     * What the system can still give us before it starts killing things.
     * Not the Java heap: a bitmap's pixels live outside it (since Android 8),
     * so a photo that doesn't fit gets the app killed rather than an
     * OutOfMemoryError to catch.
     */
    private fun spareMemory(context: Context): Long {
        val info = ActivityManager.MemoryInfo()
        (context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager).getMemoryInfo(info)
        return (info.availMem - info.threshold).coerceAtLeast(0)
    }

    /** Decode [file] at 1/[sample], rotate it by [deg], write that to [tmp] and rename it over [file]. */
    private fun rotate(file: File, tmp: File, deg: Int, sample: Int) {
        val bmp = BitmapFactory.decodeFile(file.path, BitmapFactory.Options().apply { inSampleSize = sample })
            ?: error("couldn't decode it")
        val rotated = try {
            Bitmap.createBitmap(bmp, 0, 0, bmp.width, bmp.height, Matrix().apply { postRotate(deg.toFloat()) }, true)
        } catch (e: Throwable) {
            bmp.recycle()
            throw e
        }
        try {
            // (createBitmap may hand the same bitmap back)
            if (rotated !== bmp) bmp.recycle()
            FileOutputStream(tmp).use { if (!rotated.compress(Bitmap.CompressFormat.JPEG, 95, it)) error("couldn't encode it") }
        } finally {
            rotated.recycle()
        }
        if (!tmp.renameTo(file)) error("couldn't replace the original")
    }

    /**
     * The `inSampleSize` (a power of two) at which a [width]×[height] photo
     * can be rotated within [freeBytes] of memory: rotating keeps the decoded
     * source and the rotated copy alive together. 1 — full size — whenever
     * that fits, and always for an ordinary photo: agents read these, so
     * they aren't shrunk on an estimate (only if memory actually runs out,
     * see [normalizeImage]).
     */
    fun sampleSizeFor(width: Int, height: Int, freeBytes: Long): Int {
        if (width.toLong() * height <= ORDINARY_PIXELS) return 1
        val budget = (freeBytes * ROTATE_MEMORY_SHARE).toLong()
        var sample = 1
        while (sample < MAX_SAMPLE && 2 * BYTES_PER_PIXEL * (width.toLong() / sample) * (height.toLong() / sample) > budget) sample *= 2
        return sample
    }

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
