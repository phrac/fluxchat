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

    @Test fun `the agent answering meanwhile is not our message`() {
        assertNull(QuickSend.echoState(before, history + msg("\$answer", own = false)))
    }

    @Test fun `the body that comes back need not be the text that went in`() {
        // Markdown, a mention turned into a pill: matched by being new and ours, not by its words.
        assertEquals(SendState.Sent, QuickSend.echoState(before, history + msg("\$new", body = "@hermes run the tests")))
    }
}
