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
    fun newestRuntimeWinsPerRoomAndFooterNeedsTheHarnessFlag() {
        fun msg(id: String, sender: String, ts: Long, body: String, agent: Protocol? = null) =
            Msg(id, sender, sender, body, ts, isOwn = false, agent = agent)
        val runtimes = HashMap<String, Pair<Long, Runtime>>()
        val msgs = listOf(
            msg("a", "@hermes:x", 1_000, "answer\n\nopus-5 · 42%"),
            msg("b", "@hermes:x", 2_000, "later", agent = Protocol(model = "glm-5")),
            msg("c", "@me:x", 3_000, "footer\n\nsonnet-4 · 10%"), // not an agent: skipped
        )
        Runtime.noteAll(runtimes, "!r:x", msgs, listOf("@hermes:x")) { true }
        assertEquals(2_000L to Runtime(model = "glm-5"), runtimes["!r:x"]) // newest, protocol counted
        // Footer off (harness without runtime_footer): only protocol fields count.
        Runtime.noteAll(runtimes, "!r:x", msgs, listOf("@hermes:x")) { false }
        assertEquals("glm-5", runtimes["!r:x"]!!.second.model)
        Runtime.noteAll(runtimes, "!f:x", listOf(msg("d", "@hermes:x", 500, "hi\n\nsonnet-4 · 9%")), listOf("@hermes:x")) { false }
        assertNull(runtimes["!f:x"])
        // Equal timestamp replaces (the desktop's >=).
        Runtime.noteAll(runtimes, "!r:x", listOf(msg("e", "@hermes:x", 2_000, "x\n\nopus-6 · 1%")), listOf("@hermes:x")) { true }
        assertEquals("opus-6", runtimes["!r:x"]!!.second.model)
    }
}
