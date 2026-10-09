package org.fluxchat.companion.push

import android.util.Log
import kotlinx.coroutines.launch
import org.fluxchat.companion.Diag
import org.fluxchat.companion.agents.Protocol
import org.fluxchat.companion.matrix.Matrix
import org.json.JSONObject
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
 */
class FluxPushService : PushService() {
    private val scope get() = Matrix.scope

    override fun onNewEndpoint(endpoint: PushEndpoint, instance: String) {
        Matrix.init(applicationContext)
        Diag.note("push: new endpoint from the distributor")
        scope.launch { Push.newEndpoint(applicationContext, endpoint.url) }
    }

    override fun onMessage(message: PushMessage, instance: String) {
        Matrix.init(applicationContext)
        val text = String(message.content, Charsets.UTF_8)
        val n = runCatching { JSONObject(text).optJSONObject("notification") }.getOrNull()
        if (n == null) {
            Diag.error("push: not a Matrix notification (${message.content.size} bytes, decrypted ${message.decrypted})")
            return
        }
        val roomId = n.optString("room_id")
        val eventId = n.optString("event_id")
        if (roomId.isEmpty() || eventId.isEmpty()) {
            Diag.note("push: counts only (${n.optJSONObject("counts")})")
            return
        }
        scope.launch {
            Diag.pushReceived(roomId, eventId)
            runCatching { handle(roomId, eventId) }.onFailure { Diag.error("Push for $eventId", it) }
        }
    }

    private suspend fun handle(roomId: String, eventId: String) {
        if (!Matrix.ensureStarted()) return Diag.error("push: no session to fetch $eventId with")
        if (Matrix.agentList.value.agents.isEmpty()) Matrix.loadAgentList()
        val nc = Matrix.notificationClient() ?: return Diag.error("push: no notification client")
        val status = nc.getNotification(roomId, eventId)
        val item = (status as? NotificationStatus.Event)?.item ?: return Diag.note("push: $eventId → ${status.javaClass.simpleName}")
        val ev = (item.event as? NotificationEvent.Timeline)?.event ?: return
        val sender = ev.senderId()
        if (sender == Matrix.userId) return
        val msgType = ((ev.content() as? TimelineEventContent.MessageLike)?.content as? MessageLikeEventContent.RoomMessage)?.messageType ?: return
        val list = Matrix.agentList.value
        val isAgent = list.agents.any { it.userId == sender }
        // An agent companion: your agents, and people mentioning you.
        if (!isAgent && item.hasMention != true) return Diag.note("push: $eventId from $sender, not an agent or a mention")
        val body = when (msgType) {
            is MessageType.Text -> msgType.content.body
            is MessageType.Notice -> msgType.content.body
            is MessageType.Emote -> "* " + msgType.content.body
            is MessageType.Image -> "[image]"
            is MessageType.File -> "[file]"
            else -> "[message]"
        }
        val senderName = item.senderInfo.displayName ?: sender
        val note = Notifier.noteFor(
            roomId, eventId, item.threadId ?: ev.threadRootEventId(), sender, senderName,
            item.roomInfo.displayName, body, Protocol.fromEventJson(item.rawEvent), list.harnessOf(sender),
            System.currentTimeMillis(), ev.timestamp().toLong(),
        ) ?: return
        Notifier.show(applicationContext, note)
    }

    override fun onRegistrationFailed(reason: FailedReason, instance: String) {
        Push.failed("The push distributor refused: $reason")
    }

    override fun onUnregistered(instance: String) {
        Matrix.init(applicationContext)
        scope.launch { Push.forget(applicationContext) }
    }
}
