package org.fluxchat.companion.ui

import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.ViewKey
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Which session opens by itself after you send a request (ChatScreen's follow). */
class FollowTest {
    private val room = "!r:x"
    private val sentAt = 100_000L

    private fun runs(vararg r: Pair<String?, RunInfo?>) = r.associate { (root, run) -> ViewKey(room, root) to run }

    @Test fun `the session the agent starts for the request you just sent is followed, once`() {
        val started = runs("\$a" to RunInfo(RunState.Working, sentAt + 2_000))
        assertEquals("\$a", followTarget(started, sentAt, emptySet(), sentAt + 3_000))
        // Decided once — opened, or skipped because you were typing: either way not again.
        assertNull(followTarget(started, sentAt, setOf("\$a"), sentAt + 4_000))
    }

    @Test fun `nothing is followed unless a request was sent from here, recently`() {
        val started = runs("\$a" to RunInfo(RunState.Working, sentAt + 2_000))
        assertNull(followTarget(started, 0, emptySet(), sentAt + 3_000))
        assertNull(followTarget(started, sentAt, emptySet(), sentAt + 3 * 60_000))
    }

    @Test fun `older, finished and unthreaded runs are not followed`() {
        val others = runs(
            "\$old" to RunInfo(RunState.Working, sentAt - 60_000),
            "\$done" to RunInfo(RunState.Done, sentAt + 1_000),
            null to RunInfo(RunState.Working, sentAt + 1_000),
            "\$gone" to null,
        )
        assertNull(followTarget(others, sentAt, emptySet(), sentAt + 3_000))
        // The request's own timestamp may be a little before the tap was noted.
        assertEquals("\$a", followTarget(runs("\$a" to RunInfo(RunState.NeedsYou, sentAt - 4_000)), sentAt, emptySet(), sentAt + 1_000))
    }
}
