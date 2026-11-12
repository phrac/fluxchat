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
    }
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
