package org.fluxchat.companion.matrix

import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.ui.agentRoomOrder
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PinsTest {
    private val room = "!a:x"
    private val other = "!b:x"
    private val r1 = "\$r1"
    private val r2 = "\$r2"
    private val r3 = "\$r3"

    private fun pin(roomId: String = room, root: String, at: Long) = Pin(roomId, root, at)

    // --- codec ---

    @Test fun `round trip keeps every pin`() {
        val pins = PinStore.Pins(listOf(pin(root = r1, at = 30), pin(other, r2, 20), pin(root = r3, at = 10)))
        val back = PinStore.parse(pins.toJson())
        assertEquals(pins.pins, back.pins)
    }

    @Test fun `garbage is no pins, never a crash`() {
        assertEquals(emptyList<Pin>(), PinStore.parse(null).pins)
        assertEquals(emptyList<Pin>(), PinStore.parse("not json").pins)
        assertEquals(emptyList<Pin>(), PinStore.parse("{}").pins)
        assertEquals(emptyList<Pin>(), PinStore.parse("""{"pins": "nope"}""").pins)
        // Entries with wrong shapes (another client, a broken build) are dropped.
        assertEquals(emptyList<Pin>(), PinStore.parse("""{"pins":[{"room_id":"a","root":"b"}]}""").pins)
    }

    @Test fun `a session pinned twice keeps the newest stamp`() {
        val json = """{"pins":[{"room_id":"$room","root":"$r1","pinned_at":10},{"room_id":"$room","root":"$r1","pinned_at":40}]}"""
        val pins = PinStore.parse(json)
        assertEquals(listOf(pin(root = r1, at = 40)), pins.pins)
    }

    @Test fun `parse orders newest pin first`() {
        val pins = PinStore.parse("""{"pins":[{"room_id":"$room","root":"$r1","pinned_at":10},{"room_id":"$room","root":"$r2","pinned_at":30}]}""")
        assertEquals(listOf(r2, r1), pins.pins.map { it.root })
    }

    // --- lookups ---

    @Test fun `isPinned matches project and session together`() {
        val pins = PinStore.Pins(listOf(pin(root = r1, at = 1)))
        assertTrue(pins.isPinned(room, r1))
        assertFalse(pins.isPinned(room, r2))
        assertFalse(pins.isPinned(other, r1))
    }

    @Test fun `a project counts as pinned when any session in it is`() {
        val pins = PinStore.Pins(listOf(pin(other, r1, 5)))
        assertTrue(pins.hasRoom(other))
        assertFalse(pins.hasRoom(room))
        // The project's spot among pins: its earliest pin.
        val two = PinStore.Pins(listOf(pin(other, r1, 5), pin(other, r2, 2), pin(room, r3, 9)))
        assertEquals(2L, two.roomPinnedAt(other))
        assertEquals(9L, two.roomPinnedAt(room))
        assertNull(two.roomPinnedAt("!gone:x"))
    }

    @Test fun `pins for projects you left are pruned away`() {
        val pins = PinStore.Pins(listOf(pin(root = r1, at = 1), pin(other, r2, 2)))
        val left = pins.prune(setOf(other))
        assertEquals(listOf(other), left.pins.map { it.roomId })
    }

    // --- ordering ---

    @Test fun `pinned rows come first and keep their own order`() {
        val rows = listOf("new", "pinned-old", "mid", "pinned-new")
        val pinnedSet = setOf("pinned-old", "pinned-new")
        val out = pinnedFirst(rows) { it in pinnedSet }
        // Pinned first, each group in the list's own (newest-first) order.
        assertEquals(listOf("pinned-old", "pinned-new", "new", "mid"), out)
    }

    @Test fun `no pins leaves the order unchanged`() {
        val rows = listOf("a", "b", "c")
        assertEquals(rows, pinnedFirst(rows) { false })
    }

    @Test fun `projects sort pinned first, then by last activity`() {
        val pins = PinStore.Pins(listOf(Pin("!quiet:x", r1, 1)))
        val quiet = AgentRoom("!quiet:x", "Quiet", false, listOf("@hermes:x"), last = Msg("e", "@hermes:x", "H", "old", 100, isOwn = false))
        val busy = AgentRoom("!busy:x", "Busy", false, listOf("@hermes:x"), last = Msg("e", "@hermes:x", "H", "new", 900, isOwn = false))
        val sorted = listOf(busy, quiet).sortedWith(agentRoomOrder(pins))
        assertEquals(listOf("!quiet:x", "!busy:x"), sorted.map { it.roomId })
        // Without the pin, recency wins.
        val unpinned = listOf(busy, quiet).sortedWith(agentRoomOrder(PinStore.Pins()))
        assertEquals(listOf("!busy:x", "!quiet:x"), unpinned.map { it.roomId })
    }
}
