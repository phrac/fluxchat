package org.fluxchat.companion.matrix

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AgentRoomSeedTest {
    private val me = "@derek:x"
    private val hermes = "@hermes:x"
    private val ricky = "@ricky:x"
    private val ops = "!ops:x"
    private val build = "!build:x"

    @Test fun `what one start found is there for the next`() {
        val found = mapOf(ops to listOf(hermes, ricky), build to listOf(ricky))
        assertEquals(found, AgentRoomSeed.decode(AgentRoomSeed.encode(me, found), me, listOf(hermes, ricky)))
    }

    @Test fun `another account's projects are not yours`() {
        val json = AgentRoomSeed.encode(me, mapOf(ops to listOf(hermes)))
        assertTrue(AgentRoomSeed.decode(json, "@someone:else", listOf(hermes)).isEmpty())
        assertTrue("not signed in yet", AgentRoomSeed.decode(json, "", listOf(hermes)).isEmpty())
    }

    @Test fun `an agent taken off the list no longer makes a room a project`() {
        val json = AgentRoomSeed.encode(me, mapOf(ops to listOf(hermes, ricky), build to listOf(ricky)))
        assertEquals(mapOf(ops to listOf(hermes)), AgentRoomSeed.decode(json, me, listOf(hermes)))
        assertTrue("no agent list yet, no projects", AgentRoomSeed.decode(json, me, emptyList()).isEmpty())
    }

    @Test fun `agents come back in the list's order`() {
        val json = AgentRoomSeed.encode(me, mapOf(ops to listOf(ricky, hermes)))
        assertEquals(listOf(hermes, ricky), AgentRoomSeed.decode(json, me, listOf(hermes, ricky))[ops])
    }

    @Test fun `a seed that won't parse is no seed, never a crash`() {
        assertTrue(AgentRoomSeed.decode(null, me, listOf(hermes)).isEmpty())
        assertTrue(AgentRoomSeed.decode("not json", me, listOf(hermes)).isEmpty())
        assertTrue(AgentRoomSeed.decode("{\"user\":\"$me\"}", me, listOf(hermes)).isEmpty())
        assertTrue(AgentRoomSeed.decode("{\"user\":\"$me\",\"rooms\":{\"$ops\":7}}", me, listOf(hermes)).isEmpty())
    }

    @Test fun `known projects are looked at first, then the small rooms`() {
        val rooms = listOf("!huge:x" to 4000L, "!dm:x" to 2L, ops to 900L, "!team:x" to 12L)
        assertEquals(
            listOf(ops, "!dm:x", "!team:x", "!huge:x"),
            AgentRoomSeed.order(rooms, known = setOf(ops)),
        )
    }

    @Test fun `with nothing known the big room still comes last`() {
        assertEquals(listOf("!b:x", "!a:x"), AgentRoomSeed.order(listOf("!a:x" to 5000L, "!b:x" to 3L), emptySet()))
    }
}
