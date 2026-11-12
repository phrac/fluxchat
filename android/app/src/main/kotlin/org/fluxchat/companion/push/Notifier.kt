package org.fluxchat.companion.push

import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.RemoteInput
import org.fluxchat.companion.Diag
import org.fluxchat.companion.MainActivity
import org.fluxchat.companion.Notifications
import org.fluxchat.companion.R
import org.fluxchat.companion.agents.Harness
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.Protocol
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.ui.Screen

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

    /** On each notification of ours: what it is about, so it can be taken down when that comes into view. */
    private const val X_ROOM = "fluxchat.room"
    private const val X_THREAD = "fluxchat.thread"
    /** It waits on an answer (a prompt, a question, an answer that didn't go out). */
    private const val X_ASKS = "fluxchat.asks"

    private val quiet = Streak()

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

    /**
     * A pushed message whose keys haven't reached this phone yet. It used to
     * be dropped as "not a renderable message": the agent had answered, or
     * was waiting on you, and the phone said nothing. What it says can't be
     * shown, so: who, where, and a tap opens the session. Replaced by the
     * real thing if it decrypts a moment later.
     */
    fun undecryptedNote(roomId: String, eventId: String, threadRoot: String?, sender: String, senderName: String, roomName: String): Note {
        val title = if (senderName == roomName) senderName else "$senderName · $roomName"
        return Note(roomId, eventId, threadRoot, sender, title, "New message from $senderName", Note.Kind.Message)
    }

    /** Notification text: Markdown code fences dropped (their contents kept). */
    private fun plain(body: String): String =
        body.lineSequence().filterNot { it.trimStart().startsWith("```") }.joinToString("\n").trim()

    /** The latest notes shown, by event: a failed answer puts its prompt back (see [failed]). */
    private val shown = object : LinkedHashMap<String, Note>() {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Note>?) = size > 40
    }

    /**
     * Events the push path notified (newest last, bounded): the prompt
     * watcher gives exactly these their answer buttons however late the
     * agent's reactions arrive, and nothing it merely finds in history.
     */
    private val pushed = LinkedHashSet<String>()
    private const val PUSHED_KEPT = 200

    fun markPushed(eventId: String) = synchronized(pushed) {
        pushed.remove(eventId)
        pushed.add(eventId)
        while (pushed.size > PUSHED_KEPT) pushed.remove(pushed.first())
    }

    fun wasPushed(eventId: String): Boolean = synchronized(pushed) { eventId in pushed }

    /** Its notification is gone (seen, read, answered): nothing left to give buttons to. */
    private fun forget(eventId: String) {
        synchronized(pushed) { pushed.remove(eventId) }
        synchronized(shown) { shown.remove(eventId) }
    }

    /** [quietly]: a replacement for a notification that already alerted (same news, better text). */
    fun show(context: Context, note0: Note, quietly: Boolean = false) {
        val note = note0.copy(body = plain(note0.body))
        synchronized(shown) { shown[note.eventId] = note0 }
        val nm = NotificationManagerCompat.from(context)
        if (!nm.areNotificationsEnabled()) {
            // Once, not per push: with notifications off every push said so.
            if (quiet.starts("off")) Diag.error("Notifications are turned off for the app: nothing is shown until they're allowed")
            return
        }
        if (quiet.ends("off")) Diag.note("notifications are allowed again")
        val prompt = note.kind != Note.Kind.Message
        val b = NotificationCompat.Builder(context, if (prompt) Notifications.PROMPTS else Notifications.RUNS)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(note.title)
            .setContentText(note.body.lineSequence().firstOrNull().orEmpty())
            .setStyle(NotificationCompat.BigTextStyle().bigText(note.body.take(1500)))
            .setSubText(note.headline)
            .setContentIntent(openIntent(context, note))
            .setAutoCancel(true)
            .setOnlyAlertOnce(quietly)
            .addExtras(about(note.roomId, note.threadRoot, prompt))
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
            .onSuccess { Diag.notificationShown(note.kind.name, note.eventId) }
            .onFailure { Diag.error("Notification for ${note.eventId}", it) }
    }

    /** Replace a notification with a short "done" line (after an answer from it). */
    fun answered(context: Context, eventId: String, text: String) {
        val b = NotificationCompat.Builder(context, Notifications.RUNS)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(text)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setTimeoutAfter(4000)
        forget(eventId)
        runCatching { NotificationManagerCompat.from(context).notify(eventId, 0, b.build()) }
            .onFailure { Diag.error("Notification for $eventId", it) }
    }

    /**
     * An answer from a notification didn't go out. The prompt comes back,
     * buttons and all — it used to be replaced by a line that vanished after
     * four seconds, and nothing showed it again: the agent waited with
     * nothing on the phone. When this process no longer has the prompt (it
     * was restarted since), a note that opens the session stands in.
     */
    fun failed(context: Context, roomId: String, threadRoot: String?, eventId: String) {
        val note = synchronized(shown) { shown[eventId] }
        if (note != null) {
            show(context, note.copy(title = "Not sent · " + note.title.removePrefix("Not sent · ")))
            return
        }
        val stand = Note(roomId, eventId, threadRoot, "", "Couldn't send your answer", "Open fluxchat to answer.", Note.Kind.Message)
        val b = NotificationCompat.Builder(context, Notifications.PROMPTS)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(stand.title)
            .setContentText(stand.body)
            .setContentIntent(openIntent(context, stand))
            .setAutoCancel(true)
            .addExtras(about(roomId, threadRoot, asks = true))
        runCatching { NotificationManagerCompat.from(context).notify(eventId, 0, b.build()) }
            .onFailure { Diag.error("Notification for $eventId", it) }
    }

    fun cancel(context: Context, eventId: String) {
        forget(eventId)
        NotificationManagerCompat.from(context).cancel(eventId, 0)
    }

    private fun about(roomId: String, threadRoot: String?, asks: Boolean) = Bundle().apply {
        putString(X_ROOM, roomId)
        putString(X_THREAD, threadRoot)
        putBoolean(X_ASKS, asks)
    }

    /**
     * Take down the notifications [gone] picks, by what each is about (room,
     * thread root, event, whether it waits on an answer). Read off the
     * notifications themselves, so ones an earlier process posted go too.
     */
    private fun cancelWhere(context: Context, why: String, gone: (String, String?, String, Boolean) -> Boolean) {
        try {
            val nm = context.getSystemService(NotificationManager::class.java) ?: return
            var n = 0
            for (sbn in nm.activeNotifications) {
                val x = sbn.notification.extras ?: continue
                val room = x.getString(X_ROOM) ?: continue
                val tag = sbn.tag ?: continue
                if (!gone(room, x.getString(X_THREAD), tag, x.getBoolean(X_ASKS))) continue
                forget(tag)
                nm.cancel(tag, sbn.id)
                n++
            }
            if (n > 0) Diag.note("notifications: $n cleared ($why)")
        } catch (e: Exception) {
            Diag.error("Clearing notifications ($why)", e)
        }
    }

    /** [top] is in front of you: what it shows needs no notification any more. */
    fun cancelSeen(context: Context, top: Screen) {
        if (top !is Screen.Room && top !is Screen.Thread) return
        cancelWhere(context, "opened") { room, thread, event, asks -> Attention.covers(top, room, thread, event, asks) }
    }

    /**
     * Everything was read somewhere else (the server reports nothing unread):
     * the news goes. What waits on an answer stays — reading it isn't
     * answering it; that goes when it is answered.
     */
    fun cancelRead(context: Context) {
        cancelWhere(context, "read elsewhere") { _, _, _, asks -> !asks }
    }

    /** Whether [eventId]'s notification is still up (not opened or swiped away). */
    fun isShowing(context: Context, eventId: String): Boolean = try {
        context.getSystemService(NotificationManager::class.java)?.activeNotifications?.any { it.tag == eventId } == true
    } catch (e: Exception) {
        Diag.error("Reading the notifications shown", e)
        false
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
