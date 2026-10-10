package org.fluxchat.companion.agents

import org.fluxchat.companion.matrix.ReplyHit
import org.fluxchat.companion.matrix.Session
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SessionSearchTest {
    private fun session(root: String, latest: String?, lastTs: Long = 0) =
        Session(root = root, rootBody = root, rootTs = 0, lastTs = lastTs, latest = latest, latestSender = null, replies = 1)

    private val a = "a" to session("set up the sdxl workflow", "done, here's the config")
    private val b = "b" to session("fix the welding exhaust fan", "fan wired to the relay")
    private val c = "a" to session("REVIEW the trading bot", null)

    private val all = listOf(a, b, c)

    @Test fun `every word must match`() {
        assertEquals(listOf(a), SessionSearch.matching("sdxl config", all))
    }

    @Test fun `matches the latest reply too`() {
        assertEquals(listOf(b), SessionSearch.matching("relay", all))
    }

    @Test fun `case insensitive`() {
        assertEquals(listOf(c), SessionSearch.matching("review TRADING", all))
    }

    @Test fun `empty query matches nothing`() {
        assertTrue(SessionSearch.matching("   ", all).isEmpty())
    }

    @Test fun `no match is empty`() {
        assertTrue(SessionSearch.matching("parcels", all).isEmpty())
    }

    // --- full-text merge ---

    private fun hit(room: String, event: String, root: String, body: String, ts: Long = 5) =
        ReplyHit(roomId = room, eventId = event, root = root, body = body, ts = ts, sender = "@agent:srv")

    private fun merge(query: String, hits: List<ReplyHit>) =
        SessionSearch.merge(query, all, hits) { "Room $it" }

    @Test fun `a full-text hit opens its session by thread root`() {
        val h = hit("a", "\$ev1", "set up the sdxl workflow", "the comfy node graph needs this")
        val out = merge("comfy", listOf(h))
        assertEquals(1, out.size)
        val m = out[0] as SearchResult.Reply
        assertEquals("a", m.roomId)
        assertEquals("set up the sdxl workflow", m.root)
        assertEquals("\$ev1", m.eventId)
    }

    @Test fun `a session found locally is not repeated by its full-text hit`() {
        val h = hit("a", "\$ev1", "set up the sdxl workflow", "sdxl checkpoint here")
        val out = merge("sdxl", listOf(h))
        assertEquals(1, out.size)
        assertTrue(out[0] is SearchResult.SessionHit)
    }

    @Test fun `local matches come before full-text hits`() {
        val h = hit("b", "\$ev9", "some other root", "the fan relay was replaced twice")
        val out = merge("relay", listOf(h))
        assertEquals(2, out.size)
        assertTrue(out[0] is SearchResult.SessionHit) // b matched on its latest reply
        assertTrue(out[1] is SearchResult.Reply)
    }

    @Test fun `two hits in one session collapse to one result`() {
        val h1 = hit("a", "\$e1", "deep root", "alpha word here")
        val h2 = hit("a", "\$e2", "deep root", "another alpha mention")
        val out = merge("alpha", listOf(h1, h2))
        assertEquals(1, out.size)
        assertEquals("\$e1", (out[0] as SearchResult.Reply).eventId) // first (most relevant) hit wins
    }

    @Test fun `top-level hit roots at itself`() {
        val h = hit("b", "\$top", "\$top", "wording in a room message")
        val out = merge("wording", listOf(h))
        assertEquals("\$top", (out[0] as SearchResult.Reply).root)
    }

    @Test fun `snippet centres on the matching word`() {
        val body = "before ".repeat(30) + "zygote " + "after ".repeat(30)
        val s = SessionSearch.snippet(body, listOf("zygote"))
        assertTrue(s.contains("zygote"))
        assertTrue(s.length < body.length)
        assertTrue(s.startsWith("…") && s.endsWith("…"))
    }

    @Test fun `snippet of a body without the word is its head`() {
        assertEquals("hello there", SessionSearch.snippet("hello there", listOf("nope")))
    }
}
