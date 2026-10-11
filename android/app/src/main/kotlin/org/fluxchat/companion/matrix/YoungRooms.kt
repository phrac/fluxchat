package org.fluxchat.companion.matrix

/**
 * Timeline gate for rooms this client has just created. Opening a room's
 * sliding-sync timeline before the sync response has delivered the room
 * SIGSEGVs inside the SDK — a null deref in `timelineWithConfiguration`
 * ~200 ms after `createRoom` returns (t_aa662dd8: fault addr 0x8 on
 * DefaultDispatch, uniffi future poll; same native family as the event-cache
 * crashes #7204/#7248, and no `runCatching` walks past a SEGV). The room is
 * in the store instantly — `getRoom` answers and the room list even appends
 * it locally — but its sliding-sync backing doesn't exist yet, and the
 * timeline builder assumes it does. So mark the room young at creation and
 * wait the grace out before ANY timeline is opened for it; afterwards it is
 * an ordinary room. Only rooms WE created are tracked: rooms that arrive via
 * sync already carry their sliding-sync data.
 */
object YoungRooms {
    /**
     * How long to hold timeline creation back: a couple of sliding-sync
     * round-trips on a LAN server (a few hundred ms each) with wide headroom.
     * A project opening two seconds late is invisible; a SIGSEGV is not.
     */
    const val GRACE_MS = 2_500L

    private val readyAt = HashMap<String, Long>()

    /** The room [roomId] was created now: hold its timeline for [graceMs]. */
    fun mark(roomId: String, nowMs: Long = System.currentTimeMillis(), graceMs: Long = GRACE_MS) {
        synchronized(readyAt) { readyAt[roomId] = nowMs + graceMs }
    }

    /** Ms to wait before touching [roomId]'s timeline; 0 when it isn't young (or isn't ours). */
    fun waitMs(roomId: String, nowMs: Long = System.currentTimeMillis()): Long =
        synchronized(readyAt) { readyAt[roomId] }?.let { (it - nowMs).coerceAtLeast(0L) } ?: 0L

    /** The wait for [roomId] has been served: from now on it's an ordinary room. */
    fun settle(roomId: String) {
        synchronized(readyAt) { readyAt.remove(roomId) }
    }

    /** Signed out: nothing is young any more. */
    fun clear() {
        synchronized(readyAt) { readyAt.clear() }
    }
}
