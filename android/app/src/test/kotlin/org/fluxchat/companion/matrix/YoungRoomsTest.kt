package org.fluxchat.companion.matrix

import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

/**
 * The young-room gate behind the post-create SIGSEGV (t_aa662dd8): opening
 * a just-created room's sliding-sync timeline crashes the SDK, so the gate
 * must hold for the grace, report exactly that wait, and release per room.
 */
class YoungRoomsTest {
    private val t0 = 1_791_679_862_612L // a real create timestamp

    @Before fun freshGate() = YoungRooms.clear()

    @Test fun `an ordinary room is never gated`() {
        assertEquals(0L, YoungRooms.waitMs("!unknown:example.org", nowMs = t0))
    }

    @Test fun `a fresh room waits the whole grace`() {
        YoungRooms.mark("!fresh:example.org", nowMs = t0, graceMs = 2_500L)
        assertEquals(2_500L, YoungRooms.waitMs("!fresh:example.org", nowMs = t0))
        assertEquals(1_500L, YoungRooms.waitMs("!fresh:example.org", nowMs = t0 + 1_000L))
    }

    @Test fun `the wait never goes negative`() {
        YoungRooms.mark("!fresh:example.org", nowMs = t0, graceMs = 100L)
        assertEquals(0L, YoungRooms.waitMs("!fresh:example.org", nowMs = t0 + 5_000L))
    }

    @Test fun `settling releases one room, not the others`() {
        YoungRooms.mark("!a:example.org", nowMs = t0, graceMs = 2_500L)
        YoungRooms.mark("!b:example.org", nowMs = t0, graceMs = 2_500L)
        YoungRooms.settle("!a:example.org")
        assertEquals(0L, YoungRooms.waitMs("!a:example.org", nowMs = t0))
        assertEquals(2_500L, YoungRooms.waitMs("!b:example.org", nowMs = t0))
    }

    @Test fun `clear forgets every young room`() {
        YoungRooms.mark("!a:example.org", nowMs = t0, graceMs = 2_500L)
        YoungRooms.clear()
        assertEquals(0L, YoungRooms.waitMs("!a:example.org", nowMs = t0))
    }
}
