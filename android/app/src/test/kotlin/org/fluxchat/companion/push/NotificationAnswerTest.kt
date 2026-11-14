package org.fluxchat.companion.push

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * A tap on a notification, as the answer it sends: a button, a chip picked
 * in the reply box (which must answer as that choice's button would), or
 * words of your own — and each of them once.
 */
class NotificationAnswerTest {
    private val approval = listOf("✅" to "Approve once", "🌀" to "For this session", "♾️" to "Always", "❌" to "Deny")
    private val titles = approval.map { (k, l) -> Notifier.choiceTitle(k, l) }.toTypedArray()
    private val keys = approval.map { it.first }.toTypedArray()

    /** The reply action of an approval: Deny with a reason, unless a chip is picked. */
    private fun reply(text: String?, titles: Array<String> = this.titles, keys: Array<String> = this.keys) = ActionReceiver.ask(
        ActionReceiver.DENY, "❌", "Deny", text, ActionReceiver.pickedChoice(text, titles), keys, ActionReceiver.REACT,
    )

    @Test fun `a button answers with its own choice`() {
        val ask = ActionReceiver.ask(ActionReceiver.REACT, "✅", "Approve once", null, null, null, null)
        assertEquals(ActionReceiver.Ask(ActionReceiver.REACT, "✅", "Answered: ✅ Approve once"), ask)
    }

    @Test fun `a picked chip answers as its button would, and says which`() {
        // It said "Answered: 🌀 Deny": the reply action's own label, whatever was picked.
        val ask = reply("🌀 For this session")
        assertEquals(ActionReceiver.REACT, ask?.action)
        assertEquals("🌀", ask?.value)
        assertEquals("Answered: 🌀 For this session", ask?.done)
    }

    @Test fun `words of your own on an approval deny it with that reason`() {
        val ask = reply("not on the production box")
        assertEquals(ActionReceiver.DENY, ask?.action)
        assertEquals("❌", ask?.value)
        assertEquals("Denied: not on the production box", ask?.done)
    }

    @Test fun `a chip whose title was cut at a space is still its choice`() {
        // 40 characters end right after "branch": the title carried the space,
        // the reply box's text didn't match it, and picking "approve" went
        // out as a Deny with the choice's name for a reason.
        val choices = listOf("✅" to "Approve and merge the releases branch into main", "🌀" to "For this session", "❌" to "Deny")
        val raw = choices.map { (k, l) -> "$k $l".take(40) }.toTypedArray()
        assertTrue(raw[0].endsWith(" "))
        val ask = reply(raw[0], titles = raw, keys = choices.map { it.first }.toTypedArray())
        assertEquals(ActionReceiver.REACT, ask?.action)
        assertEquals("✅", ask?.value)
        // And a title is made without that space to begin with.
        assertEquals(raw[0].trim(), Notifier.choiceTitle(choices[0].first, choices[0].second))
        assertEquals("1 Deploy", Notifier.choiceTitle("1", "Deploy "))
    }

    @Test fun `a question's chip sends its number`() {
        val q = listOf("1" to "Deploy", "2" to "Staging only", "3" to "Neither")
        val t = q.map { (k, l) -> Notifier.choiceTitle(k, l) }.toTypedArray()
        val ask = ActionReceiver.ask(
            ActionReceiver.REPLY, "", "Deny", "2 Staging only", ActionReceiver.pickedChoice("2 Staging only", t),
            q.map { it.first }.toTypedArray(), ActionReceiver.SEND,
        )
        assertEquals(ActionReceiver.Ask(ActionReceiver.SEND, "2", "Sent: 2"), ask)
        // In your own words it is a reply.
        val own = ActionReceiver.ask(ActionReceiver.REPLY, "", "Deny", "ask me tomorrow", ActionReceiver.pickedChoice("ask me tomorrow", t), null, ActionReceiver.SEND)
        assertEquals(ActionReceiver.Ask(ActionReceiver.REPLY, "", "Sent: ask me tomorrow"), own)
    }

    @Test fun `an intent that does not say what to do is no answer`() {
        assertNull(ActionReceiver.ask(null, "", "", null, null, null, null))
        // A chip with no key behind it.
        assertNull(ActionReceiver.ask(ActionReceiver.DENY, "❌", "Deny", titles[1], 1, null, ActionReceiver.REACT))
        assertNull(ActionReceiver.pickedChoice("   ", titles))
        assertNull(ActionReceiver.pickedChoice(null, titles))
        assertNull(ActionReceiver.pickedChoice("✅ Approve once", null))
    }

    @Test fun `a second tap on the same button is not a second answer`() {
        // The buttons stay up until the answer is through: "1", then "1" again
        // as your next message; a reaction twice, refused by the server.
        val taps = Taps(recentMs = 10_000)
        assertTrue(taps.begin("react|✅", now = 0))
        assertFalse(taps.begin("react|✅", now = 300))
        // Another choice is another answer.
        assertTrue(taps.begin("react|❌", now = 400))
        // Sent: the tap that was already on its way to the receiver changes nothing.
        taps.end("react|✅", ok = true, now = 2_000)
        assertFalse(taps.begin("react|✅", now = 2_100))
    }

    @Test fun `an answer that failed can be given again at once, a sent one a while later`() {
        val taps = Taps(recentMs = 10_000)
        assertTrue(taps.begin("send|!stop", now = 0))
        taps.end("send|!stop", ok = false, now = 1_000)
        assertTrue(taps.begin("send|!stop", now = 1_100))
        taps.end("send|!stop", ok = true, now = 2_000)
        assertFalse(taps.begin("send|!stop", now = 5_000))
        // Stop, for the run that started since.
        assertTrue(taps.begin("send|!stop", now = 12_001))
    }
}
