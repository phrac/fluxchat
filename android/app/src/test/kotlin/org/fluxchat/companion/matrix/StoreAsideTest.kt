package org.fluxchat.companion.matrix

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class StoreAsideTest {
    private val root = File(System.getProperty("java.io.tmpdir"), "storeaside-${System.nanoTime()}").apply { mkdirs() }
    private val store = File(root, "matrix")
    private val cache = File(root, "cache-matrix")
    private val aside = StoreAside(store, cache)

    private fun fill(dir: File, text: String) {
        dir.mkdirs()
        File(dir, "crypto.db").writeText(text)
    }

    private fun contents(dir: File) = File(dir, "crypto.db").takeIf { it.exists() }?.readText()

    @After fun clean() {
        root.deleteRecursively()
    }

    @Test fun `a failed sign-in gets the previous store back`() {
        fill(store, "old keys")
        fill(cache, "old cache")
        aside.setAside()
        assertFalse(store.exists())
        fill(store, "half a new device")
        fill(cache, "its cache")
        aside.putBack()
        assertEquals("old keys", contents(store))
        assertFalse("the cache is keyed to the store it was made for", cache.exists())
    }

    @Test fun `a sign-in that worked drops the previous store`() {
        fill(store, "old keys")
        aside.setAside()
        fill(store, "new device")
        aside.drop()
        assertEquals("new device", contents(store))
        aside.putBack() // nothing waiting: the new store stays
        assertEquals("new device", contents(store))
    }

    @Test fun `an interrupted sign-in is undone at the next start, and a second attempt keeps the original`() {
        fill(store, "old keys")
        aside.setAside()
        fill(store, "attempt one")
        // The process died here. Another attempt before any restart:
        aside.setAside()
        fill(store, "attempt two")
        aside.putBack()
        assertEquals("old keys", contents(store))
    }

    @Test fun `with nothing to set aside there is nothing to put back`() {
        aside.setAside()
        aside.putBack()
        assertFalse(store.exists())
        assertTrue(root.exists())
    }

    @Test fun `a drop that was interrupted never comes back over the new store`() {
        fill(store, "old keys")
        aside.setAside()
        fill(store, "new device")
        // The process died mid-delete: what's left of the old store sits in
        // the discard directory, not in `.previous`.
        File(root, "matrix.previous").renameTo(File(root, "matrix.discard"))
        aside.putBack()
        assertEquals("new device", contents(store))
        assertFalse(File(root, "matrix.discard").exists())
    }
}
