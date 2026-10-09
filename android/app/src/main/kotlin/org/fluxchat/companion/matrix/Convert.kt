package org.fluxchat.companion.matrix

import org.fluxchat.companion.agents.FileKind
import org.fluxchat.companion.agents.FileRef
import org.fluxchat.companion.agents.Media
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.Protocol
import org.fluxchat.companion.agents.Reaction
import org.fluxchat.companion.agents.SendState
import org.matrix.rustcomponents.sdk.EmbeddedEventDetails
import org.matrix.rustcomponents.sdk.EventOrTransactionId
import org.matrix.rustcomponents.sdk.EventSendState
import org.matrix.rustcomponents.sdk.EventTimelineItem
import org.matrix.rustcomponents.sdk.MessageContent
import org.matrix.rustcomponents.sdk.MessageType
import org.matrix.rustcomponents.sdk.MsgLikeKind
import org.matrix.rustcomponents.sdk.ProfileDetails
import org.matrix.rustcomponents.sdk.TimelineItem
import org.matrix.rustcomponents.sdk.TimelineItemContent

/** A timeline item as a [Msg] (null: date dividers and other virtual items). */
fun convert(item: TimelineItem): Msg? {
    val ev = item.asEvent() ?: return null
    return convert(ev)
}

fun convert(ev: EventTimelineItem): Msg {
    val id = when (val i = ev.eventOrTransactionId) {
        is EventOrTransactionId.EventId -> i.eventId
        is EventOrTransactionId.TransactionId -> "~" + i.transactionId
    }
    val profile = ev.senderProfile as? ProfileDetails.Ready
    val name = profile?.displayName ?: ev.sender
    var body = ""
    var formatted: String? = null
    var isMessage = false
    var edited = false
    var threadRoot: String? = null
    var threadCount = 0
    var threadLatest: String? = null
    var protocol: Protocol? = null
    var media: Media? = null
    var file: FileRef? = null
    when (val c = ev.content) {
        is TimelineItemContent.MsgLike -> {
            val ml = c.content
            threadRoot = ml.threadRoot
            ml.threadSummary?.let { s ->
                threadCount = s.numReplies().toInt()
                threadLatest = (s.latestEvent() as? EmbeddedEventDetails.Ready)?.let { summary(it.content) }
            }
            when (val k = ml.kind) {
                is MsgLikeKind.Message -> {
                    isMessage = true
                    body = messageBody(k.content)
                    (k.content.msgType as? MessageType.Image)?.content?.let { img ->
                        media = runCatching {
                            Media(img.source.toJson(), img.info?.thumbnailSource?.toJson(), img.info?.width?.toInt(), img.info?.height?.toInt())
                        }.getOrNull()
                        if (media != null) body = img.caption.orEmpty()
                    }
                    file = runCatching { fileRef(k.content.msgType) }.getOrNull()
                    if (file != null) body = caption(k.content.msgType).orEmpty()
                    formatted = formattedBody(k.content.msgType)
                    edited = k.content.isEdited
                    protocol = Protocol.fromEventJson(ev.lazyProvider.latestJson() ?: ev.lazyProvider.debugInfo().originalJson)
                }
                is MsgLikeKind.Sticker -> { isMessage = true; body = "[sticker] ${k.body}" }
                is MsgLikeKind.Poll -> { isMessage = true; body = "[poll] ${k.question}" }
                is MsgLikeKind.UnableToDecrypt -> { isMessage = true; body = "Unable to decrypt this message" }
                is MsgLikeKind.Redacted -> body = "Message deleted"
                else -> {}
            }
        }
        else -> {}
    }
    val sendState = when (ev.localSendState) {
        null, is EventSendState.Sent -> SendState.Sent
        is EventSendState.NotSentYet -> SendState.Sending
        else -> SendState.Failed
    }
    return Msg(
        id = id,
        sender = ev.sender,
        senderName = name,
        body = body,
        ts = ev.timestamp.toLong(),
        isOwn = ev.isOwn,
        reactions = ev.reactions.map { r -> Reaction(r.key, r.senders.map { it.senderId }) },
        threadRoot = threadRoot,
        threadCount = threadCount,
        threadLatest = threadLatest,
        agent = protocol,
        sendState = sendState,
        isMessage = isMessage,
        edited = edited,
        formatted = formatted,
        senderAvatar = profile?.avatarUrl,
        media = media,
        file = file,
    )
}

private fun messageBody(c: MessageContent): String = when (val t = c.msgType) {
    is MessageType.Image -> "[image] ${c.body}"
    is MessageType.Video -> "[video] ${c.body}"
    is MessageType.Audio -> "[audio] ${c.body}"
    is MessageType.File -> "[file] ${c.body}"
    is MessageType.Location -> "[location] ${c.body}"
    is MessageType.Emote -> "* ${t.content.body}"
    else -> c.body
}

/** A file, video or audio message's attachment. */
private fun fileRef(t: MessageType): FileRef? = when (t) {
    is MessageType.File -> t.content.let { c ->
        FileRef(c.source.toJson(), c.filename, c.info?.mimetype, c.info?.size?.toLong(), FileKind.File)
    }
    is MessageType.Video -> t.content.let { c ->
        val thumb = c.info?.thumbnailSource?.toJson()?.let { Media(it, it, c.info?.width?.toInt(), c.info?.height?.toInt()) }
        FileRef(c.source.toJson(), c.filename, c.info?.mimetype, c.info?.size?.toLong(), FileKind.Video, thumb, c.info?.duration?.toMillis())
    }
    is MessageType.Audio -> t.content.let { c ->
        FileRef(c.source.toJson(), c.filename, c.info?.mimetype, c.info?.size?.toLong(), FileKind.Audio, durationMs = c.info?.duration?.toMillis())
    }
    else -> null
}

/** A file message's caption (its body is then the file name). */
private fun caption(t: MessageType): String? = when (t) {
    is MessageType.File -> t.content.caption
    is MessageType.Video -> t.content.caption
    is MessageType.Audio -> t.content.caption
    else -> null
}

private fun formattedBody(t: MessageType): String? = when (t) {
    is MessageType.Text -> t.content.formatted?.body
    is MessageType.Notice -> t.content.formatted?.body
    is MessageType.Emote -> t.content.formatted?.body
    else -> null
}

private fun summary(c: TimelineItemContent): String? = contentBody(c)

/** A message's text (other content: null). */
fun contentBody(c: TimelineItemContent): String? =
    ((c as? TimelineItemContent.MsgLike)?.content?.kind as? MsgLikeKind.Message)?.content?.body
