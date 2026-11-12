package org.fluxchat.companion.agents

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class RunsTest {
    private val working = "👀"
    private val done = "✅"
    private val failed = "❌"
    private val agent = "@hermes:x"
    private val me = "@me:x"
    private val room = "!r:x"
    private val now = 10_000_000L

    private val hermes: Harness = Harness.parse(File("../../assets/agents/hermes.toml").readText())

    private fun msg(id: String, sender: String, ts: Long, body: String = "hi", vararg reactions: Pair<String, List<String>>, threadRoot: String? = null, threadCount: Int = 0) =
        Msg(id, sender, sender, body, ts, isOwn = sender == me, reactions = reactions.map { Reaction(it.first, it.second) }, threadRoot = threadRoot, threadCount = threadCount)

    @Test
    fun hermesProfileLoadsFromTheDesktopsToml() {
        assertEquals("hermes", hermes.id)
        assertEquals("!", hermes.commandPrefix)
        assertEquals(working, hermes.workingReaction)
        assertTrue(hermes.replyInThread)
        assertEquals("Approve once", hermes.approvals.first().label)
        assertTrue(hermes.hasCommand("stop"))
    }

    @Test
    fun aRequestsReactionsGiveItsState() {
        assertEquals(RunState.Working, Runs.requestState(hermes, msg("a", me, 0, "x", working to listOf(agent)), agent))
        assertEquals(RunState.Done, Runs.requestState(hermes, msg("a", me, 0, "x", working to listOf(agent), done to listOf(agent)), agent))
        assertEquals(RunState.Failed, Runs.requestState(hermes, msg("a", me, 0, "x", failed to listOf(agent)), agent))
        assertNull(Runs.requestState(hermes, msg("a", me, 0, "x", working to listOf(me)), agent))
    }

    @Test
    fun aQuietWorkingRunIsStalledNotWorking() {
        // agents.rs `stalled`: Working silent past STALE_RUN_MS drops from live views.
        val old = now - 5 * 3_600_000L
        val working = RunInfo(RunState.Working, old, agent = agent)
        assertTrue(Runs.stalled(working, old, now))
        // Talking an hour ago: a long run, not a stalled one.
        assertFalse(Runs.stalled(working, now - 3_600_000L, now))
        // Exactly at the limit isn't past it (the desktop's strict >).
        assertFalse(Runs.stalled(working, now - History.STALE_RUN_MS, now))
        // Only Working stalls: a finished run was already settled.
        assertFalse(Runs.stalled(working.copy(state = RunState.Done), old, now))
        assertFalse(Runs.stalled(working.copy(state = RunState.NeedsYou), old, now))
    }

    @Test
    fun aPromptIsItsSendersOwnReactionsUntilItMovesOn() {
        val prompt = msg("p", agent, now - 1000, "Run `rm -rf build`?", done to listOf(agent), failed to listOf(agent))
        val msgs = listOf(prompt)
        assertEquals(listOf(done to "Approve once", failed to "Deny"), Runs.openPrompt(msgs, 0, hermes, now))
        val after = msgs + msg("n", agent, now, "Done, removed it.")
        assertTrue(Runs.openPrompt(after, 0, hermes, now).isEmpty())
        // A step line doesn't end it (Hermes's heartbeat).
        val step = msgs + msg("s", agent, now, "⏳ Working — 3 min")
        assertEquals(2, Runs.openPrompt(step, 0, hermes, now).size)
    }

    @Test
    fun choicesFollowTheHarnessOrderNotTheReactionOrder() {
        val prompt = msg("p", agent, now - 1000, "Allow?", failed to listOf(agent), done to listOf(agent))
        assertEquals(listOf(done, failed), Runs.openPrompt(listOf(prompt), 0, hermes, now).map { it.first })
    }

    @Test
    fun questionsListTheirNumberedChoices() {
        val q = msg("q", agent, now - 1000, "❓ Which branch?\n  1. main\n  2. dev")
        assertEquals(listOf("1" to "main", "2" to "dev"), Runs.pendingQuestion(listOf(q), 0, agent, hermes, now))
        assertNull(Runs.pendingQuestion(listOf(q, msg("a", me, now, "1")), 0, agent, hermes, now))
    }

    @Test
    fun stepLogsAreToldApartFromAnswers() {
        assertEquals(2, Runs.progressSteps("📖 Reading notes.md\n🔍 Searching the web for x")?.size)
        assertNull(Runs.progressSteps("Here is the answer.\n📖 Reading notes.md"))
        assertNull(Runs.progressSteps("1 thing"))
        // Hermes's terminal commands: a fenced block after `💻 terminal`, or alone after one.
        assertEquals(
            listOf("📖 Reading Makefile", "💻 make clean", "💻 rm -rf build ...", "🔍 Searching for x"),
            Runs.progressSteps("📖 Reading Makefile\n💻 terminal\n```\nmake clean\n```\n```\nrm -rf build ...\n```\n🔍 Searching for x"),
        )
        assertNull("code alone is an answer", Runs.progressSteps("```\nfn main() {}\n```"))
        assertNull("an unclosed fence", Runs.progressSteps("💻 terminal\n```\nls"))
        assertTrue(Runs.isEmoji("⚙️"))
        assertTrue(Runs.isEmoji("👩‍💻"))
        assertFalse(Runs.isEmoji("ok"))
    }

    @Test
    fun aThreadedRequestIsTheThreadsRunWithItsLatestStep() {
        val req = msg("req", me, now - 5000, "do it", working to listOf(agent), threadCount = 1)
        val steps = msg("st", agent, now - 4000, "📖 Reading a.md\n🔧 Running tests", threadRoot = "req")
        val r = Runs.agentUpdates(room, listOf(req, steps), agent, hermes, isDm = false, now = now)
        val run = r.runs[ViewKey(room, "req")]!!
        assertEquals(RunState.Working, run.state)
        assertEquals("🔧 Running tests", run.step)
        assertEquals(2, run.steps)
        assertNull(r.runs[ViewKey(room)])
    }

    @Test
    fun aPromptInAThreadMakesTheThreadNeedYou() {
        val req = msg("req", me, now - 5000, "deploy", working to listOf(agent), threadCount = 1)
        val prompt = msg("p", agent, now - 1000, "Run deploy.sh?", done to listOf(agent), failed to listOf(agent), threadRoot = "req")
        val r = Runs.agentUpdates(room, listOf(req, prompt), agent, hermes, isDm = false, now = now)
        assertEquals(RunState.NeedsYou, r.runs[ViewKey(room, "req")]?.state)
        assertEquals(listOf("p"), r.prompts)
    }

    @Test
    fun theProtocolWinsOverReactionsExceptAClose() {
        // Richer info comes from the protocol even with a 👀 on the request.
        val req = msg("req", me, now - 5000, "go", working to listOf(agent))
        val status = msg("s", agent, now - 1000, "📖 Reading").copy(
            agent = Protocol(run = "req", state = "working", step = "Reading", steps = 3, costUsd = 0.12),
        )
        val r = Runs.agentUpdates(room, listOf(req, status), agent, hermes, isDm = true, now = now)
        val run = r.runs[ViewKey(room)]!!
        assertEquals(RunState.Working, run.state)
        assertEquals("Reading", run.step)
        assertEquals(0.12, run.costUsd!!, 1e-9)
        // But the agent's own ✅ on the request closes a run its protocol
        // status left working: with an unconfirmed stream delivery the
        // gateway settles whichever message it tracked last, not the working
        // bubble, which then says "working" on the wire forever.
        val closed = msg("req", me, now - 5000, "go", working to listOf(agent), done to listOf(agent))
        val r2 = Runs.agentUpdates(room, listOf(closed, status), agent, hermes, isDm = true, now = now)
        assertEquals(RunState.Done, r2.runs[ViewKey(room)]!!.state)
        // ❌ closes the same way; a still-working (👀-only) request doesn't.
        val fail = msg("req", me, now - 5000, "go", failed to listOf(agent))
        val r3 = Runs.agentUpdates(room, listOf(fail, status), agent, hermes, isDm = true, now = now)
        assertEquals(RunState.Failed, r3.runs[ViewKey(room)]!!.state)
    }

    @Test
    fun protocolParsesFromTheLatestEdit() {
        val json = """{"type":"m.room.message","content":{"body":"* x","m.new_content":{"body":"x","org.fluxchat.agent":{"state":"waiting","run":"${'$'}r","choices":[{"key":"✅","label":"Allow"}]}}}}"""
        val p = Protocol.fromEventJson(json)!!
        assertEquals(RunState.NeedsYou, p.runState())
        assertEquals("Allow", p.choices.single().label)
        assertNull(Protocol.fromEventJson("""{"content":{"body":"plain"}}"""))
    }

    @Test
    fun theFinalEditCarriesTheResultOntoTheRun() {
        val req = msg("req", me, now - 5000, "go")
        val status = msg("s", agent, now - 1000, "All green.").copy(
            agent = Protocol(run = "req", state = "done", steps = 3, result = "All 42 tests pass."),
        )
        val r = Runs.agentUpdates(room, listOf(req, status), agent, hermes, isDm = true, now = now)
        val run = r.runs[ViewKey(room)]!!
        assertEquals(RunState.Done, run.state)
        assertEquals("All 42 tests pass.", run.result)
    }

    @Test
    fun commandsAndBusyModeGoOutAsTheHarnessTakesThem() {
        assertEquals("!status", Runs.outgoing(hermes, "/status", working = false, mode = BusyMode.Send, addressedTo = null))
        assertEquals("/me", Runs.outgoing(hermes, "//me", working = false, mode = BusyMode.Send, addressedTo = null))
        assertEquals("!steer look at b", Runs.outgoing(hermes, "look at b", working = true, mode = BusyMode.Steer, addressedTo = null))
        assertEquals("look at b", Runs.outgoing(hermes, "look at b", working = false, mode = BusyMode.Steer, addressedTo = null))
    }

    @Test
    fun onlyRealEmojiStartAStep() {
        // A file tree, a tick list and an arrow are text an agent wrote, not
        // a step log (they used to fold away, and stopped ending prompts).
        assertFalse(Runs.isEmoji("├──"))
        assertFalse(Runs.isEmoji("✓"))
        assertFalse(Runs.isEmoji("→"))
        assertFalse(Runs.isEmoji("●"))
        assertNull(Runs.progressSteps("├── src\n└── build"))
        assertNull(Runs.progressSteps("✓ tests pass\n✓ lint clean"))
        // Emoji below U+1F000 still count, with or without the variation selector.
        for (e in listOf("⚙️", "⚙", "✅", "❌", "⏳", "⌛", "✏️", "⚡", "⚠️", "❓", "♾️", "▶️", "ℹ️", "‼️", "⭐", "1️⃣")) {
            assertTrue(e, Runs.isEmoji(e))
        }
    }

    @Test
    fun anUnthreadedRequestIsOneRun() {
        // agents.rs `an_unthreaded_request_is_one_run`.
        val earlier = msg("earlier", me, now - 60_000, "first", done to listOf(agent))
        val ask = msg("ask", me, now - 5000, "second", working to listOf(agent))
        val r = Runs.agentUpdates(room, listOf(earlier, ask), agent, hermes, isDm = false, now = now)
        assertEquals(RunState.Working, r.runs[ViewKey(room)]?.state)
        assertFalse(r.runs.containsKey(ViewKey(room, "ask")))
        assertEquals(1, r.runs.values.count { it?.state?.live == true })
    }

    @Test
    fun aTrackedThreadRunStillGetsItsEnd() {
        // Hermes answers in threads here, so its 👀 puts the run on the
        // request's thread before any reply…
        val tk = ViewKey(room, "ask")
        val going = msg("ask", me, now - 5000, "do it", working to listOf(agent))
        val r1 = Runs.agentUpdates(room, listOf(going), agent, hermes, isDm = false, now = now)
        assertEquals(RunState.Working, r1.runs[tk]?.state)
        // …and when it ends with no reply in that thread, the key is only
        // still reported because it was being tracked.
        val ended = msg("ask", me, now - 5000, "do it", done to listOf(agent))
        assertFalse(Runs.agentUpdates(room, listOf(ended), agent, hermes, isDm = false, now = now).runs.containsKey(tk))
        val r2 = Runs.agentUpdates(room, listOf(ended), agent, hermes, isDm = false, now = now, tracked = setOf(tk))
        assertEquals(RunState.Done, r2.runs[tk]?.state)
        val h = ArrayList<RunRecord>()
        assertTrue(History.record(h, tk, r1.runs[tk]!!, "do it", "do it", now))
        assertTrue(History.record(h, tk, r2.runs[tk]!!, "do it", "do it", now + 1000))
        assertEquals("done", h.single().state)
        assertEquals(now + 1000, h.single().ended)
    }

    @Test
    fun aPromptInAThreadIsThatThreadsAndIsOneRecord() {
        // agents.rs `a_prompt_in_a_thread_is_that_threads_and_is_one_record`:
        // no reaction on the request to date the run from, so the prompt does.
        val tk = ViewKey(room, "ask")
        val ask = msg("ask", me, now - 5000, "deploy", threadCount = 1)
        val prompt = msg("p", agent, now - 1000, "Run deploy.sh?", done to listOf(agent), failed to listOf(agent), threadRoot = "ask")
        val h = ArrayList<RunRecord>()
        for (later in listOf(0L, 1000L, 2000L)) {
            val r = Runs.agentUpdates(room, listOf(ask, prompt), agent, hermes, isDm = false, now = now + later)
            val run = r.runs[tk]!!
            assertEquals(RunState.NeedsYou, run.state)
            assertEquals("dated from the prompt, not from now", prompt.ts, run.since)
            assertNull("the room isn't waiting on you", r.runs[ViewKey(room)])
            History.record(h, tk, run, "deploy", "deploy", now + later)
        }
        assertEquals("one record, however often it's looked at", 1, h.size)
        // Answered (the agent took its reactions back): the run is gone, and its record ends.
        val answered = prompt.copy(reactions = emptyList())
        val gone = Runs.agentUpdates(room, listOf(ask, answered), agent, hermes, isDm = false, now = now + 3000)
        assertTrue(gone.runs.containsKey(tk))
        assertNull(gone.runs[tk])
        assertTrue(History.close(h, tk, prompt.ts, now + 3000))
        assertEquals("done", h.single().state)
        assertEquals(now + 3000, h.single().ended)
    }

    @Test
    fun anUnthreadedPromptIsTheRoomsAndDatedFromItself() {
        val prompt = msg("p", agent, now - 1000, "Run deploy.sh?", done to listOf(agent), failed to listOf(agent))
        val r1 = Runs.agentUpdates(room, listOf(prompt), agent, hermes, isDm = true, now = now)
        val r2 = Runs.agentUpdates(room, listOf(prompt), agent, hermes, isDm = true, now = now + 5000)
        assertEquals(RunState.NeedsYou, r1.runs[ViewKey(room)]?.state)
        assertEquals(prompt.ts, r1.runs[ViewKey(room)]?.since)
        assertEquals(r1.runs[ViewKey(room)], r2.runs[ViewKey(room)])
    }

    @Test
    fun aProtocolRunAskedInsideAThreadStaysWithTheThread() {
        // agents.rs `a_protocol_run_asked_inside_a_thread_stays_with_the_thread`:
        // filed under the room, two sessions' running cost totals interleave
        // and spend counts the difference as new money.
        val generic = Harness()
        val root = msg("root", me, now - 60_000, "start", threadCount = 3)
        val again = msg("again", me, now - 5000, "and now this", threadRoot = "root")
        val status = msg("s", agent, now - 1000, "working").copy(agent = Protocol(run = "again", state = "working", costUsd = 3.6))
        for (where in listOf(status.copy(threadRoot = "root"), status)) {
            val r = Runs.agentUpdates(room, listOf(root, again, where), agent, generic, isDm = false, now = now)
            val run = r.runs[ViewKey(room, "root")]!!
            assertEquals(RunState.Working, run.state)
            assertEquals(3.6, run.costUsd!!, 1e-9)
            assertEquals(again.ts, run.since)
            assertNull(r.runs[ViewKey(room)])
        }
    }

    @Test
    fun loadedRepliesSayAnAgentAnswersInThreads() {
        // The thread summary can trail the replies: the last finished request
        // has a reply loaded but no count yet.
        val finished = msg("a", me, now - 60_000, "first", done to listOf(agent))
        assertFalse(Runs.repliesInThreads(listOf(finished), agent, hermes, isDm = false))
        assertTrue(Runs.repliesInThreads(listOf(finished), agent, hermes, isDm = false, loaded = setOf("a")))
    }

    @Test
    fun aSpendIsAFiniteAmountOrNothing() {
        fun cost(v: Any) = Protocol.fromJson(org.json.JSONObject().put("state", "working").put("cost_usd", v)).costUsd
        assertEquals(1.25, cost(1.25)!!, 1e-9)
        assertNull(cost("Infinity"))
        assertNull(cost("NaN"))
        assertNull(cost(-3.0))
    }
}
