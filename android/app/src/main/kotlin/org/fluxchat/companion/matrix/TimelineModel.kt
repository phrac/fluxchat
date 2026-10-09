package org.fluxchat.companion.matrix

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.StateFlow
import org.fluxchat.companion.agents.Msg
import org.matrix.rustcomponents.sdk.EventOrTransactionId
import org.matrix.rustcomponents.sdk.FileInfo
import org.matrix.rustcomponents.sdk.ImageInfo
import org.matrix.rustcomponents.sdk.UploadParameters
import org.matrix.rustcomponents.sdk.UploadSource
import org.matrix.rustcomponents.sdk.VideoInfo
import org.matrix.rustcomponents.sdk.ReceiptType
import org.matrix.rustcomponents.sdk.TaskHandle
import org.matrix.rustcomponents.sdk.Timeline
import org.matrix.rustcomponents.sdk.TimelineDiff
import org.matrix.rustcomponents.sdk.TimelineItem
import org.matrix.rustcomponents.sdk.TimelineListener
import org.matrix.rustcomponents.sdk.messageEventContentFromMarkdown

/**
 * One SDK timeline (a room's, thread replies included, or a thread's),
 * mirrored as a list of [Msg] by applying its diffs — like the desktop's
 * timeline watcher.
 */
class TimelineModel(
    val roomId: String,
    val threadRoot: String?,
    private val timeline: Timeline,
    private val scope: kotlinx.coroutines.CoroutineScope,
) : AutoCloseable {
    private val items = ArrayList<Msg?>()
    private val _messages = MutableStateFlow<List<Msg>>(emptyList())
    val messages: StateFlow<List<Msg>> = _messages
    private val _atStart = MutableStateFlow(false)
    val atStart: StateFlow<Boolean> = _atStart
    private var handle: TaskHandle? = null
    private var convertErrors = 0

    suspend fun start() = io {
        handle = timeline.addListener(object : TimelineListener {
            override fun onUpdate(diff: List<TimelineDiff>) = apply(diff)
        })
    }

    private fun take(item: TimelineItem): Msg? = try {
        convert(item)
    } catch (e: Throwable) {
        // Never silently: a conversion bug empties the timeline.
        if (convertErrors++ < 5) org.fluxchat.companion.Diag.error("Converting a timeline item in $roomId", e)
        null
    } finally {
        item.destroy()
    }

    private fun apply(diffs: List<TimelineDiff>) = synchronized(items) {
        val before = items.size
        val kinds = diffs.joinToString(",") { it.javaClass.simpleName }
        for (d in diffs) {
            when (d) {
                is TimelineDiff.Append -> items.addAll(d.values.map(::take))
                is TimelineDiff.Clear -> items.clear()
                is TimelineDiff.PushFront -> items.add(0, take(d.value))
                is TimelineDiff.PushBack -> items.add(take(d.value))
                is TimelineDiff.PopFront -> if (items.isNotEmpty()) items.removeAt(0)
                is TimelineDiff.PopBack -> if (items.isNotEmpty()) items.removeAt(items.size - 1)
                is TimelineDiff.Insert -> items.add(d.index.toInt().coerceAtMost(items.size), take(d.value))
                is TimelineDiff.Set -> if (d.index.toInt() < items.size) items[d.index.toInt()] = take(d.value) else take(d.value)
                is TimelineDiff.Remove -> if (d.index.toInt() < items.size) items.removeAt(d.index.toInt())
                is TimelineDiff.Truncate -> while (items.size > d.length.toInt()) items.removeAt(items.size - 1)
                is TimelineDiff.Reset -> {
                    items.clear()
                    items.addAll(d.values.map(::take))
                }
            }
        }
        val shown = items.filterNotNull()
        _messages.value = shown
        // The SDK clears a live timeline when sync reports a gap; it fills
        // again only from new events, so fetch the history back.
        if (diffs.any { it is TimelineDiff.Clear || it is TimelineDiff.Reset } && shown.size < REFILL_BELOW) {
            org.fluxchat.companion.Diag.note("timeline $roomId reset ($before → ${items.size} items, [$kinds]): loading history again")
            refill()
        }
    }

    @Volatile private var refilling = false

    /**
     * Back-paginate until there's a screenful again (or the start), with some
     * of the room's own messages among it: in agent rooms the newest events
     * are mostly session-thread replies, and the requests (whose 👀 says a
     * run is going) sit further back.
     */
    fun refill() {
        if (refilling) return
        refilling = true
        _atStart.value = false
        scope.launch {
            try {
                repeat(REFILL_PAGES) {
                    val msgs = _messages.value
                    val own = msgs.count { it.threadRoot == null && it.isMessage }
                    if (msgs.size >= REFILL_BELOW && (threadRoot != null || own >= REFILL_TOP_LEVEL) || !paginateBack()) return@launch
                }
            } finally {
                refilling = false
            }
        }
    }

    /** Off the main thread: the SDK does network work (and TLS checks) on the polling thread. */
    private suspend fun <T> io(block: suspend kotlinx.coroutines.CoroutineScope.() -> T): T =
        kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO, block)

    companion object {
        /** Messages a timeline should hold after a reset / at start. */
        const val REFILL_BELOW = 30
        /** Room messages (not thread replies) a room timeline should hold after a reset / at start. */
        private const val REFILL_TOP_LEVEL = 8
        private const val REFILL_PAGES = 10
    }

    /** Load older messages. False once the start of the timeline is reached. */
    suspend fun paginateBack(events: Int = 40): Boolean = io {
        val hitStart = runCatching { timeline.paginateBackwards(events.toUShort()) }.getOrDefault(false)
        if (hitStart) _atStart.value = true
        !hitStart
    }

    /** Markdown, like the desktop composer (a thread timeline adds the thread relation). */
    suspend fun send(text: String) = io {
        timeline.send(messageEventContentFromMarkdown(text))
    }

    /**
     * A file, with [caption] as its caption (one event, Matrix v1.10), through
     * the send queue (local echo; a thread timeline sends it in the thread).
     */
    suspend fun sendAttachment(context: android.content.Context, a: Attachment, caption: String?) = io {
        val file = Uploads.copy(context, a)
        try {
            val params = UploadParameters(
                source = UploadSource.File(file.path),
                caption = caption?.takeIf { it.isNotBlank() },
                formattedCaption = null,
                mentions = null,
                inReplyTo = null,
                extraContentJson = null,
            )
            val size = file.length().toULong()
            val handle = when {
                a.isImage -> {
                    val (w, h) = Uploads.imageSize(file) ?: (null to null)
                    timeline.sendImage(params, null, ImageInfo(
                        height = h?.toULong(), width = w?.toULong(), mimetype = a.mime, size = size,
                        thumbnailInfo = null, thumbnailSource = null, blurhash = null, isAnimated = a.mime == "image/gif",
                    ))
                }
                a.isVideo -> {
                    val v = Uploads.videoMeta(file)
                    timeline.sendVideo(params, null, VideoInfo(
                        duration = v.durationMs?.let { java.time.Duration.ofMillis(it) }, height = v.height?.toULong(), width = v.width?.toULong(),
                        mimetype = a.mime, size = size, thumbnailInfo = null, thumbnailSource = null, blurhash = null,
                    ))
                }
                else -> timeline.sendFile(params, FileInfo(mimetype = a.mime, size = size, thumbnailInfo = null, thumbnailSource = null))
            }
            // Done once the send queue has taken the file (it keeps its own copy).
            handle.join()
        } finally {
            file.parentFile?.deleteRecursively()
        }
    }

    suspend fun react(eventId: String, key: String) = io {
        timeline.toggleReaction(EventOrTransactionId.EventId(eventId), key)
    }

    suspend fun markRead() = io {
        runCatching { timeline.markAsRead(ReceiptType.READ) }
    }

    override fun close() {
        handle?.cancel()
        handle = null
        timeline.destroy()
    }
}
