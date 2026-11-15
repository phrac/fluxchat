package org.fluxchat.companion.matrix

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Saving the agent list before the account's has been read: the edit was
 * built on the empty list the screen showed, and uploaded as it was it
 * replaced the account's list (the desktop's too) with the one agent added.
 */
class AgentListTest {
    private val hermes = AgentEntry("@hermes:x", "hermes")
    private val claude = AgentEntry("@claude:x", "claude-code")
    private val theirs = AgentList(listOf(hermes, claude), notAgents = listOf("@bridge:x"))

    @Test fun `an agent added before the account's list was read goes on top of it, not in its place`() {
        val edit = AgentList(listOf(AgentEntry("@new:x")))
        assertEquals(AgentList(listOf(hermes, claude, AgentEntry("@new:x")), listOf("@bridge:x")), edit.addedTo(theirs))
    }

    @Test fun `adding one the account already has keeps the account's entry and its harness`() {
        assertEquals(theirs, AgentList(listOf(AgentEntry("@claude:x"))).addedTo(theirs))
    }

    @Test fun `an agent you add is no longer listed as not an agent`() {
        val merged = AgentList(listOf(AgentEntry("@bridge:x"))).addedTo(theirs)
        assertEquals(listOf(hermes, claude, AgentEntry("@bridge:x")), merged.agents)
        assertEquals(emptyList<String>(), merged.notAgents)
    }

    @Test fun `with nothing on the account the edit is the list`() {
        val edit = AgentList(listOf(AgentEntry("@new:x")))
        assertEquals(edit, edit.addedTo(AgentList()))
    }

    @Test fun `what is saved reads back as it was`() {
        assertEquals(theirs, AgentList.parse(theirs.toJson()))
    }
}
