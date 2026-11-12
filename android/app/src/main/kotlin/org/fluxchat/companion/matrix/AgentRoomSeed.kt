package org.fluxchat.companion.matrix

import android.content.Context
import org.fluxchat.companion.Diag
import org.json.JSONArray
import org.json.JSONObject

/**
 * The projects the last process found (room → your agents in it), kept so
 * the next start can show them before anyone's member list has been read.
 * Finding them means reading the members of every joined room; an account
 * that is also in large rooms looked at an empty Projects list for as long
 * as that took, on every start.
 *
 * Only a head start: every room is still looked at again each start (an
 * agent may have left, a room may have been left), these first. Rooms
 * *without* agents are deliberately not kept — a remembered "no agents here"
 * could hide an agent invited since, for good.
 */
object AgentRoomSeed {
    private const val PREFS = "agent-rooms"
    private const val KEY = "found"

    fun encode(user: String, rooms: Map<String, List<String>>): String = JSONObject()
        .put("user", user)
        .put("rooms", JSONObject().apply { for ((room, agents) in rooms.toSortedMap()) put(room, JSONArray(agents)) })
        .toString()

    /**
     * What [json] remembers for [user], cut down to today's [agents]: another
     * account's rooms, or a room whose agents have all been removed from the
     * list since, are not projects. Never throws — a seed that won't parse is
     * no seed.
     */
    fun decode(json: String?, user: String, agents: List<String>): Map<String, List<String>> {
        val o = json?.let { runCatching { JSONObject(it) }.getOrNull() } ?: return emptyMap()
        if (user.isEmpty() || o.optString("user") != user) return emptyMap()
        val rooms = o.optJSONObject("rooms") ?: return emptyMap()
        val out = LinkedHashMap<String, List<String>>()
        for (room in rooms.keys()) {
            val there = rooms.optJSONArray(room)?.let { a -> (0 until a.length()).map { a.optString(it) } }.orEmpty()
            // In the agent list's order, like a fresh look gives.
            val mine = agents.filter { it in there }
            if (mine.isNotEmpty()) out[room] = mine
        }
        return out
    }

    /**
     * The order to look through the rooms in: the ones [known] to be
     * projects first (they are on screen and want confirming), then the rest
     * smallest first — a project is a handful of members, and the room with
     * thousands is the one that takes the time.
     */
    fun order(rooms: List<Pair<String, Long>>, known: Set<String>): List<String> =
        rooms.sortedWith(compareBy<Pair<String, Long>> { if (it.first in known) 0 else 1 }.thenBy { it.second }).map { it.first }

    fun load(context: Context, user: String, agents: List<String>): Map<String, List<String>> =
        decode(context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY, null), user, agents)

    fun save(context: Context, user: String, rooms: Map<String, List<String>>) {
        if (user.isEmpty()) return
        runCatching { context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString(KEY, encode(user, rooms)).apply() }
            .onFailure { Diag.error("Remembering the projects for the next start", it) }
    }

    fun clear(context: Context) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().clear().apply()
    }
}
