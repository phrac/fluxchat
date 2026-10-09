package org.fluxchat.companion.ui

import org.fluxchat.companion.agents.Msg
import org.junit.Assert.assertEquals
import org.junit.Test

class TranscriptTest {
    private val me = "@me:x"
    private val agent = "@hermes:x"
    private fun msg(id: String, sender: String, ts: Long, body: String) = Msg(id, sender, sender, body, ts, isOwn = sender == me)

    private val turn = listOf(
        msg("p", me, 0, "Add the photo to the album"),
        msg("s1", agent, 10_000, "🔍 Searching for photos\n💻 Running ls"),
        msg("n", agent, 20_000, "Found it, adding now."),
        msg("s2", agent, 30_000, "💻 Running curl"),
        msg("a", agent, 360_000, "Done: it's in the **AI Generated** album."),
        msg("r", agent, 365_000, "💾 Self-improvement review: Skill 'immich' patched"),
    )

    private fun shape(e: List<TranscriptEntry>) = e.map {
        when (it) {
            is TranscriptEntry.Row -> "row:${it.id}"
            is TranscriptEntry.Work -> "work:${it.indices.size}/${it.toolCalls}/${it.updates}"
            is TranscriptEntry.Done -> "done:${it.id}"
            is TranscriptEntry.Answer -> "answer:${it.work?.let { w -> "${w.indices.size}/${w.toolCalls}/${w.updates}" } ?: "-"}:${it.id}"
        }
    }

    @Test
    fun aFinishedTurnFoldsItsWorkAndEndsWithTheAnswer() {
        val e = transcriptEntries(turn, listOf(agent), live = false) { false }
        assertEquals(listOf("row:p", "answer:4/4/1:a"), shape(e))
        assertEquals(360_000L, (e[1] as TranscriptEntry.Answer).done.durationMs)
    }

    @Test
    fun theTurnInProgressStaysOpen() {
        assertEquals(turn.map { "row:${it.id}" }, shape(transcriptEntries(turn, listOf(agent), live = true) { false }))
    }

    @Test
    fun aTurnWaitingOnYouStaysOpen() {
        assertEquals(turn.map { "row:${it.id}" }, shape(transcriptEntries(turn, listOf(agent), live = false) { it == 2 }))
    }

    @Test
    fun anAnswerWithNoWorkHasJustItsDoneLine() {
        val quick = listOf(msg("p", me, 0, "hi"), msg("a", agent, 2_000, "Hello!"))
        assertEquals(listOf("row:p", "answer:-:a"), shape(transcriptEntries(quick, listOf(agent), live = false) { false }))
    }
}
