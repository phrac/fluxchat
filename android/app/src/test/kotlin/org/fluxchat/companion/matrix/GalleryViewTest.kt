package org.fluxchat.companion.matrix

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.fluxchat.companion.agents.GALLERY_BURST_EVENTS
import org.fluxchat.companion.agents.Media
import org.fluxchat.companion.agents.Msg
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Paging over a fake pager (the bindings' timeline isn't available off-device):
 * the pump must actually page, stop at the burst cap, treat the "Load older"
 * tap as the only burst reset, report failures instead of spinning, and drop
 * its timeline on close.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35]) // like ScreenshotTest: the newer sandbox image breaks on the build host's JDK
class GalleryViewTest {
    private val scope = CoroutineScope(Dispatchers.Unconfined + Job())

    private fun image(id: String, t: Long) =
        Msg(id, "@a:x", "a", "pic", t, isOwn = false, media = Media("mxc://x/$id", null, 10, 10))

    /** Pages out [pages], then reports the start of history. */
    private class Paged(val pages: List<List<Msg>>) : GalleryPager {
        private val _messages = MutableStateFlow<List<Msg>>(emptyList())
        override val messages: StateFlow<List<Msg>> = _messages
        private val _atStart = MutableStateFlow(false)
        override val atStart: StateFlow<Boolean> = _atStart
        var calls = 0
            private set
        var closed = false
            private set

        override suspend fun paginate(events: Int): Boolean {
            val page = pages.getOrNull(calls++) ?: run { _atStart.value = true; return false }
            _messages.value = _messages.value + page
            return calls < pages.size
        }

        override fun close() {
            closed = true
        }
    }

    /** Never-ending empty history (an encrypted project with no media). */
    private class Endless : GalleryPager {
        private val _messages = MutableStateFlow<List<Msg>>(emptyList())
        override val messages: StateFlow<List<Msg>> = _messages
        private val _atStart = MutableStateFlow(false)
        override val atStart: StateFlow<Boolean> = _atStart
        var calls = 0
            private set

        override suspend fun paginate(events: Int): Boolean {
            calls++
            return true
        }

        override fun close() {}
    }

    @Test
    fun attachingPagesThroughHistoryToTheStart() {
        val pager = Paged(listOf(listOf(image("i1", 10)), listOf(image("i2", 20))))
        val view = GalleryView("!r:x", null, scope)
        view.attach(pager) // Unconfined: the pump has run by the time attach returns
        val s = view.state.value
        assertEquals(listOf("i2", "i1"), s.items.map { it.id })
        assertTrue("the start of history ends paging", s.done)
        assertFalse(s.loading)
        assertEquals(2, pager.calls) // the last page itself reports the start, as paginateBackwards does
        assertEquals(200, s.burst)
    }

    @Test
    fun automaticPagingStopsAtTheBurstCap() {
        val pager = Endless()
        val view = GalleryView("!r:x", null, scope)
        view.attach(pager)
        val s = view.state.value
        assertEquals("one burst of pages, then it waits for a tap", GALLERY_BURST_EVENTS / 100, pager.calls)
        assertEquals(GALLERY_BURST_EVENTS, s.burst)
        assertFalse("not at the start of history", s.done)
        assertFalse("no spinner while waiting for the tap", s.loading)

        // Scrolling near the end keeps the cap; the tap is what starts a new burst.
        view.continueLoading()
        assertEquals(GALLERY_BURST_EVENTS / 100, pager.calls)
        view.loadMore()
        assertEquals(2 * GALLERY_BURST_EVENTS / 100, pager.calls)
        assertEquals(GALLERY_BURST_EVENTS, view.state.value.burst)
    }

    @Test
    fun aPagingFailureIsReportedAndKeepsWhatWasLoaded() {
        val pager = object : GalleryPager {
            override val messages: StateFlow<List<Msg>> = MutableStateFlow(emptyList())
            override val atStart: StateFlow<Boolean> = MutableStateFlow(false)
            var calls = 0
            override suspend fun paginate(events: Int): Boolean {
                calls++
                throw java.io.IOException("network down")
            }

            override fun close() {}
        }
        val view = GalleryView("!r:x", null, scope)
        view.seed(listOf(image("kept", 1)))
        view.attach(pager)
        val s = view.state.value
        assertEquals(listOf("kept"), s.items.map { it.id })
        assertNotNull("the burst stops on the error", s.error)
        assertFalse(s.loading)
        assertEquals(1, pager.calls)
        view.continueLoading()
        assertEquals("no storm on a dead connection", 1, pager.calls)
        view.loadMore()
        assertEquals("the tap retries", 2, pager.calls)
    }

    @Test
    fun closingReleasesTheTimelineAndStopsTheView() {
        val pager = Paged(listOf(listOf(image("i1", 10))))
        val view = GalleryView("!r:x", null, scope)
        view.attach(pager)
        view.close()
        assertTrue("the dedicated timeline goes with the view", pager.closed)
        assertFalse(view.state.value.loading)
        val late = Paged(listOf(listOf(image("i2", 20))))
        view.attach(late) // a reopen after close (logout race) can't resurrect it
        assertTrue(late.closed)
        view.loadMore()
        assertEquals(0, late.calls)
    }
}
