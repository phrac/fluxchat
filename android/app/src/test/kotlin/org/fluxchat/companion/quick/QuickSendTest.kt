package org.fluxchat.companion.quick

import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.SendState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * What became of the quick prompt's message, read off the project's
 * timeline. The sheet used to toast "Sent" as soon as the message was in the
 * send queue — with no signal that is a message that hasn't gone anywhere.
 */
class QuickSendTest {
    private fun msg(id: String, own: Boolean = true, state: SendState = SendState.Sent, body: String = "run the tests") =
        Msg(id, if (own) "@me:x" else "@hermes:x", "name", body, 1_000, isOwn = own, sendState = state)

    private val before = setOf("\$old", "\$older")
    private val history = listOf(msg("\$older"), msg("\$old"))

    @Test fun `nothing new in the timeline yet is not sent`() {
        assertNull(QuickSend.echoState(before, history))
    }

    @Test fun `the local echo on its way is still sending`() {
        assertNull(QuickSend.echoState(before, history + msg("~txn1", state = SendState.Sending)))
    }

    @Test fun `sent once the server has taken it`() {
        // The local echo marked sent, or the event that replaces it (a new ID again).
        assertEquals(SendState.Sent, QuickSend.echoState(before, history + msg("~txn1", state = SendState.Sent)))
        assertEquals(SendState.Sent, QuickSend.echoState(before, history + msg("\$new")))
    }

    @Test fun `a failed echo is a failed send`() {
        assertEquals(SendState.Failed, QuickSend.echoState(before, history + msg("~txn1", state = SendState.Failed)))
    }

    @Test fun `an earlier message of ours with the same words does not count`() {
        // You asked the same thing yesterday: that one being sent says nothing about this one.
        assertNull(QuickSend.echoState(before, history))
        assertNull(QuickSend.echoState(setOf("\$old", "\$older", "\$same"), history + msg("\$same")))
    }

    @Test fun `history that loads while the message is on its way is not its echo`() {
        // The sheet opened on a cold start, the project's history still
        // arriving: last week's request of yours turned up, sent of course,
        // and the sheet said "Sent" and closed with the new one in the queue.
        val since = 1_700_000_000_000L
        val lastWeek = msg("\$lastWeek").copy(ts = since - 7 * 24 * 3_600_000L)
        assertNull(QuickSend.echoState(before, history + lastWeek, since))
        val echo = msg("~txn1", state = SendState.Sending).copy(ts = since + 20)
        assertNull(QuickSend.echoState(before, history + lastWeek + echo, since))
        assertEquals(SendState.Sent, QuickSend.echoState(before, history + lastWeek + echo.copy(sendState = SendState.Sent), since))
        // Nor an old send that failed and is back in the list with the rest.
        val stuck = msg("~old", state = SendState.Failed).copy(ts = since - 3_600_000L)
        assertNull(QuickSend.echoState(before, history + stuck, since))
    }

    @Test fun `the server's clock need not agree with the phone's`() {
        // The event that replaces the echo carries the server's time: a little behind is still ours.
        val since = 1_700_000_000_000L
        assertEquals(SendState.Sent, QuickSend.echoState(before, history + msg("\$new").copy(ts = since - 20_000), since))
    }

    @Test fun `the agent answering meanwhile is not our message`() {
        assertNull(QuickSend.echoState(before, history + msg("\$answer", own = false)))
    }

    @Test fun `the body that comes back need not be the text that went in`() {
        // Markdown, a mention turned into a pill: matched by being new and ours, not by its words.
        assertEquals(SendState.Sent, QuickSend.echoState(before, history + msg("\$new", body = "@hermes run the tests")))
    }
}
