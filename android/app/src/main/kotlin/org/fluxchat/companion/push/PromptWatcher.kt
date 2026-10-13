package org.fluxchat.companion.push

import android.content.Context
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import org.fluxchat.companion.App
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.agents.ViewKey
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

    fun start(context: Context) {
        val app = context.applicationContext
        Matrix.agentRooms.onEach { rooms ->
            val now = System.currentTimeMillis()
            val open = HashSet<String>()
            val list = Matrix.agentList.value
            for (room in rooms.values) {
                val all = Matrix.roomTimeline(room.roomId)?.messages?.value ?: continue
                for (id in room.prompts) {
                    open += id
                    if (!announced.add(id)) continue
                    val m = all.firstOrNull { it.id == id } ?: continue
                    if (now - m.ts > FRESH_MS || App.foreground) continue
                    val h = list.harnessOf(m.sender)
                    val view = Runs.viewMessages(all, ViewKey(room.roomId, m.threadRoot))
                    val i = view.indexOfFirst { it.id == id }
                    if (i < 0) continue
                    val choices = Runs.openPrompt(view, i, h, now)
                    val question = Runs.pendingQuestion(view, i, m.sender, h, now)
                    val kind = if (choices.isNotEmpty()) Note.Kind.Prompt else Note.Kind.Question
                    val name = m.senderName
                    // Title = the project + what waits; subtext = the run's
                    // headline (the thread's request), as the live-runs card has it.
                    val headline = m.threadRoot?.let { root ->
                        threadTitle(all.firstOrNull { it.id == root }, all.filter { it.threadRoot == root })
                    }
                    Notifier.show(
                        app,
                        Note(
                            room.roomId, id, m.threadRoot, m.sender,
                            "${room.name} · $name needs you", m.body, kind,
                            choices.ifEmpty { question.orEmpty() },
                            headline = headline,
                        ),
                    )
                }
            }
            // Answered (desktop, the app, a notification): the notification goes.
            (lastOpen - open).forEach { Notifier.cancel(app, it) }
            lastOpen = open
        }.launchIn(Matrix.scope)
    }
}
