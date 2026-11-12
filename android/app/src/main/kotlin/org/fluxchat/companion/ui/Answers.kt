package org.fluxchat.companion.ui

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.SendState

/** Where your answer to an agent's prompt stands. */
enum class AnswerPhase {
    /** On its way out. */
    Sending,
    /** The agent has it and hasn't reacted to it yet. */
    Waiting,
}

/** The answer you gave a prompt ([key]: the choice), while its card is still up. */
data class PromptAnswer(val key: String, val phase: AnswerPhase)

/**
 * Answers given to agents' prompts (approvals, pickers, questions), from the
 * tap until the prompt goes away. The answer buttons used to be live the whole
 * time: an approval is a reaction, a reaction tap is a toggle, so a second,
 * impatient tap took the approval back — and nothing said the first had been
 * sent. Kept here rather than in the row: the send runs on the app's scope
 * and the row may be scrolled away, or the screen left, before it's done.
 */
object Answers {
    /** How long a queued answer keeps its buttons waiting for the local echo to show. */
    const val GRACE_MS = 10_000L
    private const val KEEP = 64

    enum class Stage {
        /** The send hasn't come back yet. */
        InFlight,
        /** In the send queue: its row shows how it goes from here. */
        Queued,
        /** The server has it. */
        Delivered,
    }

    data class Entry(val key: String, val at: Long, val stage: Stage)

    private val entries = MutableStateFlow<Map<String, Entry>>(emptyMap())
    /** By the prompt's event ID. */
    val all: StateFlow<Map<String, Entry>> = entries

    /**
     * Note a tap on [promptId]'s choice [key]. False — ignore the tap — while
     * an answer to that prompt is on its way or with the agent.
     */
    fun begin(promptId: String, key: String, now: Long): Boolean {
        var taken = false
        entries.update { m ->
            val old = m[promptId]
            taken = old == null || (old.stage == Stage.Queued && now - old.at >= GRACE_MS)
            if (!taken) m else (if (m.size >= KEEP) m - m.keys.first() else m) + (promptId to Entry(key, now, Stage.InFlight))
        }
        return taken
    }

    fun queued(promptId: String, now: Long) = settle(promptId, now, Stage.Queued)

    fun delivered(promptId: String, now: Long) = settle(promptId, now, Stage.Delivered)

    private fun settle(promptId: String, now: Long, stage: Stage) =
        entries.update { m -> m[promptId]?.let { m + (promptId to it.copy(at = now, stage = stage)) } ?: m }

    /** The send failed: the buttons are live again. */
    fun failed(promptId: String) = entries.update { it - promptId }

    fun clear() {
        entries.value = emptyMap()
    }
}

/**
 * Your answer to a reaction prompt ([choices]: key to label), null while it's
 * still open. Your own reaction with one of the choices counts wherever it
 * was given (a notification, the desktop, before a restart), so the buttons
 * never offer to "answer" again — which would withdraw it.
 */
fun promptAnswer(m: Msg, choices: List<Pair<String, String>>, me: String, entry: Answers.Entry?): PromptAnswer? {
    if (entry?.stage == Answers.Stage.InFlight) return PromptAnswer(entry.key, AnswerPhase.Sending)
    val mine = m.reactions.firstOrNull { r -> me.isNotEmpty() && me in r.users && choices.any { it.first == r.emoji } }
    if (mine != null) return PromptAnswer(mine.emoji, AnswerPhase.Waiting)
    return entry?.let { PromptAnswer(it.key, AnswerPhase.Waiting) }
}

/**
 * Your answer to the question at `msgs[i]` while it's going out, null once
 * the buttons may be used (again). A question stays up until a message lands
 * after it, and your answer on its way is a local echo that doesn't count
 * yet: every tap in between sent the answer once more. A failed echo frees
 * the buttons — its own row offers Retry and Delete.
 */
fun questionAnswer(msgs: List<Msg>, i: Int, entry: Answers.Entry?, now: Long): PromptAnswer? {
    val key = entry?.key.orEmpty()
    if (entry?.stage == Answers.Stage.InFlight) return PromptAnswer(key, AnswerPhase.Sending)
    val onItsWay = msgs.drop(i + 1).any { it.isOwn && it.isLocal && it.isMessage && it.sendState != SendState.Failed }
    // Just handed over: the echo (or, sent directly, the message itself) is about to show.
    val justSent = entry != null && now - entry.at < Answers.GRACE_MS
    return if (onItsWay || justSent) PromptAnswer(key, AnswerPhase.Sending) else null
}
