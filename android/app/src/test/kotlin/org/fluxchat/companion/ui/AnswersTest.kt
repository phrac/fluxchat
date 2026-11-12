package org.fluxchat.companion.ui

import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.Reaction
import org.fluxchat.companion.agents.SendState
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** An answer to an agent's prompt is given once: the buttons wait until it's through, or has failed. */
class AnswersTest {
    private val me = "@me:x"
    private val agent = "@hermes:x"
    private val choices = listOf("✅" to "Approve once", "❌" to "Deny")

    private fun prompt(vararg reactions: Reaction) =
        Msg("\$p", agent, "Hermes", "Allow `rm -rf build`?", 1_000, isOwn = false, reactions = reactions.toList())

    @Before @After fun reset() = Answers.clear()

    @Test fun `a second tap on a prompt is ignored while the first answer is on its way`() {
        assertTrue(Answers.begin("\$p", "✅", 1_000))
        assertFalse(Answers.begin("\$p", "✅", 1_100))
        assertFalse("nor may it be changed mid-flight", Answers.begin("\$p", "❌", 1_200))
        assertEquals(PromptAnswer("✅", AnswerPhase.Sending), promptAnswer(prompt(), choices, me, Answers.all.value["\$p"]))
        // Another prompt is its own matter.
        assertTrue(Answers.begin("\$q", "❌", 1_300))
    }

    @Test fun `once the server has the answer the buttons stay off until the agent resolves the prompt`() {
        Answers.begin("\$p", "✅", 1_000)
        Answers.delivered("\$p", 1_500)
        assertFalse(Answers.begin("\$p", "✅", 999_999))
        assertEquals(PromptAnswer("✅", AnswerPhase.Waiting), promptAnswer(prompt(), choices, me, Answers.all.value["\$p"]))
    }

    @Test fun `a failed answer gives the buttons back`() {
        Answers.begin("\$p", "✅", 1_000)
        Answers.failed("\$p")
        assertNull(promptAnswer(prompt(), choices, me, Answers.all.value["\$p"]))
        assertTrue(Answers.begin("\$p", "❌", 2_000))
    }

    @Test fun `an answer given elsewhere counts, so the buttons never offer to take it back`() {
        // From a notification, the desktop, or before the app was restarted: nothing noted here.
        val answered = prompt(Reaction("✅", listOf(agent, me)), Reaction("❌", listOf(agent)))
        assertEquals(PromptAnswer("✅", AnswerPhase.Waiting), promptAnswer(answered, choices, me, null))
        // The agent's own seeded reactions are the choices, not an answer.
        assertNull(promptAnswer(prompt(Reaction("✅", listOf(agent)), Reaction("❌", listOf(agent))), choices, me, null))
        // A reaction of yours that isn't one of the choices isn't one either.
        assertNull(promptAnswer(prompt(Reaction("👍", listOf(me))), choices, me, null))
        // Signed out mid-way (no user ID): nobody's reaction is "yours".
        assertNull(promptAnswer(prompt(Reaction("✅", listOf(""))), choices, "", null))
    }

    @Test fun `a question's buttons wait while the answer is going out, and come back if it fails`() {
        val question = Msg("\$q", agent, "Hermes", "❓ Which one?\n  1. staging\n  2. production", 1_000, isOwn = false)
        fun echo(state: SendState) = Msg("~t", me, "Me", "1", 2_000, isOwn = true, sendState = state)
        // Tapped, the send not back yet.
        Answers.begin("\$q", "1", 2_000)
        assertEquals(PromptAnswer("1", AnswerPhase.Sending), questionAnswer(listOf(question), 0, Answers.all.value["\$q"], 2_100))
        // Handed to the queue: its echo keeps the buttons off for as long as it's going out…
        Answers.queued("\$q", 2_200)
        val entry = Answers.all.value["\$q"]
        assertEquals(PromptAnswer("1", AnswerPhase.Sending), questionAnswer(listOf(question, echo(SendState.Sending)), 0, entry, 60_000))
        assertFalse(Answers.begin("\$q", "1", 2_300))
        // …and a failed one gives them back (its row has Retry and Delete).
        assertNull(questionAnswer(listOf(question, echo(SendState.Failed)), 0, entry, 60_000))
        assertTrue(Answers.begin("\$q", "2", 60_000))
    }

    @Test fun `an answer typed in the box while its echo is going out holds the buttons too`() {
        val question = Msg("\$q", agent, "Hermes", "❓ Which one?\n  1. staging", 1_000, isOwn = false)
        val typed = Msg("~t", me, "Me", "staging please", 2_000, isOwn = true, sendState = SendState.Sending)
        assertEquals(PromptAnswer("", AnswerPhase.Sending), questionAnswer(listOf(question, typed), 0, null, 2_100))
        // Nothing sent, nothing noted: open.
        assertNull(questionAnswer(listOf(question), 0, null, 2_100))
        // An echo from before the question is not an answer to it.
        assertNull(questionAnswer(listOf(typed.copy(ts = 500), question), 1, null, 2_100))
    }
}
