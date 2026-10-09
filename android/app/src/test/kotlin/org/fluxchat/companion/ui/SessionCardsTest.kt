package org.fluxchat.companion.ui

import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.ViewKey
import org.fluxchat.companion.matrix.Session
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SessionCardsTest {
    private val me = "@me:x"
    private val agent = "@hermes:x"
    private val room = "!r:x"

    private fun msg(id: String, sender: String, ts: Long, body: String, threadRoot: String? = null, threadCount: Int = 0) =
        Msg(id, sender, if (sender == agent) "Hermes" else "Me", body, ts, isOwn = sender == me, threadRoot = threadRoot, threadCount = threadCount)

    @Test
    fun aRequestWithItsThreadIsOneCardSummedUpByTheAgentsLatestAnswer() {
        val all = listOf(
            msg("r1", me, 1_000, "Clean up the build dir", threadCount = 3),
            msg("s", agent, 2_000, "📖 Reading Makefile\n💻 Running make clean", threadRoot = "r1"),
            msg("a", agent, 3_000, "## Done\nRemoved **build/** and rebuilt.", threadRoot = "r1"),
            msg("r2", me, 4_000, "And deploy it"),
        )
        val runs = mapOf(ViewKey(room, "r1") to RunInfo(RunState.Done, 1_000, agent = agent))
        val cards = sessionCards(room, emptyList(), all, runs, listOf(agent))
        assertEquals(listOf("r2", "r1"), cards.map { it.root })
        val c = cards[1]
        assertEquals("Clean up the build dir", c.title)
        assertEquals("Done\nRemoved build/ and rebuilt.", c.summary)
        assertEquals("Hermes", c.summaryBy)
        assertEquals(3, c.replies)
        assertEquals(RunState.Done, c.run?.state)
        // Just sent: no thread yet, nothing to sum up.
        assertNull(cards[0].summary)
        assertEquals(true, cards[0].own)
    }

    @Test
    fun serverSessionsOutsideTheLoadedHistoryStillGetCards() {
        val old = Session("old", "Summarise the papers", 100, 500, "Here are the three papers.", "Hermes", 4)
        val cards = sessionCards(room, listOf(old), emptyList(), emptyMap(), listOf(agent))
        assertEquals(1, cards.size)
        assertEquals("Summarise the papers", cards[0].title)
        assertEquals("Here are the three papers.", cards[0].summary)
        assertEquals(4, cards[0].replies)
        assertEquals(500L, cards[0].lastTs)
    }

    @Test
    fun plainSummaryDropsMarkdownAndCode() {
        assertEquals("Title\n• one\nquote", plainSummary("# Title\n\n```sh\nrm -rf /\n```\n- one\n> quote"))
    }
}
