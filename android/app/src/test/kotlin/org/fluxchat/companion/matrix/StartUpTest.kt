package org.fluxchat.companion.matrix

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class StartUpTest {
    // --- "loading" versus the empty states ---

    @Test fun `nothing read yet is loading, not no agents`() {
        assertTrue(Loading.still(agentListRead = false, roomListReady = false, looked = false, gaveUp = false))
    }

    @Test fun `loading lasts until the agent list is read and the rooms are looked through`() {
        assertTrue("the rooms haven't arrived", Loading.still(agentListRead = true, roomListReady = false, looked = false, gaveUp = false))
        assertTrue("the agent list hasn't", Loading.still(agentListRead = false, roomListReady = true, looked = false, gaveUp = false))
        assertTrue("nobody has looked through them", Loading.still(agentListRead = true, roomListReady = true, looked = false, gaveUp = false))
        assertFalse(Loading.still(agentListRead = true, roomListReady = true, looked = true, gaveUp = false))
    }

    @Test fun `offline with nothing saved it stops saying loading in the end`() {
        assertFalse(Loading.still(agentListRead = false, roomListReady = false, looked = false, gaveUp = true))
    }

    // --- account data the server says isn't set ---

    private class Clock(var now: Long = 1_000_000L)

    @Test fun `a not-set answer is not asked for again for a while`() {
        val clock = Clock()
        val cache = NotSetCache(ttlMs = 600_000, now = { clock.now })
        assertFalse(cache.notSet("org.fluxchat.pins"))
        cache.answered("org.fluxchat.pins", set = false, asked = cache.asking())
        assertTrue(cache.notSet("org.fluxchat.pins"))
        assertFalse("one type's answer is not another's", cache.notSet("org.fluxchat.agents"))
        clock.now += 599_999
        assertTrue(cache.notSet("org.fluxchat.pins"))
        clock.now += 1
        assertFalse("asked again after the time is up", cache.notSet("org.fluxchat.pins"))
    }

    @Test fun `our own write ends the not-set memory at once`() {
        val cache = NotSetCache(now = { 0L })
        cache.answered("org.fluxchat.agents", set = false, asked = cache.asking())
        cache.wrote("org.fluxchat.agents")
        assertFalse(cache.notSet("org.fluxchat.agents"))
    }

    @Test fun `a not-set answer that crossed our own write is not remembered`() {
        // Asked, then we saved the list, then the 404 from before came in:
        // remembered, the list read as "no agents" until sync echoed it.
        val cache = NotSetCache(now = { 0L })
        val asked = cache.asking()
        cache.wrote("org.fluxchat.agents")
        cache.answered("org.fluxchat.agents", set = false, asked = asked)
        assertFalse(cache.notSet("org.fluxchat.agents"))
    }

    @Test fun `the server having it clears the memory`() {
        val cache = NotSetCache(now = { 0L })
        cache.answered("org.fluxchat.pins", set = false, asked = cache.asking())
        cache.answered("org.fluxchat.pins", set = true, asked = cache.asking())
        assertFalse(cache.notSet("org.fluxchat.pins"))
    }

    @Test fun `asking for fresh data forgets what was remembered`() {
        val cache = NotSetCache(now = { 0L })
        cache.answered("org.fluxchat.pins", set = false, asked = cache.asking())
        cache.answered("org.fluxchat.agents", set = false, asked = cache.asking())
        cache.forget("org.fluxchat.agents")
        assertFalse(cache.notSet("org.fluxchat.agents"))
        assertTrue(cache.notSet("org.fluxchat.pins"))
        cache.forget()
        assertFalse(cache.notSet("org.fluxchat.pins"))
    }

    // --- a throw in the background ---

    @Test fun `a one-off task that throws just ends`() {
        assertFalse(endsTheProcess(IllegalStateException("a malformed release tag"), oneOff = true))
    }

    @Test fun `a bare coroutine that throws is still fatal, never quietly gone`() {
        // It may be a collector, and nothing would start it again.
        assertTrue(endsTheProcess(IllegalStateException("x"), oneOff = false))
    }

    @Test fun `an Error is fatal wherever it came from`() {
        assertTrue(endsTheProcess(OutOfMemoryError(), oneOff = true))
        assertTrue(endsTheProcess(UnsatisfiedLinkError(), oneOff = false))
    }

    // --- loops that must not stop ---

    @Test fun `restarts back off from a second to a minute`() {
        assertEquals(listOf(1_000L, 2_000L, 4_000L, 8_000L), (1..4).map(::restartDelayMs))
        assertEquals(60_000L, restartDelayMs(7))
        assertEquals(60_000L, restartDelayMs(500))
        assertEquals(1_000L, restartDelayMs(0))
    }

    @Test fun `a loop that throws is reported and started again`() = runBlocking {
        val reports = ArrayList<String>()
        val pauses = ArrayList<Long>()
        var runs = 0
        keepRunning("Working out the runs", { msg, e -> reports += "$msg: ${e.message}" }, pause = { pauses += it }, now = { 0L }) {
            runs++
            if (runs < 3) throw IllegalStateException("bad message $runs")
        }
        assertEquals(3, runs)
        assertEquals(listOf(1_000L, 2_000L), pauses)
        assertEquals(2, reports.size)
        assertTrue(reports[0], reports[0].startsWith("Working out the runs stopped") && reports[0].endsWith("bad message 1"))
    }

    @Test fun `a loop that had been running fine restarts quickly again`() = runBlocking {
        var clock = 0L
        val pauses = ArrayList<Long>()
        var runs = 0
        keepRunning("x", { _, _ -> }, pause = { pauses += it }, now = { clock }) {
            runs++
            when (runs) {
                1, 2 -> throw IllegalStateException("at once")
                3 -> {
                    clock += RESTART_HEALTHY_MS
                    throw IllegalStateException("after a healthy minute")
                }
            }
        }
        assertEquals(listOf(1_000L, 2_000L, 1_000L), pauses)
    }

    @Test fun `cancelling the loop ends it without a report`() = runBlocking {
        val reports = ArrayList<String>()
        val job = launch(Dispatchers.Default, start = CoroutineStart.UNDISPATCHED) {
            keepRunning("x", { msg, _ -> reports += msg }) { awaitCancellation() }
        }
        job.cancelAndJoin()
        assertTrue(job.isCancelled)
        assertTrue(reports.isEmpty())
    }

    @Test fun `a cancellation from inside is a failure, not an ending`() = runBlocking {
        // A cancelled SDK future surfaces as CancellationException while the
        // loop itself is still wanted: let through, the loop would end
        // without a word.
        val reports = ArrayList<String>()
        var runs = 0
        keepRunning("x", { msg, _ -> reports += msg }, pause = {}, now = { 0L }) {
            runs++
            if (runs == 1) throw CancellationException("the future was dropped")
        }
        assertEquals(2, runs)
        assertEquals(1, reports.size)
    }

    // --- the second invite ---

    @Test fun `an agent that is already there is not an invite failure`() {
        assertTrue(alreadyInRoom("kind=Forbidden, code=M_FORBIDDEN, msg=@hermes:x is already in the room."))
        assertTrue(alreadyInRoom("M_FORBIDDEN: User is already invited"))
        assertTrue(alreadyInRoom("user is already joined to room"))
        assertTrue(alreadyInRoom("@hermes:x is already a member"))
    }

    @Test fun `a real invite failure is still one`() {
        assertFalse(alreadyInRoom("M_FORBIDDEN: You don't have permission to invite users"))
        assertFalse(alreadyInRoom("M_LIMIT_EXCEEDED: Too many requests"))
        assertFalse(alreadyInRoom("already tried twice, giving up"))
        assertFalse(alreadyInRoom(null))
    }
}
