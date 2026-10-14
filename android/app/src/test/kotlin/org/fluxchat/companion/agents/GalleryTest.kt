package org.fluxchat.companion.agents

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.LocalDate
import java.time.ZoneId

/**
 * Ports the desktop's gallery tests (`update_gallery.rs` / `gallery.rs`)
 * test-for-test onto the Kotlin pure logic; the paging itself goes through
 * the Timeline API on this side and is device-verified.
 */
class GalleryTest {
    private val zone = ZoneId.of("UTC")
    private fun ts(y: Int, m: Int, d: Int): Long = LocalDate.of(y, m, d).atStartOfDay(zone).toInstant().toEpochMilli()

    private fun image(id: String, t: Long, threadRoot: String? = null) =
        Msg(id, "@a:x", "a", "pic", t, isOwn = false, threadRoot = threadRoot, media = Media("mxc://x/$id", null, 10, 10))

    private fun video(id: String, t: Long) = Msg(
        id, "@a:x", "a", "clip", t, isOwn = false,
        file = FileRef("mxc://x/$id", "clip.mp4", "video/mp4", 5, FileKind.Video, Media("mxc://x/${id}_thumb", null, 10, 10)),
    )

    private fun file(id: String, t: Long) =
        Msg(id, "@a:x", "a", "doc.pdf", t, isOwn = false, file = FileRef("mxc://x/$id", "doc.pdf", "application/pdf", 1234, FileKind.File))

    private fun imageSentAsFile(id: String, t: Long) =
        Msg(id, "@a:x", "a", "shot.png", t, isOwn = false, file = FileRef("mxc://x/$id", "shot.png", "image/png", 9, FileKind.File))

    private fun text(id: String, t: Long) = Msg(id, "@a:x", "a", "hi", t, isOwn = false)

    @Test
    fun tabsSplitLikeTheDesktopsFilters() {
        assertTrue(isGalleryMedia(image("i", 10)))
        assertTrue("videos sit on the Media tab", isGalleryMedia(video("v", 10)))
        assertTrue("images sent as files are media, like the desktop", isGalleryMedia(imageSentAsFile("p", 10)))
        assertFalse(isGalleryMedia(file("f", 10)))
        assertTrue(isGalleryFile(file("f", 10)))
        assertFalse("a text message belongs to neither tab", isGalleryMedia(text("t", 10)) || isGalleryFile(text("t", 10)))
        assertFalse("a video is not a file-tab item", isGalleryFile(video("v", 10)))
    }

    @Test
    fun loadedAttachmentsShowFirstAndServerPagesMergeWithoutDuplicates() {
        // loaded_media_shows_first_then_server_pages_merge: loaded items first
        // (text left out), then a server page that repeats and extends.
        val loaded = listOf(text("text", 1), file("f", 50), image("i", 100))
        val items = ArrayList<Msg>()
        mergeGalleryItems(items, loaded.filter { it.sendState == SendState.Sent && (isGalleryMedia(it) || isGalleryFile(it)) })
        assertEquals(listOf("i", "f"), items.map { it.id })
        mergeGalleryItems(items, listOf(image("i", 100), image("old", 10)))
        assertEquals("\$i stays once; older lands last", listOf("i", "f", "old"), items.map { it.id })
    }

    @Test
    fun mergeKeepsNewestFirstAndIsStableWithinASecond() {
        val items = ArrayList(listOf(image("a", 100)))
        mergeGalleryItems(items, listOf(image("b", 100), image("c", 300)))
        assertEquals("same-second items keep incoming order; newer first overall", listOf("c", "a", "b"), items.map { it.id })
    }

    @Test
    fun monthSectionsGroupConsecutiveItemsOnly() {
        val items = listOf(
            image("oct2", ts(2026, 10, 12)), image("oct1", ts(2026, 10, 3)),
            image("sep1", ts(2026, 9, 28)), image("oct0", ts(2026, 10, 1)),
        )
        val sections = galleryMonthSections(items, zone)
        assertEquals(
            listOf("October 2026" to (0 to 2), "September 2026" to (2 to 3), "October 2026" to (3 to 4)),
            sections.map { it.label to (it.start to it.end) },
        )
    }

    @Test
    fun automaticLoadingStopsAtTheCap() {
        // automatic_loading_stops_at_its_cap: a page of 2000 events with
        // nothing in it (an encrypted room) waits for "Load older".
        assertFalse(galleryShouldLoadMore(GALLERY_BURST_EVENTS, null, done = false))
        assertTrue(galleryShouldLoadMore(GALLERY_BURST_EVENTS - 100, null, done = false))
        assertFalse("the start of history is reached", galleryShouldLoadMore(0, null, done = true))
        assertFalse("an error stops the burst", galleryShouldLoadMore(0, "boom", done = false))
    }

    @Test
    fun onlyRealImagesStepThroughTheLightbox() {
        assertTrue(galleryIsImage(image("i", 1)))
        assertTrue(galleryIsImage(imageSentAsFile("p", 1)))
        assertFalse("videos play in the timeline, like the desktop", galleryIsImage(video("v", 1)))
        assertEquals("mxc://x/v_thumb", galleryMediaOf(video("v", 1))?.source)
        assertEquals("mxc://x/p", galleryMediaOf(imageSentAsFile("p", 1))?.source)
        assertNull(galleryMediaOf(file("f", 1)))
    }
}
