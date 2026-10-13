package org.fluxchat.companion.agents

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class HistoryTest {
    private val agent = "@hermes:x"
    private val room = "!r:x"
    private val now = 10_000_000L

    private fun run(state: RunState, since: Long, steps: Int = 0, cost: Double? = null, model: String? = null) =
        RunInfo(state, since, steps = steps, agent = agent, costUsd = cost, model = model)

    private val a = "\$a"
    private val thread = ViewKey(room, a)

    @Test
    fun onlyRunsSeenLiveAreRecordedAndTheyGetAnEnd() {
        val h = ArrayList<RunRecord>()
        val started = now - 90_000
        assertFalse("never seen live: not recorded", History.record(h, thread, run(RunState.Done, started), "Ship it", "ship it", now))
        assertTrue(History.record(h, thread, run(RunState.Working, started, steps = 2), "Ship it", "ship it", now))
        assertEquals(1, h.size)
        assertNull(h[0].ended)
        assertTrue(History.record(h, thread, run(RunState.Done, started, steps = 5), "Ship it", "ship it", now))
        assertEquals("done", h[0].state)
        assertEquals(5, h[0].steps)
        assertTrue(h[0].duration(now).endsWith("m"))
        assertEquals(1, History.matching(h, "hermes") { "" }.size)
        assertTrue(History.matching(h, "nothing like it") { "" }.isEmpty())
    }

    @Test
    fun aLoneCommandIsntARunButACommandWithATaskIs() {
        val h = ArrayList<RunRecord>()
        val working = run(RunState.Working, now)
        assertFalse(History.record(h, ViewKey(room, "\$u"), working, "!usage", "!usage", now))
        assertTrue(History.record(h, ViewKey(room, "\$g"), working, "!goal ship it", "!goal ship it", now))
        assertEquals(1, h.size)
    }

    @Test
    fun costKeepsItsMaxAndModelArrivesWithTheFinish() {
        val h = ArrayList<RunRecord>()
        History.record(h, thread, run(RunState.Working, now - 5000, cost = 0.10), "x", "x", now)
        History.record(h, thread, run(RunState.Working, now - 5000, cost = 0.05), "x", "x", now)
        assertEquals(0.10, h[0].costUsd!!, 1e-9)
        assertTrue(History.record(h, thread, run(RunState.Done, now - 5000, cost = 0.30, model = "glm-5"), "x", "x", now))
        assertEquals(0.30, h[0].costUsd!!, 1e-9)
        assertEquals("glm-5", h[0].model)
        assertEquals(now, h[0].ended)
    }

    @Test
    fun theFinalResultLandsOnTheRecordAndSearches() {
        val h = ArrayList<RunRecord>()
        History.record(h, thread, run(RunState.Working, now - 5000), "x", "x", now)
        assertTrue(History.record(h, thread, run(RunState.Done, now - 5000).copy(result = "Deployed v0.2.0"), "x", "x", now))
        assertEquals("Deployed v0.2.0", h[0].result)
        // The result never changes once set (the desktop's record_run).
        History.record(h, thread, run(RunState.Done, now - 5000).copy(result = "overwritten?"), "x", "x", now)
        assertEquals("Deployed v0.2.0", h[0].result)
        assertEquals(1, History.matching(h, "deployed") { "" }.size)
        val saved = History.decode(History.encode(h))
        assertEquals("Deployed v0.2.0", saved[0].result)
    }

    @Test
    fun aThreadTitleReplacesThePlaceholderOnly() {
        val h = ArrayList<RunRecord>()
        History.record(h, thread, run(RunState.Working, now), "Thread", null, now)
        assertEquals("Thread", h[0].title)
        History.record(h, thread, run(RunState.Working, now), "Fix the pane focus bug", null, now)
        assertEquals("Fix the pane focus bug", h[0].title)
        History.record(h, thread, run(RunState.Working, now), "Thread", null, now)
        assertEquals("Fix the pane focus bug", h[0].title)
    }

    @Test
    fun stalledUsesTheLastSignOfLife() {
        val h = ArrayList<RunRecord>()
        val started = now - 3 * 60 * 60 * 1000L
        val last = now - 60_000
        History.record(h, thread, run(RunState.Working, started), "x", "x", started)
        assertFalse("alive: signs of life", History.settleStalled(h, { last }, now))
        assertTrue(History.settleStalled(h, { started }, now))
        assertEquals(History.STALLED, h[0].state)
        assertEquals(started, h[0].ended)
    }

    @Test
    fun historyIsCappedAtTheLimitDroppingTheOldest() {
        val h = ArrayList<RunRecord>()
        for (i in 0..History.HISTORY_LIMIT) {
            History.record(h, ViewKey(room, "\$r$i"), run(RunState.Working, now + i), "task $i", "task $i", now)
        }
        assertEquals(History.HISTORY_LIMIT, h.size)
        assertEquals("\$r1", h.first().key.substringAfter("::"))
    }

    @Test
    fun encodeDecodeRoundTrips() {
        val h = listOf(
            RunRecord("$room::\$a", room, agent, "Ship it", "done", now - 1000, ended = now, steps = 4, model = "glm-5", costUsd = 0.12),
            RunRecord(room, room, agent, "Request in the room", "failed", now - 500),
        )
        val back = History.decode(History.encode(h))
        assertEquals(h, back)
        assertTrue(History.decode("not json".toByteArray()).isEmpty())
    }

    @Test
    fun weekWindowStartsAtMondayMidnightLikeTheDesktop() {
        // The desktop's budgets Period::start: weeks begin Monday 00:00 local.
        // Thursday 2026-10-08 15:30 local → its week starts Monday 00:00 of 10-05.
        fun local(y: Int, mo: Int, d: Int, h: Int, mi: Int): Long =
            java.util.Calendar.getInstance().apply {
                clear(); set(y, mo, d, h, mi)
            }.timeInMillis
        val thu = local(2026, 9, 8, 15, 30)
        assertEquals(local(2026, 9, 8, 0, 0), History.periodStart(thu, week = false))
        assertEquals(local(2026, 9, 5, 0, 0), History.periodStart(thu, week = true))
        // Monday itself (even at 00:00): the week is today.
        val mon = local(2026, 9, 5, 0, 0)
        assertEquals(mon, History.periodStart(mon, week = true))
        // Sunday: still the week that started six days earlier.
        assertEquals(local(2026, 9, 5, 0, 0), History.periodStart(local(2026, 9, 11, 23, 59), week = true))
        // A rolling 7-day window would start 7 days back (Oct 1 15:30), not
        // Monday 00:00: the week start sits inside it, never before it.
        assertTrue(History.periodStart(thu, week = true) > thu - 7 * 24 * 3600 * 1000L)
    }

    @Test
    fun spendAddsUpPerAgentOverAPeriod() {
        val h = listOf(
            RunRecord("$room::\$a", room, agent, "a", "done", now - 100, costUsd = 0.10),
            RunRecord("$room::\$b", room, agent, "b", "done", now - 50, costUsd = 0.25),
            RunRecord("$room::\$c", room, agent, "c", "done", now - 10_000, costUsd = 5.0),
            RunRecord("$room::\$d", room, "@other:x", "d", "done", now - 10, costUsd = 1.0),
        )
        assertEquals(0.35, History.spend(h, agent, now - 1000), 1e-9)
        assertEquals(5.35, History.spend(h, agent, 0), 1e-9)
        assertEquals(1.0, History.spend(h, "@other:x", 0), 1e-9)
    }

    @Test
    fun costUsdIsASessionTotalSoOnlyTheIncreaseCounts() {
        // Same session (key): records report 0.10 then 0.40 → the second run cost
        // 0.30, not 0.40; a different key counts whole. The desktop's
        // spend_adds_up_session_increases is the reference.
        val h = listOf(
            RunRecord("$room::\$a", room, agent, "a", "done", now - 300, costUsd = 0.10),
            RunRecord("$room::\$a", room, agent, "a", "done", now - 200, costUsd = 0.40),
            RunRecord("$room::\$b", room, agent, "b", "done", now - 100, costUsd = 0.20),
        )
        assertEquals(0.60, History.spend(h, agent, 0), 1e-9)
    }

    @Test
    fun aCostDropCountsWholeAsANewSession() {
        // Same key: 0.40 then 0.10 — the drop is a new session, so 0.10 counts whole (budgets.rs).
        val h = listOf(
            RunRecord("$room::\$a", room, agent, "a", "done", now - 300, costUsd = 0.40),
            RunRecord("$room::\$a", room, agent, "b", "done", now - 200, costUsd = 0.10),
        )
        assertEquals(0.50, History.spend(h, agent, 0), 1e-9)
    }

    @Test
    fun aRecordBeforeThePeriodSetsTheBaselineButIsntCounted() {
        // The desktop's spent(2500) == 0.40 case: the pre-period record raises the
        // session total, the in-period record only counts the increase.
        val h = listOf(
            RunRecord("$room::\$a", room, agent, "a", "done", now - 300, costUsd = 0.60),
            RunRecord("$room::\$a", room, agent, "b", "done", now - 200, costUsd = 1.00),
        )
        assertEquals(0.40, History.spend(h, agent, now - 250), 1e-9)
        assertEquals(1.00, History.spend(h, agent, 0), 1e-9)
    }

    @Test
    fun aRowShowsTheRunsOwnCostNotTheSessionTotal() {
        val h = listOf(
            RunRecord("$room::\$a", room, agent, "a", "done", now - 300, costUsd = 0.10),
            RunRecord("$room::\$a", room, agent, "a", "done", now - 200, costUsd = 0.40),
        )
        assertEquals(0.10, History.runCost(h, h[0])!!, 1e-9)
        assertEquals(0.30, History.runCost(h, h[1])!!, 1e-9)
        assertNull(History.runCost(h, RunRecord("$room::\$z", room, agent, "z", "done", now)))
    }

    @Test
    fun aSettledStalledRunIsNeverReopened() {
        // Hermes leaving its 👀 behind: a later live sighting of the same run
        // must not flip a settled record back to working (the desktop drops
        // stalled states so record_run never sees them again).
        val h = ArrayList<RunRecord>()
        val started = now - 90_000
        assertTrue(History.record(h, thread, run(RunState.Working, started), "Ship it", "ship it", now))
        assertTrue(History.settleStalled(h, { 0L }, now + History.STALE_RUN_MS + 1))
        assertEquals(History.STALLED, h[0].state)
        assertFalse("settled stays settled", History.record(h, thread, run(RunState.Working, started), "Ship it", "ship it", now))
        assertEquals(History.STALLED, h[0].state)
    }

    @Test
    fun searchMatchesTitleRoomAgentAndState() {
        val h = listOf(
            RunRecord("$room::\$a", room, agent, "Deploy the release", "done", now),
            RunRecord("$room::\$b", room, agent, "Fix login", "failed", now - 100),
        )
        assertEquals(1, History.matching(h, "deploy") { "" }.size)
        assertEquals(1, History.matching(h, "failed") { "" }.size)
        assertEquals(2, History.matching(h, "hermes") { "" }.size)
        assertEquals(2, History.matching(h, "betting") { r -> if (r == room) "Betting" else "" }.size)
        assertEquals(1, History.matching(h, "betting deploy") { r -> if (r == room) "Betting" else "" }.size)
        assertEquals(2, History.matching(h, "  ") { "" }.size)
    }

    @Test
    fun loneCommandDetection() {
        assertTrue(History.isLoneCommand("!usage"))
        assertTrue(History.isLoneCommand("/status"))
        assertFalse(History.isLoneCommand("!goal ship it"))
        assertFalse(History.isLoneCommand("just text"))
        assertFalse(History.isLoneCommand("!"))
    }

    @Test
    fun durationsMatchTheDesktop() {
        val rec = RunRecord(room, room, agent, "t", "done", 0, ended = 59_000)
        assertEquals("59s", rec.duration(now))
        assertEquals("4m", rec.copy(ended = 240_000).duration(now))
        assertEquals("1h 20m", rec.copy(ended = 4_800_000).duration(now))
        assertEquals("2m", rec.copy(ended = null, started = 0).duration(120_000))
    }
}
