package org.fluxchat.companion.agents

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The desktop's agents.rs `split_footer` / `Protocol::runtime` ported. */
class RuntimeTest {
    @Test
    fun footerParagraphsSplitIntoRuntimeFields() {
        val body = "Deployed.\n\nopus-5 · 42% · ~/code/fluxchat"
        val (at, rt) = Runtime.splitFooter(body)!!
        assertEquals(body.indexOf("opus-5"), at)
        assertEquals(Runtime(model = "opus-5", contextPct = 42, cwd = "~/code/fluxchat"), rt)
        // Latency and alias→served shapes.
        val (_, rt2) = Runtime.splitFooter("done\n\nglm-5-turbo · 22s · /srv")!!
        assertEquals(Runtime(model = "glm-5-turbo", latency = "22s", cwd = "/srv"), rt2)
        val (_, rt3) = Runtime.splitFooter("done\n\nalias → served · 1m30s")!!
        assertEquals("1m30s", rt3.latency)
        assertEquals("alias → served", rt3.model)
    }

    @Test
    fun proseLastParagraphsAreNotFooters() {
        assertNull(Runtime.splitFooter("just an answer"))
        assertNull(Runtime.splitFooter("one\n\ntwo"))           // a word, not fields
        assertNull(Runtime.splitFooter("a\n\nb · prose word"))  // one model-ish word: could be prose
        assertNull(Runtime.splitFooter("x\n\npart one\npart two")) // multi-line: not a footer
        assertNull(Runtime.splitFooter("x\n\n" + "a".repeat(201) + " · 5%"))
        // A % or a path alone still is one.
        assertEquals(30, Runtime.splitFooter("x\n\n30%")!!.second.contextPct)
    }

    @Test
    fun protocolFieldsBecomeRuntime() {
        val p = Protocol(model = "opus-5", context = 0.42f, cwd = "~/p", costUsd = 0.18)
        val rt = Runtime.ofProtocol(p)!!
        assertEquals(Runtime(model = "opus-5", contextPct = 42, cwd = "~/p", costUsd = 0.18), rt)
        assertEquals("opus-5 · ctx 42% · \$0.18 · ~/p", rt.summary())
        assertNull(Runtime.ofProtocol(Protocol(step = "working"))) // nothing runtime-ish
    }

    @Test
    fun aRunsRuntimeIsItsNewestAndAFooterNeedsTheHarnessFlag() {
        fun msg(id: String, sender: String, ts: Long, body: String, agent: Protocol? = null) =
            Msg(id, sender, sender, body, ts, isOwn = false, agent = agent)
        val msgs = listOf(
            msg("a", "@hermes:x", 1_000, "answer\n\nopus-5 · 42%"),
            msg("b", "@hermes:x", 2_000, "later", agent = Protocol(model = "glm-5")),
            msg("c", "@me:x", 3_000, "footer\n\nsonnet-4 · 10%"), // not the agent: skipped
        )
        assertEquals(Runtime(model = "glm-5"), Runtime.ofRun(msgs, "@hermes:x", 0, footer = true)) // newest, protocol counted
        // Footer off (harness without runtime_footer): only protocol fields count.
        assertEquals("glm-5", Runtime.ofRun(msgs, "@hermes:x", 0, footer = false)!!.model)
        assertNull(Runtime.ofRun(listOf(msg("d", "@hermes:x", 500, "hi\n\nsonnet-4 · 9%")), "@hermes:x", 0, footer = false))
        // Equal timestamp: the later message replaces (the desktop's >=).
        assertEquals("opus-6", Runtime.ofRun(msgs + msg("e", "@hermes:x", 2_000, "x\n\nopus-6 · 1%"), "@hermes:x", 0, footer = true)!!.model)
    }

    @Test
    fun aRunsRuntimeIsItsOwnAgentsInItsOwnSessionSinceItStarted() {
        // What the end backfill may use. The desktop's record_run takes the
        // room's newest runtime info, whoever said it and in which session:
        // a run that reported no cost, ending in a project where another
        // session had just reported its $3.60 total, was recorded at $3.60 —
        // and spend counts a session total whole.
        fun msg(id: String, sender: String, ts: Long, root: String?, cost: Double? = null, model: String? = null) =
            Msg(id, sender, sender, "x", ts, isOwn = false, threadRoot = root, agent = Protocol(model = model, costUsd = cost))
        val room = "!r:x"
        val all = listOf(
            msg("old", "@hermes:x", 1_000, "mine", cost = 9.0),          // this session, before this run
            msg("other", "@hermes:x", 6_000, "theirs", cost = 3.6),      // another session, newest in the room
            msg("else", "@claude:x", 7_000, "mine", cost = 5.0),         // another agent, in this session
            msg("own", "@hermes:x", 5_000, "mine", model = "opus-5"),    // this run's own
        )
        val view = Runs.viewMessages(all, ViewKey(room, "mine"))
        val rt = Runtime.ofRun(view, "@hermes:x", since = 4_000, footer = true)!!
        assertEquals("opus-5", rt.model)
        assertNull("another session's total is not this run's cost", rt.costUsd)
        // Nothing of its own since it started: nothing to backfill with.
        assertNull(Runtime.ofRun(view, "@hermes:x", since = 5_500, footer = true))
        // And the record it ends up as.
        val h = ArrayList<RunRecord>()
        val key = ViewKey(room, "mine")
        History.record(h, key, RunInfo(RunState.Working, 4_000, agent = "@hermes:x"), "t", "t", 4_500)
        History.record(h, key, RunInfo(RunState.Done, 4_000, agent = "@hermes:x"), "t", "t", 8_000, runtime = rt)
        assertEquals("opus-5", h[0].model)
        assertNull(h[0].costUsd)
        assertEquals(0.0, History.spend(h, "@hermes:x", 0), 1e-9)
    }
}
