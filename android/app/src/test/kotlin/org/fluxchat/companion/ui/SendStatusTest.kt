package org.fluxchat.companion.ui

import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.SendAction
import org.fluxchat.companion.agents.SendState
import org.fluxchat.companion.agents.SLOW_SEND_MS
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** What a message that hasn't gone out says, and what its row lets you do about it. */
class SendStatusTest {
    private val me = "@me:x"
    private val room = "!r:x"

    private fun echo(id: String, ts: Long, state: SendState, retrying: Boolean = false, error: String? = null, threadRoot: String? = null) =
        Msg(id, me, "Me", "Deploy it", ts, isOwn = true, sendState = state, sendRetrying = retrying, sendError = error, threadRoot = threadRoot)

    @Test fun `a failed message offers retry and delete`() {
        assertEquals(listOf(SendAction.Retry, SendAction.Delete), echo("~t1", 1_000, SendState.Failed).sendActions(1_000))
        // Also while the queue is still trying by itself: retry now, or give up on it.
        assertEquals(listOf(SendAction.Retry, SendAction.Delete), echo("~t1", 1_000, SendState.Failed, retrying = true).sendActions(60_000))
    }

    @Test fun `a message still sending offers cancel only once it is taking long`() {
        val sending = echo("~t1", 1_000, SendState.Sending)
        assertEquals(emptyList<SendAction>(), sending.sendActions(1_200))
        assertEquals(emptyList<SendAction>(), sending.sendActions(1_000 + SLOW_SEND_MS - 1))
        assertEquals(listOf(SendAction.Cancel), sending.sendActions(1_000 + SLOW_SEND_MS))
    }

    @Test fun `a message the server has offers nothing`() {
        assertEquals(emptyList<SendAction>(), echo("\$ev", 1_000, SendState.Sent).sendActions(999_999))
        // Sent, its own copy not back yet.
        assertEquals(emptyList<SendAction>(), echo("~t1", 1_000, SendState.Sent).sendActions(999_999))
        // Not ours to abort any more, whatever the state says.
        assertEquals(emptyList<SendAction>(), echo("\$ev", 1_000, SendState.Failed).sendActions(999_999))
    }

    @Test fun `the row says why it has not gone out`() {
        assertEquals("Still sending…", sendStatusText(echo("~t", 0, SendState.Sending), heldUp = false))
        assertEquals("Waiting for an earlier message that wasn't sent", sendStatusText(echo("~t", 0, SendState.Sending), heldUp = true))
        assertEquals("Not sent yet. Trying again by itself", sendStatusText(echo("~t", 0, SendState.Failed, retrying = true), heldUp = false))
        assertEquals("Not sent", sendStatusText(echo("~t", 0, SendState.Failed), heldUp = false))
        assertEquals("Not sent: your sign-in has expired", sendStatusText(echo("~t", 0, SendState.Failed, error = "your sign-in has expired"), heldUp = false))
    }

    @Test fun `a send that failed for good holds up what was sent after it, wherever in the project`() {
        val parked = echo("~t1", 1_000, SendState.Failed, threadRoot = "\$a")
        val later = echo("~t2", 2_000, SendState.Sending, threadRoot = "\$b")
        val earlier = echo("~t0", 500, SendState.Sending)
        val retrying = echo("~t3", 800, SendState.Failed, retrying = true)
        val all = listOf(earlier, retrying, parked, later)
        // One the queue retries by itself blocks nothing.
        assertEquals(parked, sendBlocker(all))
        assertTrue(heldUpBy(parked, later))
        assertFalse(heldUpBy(parked, earlier))
        assertFalse("it isn't waiting for itself", heldUpBy(parked, parked))
        assertFalse(heldUpBy(parked, echo("\$sent", 3_000, SendState.Sent)))
        assertNull(sendBlocker(listOf(earlier, retrying, later)))
        assertFalse(heldUpBy(null, later))
    }

    @Test fun `a request that has not gone out says so on its session card`() {
        val failed = echo("~t1", 1_000, SendState.Failed)
        val sent = Msg("\$r", me, "Me", "Clean up", 500, isOwn = true)
        val cards = sessionCards(room, emptyList(), listOf(sent, failed), emptyMap(), emptyList())
        assertEquals(listOf("~t1", "\$r"), cards.map { it.root })
        assertEquals(failed, cards[0].unsent)
        assertEquals(listOf(SendAction.Retry, SendAction.Delete), cards[0].unsent!!.sendActions(2_000))
        assertNull(cards[1].unsent)
    }
}
