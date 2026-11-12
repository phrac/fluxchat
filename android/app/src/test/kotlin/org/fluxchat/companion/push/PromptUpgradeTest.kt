package org.fluxchat.companion.push

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

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
