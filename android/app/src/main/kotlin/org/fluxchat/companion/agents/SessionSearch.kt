package org.fluxchat.companion.agents

import org.fluxchat.companion.matrix.Session

/**
 * Finding a session by what it was about. Pure word-AND matching over what
 * the server's thread list already carries (the opening prompt and the latest
 * reply), so it works offline against whatever pages have loaded. Mirrors
 * [History.matching]'s rules: case-insensitive, every word must hit.
 */
object SessionSearch {
    fun matching(query: String, sessions: List<Pair<String, Session>>): List<Pair<String, Session>> {
        val words = query.trim().lowercase().split(' ').filter { it.isNotBlank() }
        if (words.isEmpty()) return emptyList()
        return sessions.filter { (_, s) ->
            val hay = (s.rootBody + " " + (s.latest ?: "")).lowercase()
            words.all(hay::contains)
        }
    }
}
