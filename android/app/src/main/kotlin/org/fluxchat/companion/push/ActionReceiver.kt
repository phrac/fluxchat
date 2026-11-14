package org.fluxchat.companion.push

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.SystemClock
import androidx.core.app.RemoteInput
import kotlinx.coroutines.delay
import org.fluxchat.companion.Diag
import org.fluxchat.companion.matrix.Matrix
import org.json.JSONObject
import java.util.concurrent.atomic.AtomicBoolean

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
        val reply = RemoteInput.getResultsFromIntent(intent)?.getCharSequence(Notifier.REPLY_KEY)?.toString()
        // A choice picked from the reply chips: answer as its button would.
        val picked = pickedChoice(reply, intent.getStringArrayExtra(CHOICE_TITLES))
        val ask = ask(
            intent.action, intent.getStringExtra(VALUE).orEmpty(), intent.getStringExtra(LABEL).orEmpty(), reply,
            picked, intent.getStringArrayExtra(CHOICE_KEYS), intent.getStringExtra(CHOICE_ACTION),
        ) ?: return Diag.note("push: notification action on $event without ${if (picked != null) "its choice's key" else "an action"}")
        val (action, value) = ask
        // The buttons stay up until the answer is through, and a second tap
        // on one sent it twice: a question got "1" and then "1" again as
        // your next message, and a second reaction is refused by the server,
        // which read as "couldn't send your answer".
        val words = if (action == REPLY || action == DENY) reply.orEmpty() else ""
        val tap = listOf(action, room, event, value, words).joinToString("\u0000")
        if (!taps.begin(tap, SystemClock.elapsedRealtime())) {
            return Diag.note("notification action $action on $event: already sent, the second tap is ignored")
        }
        // A choice made: the prompt is this notification's to finish with
        // (a typed reply to a message says nothing about a prompt it may yet become).
        if (action != REPLY) PromptWatcher.answered(event)
        val pending = goAsync()
        // A broadcast has about ten seconds. From a dead process on a slow
        // network (restore the session, share keys, send) an answer can take
        // longer, and a receiver that overruns is killed with the answer
        // half-sent and nothing said. So the broadcast is handed back in
        // time; the send carries on under a wake lock and reports as usual.
        val finished = AtomicBoolean(false)
        val finish = { if (finished.compareAndSet(false, true)) pending.finish() }
        val app = context.applicationContext
        val awake = Awake.hold(app, "answer")
        val budget = Matrix.launch {
            delay(BROADCAST_BUDGET_MS)
            finish()
        }
        Matrix.init(context)
        Matrix.launch {
            var sent = false
            try {
                val ok = Matrix.ensureStarted() && when (action) {
                    REACT -> Matrix.sendReaction(room, event, value)
                    SEND -> Matrix.sendText(room, thread, value, inReplyTo = null)
                    REPLY -> !reply.isNullOrBlank() && Matrix.sendText(room, thread, reply, inReplyTo = event)
                    // Deny with a reason: the same ❌ reaction the button
                    // sends settles it; the typed reason follows in-thread so
                    // the agent reads it next turn.
                    DENY -> Matrix.sendReaction(room, event, value) &&
                        (reply.isNullOrBlank() || Matrix.sendText(room, thread, "Denied: $reply", inReplyTo = event))
                    else -> false
                }
                Diag.note("notification action $action on $event: ${if (ok) "sent" else "failed"}")
                if (!ok) {
                    if (!Notifier.failed(app, room, thread, event)) PromptWatcher.unanswered(event)
                    return@launch
                }
                sent = true
                Notifier.answered(app, event, ask.done)
            } catch (e: Exception) {
                Diag.error("Notification action failed", e)
                if (!Notifier.failed(app, room, thread, event)) PromptWatcher.unanswered(event)
            } finally {
                taps.end(tap, sent, SystemClock.elapsedRealtime())
                budget.cancel()
                awake.release()
                finish()
            }
        }
    }

    /** What a tap on a notification asks for, and what the notification says once it's done. */
    internal data class Ask(val action: String, val value: String, val done: String)

    companion object {
        /** How long an answer may hold the broadcast (the system's limit is ten seconds). */
        private const val BROADCAST_BUDGET_MS = 8_000L

        const val REACT = "org.fluxchat.companion.REACT"
        const val SEND = "org.fluxchat.companion.SEND"
        const val REPLY = "org.fluxchat.companion.REPLY"
        const val DENY = "org.fluxchat.companion.DENY"
        const val ROOM = "room"
        const val EVENT = "event"
        const val THREAD = "thread"
        const val VALUE = "value"
        const val LABEL = "label"
        const val CHOICE_TITLES = "choice_titles"
        const val CHOICE_KEYS = "choice_keys"
        const val CHOICE_ACTION = "choice_action"

        private val taps = Taps()

        /**
         * Which of the reply box's chips [reply] is (its index among
         * [titles]), or null for words of your own. Compared without the
         * space around them: a title cut to length can end in one, and a chip
         * that then matched nothing went out as a typed reply — on an
         * approval that is a Deny, with the choice's name as the reason.
         */
        internal fun pickedChoice(reply: String?, titles: Array<String>?): Int? {
            val said = reply?.trim().orEmpty()
            if (said.isEmpty() || titles == null) return null
            return titles.indexOfFirst { it.trim() == said }.takeIf { it >= 0 }
        }

        /**
         * A notification action as what to do: a button's own [action] and
         * [value], or — a chip [picked] in the reply box — what that choice's
         * button would have done ([choiceAction] with its key). Null when the
         * intent doesn't say.
         */
        internal fun ask(
            action: String?, value: String, label: String, reply: String?,
            picked: Int?, keys: Array<String>?, choiceAction: String?,
        ): Ask? {
            val act = (if (picked != null) choiceAction else action) ?: return null
            val v = if (picked != null) keys?.getOrNull(picked) ?: return null else value
            val done = when (act) {
                // A chip says what it is itself; [label] is the reply action's ("Deny").
                REACT -> if (picked != null) "Answered: ${reply.orEmpty().trim()}" else "Answered: $v $label".trim()
                DENY -> "Denied" + (if (reply.isNullOrBlank()) "" else ": ${reply.trim()}")
                REPLY -> "Sent: ${reply.orEmpty().trim()}"
                else -> "Sent: $v"
            }
            return Ask(act, v, done)
        }

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

/**
 * Answers on their way, and the ones just sent: the same tap again is the
 * same answer, not a second one. Remembered only briefly — the same words a
 * while later are a new answer (Stop, for the run that started since).
 */
internal class Taps(private val recentMs: Long = 10_000L) {
    private val going = HashSet<String>()
    private val sent = HashMap<String, Long>()

    /** False when [tap] is on its way, or went out within the last [recentMs]. */
    @Synchronized
    fun begin(tap: String, now: Long): Boolean {
        sent.values.removeAll { now - it > recentMs }
        return tap !in sent && going.add(tap)
    }

    /** [tap] is over; [ok]: it went out. One that failed can be tried again at once. */
    @Synchronized
    fun end(tap: String, ok: Boolean, now: Long) {
        going.remove(tap)
        if (ok) sent[tap] = now
    }
}
