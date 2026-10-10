package org.fluxchat.companion.agents

import org.json.JSONArray
import org.json.JSONObject

/**
 * Starting a project: who the app invites so the room is workable for
 * agents, and where the project gets filed. Pure functions over the agent
 * list and joined-space snapshots (unit-tested in ProjectsTest); the Matrix
 * side lives in Matrix.createProject.
 */
object Projects {
    /**
     * The invite list for a new project: every agent in [list], minus
     * [alreadyPresent] (members the caller already knows about), as valid
     * Matrix IDs, in list order, without duplicates.
     *
     * You don't invite yourself: [me] (the signed-in account) is dropped.
     * Malformed entries (not `@name:server`) are dropped: the server would
     * reject the whole create request for one bad ID.
     */
    fun inviteList(list: List<String>, alreadyPresent: List<String> = emptyList(), me: String = ""): List<String> {
        val valid = Regex("^@[^:\\s\\[\\]]+:\\S+$")
        val skip = alreadyPresent.toSet() + me
        return list.filter { it.isNotBlank() && valid.matches(it) && it !in skip }.distinct()
    }

    /** The space new projects are filed into, by name. */
    const val SPACE_NAME = "disflux"

    /** State event of the space hierarchy: the child event is authoritative (Element writes only this half). */
    const val SPACE_CHILD = "m.space.child"

    /**
     * Find the space new projects are filed into: [spaces] is the signed-in
     * user's joined spaces as (room ID, name). Returns the ID of the one
     * named [name] (trimmed, case-insensitive), or null when none is — a
     * missing space must never fail a creation, the project just stands on
     * its own. Two spaces sharing the name is nobody's intent: pick the
     * lowest ID so both devices of one account file into the same one.
     */
    fun findSpace(spaces: List<Pair<String, String>>, name: String = SPACE_NAME): String? {
        val matches = spaces.filter { it.second.trim().equals(name, ignoreCase = true) }.map { it.first }
        return matches.minByOrNull { it }
    }

    /**
     * The [SPACE_CHILD] content that files [roomId] into the space: the
     * room ID keys an empty object, [via] names the homeserver to reach it
     * through — the same shape Element's existing children carry (plain
     * via, no auto_join, no suggested).
     */
    fun spaceChildContent(roomId: String, via: List<String>): String {
        val content = JSONObject().put(roomId, JSONObject())
        if (via.isNotEmpty()) content.put("via", JSONArray(via))
        return content.toString()
    }
}
