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
    fun aFinishedRunBackfillsModelAndCostFromTheRoomRuntime() {
        // The desktop's record_run: when a run ends carrying no model / cost,
        // the room's newest runtime info fills them (older agents say nothing
        // on the last edit but did show a footer earlier).
        val h = ArrayList<RunRecord>()
        val rt = Runtime(model = "opus-5", contextPct = 42, costUsd = 0.18)
        History.record(h, thread, run(RunState.Working, now - 5000), "x", "x", now)
        assertTrue(History.record(h, thread, run(RunState.Done, now - 5000), "x", "x", now, runtime = rt))
        assertEquals("opus-5", h[0].model)
        assertEquals(0.18, h[0].costUsd!!, 1e-9)
        // A run that reports its own fields is never overwritten by the runtime.
        val h2 = ArrayList<RunRecord>()
        History.record(h2, thread, run(RunState.Working, now - 5000, cost = 0.30, model = "glm-5"), "x", "x", now)
        History.record(h2, thread, run(RunState.Done, now - 5000, cost = 0.30, model = "glm-5"), "x", "x", now, runtime = rt)
        assertEquals("glm-5", h2[0].model)
        assertEquals(0.30, h2[0].costUsd!!, 1e-9)
        // While still working the runtime must not fill in (the desktop only
        // pulls it on the end transition).
        val h3 = ArrayList<RunRecord>()
        History.record(h3, thread, run(RunState.Working, now - 5000), "x", "x", now, runtime = rt)
        assertNull(h3[0].model)
        assertNull(h3[0].costUsd)
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
    fun loneCommandRecordsPruneOnLoad() {
        // The desktop's load_run_history retain: older builds recorded
        // `!usage` as a run; the stored request decides, title only as the
        // fallback for records that predate the request field.
        val h = mutableListOf(
            RunRecord("$room::\$u", room, agent, "!usage", "done", now - 300, request = "!usage"),
            RunRecord("$room::\$g", room, agent, "!goal ship it", "done", now - 200, request = "!goal ship it"),
            RunRecord("$room::\$o", room, agent, "!status", "done", now - 100), // no request: title decides
            RunRecord("$room::\$p", room, agent, "ship it", "done", now - 50, request = "!usage"), // request wins over title
        )
        assertTrue(History.pruneLoneCommands(h))
        assertEquals(listOf("$room::\$g"), h.map { it.key })
        assertFalse(History.pruneLoneCommands(h)) // idempotent
        // A command with a task after it is a run, not a lone command.
        val runs = mutableListOf(
            RunRecord("$room::\$x", room, agent, "x", "done", now, request = "/new make it red"),
        )
        assertFalse(History.pruneLoneCommands(runs))
        assertEquals(1, runs.size)
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

    @Test
    fun aRunThatIsSimplyGoneGetsAnEnd() {
        // history.rs `close_run_record`: a prompt answered, a working reaction taken back.
        val h = ArrayList<RunRecord>()
        val started = now - 90_000
        assertTrue(History.record(h, thread, run(RunState.NeedsYou, started), "Ship it", "ship it", now))
        assertFalse("another run's start", History.close(h, thread, started + 1, now))
        assertTrue(History.close(h, thread, started, now + 500))
        assertEquals("done", h[0].state)
        assertEquals(now + 500, h[0].ended)
        assertFalse("already closed", History.close(h, thread, started, now + 900))
    }

    @Test
    fun aStalledRunThatFinishesAfterAllTakesItsOutcome() {
        // Silent for more than two hours, then ✅ with a cost: no longer "stalled".
        val h = ArrayList<RunRecord>()
        val started = now - 90_000
        assertTrue(History.record(h, thread, run(RunState.Working, started), "Ship it", "ship it", now))
        val later = now + History.STALE_RUN_MS + 1
        assertTrue(History.settleStalled(h, { 0L }, later))
        assertTrue(History.record(h, thread, run(RunState.Done, started, steps = 9, cost = 0.4), "Ship it", "ship it", later + 60_000))
        assertEquals("done", h[0].state)
        assertEquals(0.4, h[0].costUsd!!, 1e-9)
        assertEquals(later + 60_000, h[0].ended)
    }

    @Test
    fun aProtocolRunDatedFromItsStatusThenItsRequestIsOneRecord() {
        // A cold start (or a timeline refilling after a sync gap) loads the
        // newest page first: the agent's status message is there, the request
        // it's about a page further back. The run is dated from the status
        // message, then from the request — by its start alone, two records:
        // the first stayed "working", then "stalled", and its session total
        // was spent a second time.
        val generic = Harness()
        val key = ViewKey(room, "req")
        val req = Msg("req", "@me:x", "me", "ship it", now - 60_000, isOwn = true, threadCount = 1)
        fun status(cost: Double, state: String = "working") = Msg(
            "s", agent, agent, "working", now - 55_000, isOwn = false, threadRoot = "req",
            agent = Protocol(run = "req", state = state, costUsd = cost),
        )
        val h = ArrayList<RunRecord>()
        val first = Runs.agentUpdates(room, listOf(status(1.0)), agent, generic, isDm = false, now = now).runs[key]!!
        assertEquals("dated from the status message until the request loads", now - 55_000, first.since)
        assertTrue(History.record(h, key, first, "Thread", null, now))
        val second = Runs.agentUpdates(room, listOf(req, status(2.0)), agent, generic, isDm = false, now = now).runs[key]!!
        assertEquals(req.ts, second.since)
        assertTrue(History.record(h, key, second, "ship it", "ship it", now + 1_000))
        assertEquals("the same run: one record, moved to its request", 1, h.size)
        assertEquals(req.ts, h[0].started)
        assertEquals("ship it", h[0].title)
        assertEquals(2.0, h[0].costUsd!!, 1e-9)
        // The run id is what says so, and it's still there after a restart.
        val saved = History.decode(History.encode(h))
        assertEquals("req", saved[0].runId)
        // The timeline clears and refills: dated from the status message
        // again for a moment. Still that record, and its start stays the request's.
        assertFalse(History.record(saved, key, first.copy(costUsd = 2.0), "ship it", null, now + 2_000))
        assertEquals(1, saved.size)
        assertEquals(req.ts, saved[0].started)
        val over = Runs.agentUpdates(room, listOf(req, status(2.5, "done")), agent, generic, isDm = false, now = now).runs[key]!!
        assertTrue(History.record(saved, key, over, "ship it", "ship it", now + 3_000))
        assertEquals("done", saved.single().state)
        assertEquals("the session's total, once", 2.5, History.spend(saved, agent, 0), 1e-9)
        // Another run in the same session is its own record.
        assertTrue(History.record(saved, key, first.copy(runId = "again", since = now + 10_000), "ship it", null, now + 11_000))
        assertEquals(2, saved.size)
        // A run with no id is only ever itself by its start (as on the desktop).
        val anon = ArrayList<RunRecord>()
        History.record(anon, key, first.copy(runId = null), "t", null, now)
        History.record(anon, key, second.copy(runId = null), "t", null, now)
        assertEquals(2, anon.size)
        // A prompt of that run answered while it's dated from the status
        // message: the record (dated from the request) closes all the same.
        val asked = ArrayList<RunRecord>()
        History.record(asked, key, second.copy(state = RunState.NeedsYou), "ship it", "ship it", now)
        assertFalse(History.close(asked, key, first.since, now + 500))
        assertTrue(History.close(asked, key, first.since, now + 500, runId = "req"))
        assertEquals(now + 500, asked[0].ended)
    }

    @Test
    fun aRunEndedWhenItEndedNotWhenThePhoneNoticed() {
        // The phone is often not running: a 3-minute run it noticed ten
        // hours later was recorded as ending then, and read "10h".
        val now = 100 * 3_600_000L
        val started = now - 10 * 3_600_000L
        val ended = started + 180_000
        val h = ArrayList<RunRecord>()
        History.record(h, thread, run(RunState.Working, started), "x", "x", started + 5_000)
        assertTrue(History.record(h, thread, run(RunState.Done, started).copy(ended = ended), "x", "x", now))
        assertEquals(ended, h[0].ended)
        assertEquals("3m", h[0].duration(now))
        // Nothing dates it: when it was noticed, as before (and as the desktop).
        val undated = ArrayList<RunRecord>()
        History.record(undated, thread, run(RunState.Working, started), "x", "x", started + 5_000)
        History.record(undated, thread, run(RunState.Done, started), "x", "x", now)
        assertEquals(now, undated[0].ended)
        // Never after it was noticed, never before it started (clocks differ).
        for ((claimed, kept) in listOf(now + 99_000 to now, started - 5_000 to started)) {
            val odd = ArrayList<RunRecord>()
            History.record(odd, thread, run(RunState.Working, started), "x", "x", started + 5_000)
            History.record(odd, thread, run(RunState.Done, started).copy(ended = claimed), "x", "x", now)
            assertEquals(kept, odd[0].ended)
        }
        // A stalled record whose run finished after all takes the run's end too.
        val late = ArrayList<RunRecord>()
        History.record(late, thread, run(RunState.Working, started), "x", "x", started + 5_000)
        assertTrue(History.settleStalled(late, { 0L }, now))
        assertEquals(started, late[0].ended)
        History.record(late, thread, run(RunState.Done, started).copy(ended = ended), "x", "x", now + 1_000)
        assertEquals("done" to ended, late[0].state to late[0].ended)
    }

    @Test
    fun oneBadRecordDoesNotTakeTheRestWithIt() {
        val good = """{"key":"k1","room":"r","agent":"a","title":"t","state":"done","started":1,"ended":2,"steps":0}"""
        val bad = """{"key":"k2","room":"r","agent":"a","title":"t","state":"done","started":1,"ended":"soon","steps":0}"""
        val open = """{"key":"k3","room":"r","agent":"a","title":"t","state":"working","started":3,"ended":null,"steps":0,"cost_usd":null}"""
        val h = History.decode("[$good,$bad,$open]".toByteArray())
        assertEquals(listOf("k1", "k3"), h.map { it.key })
        assertNull(h[1].ended)
        assertNull(h[1].costUsd)
    }

    @Test
    fun aCostThatIsNotANumberIsLeftOutOfTheFile() {
        val h = listOf(RunRecord("k", room, agent, "t", "done", now, costUsd = Double.POSITIVE_INFINITY))
        val back = History.decode(History.encode(h))
        assertEquals(1, back.size)
        assertNull(back[0].costUsd)
    }

    @Test
    fun aNeedsYouRecordNothingWaitsBehindIsClosed() {
        val h = ArrayList<RunRecord>()
        val other = ViewKey(room, "\$b")
        assertTrue(History.record(h, thread, run(RunState.NeedsYou, now - 5000), "Ship it", "ship it", now))
        assertTrue(History.record(h, other, run(RunState.NeedsYou, now - 4000), "Deploy", "deploy", now))
        assertTrue(History.record(h, ViewKey(room, "\$c"), run(RunState.Working, now - 3000), "Build", "build", now))
        // Can't tell (the project isn't loaded): nothing closes.
        assertFalse(History.closeAnswered(h, { null }, now + 1000))
        // The first is still waiting, the second was answered; a working run isn't this function's.
        assertTrue(History.closeAnswered(h, { it.key == History.keyOf(thread) }, now + 1000))
        assertNull(h[0].ended)
        assertEquals("done", h[1].state)
        assertEquals(now + 1000, h[1].ended)
        assertEquals("working", h[2].state)
    }
}
