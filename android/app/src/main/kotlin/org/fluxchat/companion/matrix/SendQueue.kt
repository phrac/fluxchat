package org.fluxchat.companion.matrix

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.fluxchat.companion.Diag
import org.matrix.rustcomponents.sdk.Client
import org.matrix.rustcomponents.sdk.ClientException
import org.matrix.rustcomponents.sdk.QueueWedgeError
import org.matrix.rustcomponents.sdk.RoomSendQueueUpdate
import org.matrix.rustcomponents.sdk.SendQueueRoomErrorListener
import org.matrix.rustcomponents.sdk.SendQueueRoomUpdateListener
import org.matrix.rustcomponents.sdk.TaskHandle

/**
 * How long to wait before a room's send queue is switched back on after a
 * failed request (the desktop's `send_queue.rs`, same numbers): at once after
 * an unrecoverable one — that request is parked and never picked again by
 * itself —, after a delay that doubles while the room keeps failing after a
 * recoverable one (network, server busy).
 */
class SendBackoff(
    private val first: Long = FIRST_RETRY_MS,
    private val max: Long = MAX_RETRY_MS,
    private val resetAfter: Long = RETRY_RESET_AFTER_MS,
) {
    /** Per room: the next delay, and when it last failed. */
    private val rooms = HashMap<String, Pair<Long, Long>>()

    @Synchronized
    fun delayFor(roomId: String, recoverable: Boolean, now: Long): Long {
        if (!recoverable) return 0
        val (next, failedAt) = rooms[roomId] ?: (first to now)
        // A room that hasn't failed for a while starts again at the first delay.
        val delay = if (now - failedAt > resetAfter) first else next
        rooms[roomId] = (delay * 2).coerceAtMost(max) to now
        return delay
    }

    companion object {
        const val FIRST_RETRY_MS = 5_000L
        const val MAX_RETRY_MS = 120_000L
        const val RETRY_RESET_AFTER_MS = 600_000L
    }
}

/**
 * Keeps the SDK's send queue moving (port of the desktop's
 * `spawn_send_queue_keeper`).
 *
 * The SDK stops a room's send queue after a failed request and leaves turning
 * it back on to the app; until then every later message in that room stays
 * "sending". Nothing here did, so one message sent in a tunnel held up the
 * project until the app was restarted. What this SDK build does (read from
 * its source, the strings checked in the shipped library):
 *  - a recoverable failure (network, server busy) keeps the request and
 *    disables the room's queue → re-enabled here after [SendBackoff];
 *  - an unrecoverable one parks ("wedges") the request, which then blocks
 *    the requests queued behind it until it is retried or deleted — the row's
 *    Retry / Delete ([TimelineModel.retrySend] / [TimelineModel.abortSend]).
 *    The queue is re-enabled at once all the same: the SDK disables it when
 *    it couldn't record the parking.
 *  - requests left unsent by an earlier session are respawned by
 *    `subscribeToSendQueueStatus` itself.
 */
object SendQueue {
    /**
     * Start keeping [c]'s send queues; cancel the returned job to stop (it
     * cancels the SDK subscriptions and any re-enable still waiting).
     */
    fun keep(c: Client, scope: CoroutineScope): Job = scope.launch {
        val keeper = this
        val backoff = SendBackoff()
        fun failed(roomId: String, recoverable: Boolean): Long {
            val wait = backoff.delayFor(roomId, recoverable, System.currentTimeMillis())
            keeper.launch {
                delay(wait)
                resume(roomId)
            }
            return wait
        }
        var updates: TaskHandle? = null
        var status: TaskHandle? = null
        try {
            // The per-request updates say whether a failure is recoverable;
            // the error listener below doesn't.
            updates = try {
                c.subscribeToSendQueueUpdates(object : SendQueueRoomUpdateListener {
                    override fun onUpdate(roomId: String, update: RoomSendQueueUpdate) {
                        if (update !is RoomSendQueueUpdate.SendError) return
                        val wait = failed(roomId, update.isRecoverable)
                        Diag.error(
                            "Sending ${update.transactionId} in $roomId failed (${describe(update.error)}): " +
                                if (update.isRecoverable) "trying again in ${wait / 1000}s" else "parked until it's retried or deleted",
                        )
                    }
                })
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                Diag.error("Watching the send queue (failed sends are all retried with a delay)", e)
                null
            }
            val precise = updates != null
            // Subscribing also respawns the requests earlier sessions left unsent.
            status = c.subscribeToSendQueueStatus(object : SendQueueRoomErrorListener {
                override fun onError(roomId: String, error: ClientException) {
                    if (precise) return
                    // Without the updates there's no telling which kind it
                    // was: wait, as for a recoverable one.
                    val wait = failed(roomId, recoverable = true)
                    Diag.error("Sending in $roomId failed: trying again in ${wait / 1000}s", error)
                }
            })
            awaitCancellation()
        } finally {
            updates?.cancel()
            status?.cancel()
            // Signed out (or the client is being replaced): what this
            // account's screens were holding goes with it.
            org.fluxchat.companion.ui.Unsent.clear()
            org.fluxchat.companion.ui.Answers.clear()
        }
    }

    /** Switch a room's send queue back on (it picks up where it stopped). */
    fun resume(roomId: String) {
        try {
            Matrix.room(roomId)?.enableSendQueue(true)
        } catch (e: Exception) {
            Diag.error("Resuming sends in $roomId", e)
        }
    }

    private fun describe(e: QueueWedgeError): String = when (e) {
        is QueueWedgeError.GenericApiError -> e.msg
        else -> e.javaClass.simpleName
    }
}
