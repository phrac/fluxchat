package org.fluxchat.companion.push

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.io.File
import org.fluxchat.companion.agents.Harness
import org.fluxchat.companion.agents.Protocol
import org.fluxchat.companion.agents.Runs

/**
 * The notification side of run approvals: a waiting protocol prompt notifies
 * as a Prompt with the project in its title and the run's headline below it,
 * and its quick reply is a deny-with-reason when the prompt has a Deny.
 */
class NotifierApprovalTest {
    private val hermes: Harness = Harness.parse(File("../../assets/agents/hermes.toml").readText())
    private val now = 10_000_000L

    private fun protocolJson(state: String, choices: List<Pair<String, String>>): String {
        val cs = choices.joinToString(",") { """{"key":"${it.first}","label":"${it.second}"}""" }
        return """{"event_id":"p","content":{"org.fluxchat.agent":{"v":1,"state":"$state","choices":[$cs]}}}"""
    }

    @Test
    fun aWaitingProtocolPromptNotifiesAsAPromptWithTheProjectTitle() {
        val json = protocolJson(
            "waiting",
            listOf("✅" to "Approve once", "🌀" to "For this session", "♾️" to "Always", "❌" to "Deny"),
        )
        val note = Notifier.noteFor(
            "!r:x", "$e", null, "@hermes:x", "Hermes", "maintdx",
            "Run `rm -rf build`?", Protocol.fromEventJson(json), hermes, now, now,
            headline = "fix the welder fault",
        )
        assertEquals(Note.Kind.Prompt, note?.kind)
        assertEquals("maintdx · Hermes needs you", note?.title)
        assertEquals("fix the welder fault", note?.headline)
        assertEquals(listOf("✅", "🌀", "♾️", "❌"), note?.choices?.map { it.first })
    }

    @Test
    fun theFirstAndLastChoicesAreTheButtonsForAApproval() {
        val note = Note(
            "!r:x", "$e", null, "@hermes:x", "Hermes · maintdx needs you", "Run it?",
            Note.Kind.Prompt,
            listOf("✅" to "Approve once", "🌀" to "For this session", "♾️" to "Always", "❌" to "Deny"),
        )
        assertEquals(listOf("✅" to "Approve once", "❌" to "Deny"), Notifier.buttons(note))
    }

    @Test
    fun stepLogsStillAreNotNews() {
        val note = Notifier.noteFor(
            "!r:x", "$e", null, "@hermes:x", "Hermes", "maintdx",
            "🔧 Searching files", null, hermes, now, now,
        )
        assertNull(note)
    }

    @Test
    fun denyChoiceFindsTheApprovalsDeny() {
        val approval = listOf("✅" to "Approve once", "🌀" to "For this session", "❌" to "Deny")
        assertEquals("❌", Runs.denyChoice(approval))
        // A protocol prompt whose deny is keyed differently but labelled Deny.
        assertEquals("no", Runs.denyChoice(listOf("yes" to "Allow once", "no" to "Deny")))
        // Questions (and prompts without a Deny) keep the plain reply.
        assertNull(Runs.denyChoice(listOf("1" to "Deploy", "2" to "Staging only")))
        assertNull(Runs.denyChoice(emptyList()))
    }

    private val e = "!ev:x"
}
