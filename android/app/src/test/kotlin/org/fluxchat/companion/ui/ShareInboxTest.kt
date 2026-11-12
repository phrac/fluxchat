package org.fluxchat.companion.ui

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.io.File
import java.nio.file.Files

/**
 * What's shared into the app waits in the inbox for its project, and its
 * file is a copy in our cache. The copy was never removed — every share left
 * one behind, sent or not.
 */
class ShareInboxTest {
    private lateinit var root: File
    private val hour = 60 * 60 * 1000L

    @Before
    fun setUp() {
        root = Files.createTempDirectory("shared").toFile()
        ShareInbox.pending.value = null
        ShareInbox.target.value = null
    }

    @After
    fun tearDown() {
        ShareInbox.pending.value = null
        ShareInbox.target.value = null
        root.deleteRecursively()
    }

    /** A copy as `fromIntent` leaves it: its own folder, the file inside. */
    private fun copy(name: String, claimed: Boolean = true): File {
        val dir = File(root, name)
        if (claimed) ShareInbox.claim(dir)
        dir.mkdirs()
        return File(dir, "photo.jpg").apply { writeText("x") }
    }

    @Test fun `a share nobody picked a project for takes its file with it`() {
        val file = copy("a")
        ShareInbox.offer(Shared("look at this", null, file = file))
        ShareInbox.discard()
        assertNull(ShareInbox.pending.value)
        assertFalse(file.parentFile!!.exists())
    }

    @Test fun `a newer share replaces one still waiting, and its file`() {
        val old = copy("a")
        val new = copy("b")
        ShareInbox.offer(Shared(null, null, file = old))
        ShareInbox.offer(Shared("second", null, file = new))
        assertEquals("second", ShareInbox.pending.value?.text)
        assertFalse(old.parentFile!!.exists())
        assertTrue(new.exists())
    }

    @Test fun `the file stays while its project's composer has it`() {
        // Taken is not sent: the composer still reads the copy when you press Send.
        val file = copy("a")
        ShareInbox.offer(Shared(null, null, file = file))
        ShareInbox.target.value = "!r:x"
        assertNull(ShareInbox.take("!other:x"))
        assertTrue(ShareInbox.take("!r:x") != null)
        ShareInbox.sweep(root, now = file.parentFile!!.lastModified() + hour)
        assertTrue(file.exists())
        // …but not for ever: nothing tells the inbox it was sent.
        ShareInbox.sweep(root, now = file.parentFile!!.lastModified() + ShareInbox.KEEP_MS + hour)
        assertFalse(file.parentFile!!.exists())
    }

    @Test fun `copies an earlier process left behind are swept`() {
        // Nothing shared survives the process (the inbox is in memory), so
        // a folder this process didn't make is a leftover, whatever its age.
        val leftover = copy("old", claimed = false)
        val mine = copy("mine")
        ShareInbox.sweep(root, now = System.currentTimeMillis())
        assertFalse(leftover.parentFile!!.exists())
        assertTrue(mine.exists())
    }

    @Test fun `the share still waiting for a project is never swept`() {
        val file = copy("a")
        ShareInbox.offer(Shared(null, null, file = file))
        ShareInbox.sweep(root, now = file.parentFile!!.lastModified() + 10 * ShareInbox.KEEP_MS)
        assertTrue(file.exists())
    }

    @Test fun `the quick prompt's draft is handed to its project's composer`() {
        assertTrue(ShareInbox.handOver("!r:x", "fix the build"))
        // Taken exactly as shared text is: by that project, once.
        assertNull(ShareInbox.take("!other:x"))
        assertEquals("fix the build", ShareInbox.take("!r:x")?.text)
        assertNull(ShareInbox.take("!r:x"))
    }

    @Test fun `a draft does not push aside a share waiting for its project`() {
        val file = copy("a")
        ShareInbox.offer(Shared(null, null, file = file))
        assertFalse(ShareInbox.handOver("!r:x", "fix the build"))
        assertTrue(file.exists())
        assertNull(ShareInbox.target.value)
    }

    @Test fun `a hand-over that never reached a composer is replaced by the next`() {
        assertTrue(ShareInbox.handOver("!r:x", "first"))
        assertTrue(ShareInbox.handOver("!s:x", "second"))
        assertNull(ShareInbox.take("!r:x"))
        assertEquals("second", ShareInbox.take("!s:x")?.text)
    }

    @Test fun `only another app's content can be shared in`() {
        val own = "org.fluxchat.companion.files"
        assertTrue(ShareInbox.acceptable("content", "media", own))
        assertTrue(ShareInbox.acceptable("content", "com.android.providers.downloads.documents", own))
        // A file path we can open is one of our own private files (the
        // session store, decrypted attachments): a share naming it is asking
        // us to send that somewhere.
        assertFalse(ShareInbox.acceptable("file", null, own))
        assertFalse(ShareInbox.acceptable("file", "", own))
        // Nobody else can hold a grant to our provider.
        assertFalse(ShareInbox.acceptable("content", own, own))
        assertFalse(ShareInbox.acceptable(null, null, own))
        assertFalse(ShareInbox.acceptable("content", null, own))
        assertFalse(ShareInbox.acceptable("http", "example.org", own))
    }
}
