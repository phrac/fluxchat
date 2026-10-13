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
}
