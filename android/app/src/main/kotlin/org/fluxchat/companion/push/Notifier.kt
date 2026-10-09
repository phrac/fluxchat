package org.fluxchat.companion.push

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.RemoteInput
import org.fluxchat.companion.MainActivity
import org.fluxchat.companion.Notifications
import org.fluxchat.companion.R
import org.fluxchat.companion.agents.Harness
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.Protocol
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.Runs

/** What a notification is about. */
data class Note(
    val roomId: String,
    val eventId: String,
    val threadRoot: String?,
    val sender: String,
    val title: String,
    val body: String,
    val kind: Kind,
    /** Answers: (key, label); reacted with ([Kind.Prompt]) or sent ([Kind.Question]). */
    val choices: List<Pair<String, String>> = emptyList(),
    /** Stop, as the agent takes it ("!stop"), for a run still going. */
    val stopCommand: String? = null,
) {
    enum class Kind { Prompt, Question, Message }
}

object Notifier {
    /** Answer buttons that fit beside Reply. */
    private const val MAX_CHOICE_ACTIONS = 3
    const val REPLY_KEY = "reply"

    /**
     * What an agent message should notify as, or null: step logs aren't news
     * (unless they wait on you).
     */
    fun noteFor(
        roomId: String, eventId: String, threadRoot: String?, sender: String, senderName: String,
        roomName: String, body: String, protocol: Protocol?, harness: Harness, now: Long, ts: Long,
    ): Note? {
        val name = if (senderName == roomName) senderName else "$senderName · $roomName"
        val waiting = protocol?.runState() == RunState.NeedsYou && protocol.choices.isNotEmpty()
        if (waiting) {
            return Note(roomId, eventId, threadRoot, sender, "$name needs you", body, Note.Kind.Prompt, protocol.choices.map { it.key to it.label })
        }
        if (Runs.progressSteps(body) != null) return null
        val asQuestion = Msg(eventId, sender, senderName, body, ts, isOwn = false)
        Runs.pendingQuestion(listOf(asQuestion), 0, sender, harness, now)?.let { choices ->
            return Note(roomId, eventId, threadRoot, sender, "$name asks", body.trimStart().removePrefix(harness.questionPrefix).trim(), Note.Kind.Question, choices)
        }
        val title = when (protocol?.runState()) {
            RunState.Done -> "$name · done"
            RunState.Failed -> "$name · failed"
            else -> name
        }
        return Note(roomId, eventId, threadRoot, sender, title, body, Note.Kind.Message)
    }

    /** Notification text: Markdown code fences dropped (their contents kept). */
    private fun plain(body: String): String =
        body.lineSequence().filterNot { it.trimStart().startsWith("```") }.joinToString("\n").trim()

    fun show(context: Context, note0: Note) {
        val note = note0.copy(body = plain(note0.body))
        val nm = NotificationManagerCompat.from(context)
        if (!nm.areNotificationsEnabled()) {
            org.fluxchat.companion.Diag.error("Notifications are turned off for the app")
            return
        }
        val prompt = note.kind != Note.Kind.Message
        val b = NotificationCompat.Builder(context, if (prompt) Notifications.PROMPTS else Notifications.RUNS)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(note.title)
            .setContentText(note.body.lineSequence().firstOrNull().orEmpty())
            .setStyle(NotificationCompat.BigTextStyle().bigText(note.body.take(1500)))
            .setContentIntent(openIntent(context, note))
            .setAutoCancel(true)
            .setGroup(note.roomId)
            .setCategory(if (prompt) NotificationCompat.CATEGORY_REMINDER else NotificationCompat.CATEGORY_MESSAGE)
            .setPriority(if (prompt) NotificationCompat.PRIORITY_HIGH else NotificationCompat.PRIORITY_DEFAULT)
        note.choices.take(MAX_CHOICE_ACTIONS).forEachIndexed { i, (key, label) ->
            val action = if (note.kind == Note.Kind.Prompt) ActionReceiver.REACT else ActionReceiver.SEND
            val title = if (label == key) key else "$key $label"
            b.addAction(0, title.take(30), actionIntent(context, note, action, key, label, i))
        }
        note.stopCommand?.let { b.addAction(0, "Stop", actionIntent(context, note, ActionReceiver.SEND, it, "Stop", 7)) }
        // Reply: to the thread the message is in (or the room).
        val input = RemoteInput.Builder(REPLY_KEY).setLabel("Reply to the agent").build()
        b.addAction(
            NotificationCompat.Action.Builder(0, "Reply", actionIntent(context, note, ActionReceiver.REPLY, "", "", 9, mutable = true))
                .addRemoteInput(input)
                .setAllowGeneratedReplies(false)
                .build(),
        )
        runCatching { nm.notify(note.eventId, 0, b.build()) }
            .onSuccess { org.fluxchat.companion.Diag.notificationShown(note.kind.name, note.eventId) }
            .onFailure { org.fluxchat.companion.Diag.error("Notification for ${note.eventId}", it) }
    }

    /** Replace a notification with a short "done" line (after an answer from it). */
    fun answered(context: Context, eventId: String, text: String) {
        val b = NotificationCompat.Builder(context, Notifications.RUNS)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(text)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setTimeoutAfter(4000)
        runCatching { NotificationManagerCompat.from(context).notify(eventId, 0, b.build()) }
    }

    fun cancel(context: Context, eventId: String) {
        NotificationManagerCompat.from(context).cancel(eventId, 0)
    }

    private fun openIntent(context: Context, note: Note): PendingIntent {
        val i = Intent(context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            .putExtra(MainActivity.EXTRA_ROOM, note.roomId)
        note.threadRoot?.let { i.putExtra(MainActivity.EXTRA_THREAD, it) }
        return PendingIntent.getActivity(context, ("open" + note.eventId).hashCode(), i, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
    }

    private fun actionIntent(context: Context, note: Note, action: String, value: String, label: String, n: Int, mutable: Boolean = false): PendingIntent {
        val i = Intent(context, ActionReceiver::class.java)
            .setAction(action)
            .putExtra(ActionReceiver.ROOM, note.roomId)
            .putExtra(ActionReceiver.EVENT, note.eventId)
            .putExtra(ActionReceiver.THREAD, note.threadRoot)
            .putExtra(ActionReceiver.VALUE, value)
            .putExtra(ActionReceiver.LABEL, label)
        val flags = PendingIntent.FLAG_UPDATE_CURRENT or (if (mutable) PendingIntent.FLAG_MUTABLE else PendingIntent.FLAG_IMMUTABLE)
        return PendingIntent.getBroadcast(context, (note.eventId + n).hashCode(), i, flags)
    }
}
