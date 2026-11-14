package org.fluxchat.companion.push

import org.fluxchat.companion.agents.Harness
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.Protocol
import org.fluxchat.companion.agents.ProtocolChoice
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Which prompts the watcher announces when it finds them. A reaction prompt
 * (a Hermes approval without the protocol) is pushed as a plain message and
 * only becomes a prompt when the agent's reactions sync; the watcher then
 * re-posts it with its buttons.
 */
class PromptUpgradeTest {
    private val minute = 60_000L

    @Test fun `a prompt the push already notified gets its buttons however old it is`() {
        // The push sat in Doze for ten minutes, or the cold start took three:
        // the two-minute freshness window left the approval with nothing to tap.
        assertTrue(PromptWatcher.shouldAnnounce(pushed = true, ageMs = 10 * minute, watching = false))
        assertTrue(PromptWatcher.shouldAnnounce(pushed = true, ageMs = 3 * 60 * minute, watching = false))
    }

    @Test fun `a prompt merely found in loaded history is not news`() {
        // Opening the app loads yesterday's open approval: no notification for that.
        assertFalse(PromptWatcher.shouldAnnounce(pushed = false, ageMs = 10 * minute, watching = false))
    }

    @Test fun `a fresh prompt is announced without a push`() {
        // Sync is running (an agent is working, the service keeps it alive): no push needed.
        assertTrue(PromptWatcher.shouldAnnounce(pushed = false, ageMs = 30_000, watching = false))
    }

    @Test fun `never for the prompt on screen in front of you`() {
        assertFalse(PromptWatcher.shouldAnnounce(pushed = true, ageMs = 1_000, watching = true))
        assertFalse(PromptWatcher.shouldAnnounce(pushed = false, ageMs = 1_000, watching = true))
    }

    private val hermes = Harness.parse(File("../../assets/agents/hermes.toml").readText())
    private val now = 10_000_000L

    private fun prompt(state: String) = Msg(
        "\$prompt", "@hermes:x", "Hermes", "Run `rm -rf build`?", now - 5 * minute, isOwn = false, threadRoot = "\$request",
        agent = Protocol(run = "\$request", state = state, choices = listOf(ProtocolChoice("✅", "Approve once"), ProtocolChoice("❌", "Deny"))),
    )

    @Test fun `a prompt answered before its push arrived is over`() {
        // Approved on the desktop; sync brought the agent's edit ("working"
        // again) before the push got here. The push still carries the
        // message as first sent — asking — and put the buttons back up,
        // for good: the watcher had already taken this prompt down once.
        val request = Msg("\$request", "@me:x", "me", "clean the build", now - 6 * minute, isOwn = true)
        assertFalse(PromptWatcher.over(listOf(request, prompt("waiting")), "\$prompt", hermes, now))
        assertTrue(PromptWatcher.over(listOf(request, prompt("working")), "\$prompt", hermes, now))
        assertTrue(PromptWatcher.over(listOf(request, prompt("done")), "\$prompt", hermes, now))
    }

    @Test fun `a question you have answered is over, one still open is not`() {
        val question = Msg("\$q", "@hermes:x", "Hermes", "❓ Which one?\n  1. Deploy\n  2. Staging only", now - minute, isOwn = false)
        assertFalse(PromptWatcher.over(listOf(question), "\$q", hermes, now))
        val answer = Msg("\$a", "@me:x", "me", "2", now - 30_000, isOwn = true)
        assertTrue(PromptWatcher.over(listOf(question, answer), "\$q", hermes, now))
    }

    @Test fun `a message the timeline does not have says nothing`() {
        // Then the push is all there is to go by: it notifies.
        assertFalse(PromptWatcher.over(emptyList(), "\$prompt", hermes, now))
    }

    @Test fun `a prompt answered from its notification is not found new by a process that just started`() {
        // The tap started the process; the prompt is still open in the
        // timeline until the agent takes it down, and fresh: it came back,
        // alert and buttons, over "Answered".
        assertFalse(PromptWatcher.wasAnnounced("\$tapped"))
        PromptWatcher.answered("\$tapped")
        assertTrue(PromptWatcher.wasAnnounced("\$tapped"))
        // The answer didn't go out and the prompt couldn't be put back: news again.
        PromptWatcher.unanswered("\$tapped")
        assertFalse(PromptWatcher.wasAnnounced("\$tapped"))
    }

    @Test fun `the push path's events are remembered, a bounded number of them`() {
        Notifier.markPushed("\$first")
        assertTrue(Notifier.wasPushed("\$first"))
        assertFalse(Notifier.wasPushed("\$never"))
        // Old ones fall out; a recent one that is marked again stays.
        repeat(150) { Notifier.markPushed("\$filler$it") }
        Notifier.markPushed("\$first")
        repeat(150) { Notifier.markPushed("\$more$it") }
        assertTrue(Notifier.wasPushed("\$first"))
        assertFalse(Notifier.wasPushed("\$filler0"))
        assertTrue(Notifier.wasPushed("\$more149"))
    }
}
