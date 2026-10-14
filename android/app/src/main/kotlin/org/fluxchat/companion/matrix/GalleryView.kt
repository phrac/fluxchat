package org.fluxchat.companion.matrix

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import org.fluxchat.companion.Diag
import org.fluxchat.companion.agents.GALLERY_BURST_EVENTS
import org.fluxchat.companion.agents.GalleryTab
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.SendState
import org.fluxchat.companion.agents.galleryShouldLoadMore
import org.fluxchat.companion.agents.isGalleryFile
import org.fluxchat.companion.agents.isGalleryMedia
import org.fluxchat.companion.agents.mergeGalleryItems

/** What a gallery pages through: one timeline's history, in older event chunks. */
interface GalleryPager : AutoCloseable {
    val messages: StateFlow<List<Msg>>
    val atStart: StateFlow<Boolean>
    suspend fun paginate(events: Int): Boolean
}

/**
 * An open media gallery for one project (or session): what's loaded shows at
 * once, a dedicated timeline back-paginates the rest — the bindings expose no
 * /messages endpoint, so paging goes through the Timeline API like everything
 * else here (the desktop pages /messages with an EventsWithUrl filter; an
 * encrypted view costs the same either way: most events are ordinary ones).
 * Items merge through [attach]'s collector only; [pump] just paginates.
 * Automatic paging stops at the burst cap until the user taps "Load older";
 * scrolling near the end ([continueLoading]) keeps going without resetting it.
 */
class GalleryView(
    val roomId: String,
    val root: String?,
    private val scope: CoroutineScope,
) : AutoCloseable {
    data class State(
        val items: List<Msg> = emptyList(),
        val tab: GalleryTab = GalleryTab.Media,
        val loading: Boolean = false,
        val done: Boolean = false,
        val error: String? = null,
        /** Events read since the last "Load older" (the desktop's burst). */
        val burst: Int = 0,
    )

    private val _state = MutableStateFlow(State())
    val state: StateFlow<State> = _state

    /** The dedicated timeline paging this view (owned; closed with it). */
    private var pager: GalleryPager? = null
    private var collect: Job? = null
    private var pumping = false
    private var closed = false

    /** What the view's screen already shows, before the server fills in. */
    fun seed(msgs: List<Msg>) = merge(msgs, replace = true)

    /** The "Load older" tap: the burst starts over. */
    fun loadMore() {
        synchronized(this) {
            if (closed) return
            _state.value = _state.value.copy(burst = 0, error = null)
        }
        pump()
    }

    /** Scrolling near the end: keep paging, but the burst cap still stops us. */
    fun continueLoading() = pump()

    fun setTab(tab: GalleryTab) {
        synchronized(this) { _state.value = _state.value.copy(tab = tab) }
    }

    /** Watch a dedicated timeline; every event it loads joins the gallery. */
    fun attach(model: GalleryPager) {
        synchronized(this) {
            if (closed || pager != null) {
                runCatching { model.close() }
                    .onFailure { Diag.error("Close an unused gallery timeline ${root ?: roomId}", it) }
                return
            }
            pager = model
            collect = scope.launch {
                model.messages.collect { msgs -> merge(msgs, replace = false) }
            }
        }
        pump()
    }

    /**
     * Live history replaces the list wholesale (redactions included); paged-in
     * older events merge per id. Sent-only, gallery kinds — unsent drafts and
     * plain messages never enter, matching the desktop's filters.
     */
    private fun merge(msgs: List<Msg>, replace: Boolean) =
        synchronized(this) {
            if (closed) return
            val media =
                msgs.filter {
                    it.sendState == SendState.Sent &&
                        (isGalleryMedia(it) || isGalleryFile(it))
                }
            val items = ArrayList(if (replace) emptyList() else _state.value.items)
            mergeGalleryItems(items, media)
            _state.value = _state.value.copy(items = items)
        }

    /** Page older history until the burst cap, the start of history, or an error. */
    private fun pump() = scope.launch {
        val go =
            synchronized(this@GalleryView) {
                if (pumping || closed) {
                    false
                } else {
                    val s = _state.value
                    // The pump *is* the loader; the gate is the desktop's may_load.
                    val ok = galleryShouldLoadMore(s.burst, s.error, s.done)
                    if (ok) {
                        pumping = true
                        _state.value = s.copy(loading = true)
                    }
                    ok
                }
            }
        if (!go) return@launch
        try {
            while (true) {
                val p =
                    synchronized(this@GalleryView) {
                        if (closed) return@launch
                        val s = _state.value
                        if (galleryShouldLoadMore(s.burst, s.error, s.done)) pager else null
                    } ?: break
                val still = p.paginate(PAGE_EVENTS)
                val finished =
                    synchronized(this@GalleryView) {
                        if (closed) return@launch
                        // Paginating a page with nothing in it still moves the burst
                        // along: in an encrypted project most pages hold no media.
                        val cur = _state.value
                        val done = !still || p.atStart.value
                        val burst = cur.burst + PAGE_EVENTS
                        // Waiting for a "load older" tap on cap; spinner only while paging.
                        _state.value =
                            cur.copy(
                                burst = burst,
                                done = done,
                                loading = !done && burst < GALLERY_BURST_EVENTS,
                            )
                        done
                    }
                if (finished) break
            }
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Throwable) {
            Diag.error("Gallery paging ${root ?: roomId}", e)
            synchronized(this@GalleryView) {
                if (!closed) {
                    _state.value =
                        _state.value.copy(
                            loading = false,
                            error = e.message ?: e.javaClass.simpleName,
                        )
                }
            }
        } finally {
            synchronized(this@GalleryView) { pumping = false }
        }
    }

    override fun close() {
        val (job, p) =
            synchronized(this) {
                closed = true
                pumping = false
                val c = collect
                collect = null
                val g = pager
                pager = null
                _state.value = _state.value.copy(loading = false)
                c to g
            }
        job?.cancel()
        runCatching { p?.close() }
            .onFailure { Diag.error("Close the gallery timeline ${root ?: roomId}", it) }
    }

    companion object {
        /** Events per page, like the desktop's GALLERY_PAGE_SIZE. */
        private const val PAGE_EVENTS = 100
    }
}
