package org.fluxchat.companion.quick

import android.content.Context
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withTimeoutOrNull
import org.fluxchat.companion.Diag
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.SendState
import org.fluxchat.companion.matrix.Matrix

/**
 * Sending the quick prompt, and knowing what became of it. The sheet used to
 * say "Sent" the moment the message was handed to the send queue and close
 * at once — also with no signal, when nothing had been sent at all.
 */
object QuickSend {
    enum class Outcome {
        /** The server has it. */
        Sent,
        /** In the send queue, not confirmed in time: it goes out when it can. */
        Queued,
        Failed,
    }

    /** How long the sheet stays up waiting for the server to take the message. */
    const val SENT_WAIT_MS = 8_000L

    /**
     * What the timeline says about the message just queued: a message of
     * ours that wasn't there [before] the send (its local echo first, then
     * the event that replaces it — a new ID both times). Not matched on the
     * text: the body that comes back needn't be the text that went in. Null
     * while it's still on its way.
     */
    internal fun echoState(before: Set<String>, msgs: List<Msg>): SendState? {
        val ours = msgs.filter { it.isOwn && it.isMessage && it.id !in before }
        return when {
            ours.any { it.sendState == SendState.Sent } -> SendState.Sent
            ours.any { it.sendState == SendState.Failed } -> SendState.Failed
            else -> null
        }
    }

    /**
     * Send [out] to [roomId]. On the app's scope (the caller's is the
     * sheet's): a sheet that goes away mid-send must not take the message
     * with it. A send that went through or is queued is no draft any more;
     * a failed one stays one.
     */
    suspend fun send(context: Context, roomId: String, out: String): Outcome {
        val app = context.applicationContext
        val outcome = try {
            deliver(roomId, out)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            Diag.error("Sending the quick prompt", e)
            Outcome.Failed
        }
        if (outcome != Outcome.Failed) {
            Quick.saveDraft(app, roomId, "")
            Quick.remember(app, roomId)
        }
        return outcome
    }

    private suspend fun deliver(roomId: String, out: String): Outcome {
        // Through the room's timeline (Markdown, local echo) when it's open, else straight to the room.
        val timeline = Matrix.roomTimeline(roomId)
            ?: return if (Matrix.sendText(roomId, null, out, inReplyTo = null)) Outcome.Sent else Outcome.Failed
        val before = timeline.messages.value.mapTo(HashSet()) { it.id }
        timeline.send(out)
        val state = withTimeoutOrNull(SENT_WAIT_MS) {
            timeline.messages.map { echoState(before, it) }.first { it != null }
        }
        return when (state) {
            SendState.Sent -> Outcome.Sent
            SendState.Failed -> {
                Diag.error("The quick prompt to $roomId was queued and then failed to send")
                Outcome.Failed
            }
            else -> {
                Diag.note("quick prompt to $roomId: queued, not confirmed after ${SENT_WAIT_MS / 1000} s")
                Outcome.Queued
            }
        }
    }
}
