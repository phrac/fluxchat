package org.fluxchat.companion.ui

import org.fluxchat.companion.agents.SearchResult
import org.fluxchat.companion.matrix.AgentRoom
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Where a search result opens, what "New session" does, and which project the quick prompt sends to. */
class WhereThingsGoTest {
    private val room = "!r:x"
    private val dm = "!dm:x"

    private fun reply(roomId: String, root: String, event: String) = SearchResult.Reply(roomId, root, event, "…deploy…", 1_000, "Hermes")

    @Test
    fun aSearchHitOpensItsSessionAtTheReply() {
        assertEquals(Screen.Thread(room, "\$root", "\$hit"), searchTarget(reply(room, "\$root", "\$hit"), sessions = true))
        // A reply in a thread is in that thread wherever it is, one to one included.
        assertEquals(Screen.Thread(dm, "\$root", "\$hit"), searchTarget(reply(dm, "\$root", "\$hit"), sessions = false))
        // The request itself, in a project of sessions: its session.
        assertEquals(Screen.Thread(room, "\$req", "\$req"), searchTarget(reply(room, "\$req", "\$req"), sessions = true))
        // A session matched by its title opens as it is.
        assertEquals(Screen.Thread(room, "\$root"), searchTarget(SearchResult.SessionHit(room, "\$root", "Deploy it", 1_000), sessions = true))
    }

    @Test
    fun aHitInAConversationWithNoSessionsOpensTheConversationThere() {
        // One to one, the agent answers in the conversation itself: the hit
        // is a message in it, not a session of its own — opened as one, it
        // came up alone on an empty screen.
        assertEquals(Screen.Room(dm, "\$hit"), searchTarget(reply(dm, "\$hit", "\$hit"), sessions = false))
    }

    @Test
    fun newSessionAsksTheAgentWhereItAnswersInTheConversationItself() {
        // One to one: the agent's own command, as before.
        assertEquals(NewSession.Command, newSessionAction(isDm = true, inSession = false, threaded = false, hasNew = true))
        // A project whose agent doesn't thread its answers is the same kind of
        // conversation: the button closed the list there and did nothing.
        assertEquals(NewSession.Command, newSessionAction(isDm = false, inSession = false, threaded = false, hasNew = true))
        // Each request in its own thread: a session is started from the project.
        assertEquals(NewSession.Project, newSessionAction(isDm = false, inSession = true, threaded = false, hasNew = true))
        assertEquals(NewSession.Here, newSessionAction(isDm = false, inSession = false, threaded = true, hasNew = true))
        // No such command to send: nothing is.
        assertEquals(NewSession.Here, newSessionAction(isDm = false, inSession = false, threaded = false, hasNew = false))
        assertEquals(NewSession.Project, newSessionAction(isDm = true, inSession = true, threaded = false, hasNew = false))
    }

    private fun project(id: String) = AgentRoom(id, id, false, listOf("@hermes:x"))

    @Test
    fun theQuickPromptWaitsForTheProjectItWasOpenedFor() {
        val a = project("!a:x")
        val b = project("!b:x")
        // At a start the projects turn up one by one: B is there, A not yet.
        val soFar = mapOf(b.roomId to b)
        // Not B meanwhile — Send would go there, and B's draft into the box.
        assertNull(quickRoom("!a:x", soFar, listOf(b), loading = true))
        assertEquals(a, quickRoom("!a:x", soFar + (a.roomId to a), listOf(b, a), loading = true))
        // Everything has been looked through and A isn't one of them (you left it): the first.
        assertEquals(b, quickRoom("!a:x", soFar, listOf(b), loading = false))
        // Opened for no project in particular: the first, as soon as there is one.
        assertEquals(b, quickRoom(null, soFar, listOf(b), loading = true))
        assertNull(quickRoom(null, emptyMap(), emptyList(), loading = true))
    }
}
