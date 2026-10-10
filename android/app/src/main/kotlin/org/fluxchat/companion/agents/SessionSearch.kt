package org.fluxchat.companion.agents

import org.fluxchat.companion.matrix.ReplyHit
import org.fluxchat.companion.matrix.Session

/**
 * Finding a session by what it was about. Two sources, merged: word-AND
 * matching over what the thread list already carries (the opening prompt and
 * the latest reply), which works offline against loaded pages, and the
 * server's full-text hits, which reach inside every reply of a session.
 * A session counts as one result, whichever way it was found.
 */
object SessionSearch {
    fun matching(query: String, sessions: List<Pair<String, Session>>): List<Pair<String, Session>> {
        val words = words(query)
        if (words.isEmpty()) return emptyList()
        return sessions.filter { (_, s) ->
            val hay = (s.rootBody + " " + (s.latest ?: "")).lowercase()
            words.all(hay::contains)
        }
    }

    /**
     * Local matches first (newest activity first — they're the sessions you
     * can act on offline), then the full-text hits whose session wasn't
     * already found, in the index's relevance order.
     */
    fun merge(
        query: String,
        sessions: List<Pair<String, Session>>,
        hits: List<ReplyHit>,
        roomName: (String) -> String,
    ): List<SearchResult> {
        val words = words(query)
        if (words.isEmpty()) return emptyList()
        val local = matching(query, sessions).sortedByDescending { (_, s) -> s.lastTs }
        val seen = local.mapTo(HashSet()) { it.first to it.second.root }
        val out = ArrayList<SearchResult>(local.size + hits.size)
        out += local.map { (roomId, s) ->
            SearchResult.SessionHit(roomId, s.root, title = s.rootBody.ifBlank { roomName(roomId) }, lastTs = s.lastTs)
        }
        for (h in hits) {
            if ((h.roomId to h.root) in seen) continue
            seen += h.roomId to h.root
            out += SearchResult.Reply(
                roomId = h.roomId,
                root = h.root,
                eventId = h.eventId,
                snippet = snippet(h.body, words),
                lastTs = h.ts,
                senderName = h.sender,
            )
        }
        return out
    }

    /** A short line of the hit around the first word that matched. */
    fun snippet(body: String, words: List<String>, radius: Int = 70): String {
        val lower = body.lowercase()
        val at = words.firstNotNullOfOrNull { w -> lower.indexOf(w).takeIf { it >= 0 } }
            ?: return body.replace('\n', ' ').take(radius) + if (body.length > radius) "…" else ""
        val from = (at - radius).coerceAtLeast(0)
        val to = (at + radius).coerceAtMost(body.length)
        return (if (from > 0) "…" else "") +
            body.substring(from, to).replace('\n', ' ') +
            (if (to < body.length) "…" else "")
    }

    private fun words(query: String): List<String> =
        query.trim().lowercase().split(' ').filter { it.isNotBlank() }
}

/** One search result: a session (matched on its title or latest reply), or
 * a session found through a hit inside one of its replies. */
sealed interface SearchResult {
    val roomId: String
    val root: String
    val lastTs: Long

    data class SessionHit(
        override val roomId: String,
        override val root: String,
        val title: String,
        override val lastTs: Long,
    ) : SearchResult

    data class Reply(
        override val roomId: String,
        override val root: String,
        /** The matching reply itself (what to scroll to when opening). */
        val eventId: String,
        val snippet: String,
        override val lastTs: Long,
        val senderName: String?,
    ) : SearchResult
}
