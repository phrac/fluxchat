package org.fluxchat.companion.push

import org.json.JSONException
import org.json.JSONObject

/** What the homeserver pushed (`event_id_only`): an event to fetch, or just new counts. */
internal sealed interface PushPayload {
    data class Event(val roomId: String, val eventId: String) : PushPayload

    /**
     * No event, only the account's unread total: the server sends it when
     * something was read. Zero means nothing is left unread anywhere.
     */
    data class Counts(val unread: Int) : PushPayload

    companion object {
        /** Null when it isn't a Matrix notification at all. */
        fun parse(text: String): PushPayload? {
            val n = try {
                JSONObject(text).optJSONObject("notification")
            } catch (e: JSONException) {
                null
            } ?: return null
            val roomId = text(n, "room_id")
            val eventId = text(n, "event_id")
            if (roomId.isNotEmpty() && eventId.isNotEmpty()) return Event(roomId, eventId)
            // "Counts whose value is zero should be omitted" (push gateway
            // API): a missing count is a zero, as is a missing `counts`.
            return Counts(n.optJSONObject("counts")?.optInt("unread", 0) ?: 0)
        }

        /** Absent or JSON null → "" (optString hands back the string "null" for a null). */
        private fun text(o: JSONObject, key: String): String = if (o.isNull(key)) "" else o.optString(key)

        /**
         * Whether an event (its raw JSON) replaces an earlier one. The
         * relation stays readable on an encrypted event, so this works on one
         * that couldn't be decrypted.
         */
        fun isEdit(rawEvent: String): Boolean = try {
            JSONObject(rawEvent).optJSONObject("content")?.optJSONObject("m.relates_to")?.let { text(it, "rel_type") } == "m.replace"
        } catch (e: JSONException) {
            false
        }

        /**
         * An edit that could be read, as what it does: null when [rawEvent]
         * isn't one. An edit is an event of its own (a new ID, and a body
         * that is the old message's with "* " in front), but what it is
         * *about* is the message it replaces: that one is in the timeline,
         * takes the reactions and the replies, and has the notification.
         */
        fun edit(rawEvent: String): Edit? = try {
            val content = JSONObject(rawEvent).optJSONObject("content")
            val rel = content?.optJSONObject("m.relates_to")
            if (content == null || rel == null || text(rel, "rel_type") != "m.replace") null
            else text(rel, "event_id").takeIf { it.isNotEmpty() }?.let { of ->
                val now = content.optJSONObject("m.new_content")?.let { text(it, "body") }?.takeIf { it.isNotEmpty() }
                    ?: text(content, "body").removePrefix("* ")
                Edit(of, now)
            }
        } catch (e: JSONException) {
            null
        }
    }

    /** [of]: the event edited; [body]: what it says now. */
    data class Edit(val of: String, val body: String)
}

/** What a pushed edit is to the notifications ([editNews]). */
internal enum class EditNews { None, Refresh, New }

/**
 * An edit of a message is not news by itself: the message already pushed.
 * It notified all the same, once per edit and as an event of its own — a
 * step log growing a line at a time, each line a notification reading
 * "* 🔧 …", with buttons that reacted to the edit instead of the prompt.
 *
 * [asks]: as edited it waits on you (a status message turned into a prompt);
 * [showing]: the edited message's notification is still up, and gets the new
 * words; [notified]: it had one, since opened or swiped away — an agent's
 * message that never notified (a step log edited into the answer) is news
 * now. [fromAgent]: a person's edit only reaches us for its mention of you.
 */
internal fun editNews(asks: Boolean, fromAgent: Boolean, showing: Boolean, notified: Boolean): EditNews = when {
    asks -> EditNews.New
    showing -> EditNews.Refresh
    fromAgent && notified -> EditNews.None
    else -> EditNews.New
}

/**
 * Says a thing once per streak: a condition that holds for every event (the
 * service refused, notifications turned off, another step log) is one line
 * in Diagnostics, not a line per event pushing everything else out of its
 * 150 entries.
 */
internal class Streak {
    private val open = HashSet<String>()

    /** True the first time [key] is reported since it last [ends]. */
    @Synchronized
    fun starts(key: String): Boolean = open.add(key)

    /** [key] no longer holds; true when a streak was open. */
    @Synchronized
    fun ends(key: String): Boolean = open.remove(key)
}
