package org.fluxchat.companion.push

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.app.RemoteInput
import org.fluxchat.companion.Diag
import org.fluxchat.companion.matrix.Matrix
import org.json.JSONObject

/**
 * Answers from a notification, without opening the app: a prompt's choice
 * (a reaction on it), a question's choice or Stop (a message), or a typed
 * reply. Sent with the room's raw send (encrypted in encrypted rooms), so the
 * event needn't be in a loaded timeline.
 */
class ActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val room = intent.getStringExtra(ROOM) ?: return Diag.note("push: notification action without a room")
        val event = intent.getStringExtra(EVENT) ?: return Diag.note("push: notification action without an event")
        val thread = intent.getStringExtra(THREAD)
        val value0 = intent.getStringExtra(VALUE).orEmpty()
        val label = intent.getStringExtra(LABEL).orEmpty()
        val reply = RemoteInput.getResultsFromIntent(intent)?.getCharSequence(Notifier.REPLY_KEY)?.toString()
        // A choice picked from the reply chips: answer as its button would.
        val picked = reply?.let { r -> intent.getStringArrayExtra(CHOICE_TITLES)?.indexOf(r.trim())?.takeIf { it >= 0 } }
        val action = if (picked != null) intent.getStringExtra(CHOICE_ACTION) ?: return else intent.action ?: return
        val value = if (picked != null) intent.getStringArrayExtra(CHOICE_KEYS)?.getOrNull(picked) ?: return else value0
        val pending = goAsync()
        Matrix.init(context)
        Matrix.launch {
            try {
                val ok = Matrix.ensureStarted() && when (action) {
                    REACT -> Matrix.sendReaction(room, event, value)
                    SEND -> Matrix.sendText(room, thread, value, inReplyTo = null)
                    REPLY -> !reply.isNullOrBlank() && Matrix.sendText(room, thread, reply, inReplyTo = event)
                    else -> false
                }
                org.fluxchat.companion.Diag.note("notification action $action on $event: ${if (ok) "sent" else "failed"}")
                val text = when {
                    !ok -> "Couldn't send. Open fluxchat to answer."
                    action == REACT -> "Answered: $value $label".trim()
                    action == REPLY -> "Sent: $reply"
                    else -> "Sent: $value"
                }
                Notifier.answered(context, event, text)
            } catch (e: Exception) {
                org.fluxchat.companion.Diag.error("Notification action failed", e)
                Notifier.answered(context, event, "Couldn't send. Open fluxchat to answer.")
            } finally {
                pending.finish()
            }
        }
    }

    companion object {
        const val REACT = "org.fluxchat.companion.REACT"
        const val SEND = "org.fluxchat.companion.SEND"
        const val REPLY = "org.fluxchat.companion.REPLY"
        const val ROOM = "room"
        const val EVENT = "event"
        const val THREAD = "thread"
        const val VALUE = "value"
        const val LABEL = "label"
        const val CHOICE_TITLES = "choice_titles"
        const val CHOICE_KEYS = "choice_keys"
        const val CHOICE_ACTION = "choice_action"

        /** A reaction's content. */
        fun reaction(eventId: String, key: String): String = JSONObject()
            .put("m.relates_to", JSONObject().put("rel_type", "m.annotation").put("event_id", eventId).put("key", key))
            .toString()

        /** A text message, in a thread when [thread] is set (replying to [inReplyTo] there, else to the thread's latest). */
        fun text(body: String, thread: String?, inReplyTo: String?): String {
            val o = JSONObject().put("msgtype", "m.text").put("body", body)
            if (thread != null) {
                o.put(
                    "m.relates_to",
                    JSONObject().put("rel_type", "m.thread").put("event_id", thread).put("is_falling_back", inReplyTo == null)
                        .put("m.in_reply_to", JSONObject().put("event_id", inReplyTo ?: thread)),
                )
            } else if (inReplyTo != null) {
                o.put("m.relates_to", JSONObject().put("m.in_reply_to", JSONObject().put("event_id", inReplyTo)))
            }
            return o.toString()
        }
    }
}
