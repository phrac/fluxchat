package org.fluxchat.companion.matrix

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.withContext
import org.fluxchat.companion.Diag
import org.matrix.rustcomponents.sdk.ProfileDetails
import org.matrix.rustcomponents.sdk.TaskHandle
import org.matrix.rustcomponents.sdk.ThreadListEntriesListener
import org.matrix.rustcomponents.sdk.ThreadListItem
import uniffi.matrix_sdk_ui.ThreadListPaginationState
import org.matrix.rustcomponents.sdk.ThreadListPaginationStateListener
import org.matrix.rustcomponents.sdk.ThreadListService
import org.matrix.rustcomponents.sdk.ThreadListUpdate

/**
 * A session: one thread in an agent room (Hermes keys its sessions on the
 * thread). [lastTs] is the latest reply's time, else the root's.
 */
data class Session(
    val root: String,
    val rootBody: String,
    val rootTs: Long,
    val lastTs: Long,
    val latest: String?,
    val latestSender: String?,
    val replies: Int,
)

/**
 * A room's sessions from the server's thread list (all of them, not just the
 * loaded history), newest first, kept live by the SDK's ThreadListService.
 */
class SessionList(val roomId: String, private val service: ThreadListService) : AutoCloseable {
    private val raw = ArrayList<Session?>()
    private val _items = MutableStateFlow<List<Session>>(emptyList())
    val items: StateFlow<List<Session>> = _items
    /** Read at composition time (search reads many lists at once). */
    val itemsNow: List<Session> get() = _items.value
    private val _loading = MutableStateFlow(false)
    val loading: StateFlow<Boolean> = _loading
    private val _endReached = MutableStateFlow(false)
    val endReached: StateFlow<Boolean> = _endReached
    val endReachedNow: Boolean get() = _endReached.value
    private val handles = ArrayList<TaskHandle>()

    fun start() {
        handles += service.subscribeToItemsUpdates(object : ThreadListEntriesListener {
            override fun onUpdate(diff: List<ThreadListUpdate>) = apply(diff)
        })
        handles += service.subscribeToPaginationStateUpdates(object : ThreadListPaginationStateListener {
            override fun onUpdate(state: ThreadListPaginationState) {
                _loading.value = state is ThreadListPaginationState.Loading
                if (state is ThreadListPaginationState.Idle) _endReached.value = state.endReached
            }
        })
    }

    private fun apply(diffs: List<ThreadListUpdate>) = synchronized(raw) {
        for (d in diffs) {
            when (d) {
                is ThreadListUpdate.Append -> raw.addAll(d.values.map(::convert))
                is ThreadListUpdate.Clear -> raw.clear()
                is ThreadListUpdate.PushFront -> raw.add(0, convert(d.value))
                is ThreadListUpdate.PushBack -> raw.add(convert(d.value))
                is ThreadListUpdate.PopFront -> if (raw.isNotEmpty()) raw.removeAt(0)
                is ThreadListUpdate.PopBack -> if (raw.isNotEmpty()) raw.removeAt(raw.size - 1)
                is ThreadListUpdate.Insert -> raw.add(d.index.toInt().coerceAtMost(raw.size), convert(d.value))
                is ThreadListUpdate.Set -> if (d.index.toInt() < raw.size) raw[d.index.toInt()] = convert(d.value)
                is ThreadListUpdate.Remove -> if (d.index.toInt() < raw.size) raw.removeAt(d.index.toInt())
                is ThreadListUpdate.Truncate -> while (raw.size > d.length.toInt()) raw.removeAt(raw.size - 1)
                is ThreadListUpdate.Reset -> {
                    raw.clear()
                    raw.addAll(d.values.map(::convert))
                }
            }
        }
        _items.value = raw.filterNotNull().sortedByDescending { it.lastTs }
    }

    private fun convert(item: ThreadListItem): Session? = runCatching {
        val root = item.rootEvent
        val latest = item.latestEvent
        Session(
            root = root.eventId,
            rootBody = root.content?.let(::contentBody).orEmpty(),
            rootTs = root.timestamp.toLong(),
            lastTs = (latest?.timestamp ?: root.timestamp).toLong(),
            latest = latest?.content?.let(::contentBody),
            latestSender = latest?.let { (it.senderProfile as? ProfileDetails.Ready)?.displayName ?: it.sender },
            replies = item.numReplies.toInt(),
        )
    }.onFailure { Diag.error("Reading a thread in $roomId", it) }.getOrNull()

    /** The next page (older sessions). */
    suspend fun more() = withContext(Dispatchers.IO) {
        if (!_endReached.value) runCatching { service.paginate() }.onFailure { Diag.error("Loading sessions in $roomId", it) }
    }

    override fun close() {
        handles.forEach { it.cancel() }
        handles.clear()
        service.destroy()
    }
}
