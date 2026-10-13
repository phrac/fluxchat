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
    /** The run's headline (its thread's request), shown under the title. */
    val headline: String? = null,
) {
    enum class Kind { Prompt, Question, Message }
}

object Notifier {
    /** Answer buttons that fit beside Reply: Android shows three actions at most. */
    private const val MAX_CHOICE_ACTIONS = 2
    const val REPLY_KEY = "reply"

    /** A choice as its button / reply chip reads: "✅ Approve once", "1 Deploy". */
    fun choiceTitle(key: String, label: String) = (if (label == key) key else "$key $label").take(40)

    /**
     * What an agent message should notify as, or null: step logs aren't news
     * (unless they wait on you).
     */
    fun noteFor(
        roomId: String, eventId: String, threadRoot: String?, sender: String, senderName: String,
        roomName: String, body: String, protocol: Protocol?, harness: Harness, now: Long, ts: Long,
        headline: String? = null,
    ): Note? {
        val name = if (senderName == roomName) senderName else "$senderName · $roomName"
        val waiting = protocol?.runState() == RunState.NeedsYou && protocol.choices.isNotEmpty()
        if (waiting) {
            // Project first: in the shade, which room waits matters most (a DM
            // with the agent by its name collapses to one name).
            val title = if (senderName == roomName) "$roomName needs you" else "$roomName · $senderName needs you"
            return Note(roomId, eventId, threadRoot, sender, title, body, Note.Kind.Prompt, protocol.choices.map { it.key to it.label }, headline = headline)
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
            .setSubText(note.headline)
            .setContentIntent(openIntent(context, note))
            .setAutoCancel(true)
            .setGroup(note.roomId)
            .setCategory(if (prompt) NotificationCompat.CATEGORY_REMINDER else NotificationCompat.CATEGORY_MESSAGE)
            .setPriority(if (prompt) NotificationCompat.PRIORITY_HIGH else NotificationCompat.PRIORITY_DEFAULT)
        val choiceAction = if (note.kind == Note.Kind.Prompt) ActionReceiver.REACT else ActionReceiver.SEND
        buttons(note).forEachIndexed { i, (key, label) ->
            b.addAction(0, choiceTitle(key, label).take(30), actionIntent(context, note, choiceAction, key, label, i))
        }
        if (note.choices.isEmpty()) note.stopCommand?.let { b.addAction(0, "Stop", actionIntent(context, note, ActionReceiver.SEND, it, "Stop", 7)) }
        // A prompt with a Deny answers its quick reply as a deny-with-reason;
        // otherwise the reply is a plain message to the thread.
        val deny = if (note.kind == Note.Kind.Prompt) Runs.denyChoice(note.choices) else null
        // Reply: to the thread the message is in (or the room). Every choice is
        // a chip there too (the buttons only fit two); a picked chip answers
        // as its button would.
        val input = RemoteInput.Builder(REPLY_KEY)
            .setLabel(if (deny != null) "Reason to deny (or pick one)" else if (note.choices.isEmpty()) "Reply to the agent" else "Answer, or pick one")
            .apply { if (note.choices.size > MAX_CHOICE_ACTIONS) setChoices(note.choices.map { (k, l) -> choiceTitle(k, l) }.toTypedArray()) }
            .build()
        val reply = actionIntent(context, note, if (deny != null) ActionReceiver.DENY else ActionReceiver.REPLY, deny ?: "", "Deny", 9, mutable = true) { i ->
            i.putExtra(ActionReceiver.CHOICE_TITLES, note.choices.map { (k, l) -> choiceTitle(k, l) }.toTypedArray())
                .putExtra(ActionReceiver.CHOICE_KEYS, note.choices.map { it.first }.toTypedArray())
                .putExtra(ActionReceiver.CHOICE_ACTION, choiceAction)
        }
        b.addAction(
            NotificationCompat.Action.Builder(0, if (note.choices.size > MAX_CHOICE_ACTIONS) "More" else "Reply", reply)
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

    /**
     * The answers that get buttons: all of them when they fit, else the first
     * and the last (an approval's Approve once and Deny; the rest are chips).
     */
    fun buttons(note: Note): List<Pair<String, String>> = when {
        note.choices.size <= MAX_CHOICE_ACTIONS -> note.choices
        note.kind == Note.Kind.Prompt -> listOf(note.choices.first(), note.choices.last())
        else -> note.choices.take(MAX_CHOICE_ACTIONS)
    }

    private fun actionIntent(
        context: Context, note: Note, action: String, value: String, label: String, n: Int, mutable: Boolean = false,
        extra: (Intent) -> Unit = {},
    ): PendingIntent {
        val i = Intent(context, ActionReceiver::class.java)
            .setAction(action)
            .putExtra(ActionReceiver.ROOM, note.roomId)
            .putExtra(ActionReceiver.EVENT, note.eventId)
            .putExtra(ActionReceiver.THREAD, note.threadRoot)
            .putExtra(ActionReceiver.VALUE, value)
            .putExtra(ActionReceiver.LABEL, label)
        extra(i)
        val flags = PendingIntent.FLAG_UPDATE_CURRENT or (if (mutable) PendingIntent.FLAG_MUTABLE else PendingIntent.FLAG_IMMUTABLE)
        return PendingIntent.getBroadcast(context, (note.eventId + n).hashCode(), i, flags)
    }
}
