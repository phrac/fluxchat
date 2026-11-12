package org.fluxchat.companion.matrix

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.time.LocalDateTime

class UploadsTest {
    private val root = File(System.getProperty("java.io.tmpdir"), "uploads-${System.nanoTime()}").apply { mkdirs() }

    @After fun clean() {
        root.deleteRecursively()
    }

    @Test fun `a pending attachment comes back from saved state as it was`() {
        val shot = SavedAttachment("content://org.fluxchat.companion.files/uploads/1b2c/photo-20261111-153045.jpg", "photo-20261111-153045.jpg", "image/jpeg", 2_400_000)
        assertEquals(shot, SavedAttachment.fromList(shot.toList()))
        // A provider that reports no size, and a name no separator would survive.
        val odd = SavedAttachment("content://media/picker/0/1234", "notes\nfinal, v2.txt", "text/plain", null)
        assertEquals(odd, SavedAttachment.fromList(odd.toList()))
    }

    @Test fun `saved state that is not an attachment restores to none`() {
        assertNull(SavedAttachment.fromList(emptyList()))
        assertNull(SavedAttachment.fromList(listOf("content://x/1", "a.txt", "text/plain")))
        assertNull(SavedAttachment.fromList(listOf("", "a.txt", "text/plain", "")))
        assertNull(SavedAttachment.fromList(listOf("content://x/1", "a.txt", "text/plain", "big")))
    }

    @Test fun `camera shots are named by when they were taken, so one never replaces another`() {
        val at = LocalDateTime.of(2026, 11, 11, 15, 30, 45)
        assertEquals("photo-20261111-153045.jpg", Uploads.shotName(video = false, at))
        assertEquals("video-20261111-153045.mp4", Uploads.shotName(video = true, at))
        assertFalse(Uploads.shotName(false, at) == Uploads.shotName(false, at.plusSeconds(1)))
        // What it's named is what it's staged (and uploaded) as.
        assertEquals("photo-20261111-153045.jpg", Uploads.safeName(Uploads.shotName(false, at)))
    }

    @Test fun `only a camera shot's or a shared file's own directory is cleared when its attachment is done with`() {
        assertEquals("uploads/1b2c", Uploads.stagedDir(listOf("uploads", "1b2c", "photo.jpg")))
        assertEquals("files/shared/9f", Uploads.stagedDir(listOf("files", "shared", "9f", "report.pdf")))
        // A downloaded attachment's cache, another app's file, and anything that isn't one directory deep.
        assertNull(Uploads.stagedDir(listOf("files", "abc123", "report.pdf")))
        assertNull(Uploads.stagedDir(listOf("picker", "0", "1234")))
        assertNull(Uploads.stagedDir(listOf("uploads", "photo.jpg")))
        assertNull(Uploads.stagedDir(listOf("uploads", "..", "photo.jpg")))
        assertNull(Uploads.stagedDir(listOf("files", "shared", "..", "x")))
        assertNull(Uploads.stagedDir(emptyList()))
    }

    @Test fun `an ordinary photo is turned upright at full size, and only one too large for the heap is decoded smaller`() {
        val mb = 1024L * 1024
        // 12 MP and 16 MP: never shrunk, however little memory there is (agents read them).
        assertEquals(1, Uploads.sampleSizeFor(4000, 3000, 2048 * mb))
        assertEquals(1, Uploads.sampleSizeFor(4080, 3072, 8 * mb))
        assertEquals(1, Uploads.sampleSizeFor(4624, 3468, 0))
        // 50 MP is 200 MB decoded, twice while rotating: full size where that fits…
        assertEquals(1, Uploads.sampleSizeFor(8160, 6120, 1024 * mb))
        // …and at half the side where it doesn't.
        assertEquals(2, Uploads.sampleSizeFor(8160, 6120, 256 * mb))
        // 200 MP.
        assertEquals(2, Uploads.sampleSizeFor(16320, 12240, 1024 * mb))
        assertEquals(4, Uploads.sampleSizeFor(16320, 12240, 256 * mb))
        // Never an endless loop on nonsense.
        assertEquals(64, Uploads.sampleSizeFor(40_000, 30_000, 0))
    }

    @Test fun `staging directories nobody came back for go after a day, newer ones stay`() {
        val now = 10L * 24 * 60 * 60 * 1000
        val old = File(root, "old").apply { mkdirs() }
        File(old, "photo.jpg").writeText("x")
        val fresh = File(root, "fresh").apply { mkdirs() }
        File(fresh, "photo.jpg").writeText("y")
        assertTrue(old.setLastModified(now - 25L * 60 * 60 * 1000))
        assertTrue(fresh.setLastModified(now - 60 * 60 * 1000))
        Uploads.sweep(root, now)
        assertFalse(old.exists())
        assertTrue(File(fresh, "photo.jpg").exists())
        // Nothing staged yet: nothing to do.
        Uploads.sweep(File(root, "missing"), now)
    }
}
