package org.fluxchat.companion.agents

import org.fluxchat.companion.matrix.Session
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

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
}
