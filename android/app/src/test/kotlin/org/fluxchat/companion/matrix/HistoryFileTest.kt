package org.fluxchat.companion.matrix

import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class HistoryFileTest {
    private val root = File(System.getProperty("java.io.tmpdir"), "historyfile-${System.nanoTime()}").apply { mkdirs() }
    private val file = File(root, "runs.bin")
    private val aside = File(root, "runs.bin.unreadable")
    private val reports = ArrayList<String>()

    /** The keystore: works, or not. */
    private var keystoreUp = true

    /** One process: a start reads the file once, then saves as runs come in. */
    private fun process() = HistoryFile(
        file,
        seal = { "sealed:".toByteArray() + it },
        open = { if (keystoreUp) it.copyOfRange("sealed:".length, it.size) else throw IllegalStateException("keystore busy") },
    ) { msg, _ -> reports += msg }

    @After fun clean() {
        root.deleteRecursively()
    }

    @Test fun `what one process saved the next one reads`() {
        process().save("three runs".toByteArray())
        assertArrayEquals("three runs".toByteArray(), process().load())
        assertTrue(reports.isEmpty())
    }

    @Test fun `no file is no history, and saving starts one`() {
        val p = process()
        assertNull(p.load())
        assertTrue(p.save("first run".toByteArray()))
        assertArrayEquals("first run".toByteArray(), process().load())
    }

    @Test fun `a history that couldn't be read is not saved over`() {
        process().save("a year of runs".toByteArray())
        keystoreUp = false
        val p = process()
        assertNull(p.load())
        // The first run recorded after start used to save a new, nearly
        // empty history over the one that merely couldn't be opened.
        assertFalse(p.save("one new run".toByteArray()))
        assertFalse(p.save("two new runs".toByteArray()))
        keystoreUp = true
        assertArrayEquals("a year of runs".toByteArray(), process().load())
    }

    @Test fun `the failure and the skipped save are reported, the skip once`() {
        process().save("runs".toByteArray())
        keystoreUp = false
        val p = process()
        p.load()
        p.save("x".toByteArray())
        p.save("y".toByteArray())
        assertEquals(2, reports.size)
        assertTrue(reports[0], reports[0].startsWith("Reading the run history"))
        assertTrue(reports[1], reports[1].startsWith("Not saving the run history"))
    }

    @Test fun `a keystore that recovers gets saving back`() {
        process().save("runs".toByteArray())
        keystoreUp = false
        process().load()
        keystoreUp = true
        val p = process()
        assertArrayEquals("runs".toByteArray(), p.load())
        assertTrue(p.save("runs and one more".toByteArray()))
        assertArrayEquals("runs and one more".toByteArray(), process().load())
    }

    @Test fun `unreadable three starts in a row it is set aside, not deleted, and a new one starts`() {
        process().save("runs under a lost key".toByteArray())
        val sealed = file.readBytes()
        keystoreUp = false
        repeat(2) {
            val p = process()
            assertNull(p.load())
            assertFalse(p.save("new".toByteArray()))
        }
        assertFalse(aside.exists())
        val third = process()
        assertNull(third.load())
        assertArrayEquals("kept, in case the key comes back", sealed, aside.readBytes())
        assertFalse(file.exists())
        // Otherwise a key that is really gone meant no history was ever saved again.
        assertTrue(third.save("a new history".toByteArray()))
        keystoreUp = true
        assertArrayEquals("a new history".toByteArray(), process().load())
    }

    @Test fun `a good read in between starts the count again`() {
        process().save("runs".toByteArray())
        keystoreUp = false
        process().load()
        process().load()
        keystoreUp = true
        process().load()
        keystoreUp = false
        process().load()
        process().load()
        assertTrue("two failures since the good read, not four", file.exists())
        assertFalse(aside.exists())
    }

    @Test fun `trying again within one process is one failed start, not several`() {
        process().save("runs".toByteArray())
        keystoreUp = false
        val p = process()
        repeat(5) { assertNull(p.load()) }
        assertTrue(file.exists())
        assertFalse(aside.exists())
    }

    @Test fun `signing out removes the history and the copy set aside`() {
        process().save("runs".toByteArray())
        keystoreUp = false
        repeat(3) { process().load() }
        assertTrue(aside.exists())
        val p = process()
        p.save("new".toByteArray())
        p.clear()
        assertTrue(root.listFiles().orEmpty().isEmpty())
    }
}
