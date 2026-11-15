package org.fluxchat.companion.push

import android.content.Context
import org.fluxchat.companion.agents.Harness
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.agents.ViewKey
import org.fluxchat.companion.matrix.AgentRoom
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.ui.threadTitle

/**
 * Prompts seen while the app runs (sync going, e.g. after a push woke it):
 * reaction prompts (Hermes approvals without the protocol) only become
 * prompts when the agent's own reactions arrive, after the message's push.
 * Their notification gets the answer buttons then, and goes once answered.
 */
object PromptWatcher {
    /** Only prompts this recent are news (not ones found in loaded history). */
    private const val FRESH_MS = 2 * 60 * 1000L
    private val announced = HashSet<String>()
    /** Prompts open at the last look: one that closes loses its notification, whoever posted it. */
    private var lastOpen = emptySet<String>()

    /**
     * Whether a prompt the watcher has just found gets a notification.
     *
     * One the push path already notified ([pushed]) gets its buttons whatever
     * its age: a push Doze held back, or a cold start slower than two
     * minutes, used to leave the approval up as a plain message with nothing
     * to tap. One merely found in loaded history is news only while fresh.
     * Neither while you're looking at it.
     */
    internal fun shouldAnnounce(pushed: Boolean, ageMs: Long, watching: Boolean): Boolean =
        !watching && (pushed || ageMs <= FRESH_MS)

    fun start(context: Context) {
        val app = context.applicationContext
        Matrix.supervise("Watching for prompts") { Matrix.agentRooms.collect { rooms -> look(app, rooms) } }
    }

    /**
     * You answered [id] from its notification. A process that only started
     * for that tap has announced nothing yet, and the prompt stays open in
     * the timeline until the agent takes it down: found "new" there, it came
     * straight back — alert, buttons and all — over "Answered".
     */
    @Synchronized
    fun answered(id: String) {
        announced += id
    }

    /** That answer didn't go out and its prompt couldn't be put back: finding it open is news again. */
    @Synchronized
    fun unanswered(id: String) {
        announced -= id
    }

    @Synchronized
    internal fun wasAnnounced(id: String): Boolean = id in announced

    @Synchronized
    private fun look(app: Context, rooms: Map<String, AgentRoom>) {
        val now = System.currentTimeMillis()
        val open = HashSet<String>()
        for (room in rooms.values) {
            val all = Matrix.roomTimeline(room.roomId)?.messages?.value ?: continue
            for (id in room.prompts) {
                open += id
                if (!announced.add(id)) continue
                val m = all.firstOrNull { it.id == id } ?: continue
                val watching = Attention.watching(room.roomId, m.threadRoot, id, needsAnswer = true)
                if (!shouldAnnounce(Notifier.wasPushed(id), now - m.ts, watching)) continue
                promptNote(room, all, m, now)?.let { Notifier.show(app, it) }
            }
        }
        // Answered (desktop, the app, a notification): the notification goes.
        (lastOpen - open).forEach { Notifier.cancel(app, it) }
        lastOpen = open
    }

    /** [m], an open prompt of [room], as its notification: the choices as buttons. */
    private fun promptNote(room: AgentRoom, all: List<Msg>, m: Msg, now: Long): Note? {
        val h = Matrix.agentList.value.harnessOf(m.sender)
        val view = Runs.viewMessages(all, ViewKey(room.roomId, m.threadRoot))
        val i = view.indexOfFirst { it.id == m.id }
        if (i < 0) return null
        val choices = Runs.openPrompt(view, i, h, now)
        val question = Runs.pendingQuestion(view, i, m.sender, h, now)
        val kind = if (choices.isNotEmpty()) Note.Kind.Prompt else Note.Kind.Question
        // Title = the project + what waits; subtext = the run's
        // headline (the thread's request), as the live-runs card has it.
        val headline = m.threadRoot?.let { root ->
            threadTitle(all.firstOrNull { it.id == root }, all.filter { it.threadRoot == root })
        }
        return Note(
            room.roomId, m.id, m.threadRoot, m.sender,
            Notifier.needsYouTitle(room.name, m.senderName), m.body, kind,
            choices.ifEmpty { question.orEmpty() },
            headline = headline,
        )
    }

    /**
     * Whether a prompt a push has just brought is already over, as this
     * phone's timeline has it: answered on the desktop while the push was on
     * its way (sync is usually the faster of the two, and a dozing phone
     * holds pushes back for minutes). The push carries the message as it was
     * first sent — still asking — so it went up with its buttons after the
     * watcher had taken the answered prompt's notification down, and nothing
     * took it down a second time. False when the timeline doesn't have the
     * message: then the push is all there is to go by.
     */
    fun settled(note: Note, now: Long = System.currentTimeMillis()): Boolean {
        if (note.kind == Note.Kind.Message) return false
        val all = Matrix.roomTimeline(note.roomId)?.messages?.value ?: return false
        val m = all.firstOrNull { it.id == note.eventId } ?: return false
        return over(Runs.viewMessages(all, ViewKey(note.roomId, m.threadRoot)), m.id, Matrix.agentList.value.harnessOf(m.sender), now)
    }

    /** [id], loaded in [view], waits on nobody any more: neither an open prompt nor a pending question. */
    internal fun over(view: List<Msg>, id: String, h: Harness, now: Long): Boolean {
        val i = view.indexOfFirst { it.id == id }
        if (i < 0) return false
        return Runs.openPrompt(view, i, h, now).isEmpty() && Runs.pendingQuestion(view, i, view[i].sender, h, now) == null
    }

    /**
     * A notification from the push path. Under the watcher's lock, so the two
     * can't cross: when sync was faster than the push and the prompt is open
     * here already, the push shows it with its buttons (it used to put the
     * bare message back over the watcher's prompt); when the prompt opens
     * later, the watcher finds the event marked as pushed and upgrades it.
     */
    @Synchronized
    fun showPushed(app: Context, note: Note, quietly: Boolean = false) {
        Notifier.markPushed(note.eventId)
        val open = if (note.kind == Note.Kind.Message) openPrompt(note.roomId, note.eventId) else null
        val shown = open ?: note
        val asks = shown.kind != Note.Kind.Message
        // The watcher got there first and its prompt is still up: the same
        // prompt again is a refresh, not a second alert.
        val again = asks && note.eventId in announced && Notifier.isShowing(app, note.eventId)
        // Buttons are on it now: the watcher has nothing to add (and would alert a second time).
        if (asks) announced += note.eventId
        Notifier.show(app, shown, quietly || again)
    }

    private fun openPrompt(roomId: String, eventId: String): Note? {
        val room = Matrix.agentRooms.value[roomId]?.takeIf { eventId in it.prompts } ?: return null
        val all = Matrix.roomTimeline(roomId)?.messages?.value ?: return null
        val m = all.firstOrNull { it.id == eventId } ?: return null
        return promptNote(room, all, m, System.currentTimeMillis())
    }
}
