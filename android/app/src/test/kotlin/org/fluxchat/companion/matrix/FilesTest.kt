package org.fluxchat.companion.matrix

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** A message image has no file name: saved or shared, it is named for what its bytes are. */
class FilesTest {
    private fun bytes(vararg b: Int) = ByteArray(b.size) { b[it].toByte() }

    @Test
    fun anImageIsNamedForItsBytes() {
        assertEquals("jpg" to "image/jpeg", imageType(bytes(0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10)))
        assertEquals("png" to "image/png", imageType(bytes(0x89, 'P'.code, 'N'.code, 'G'.code, 0x0D, 0x0A)))
        assertEquals("gif" to "image/gif", imageType("GIF89a".toByteArray()))
        assertEquals("webp" to "image/webp", imageType("RIFF\u0000\u0000\u0000\u0000WEBPVP8 ".toByteArray(Charsets.ISO_8859_1)))
        assertEquals("heic" to "image/heic", imageType("\u0000\u0000\u0000\u0018ftypheic".toByteArray(Charsets.ISO_8859_1)))
    }

    @Test
    fun bytesThatAreNoImageWeKnowAreNotCalledOne() {
        assertNull(imageType("<html>".toByteArray()))
        assertNull(imageType(ByteArray(0)))
        assertNull("too short to tell", imageType(bytes(0xFF, 0xD8)))
        // What isn't a known image is saved as plain bytes, not mislabelled.
        assertEquals("application/octet-stream", imageMime("image-1a2b3c4d.img"))
    }

    @Test
    fun theTypeFollowsTheNameAnImageWasGiven() {
        assertEquals("image/jpeg", imageMime("image-1a2b3c4d.jpg"))
        assertEquals("image/png", imageMime("IMAGE.PNG"))
        assertEquals("image/webp", imageMime("a.b.webp"))
    }
}
