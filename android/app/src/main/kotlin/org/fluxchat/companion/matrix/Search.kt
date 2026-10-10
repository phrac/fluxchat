package org.fluxchat.companion.matrix

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.withContext
import org.fluxchat.companion.Diag
import org.matrix.rustcomponents.sdk.MessageSearchResult
import org.matrix.rustcomponents.sdk.ProfileDetails
import org.matrix.rustcomponents.sdk.SearchService
import org.matrix.rustcomponents.sdk.SearchServicePaginationStateListener
import org.matrix.rustcomponents.sdk.SearchServiceResult
import org.matrix.rustcomponents.sdk.SearchServiceResultsListener
import org.matrix.rustcomponents.sdk.SearchServiceResultsUpdate
import org.matrix.rustcomponents.sdk.TaskHandle
import uniffi.matrix_sdk_ui.SearchServicePaginationState

/**
 * One full-text hit inside a session: a message whose text matched on the
 * server's search index (any reply, not just the thread's title or latest).
 * [root] is the session's root event: the hit's thread root, or the event
 * itself when it isn't a reply — enough to deep-link into the session.
 */
data class ReplyHit(
    val roomId: String,
    val eventId: String,
    val root: String,
    val body: String,
    val ts: Long,
    val sender: String?,
)

/**
 * The client's full-text search (the SDK's SearchService over the store's
 * index). One service per sign-in, re-queried as the search box changes;
 * results arrive as diffs, newest relevance first, kept live like
 * [SessionList] keeps the thread list.
 */
class ServerSearch(private val service: SearchService) : AutoCloseable {
    private val raw = ArrayList<ReplyHit>()
    private val _hits = MutableStateFlow<List<ReplyHit>>(emptyList())
    val hits: StateFlow<List<ReplyHit>> = _hits
    /** Read at composition time (the search screen re-gathers per keystroke). */
    val hitsNow: List<ReplyHit> get() = _hits.value
    private val _loading = MutableStateFlow(false)
    val loading: StateFlow<Boolean> = _loading
    val loadingNow: Boolean get() = _loading.value
    private val _endReached = MutableStateFlow(true)
    val endReachedNow: Boolean get() = _endReached.value
    private val handles = ArrayList<TaskHandle>()

    suspend fun start() = withContext(Dispatchers.IO) {
        handles += service.subscribeToResults(object : SearchServiceResultsListener {
            override fun onUpdate(updates: List<SearchServiceResultsUpdate>) = apply(updates)
        })
        handles += service.subscribeToPaginationStateUpdates(object : SearchServicePaginationStateListener {
            override fun onUpdate(state: SearchServicePaginationState) {
                _loading.value = state is SearchServicePaginationState.Loading
                _endReached.value = state is SearchServicePaginationState.Idle && state.endReached
            }
        })
    }

    /** Run a query (results stream in as diffs; a new query clears the old). */
    suspend fun search(query: String) = withContext(Dispatchers.IO) {
        runCatching { service.setQuery(query) }
            .onFailure { Diag.error("Searching sessions for \"$query\"", it) }
    }

    /** The next page of hits. */
    suspend fun more() = withContext(Dispatchers.IO) {
        if (!_endReached.value) runCatching { service.paginate() }.onFailure { Diag.error("Loading more search results", it) }
    }

    private fun apply(updates: List<SearchServiceResultsUpdate>) = synchronized(raw) {
        for (d in updates) {
            when (d) {
                is SearchServiceResultsUpdate.Append -> raw.addAll(d.values.mapNotNull(::convert))
                is SearchServiceResultsUpdate.Clear -> raw.clear()
                is SearchServiceResultsUpdate.PushFront -> convert(d.value)?.let { raw.add(0, it) }
                is SearchServiceResultsUpdate.PushBack -> convert(d.value)?.let { raw.add(it) }
                is SearchServiceResultsUpdate.PopFront -> if (raw.isNotEmpty()) raw.removeAt(0)
                is SearchServiceResultsUpdate.PopBack -> if (raw.isNotEmpty()) raw.removeAt(raw.size - 1)
                is SearchServiceResultsUpdate.Insert ->
                    convert(d.value)?.let { raw.add(d.index.toInt().coerceAtMost(raw.size), it) }
                is SearchServiceResultsUpdate.Set ->
                    if (d.index.toInt() < raw.size) raw[d.index.toInt()] = convert(d.value) ?: raw[d.index.toInt()]
                is SearchServiceResultsUpdate.Remove -> if (d.index.toInt() < raw.size) raw.removeAt(d.index.toInt())
                is SearchServiceResultsUpdate.Truncate -> while (raw.size > d.length.toInt()) raw.removeAt(raw.size - 1)
                is SearchServiceResultsUpdate.Reset -> {
                    raw.clear()
                    raw.addAll(d.values.mapNotNull(::convert))
                }
            }
        }
        _hits.value = raw.distinctBy { it.roomId to it.eventId }
    }

    private fun convert(item: SearchServiceResult): ReplyHit? = runCatching {
        item as? SearchServiceResult.Message ?: return null
        val r: MessageSearchResult = item.result
        val body = contentBody(r.content)?.takeIf { it.isNotBlank() } ?: return null
        // The thread relation says which session the hit lives in; a
        // top-level message is its own session's root.
        val root = (r.content as? org.matrix.rustcomponents.sdk.TimelineItemContent.MsgLike)
            ?.content?.threadRoot ?: r.eventId
        ReplyHit(
            roomId = item.roomId,
            eventId = r.eventId,
            root = root,
            body = body,
            ts = r.timestamp.toLong(),
            sender = (r.senderProfile as? ProfileDetails.Ready)?.displayName ?: r.sender,
        )
    }.onFailure { Diag.error("Reading a search result", it) }.getOrNull()

    override fun close() {
        handles.forEach { it.cancel() }
        handles.clear()
        runCatching { service.destroy() }
    }
}
