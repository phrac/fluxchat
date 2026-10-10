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

    private val disflux = "!eDa74UsTQUaq6vWTcf:dsflx.wtf"
    private val other = "!zzzzUsTQUaq6vWTcf:dsflx.wtf"

    @Test fun spaceIsFoundByNameIgnoringCase() {
        assertEquals(disflux, Projects.findSpace(listOf(disflux to "Disflux", other to "Work Corner")))
        assertEquals(disflux, Projects.findSpace(listOf(disflux to "  disflux  ")))
        assertEquals(disflux, Projects.findSpace(listOf(disflux to "DISFLUX")))
    }

    @Test fun noSpaceNamedDisfluxMeansNoSpace() {
        assertEquals(null, Projects.findSpace(listOf(other to "Work Corner")))
        assertEquals(null, Projects.findSpace(emptyList()))
        // A project room called disflux isn't a space — callers only pass
        // spaces in; a substring like "disflux archive" is not a match.
        assertEquals(null, Projects.findSpace(listOf(other to "disflux archive")))
    }

    @Test fun duplicateSpaceNamesCollapseDeterministically() {
        // Two spaces sharing the name: the lowest ID wins, so both devices
        // of one account file projects into the same space.
        assertEquals(disflux, Projects.findSpace(listOf(other to "Disflux", disflux to "Disflux")))
    }

    @Test fun childContentMatchesWhatElementWrites() {
        // The shape Element's existing children carry on the homeserver:
        // content is just via (the room ID is the event's state key), plain
        // via, no auto_join, no suggested.
        val content = org.json.JSONObject(Projects.spaceChildContent(listOf("dsflx.wtf")))
        assertEquals(1, content.length())
        assertEquals("dsflx.wtf", content.getJSONArray("via").getString(0))
    }

    @Test fun childContentWithoutViaIsEmpty() {
        assertEquals("{}", Projects.spaceChildContent(emptyList()))
    }
}
