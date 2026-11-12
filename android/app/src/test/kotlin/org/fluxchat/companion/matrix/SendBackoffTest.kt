package org.fluxchat.companion.matrix

import org.junit.Assert.assertEquals
import org.junit.Test

/** When a room's send queue is switched back on after a failed request (the desktop's send_queue.rs schedule). */
class SendBackoffTest {
    private val room = "!r:x"

    @Test fun `a room that keeps failing waits five seconds, then twice as long each time, up to two minutes`() {
        val backoff = SendBackoff()
        var now = 1_000_000L
        val waits = (1..7).map {
            backoff.delayFor(room, recoverable = true, now).also { now += it + 1_000 }
        }
        assertEquals(listOf(5_000L, 10_000L, 20_000L, 40_000L, 80_000L, 120_000L, 120_000L), waits)
    }

    @Test fun `a request that failed for good resumes the queue at once and leaves the room's wait alone`() {
        val backoff = SendBackoff()
        assertEquals(5_000L, backoff.delayFor(room, recoverable = true, 0))
        assertEquals(0L, backoff.delayFor(room, recoverable = false, 1_000))
        assertEquals(0L, backoff.delayFor(room, recoverable = false, 2_000))
        assertEquals(10_000L, backoff.delayFor(room, recoverable = true, 3_000))
    }

    @Test fun `a room that has not failed for ten minutes starts again at five seconds`() {
        val backoff = SendBackoff()
        var now = 0L
        repeat(4) { now += backoff.delayFor(room, recoverable = true, now) }
        assertEquals(80_000L, backoff.delayFor(room, recoverable = true, now))
        // Exactly ten minutes is not yet "more than".
        assertEquals(120_000L, backoff.delayFor(room, recoverable = true, now + 600_000))
        assertEquals(5_000L, backoff.delayFor(room, recoverable = true, now + 600_000 + 600_001))
    }

    @Test fun `each room waits on its own failures`() {
        val backoff = SendBackoff()
        assertEquals(5_000L, backoff.delayFor("!a:x", recoverable = true, 0))
        assertEquals(10_000L, backoff.delayFor("!a:x", recoverable = true, 6_000))
        assertEquals(5_000L, backoff.delayFor("!b:x", recoverable = true, 7_000))
    }
}
