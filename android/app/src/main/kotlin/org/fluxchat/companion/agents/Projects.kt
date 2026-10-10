package org.fluxchat.companion.agents

/**
 * Starting a project: who the app invites so the room is workable for
 * agents. Pure functions over the agent list (unit-tested in ProjectsTest);
 * the Matrix side lives in Matrix.createProject.
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
}
