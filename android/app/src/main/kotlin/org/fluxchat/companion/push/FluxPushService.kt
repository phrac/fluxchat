package org.fluxchat.companion.push

import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.fluxchat.companion.Diag
import org.fluxchat.companion.agents.Protocol
import org.fluxchat.companion.matrix.Matrix
import org.matrix.rustcomponents.sdk.MessageLikeEventContent
import org.matrix.rustcomponents.sdk.MessageType
import org.matrix.rustcomponents.sdk.NotificationEvent
import org.matrix.rustcomponents.sdk.NotificationStatus
import org.matrix.rustcomponents.sdk.TimelineEventContent
import org.unifiedpush.android.connector.FailedReason
import org.unifiedpush.android.connector.PushService
import org.unifiedpush.android.connector.data.PushEndpoint
import org.unifiedpush.android.connector.data.PushMessage

/**
 * UnifiedPush events: endpoints, and pushes from the homeserver (`event_id_only`).
 *
 * Work runs on the app's scope, not one tied to this service: the connector
 * unbinds after 5 s but keeps calling the instance it had (it never learns
 * of the unbind), so a scope cancelled in onDestroy dropped every later push.
 *
 * Nothing of the connector's keeps the phone awake for that work either
 * (connector 3.3.5: its receiver takes a 10 s wake lock and `goAsync`, and
 * lets go of both the moment [onMessage] returns), so each push holds its
 * own short wake lock ([Awake]) until its notification is up.
 */
class FluxPushService : PushService() {
    private val scope get() = Matrix.scope

    override fun onNewEndpoint(endpoint: PushEndpoint, instance: String) {
        Matrix.init(applicationContext)
        // Host only: the endpoint's path is a secret topic.
        val host = runCatching { java.net.URL(endpoint.url).host }.getOrNull() ?: "?"
        Diag.note("push: new endpoint from the distributor ($host)")
        quiet.ends("unavailable")
        val awake = Awake.hold(applicationContext, "endpoint")
        scope.launch {
            try {
                Push.newEndpoint(applicationContext, endpoint.url)
            } catch (e: Exception) {
                Diag.error("Taking the new push endpoint", e)
            } finally {
                awake.release()
            }
        }
    }

    override fun onMessage(message: PushMessage, instance: String) {
        Matrix.init(applicationContext)
        when (val push = PushPayload.parse(String(message.content, Charsets.UTF_8))) {
            null -> Diag.error("push: not a Matrix notification (${message.content.size} bytes, decrypted ${message.decrypted})")
            is PushPayload.Counts -> counts(push.unread)
            is PushPayload.Event -> {
                val awake = Awake.hold(applicationContext, "push")
                scope.launch {
                    try {
                        Diag.pushReceived(push.roomId, push.eventId)
                        if (handle(push.roomId, push.eventId, again = false) == Outcome.Undecrypted) {
                            // The keys usually follow within seconds (they come
                            // by sync, the push doesn't wait for them).
                            delay(UNDECRYPTED_RETRY_MS)
                            handle(push.roomId, push.eventId, again = true)
                        }
                    } catch (e: Throwable) {
                        // Everything: nothing above this coroutine would catch it.
                        Diag.error("Push for ${push.eventId}", e)
                    } finally {
                        awake.release()
                    }
                }
            }
        }
    }

    /**
     * A push with no event: something was read on another device. With
     * nothing left unread anywhere, the news on this phone is old news.
     * (The server reports one total, not a count per project, so with
     * several unread this can't tell which one was read.)
     */
    private fun counts(unread: Int) {
        // One line per change of the number, not one per read receipt.
        if (unread != lastCounts) {
            lastCounts = unread
            Diag.note("push: counts only (unread $unread)")
        }
        if (unread == 0) Notifier.cancelRead(applicationContext)
    }

    private enum class Outcome { Notified, Undecrypted, Skipped }

