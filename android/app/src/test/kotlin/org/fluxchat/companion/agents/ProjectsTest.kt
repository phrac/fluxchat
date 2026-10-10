package org.fluxchat.companion.agents

import org.junit.Assert.assertEquals
import org.junit.Test

/** The invite list a new project is created with (Projects.inviteList). */
class ProjectsTest {
    private val me = "@phrac:dsflx.wtf"
    private val hermes = "@hermes:dsflx.wtf"
    private val ricky = "@ricky:dsflx.wtf"

    @Test fun everyAgentIsInvited() {
        assertEquals(listOf(hermes, ricky), Projects.inviteList(listOf(hermes, ricky), me = me))
    }

    @Test fun youAreNeverInvitedToYourOwnProject() {
        assertEquals(listOf(hermes), Projects.inviteList(listOf(me, hermes), me = me))
    }

    @Test fun malformedIdsAreDropped() {
        val list = listOf(hermes, "not-an-id", "@no-server", "hermes:dsflx.wtf", "", "@ spaced:bad")
        assertEquals(listOf(hermes), Projects.inviteList(list, me = me))
    }

    @Test fun duplicatesCollapseAndOrderIsKept() {
        assertEquals(listOf(ricky, hermes), Projects.inviteList(listOf(ricky, hermes, ricky), me = me))
    }

    @Test fun knownMembersAreNotReInvited() {
        assertEquals(listOf(ricky), Projects.inviteList(listOf(hermes, ricky), alreadyPresent = listOf(hermes), me = me))
    }

    @Test fun emptyAgentListInvitesNobody() {
        assertEquals(emptyList<String>(), Projects.inviteList(emptyList(), me = me))
    }
}
