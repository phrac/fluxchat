package org.fluxchat.companion.agents

import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * Media gallery (ports the desktop's `update_gallery.rs` semantics): every
 * image, video and file of a project or a session, newest first, filled from
 * the server page by page so it isn't limited to loaded history.
 * Pure logic here; the paging lives in `matrix/GalleryView.kt`.
 */

/** Which list the gallery shows: images and videos, or files and audio. */
enum class GalleryTab { Media, Files }

/** Whether a message belongs on the Media tab (images, stickers, videos). */
fun isGalleryMedia(m: Msg): Boolean =
    m.media != null || m.file?.kind == FileKind.Video ||
        m.file?.kind == FileKind.File && m.file.mime?.startsWith("image/") == true

/** Whether a message belongs on the Files tab (files and audio). */
fun isGalleryFile(m: Msg): Boolean = m.file != null && !isGalleryMedia(m)

/** Lightbox steppables: real images (videos play in the timeline, like the desktop). */
fun galleryIsImage(m: Msg): Boolean =
    m.media != null || m.file?.kind == FileKind.File && m.file.mime?.startsWith("image/") == true

/** What to show in the lightbox for a Media-tab item (a file-sent image uses its own source). */
fun galleryMediaOf(m: Msg): Media? = m.media ?: m.file?.let { f ->
    when {
        f.kind == FileKind.Video -> f.thumbnail
        f.mime?.startsWith("image/") == true -> Media(f.source, f.thumbnail?.source, null, null)
        else -> null
    }
}

/**
 * Add [new] to [items]: one per event, newest first; same-second items keep
 * the incoming (server, newest-first) order — `sort` is stable, like the
 * desktop's `merge_items`.
 */
fun mergeGalleryItems(items: MutableList<Msg>, new: List<Msg>) {
    val seen = items.mapTo(HashSet()) { it.id }
    for (m in new) if (seen.add(m.id)) items.add(m)
    items.sortByDescending { it.ts }
}

/** Consecutive runs of items from the same month: ("October 2026", start, end). */
fun galleryMonthSections(items: List<Msg>, zone: ZoneId = ZoneId.systemDefault()): List<GalleryMonth> {
    val sections = ArrayList<GalleryMonth>()
    for ((i, m) in items.withIndex()) {
        val label = galleryMonthLabel(m.ts, zone)
        val last = sections.lastOrNull()
        if (last != null && galleryMonthLabel(items[last.end - 1].ts, zone) == label)
            sections[sections.size - 1] = last.copy(end = i + 1)
        else sections.add(GalleryMonth(label, i, i + 1))
    }
    return sections
}

fun galleryMonthLabel(ts: Long, zone: ZoneId = ZoneId.systemDefault()): String =
    Instant.ofEpochMilli(ts).atZone(zone).format(GalleryMonthFormat)

private val GalleryMonthFormat = DateTimeFormatter.ofPattern("MMMM yyyy", Locale.ENGLISH)

/** A run of gallery items sharing a month (indices into the newest-first list). */
data class GalleryMonth(val label: String, val start: Int, val end: Int)

/**
 * How far automatic loading goes before waiting for a "Load older" tap: in
 * an encrypted project the server can't pick out attachments, so filling the
 * view can mean reading a lot of ordinary messages. Ports GALLERY_BURST_EVENTS.
 */
const val GALLERY_BURST_EVENTS = 2000

/** Whether another page is fetched automatically (ports `gallery_refresh`'s may_load;
 * "already paging" is the caller's own guard, as on desktop). */
fun galleryShouldLoadMore(scannedInBurst: Int, error: String?, done: Boolean): Boolean =
    !done && error == null && scannedInBurst < GALLERY_BURST_EVENTS