    /** [again]: the second look at an event that wouldn't decrypt; its stand-in is up (or was dismissed). */
    private suspend fun handle(roomId: String, eventId: String, again: Boolean): Outcome {
        if (!Matrix.ensureStarted()) return skip("push: no session to fetch $eventId with", error = true)
        if (Matrix.agentList.value.agents.isEmpty()) Matrix.loadAgentList()
        val nc = Matrix.notificationClient() ?: return skip("push: no notification client", error = true)
        val status = nc.getNotification(roomId, eventId)
        val item = (status as? NotificationStatus.Event)?.item ?: return skip("push: $eventId → ${status.javaClass.simpleName}")
        val ev = (item.event as? NotificationEvent.Timeline)?.event ?: return skip("push: $eventId is not a timeline event (${item.event.javaClass.simpleName})")
        val sender = ev.senderId()
        if (sender == Matrix.userId) return skip("push: $eventId is our own message")
        val content = (ev.content() as? TimelineEventContent.MessageLike)?.content
        val list = Matrix.agentList.value
        val isAgent = list.agents.any { it.userId == sender }
        // An agent companion: your agents, and people mentioning you.
        if (!isAgent && item.hasMention != true) return skip("push: $eventId from $sender, not an agent or a mention")
        val senderName = item.senderInfo.displayName ?: sender
        val threadRoot = item.threadId ?: ev.threadRootEventId()
        val note = when (content) {
            is MessageLikeEventContent.RoomMessage -> {
                val msgType = content.messageType
                val body = when (msgType) {
                    is MessageType.Text -> msgType.content.body
                    is MessageType.Notice -> msgType.content.body
                    is MessageType.Emote -> "* " + msgType.content.body
                    is MessageType.Image -> "[image]"
                    is MessageType.File -> "[file]"
                    else -> "[message]"
                }
                // The run's headline for the notification's subtext, when this room's
                // timeline is loaded (a push alone doesn't load one).
                val msgs = Matrix.roomTimeline(roomId)?.messages?.value.orEmpty()
                val headline = threadRoot?.takeIf { msgs.isNotEmpty() }?.let { root ->
                    org.fluxchat.companion.ui.threadTitle(msgs.firstOrNull { it.id == root }, msgs.filter { it.threadRoot == root })
                }
                val full = Notifier.noteFor(
                    roomId, eventId, threadRoot, sender, senderName,
                    item.roomInfo.displayName, body, Protocol.fromEventJson(item.rawEvent), list.harnessOf(sender),
                    System.currentTimeMillis(), ev.timestamp().toLong(), headline,
                )
                if (full == null) {
                    // It turned out to be a step log: its stand-in shouldn't have been news.
                    if (again && Notifier.isShowing(applicationContext, eventId)) Notifier.cancel(applicationContext, eventId)
                    // A working agent pushes one per step: say so once per run of them.
                    return skip("push: step logs in $roomId aren't notified (said once while they keep coming)", streak = "steps:$roomId")
                }
                full
            }
            is MessageLikeEventContent.RoomEncrypted -> {
                // Still no keys: the stand-in from the first look stays as it is.
                if (again) return skip("push: $eventId still can't be decrypted")
                // An edit of a message that already pushed (a step log growing): not news by itself.
                if (PushPayload.isEdit(item.rawEvent)) return skip("push: $eventId is an edit that can't be decrypted yet, not notified")
                Notifier.undecryptedNote(roomId, eventId, threadRoot, sender, senderName, item.roomInfo.displayName)
            }
            else -> return skip("push: $eventId is not a renderable message (${content?.javaClass?.simpleName ?: ev.content().javaClass.simpleName})")
        }
        quiet.ends("steps:$roomId")
        val undecrypted = content is MessageLikeEventContent.RoomEncrypted
        if (again) {
            // Only over the stand-in: opened or swiped away meanwhile, it stays gone.
            if (!Notifier.isShowing(applicationContext, eventId)) return skip("push: $eventId decrypted, its notification was already dismissed")
            PromptWatcher.showPushed(applicationContext, note, quietly = note.kind == Note.Kind.Message)
            return Outcome.Notified
        }
        // Not for what's on screen in front of you. A prompt still alerts
        // unless it is that very session you're looking at.
        if (Attention.watching(roomId, threadRoot, eventId, needsAnswer = note.kind != Note.Kind.Message)) {
            return skip("push: $eventId is on screen, not notified")
        }
        if (undecrypted) Diag.note("push: $eventId can't be decrypted yet, notifying without its text")
        PromptWatcher.showPushed(applicationContext, note)
        return if (undecrypted) Outcome.Undecrypted else Outcome.Notified
    }

    /** Why a push didn't notify, for Diagnostics. [streak]: said once until something else happens there. */
    private fun skip(why: String, error: Boolean = false, streak: String? = null): Outcome {
        if (streak != null && !quiet.starts(streak)) return Outcome.Skipped
        if (error) Diag.error(why) else Diag.note(why)
        return Outcome.Skipped
    }

    override fun onRegistrationFailed(reason: FailedReason, instance: String) {
        Push.failed("The push distributor refused: $reason")
    }

    override fun onTempUnavailable(instance: String) {
        // The distributor has lost its own connection; it tells us again when it's back.
        if (quiet.starts("unavailable")) Diag.note("push: the distributor is temporarily unavailable")
    }

    override fun onUnregistered(instance: String) {
        Matrix.init(applicationContext)
        Diag.note("push: the distributor unregistered us")
        val awake = Awake.hold(applicationContext, "unregistered")
        scope.launch {
            try {
                // The pusher on the server points at a dead endpoint now; removing it needs the session.
                Matrix.ensureStarted()
                Push.forget(applicationContext)
            } catch (e: Exception) {
                Diag.error("Forgetting the push registration", e)
            } finally {
                awake.release()
            }
        }
    }

    private companion object {
        /** How long after a message that wouldn't decrypt it is looked at again. */
        const val UNDECRYPTED_RETRY_MS = 6_000L
        val quiet = Streak()
        @Volatile var lastCounts = -1
    }
}
