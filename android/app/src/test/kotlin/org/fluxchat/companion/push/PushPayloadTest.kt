package org.fluxchat.companion.push

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * What a push from the homeserver says, and what a notification says when
 * the event can't be read yet.
 */
class PushPayloadTest {
    @Test fun `a push about an event names its room and event`() {
        val p = PushPayload.parse("""{"notification":{"event_id":"${'$'}e","room_id":"!r:x","counts":{"unread":2},"devices":[]}}""")
        assertEquals(PushPayload.Event("!r:x", "\$e"), p)
    }

    @Test fun `a push with no event carries the unread total`() {
        // Synapse's badge update, as it sends it: nulls and empty strings where the event would be.
        val p = PushPayload.parse("""{"notification":{"id":"","type":null,"sender":"","room_id":null,"counts":{"unread":3}}}""")
        assertEquals(PushPayload.Counts(3), p)
    }

    @Test fun `nothing unread reads as zero however the server spells it`() {
        // Zero counts "should be omitted" (push gateway API); Synapse sends them anyway.
        assertEquals(PushPayload.Counts(0), PushPayload.parse("""{"notification":{"counts":{"unread":0}}}"""))
        assertEquals(PushPayload.Counts(0), PushPayload.parse("""{"notification":{"counts":{}}}"""))
        assertEquals(PushPayload.Counts(0), PushPayload.parse("""{"notification":{}}"""))
    }

    @Test fun `anything else is not a Matrix notification`() {
        assertNull(PushPayload.parse("hello"))
        assertNull(PushPayload.parse("""{"message":"a test from the distributor"}"""))
    }

    @Test fun `an edit is recognised on an event that is still encrypted`() {
        // The relation stays in the clear on m.room.encrypted: a growing step
        // log that can't be decrypted yet must not notify once per step.
        val edit = """{"type":"m.room.encrypted","content":{"algorithm":"m.megolm.v1.aes-sha2","ciphertext":"…","m.relates_to":{"rel_type":"m.replace","event_id":"${'$'}first"}}}"""
        val reply = """{"type":"m.room.encrypted","content":{"ciphertext":"…","m.relates_to":{"rel_type":"m.thread","event_id":"${'$'}root"}}}"""
        val plain = """{"type":"m.room.encrypted","content":{"ciphertext":"…"}}"""
        assertTrue(PushPayload.isEdit(edit))
        assertFalse(PushPayload.isEdit(reply))
        assertFalse(PushPayload.isEdit(plain))
        assertFalse(PushPayload.isEdit("not json"))
    }

    @Test fun `an edit is about the message it replaces, in its new words`() {
        // As agents send them: the old body with "* " in front, the new one in m.new_content.
        val edit = """{"type":"m.room.message","event_id":"${'$'}edit","content":{"msgtype":"m.text","body":"* 🔧 Reading notes.md\n💻 Running the tests",""" +
            """"m.new_content":{"msgtype":"m.text","body":"🔧 Reading notes.md\n💻 Running the tests"},"m.relates_to":{"rel_type":"m.replace","event_id":"${'$'}bubble"}}}"""
        assertEquals(PushPayload.Edit("\$bubble", "🔧 Reading notes.md\n💻 Running the tests"), PushPayload.edit(edit))
        // Read by its fallback body, a growing step log was no step log ("* 🔧 …"): it notified once per step.
        assertNull(org.fluxchat.companion.agents.Runs.progressSteps("* 🔧 Reading notes.md\n💻 Running the tests"))
        assertEquals(2, org.fluxchat.companion.agents.Runs.progressSteps(PushPayload.edit(edit)!!.body)?.size)
        // Without m.new_content the fallback body is all there is: minus its marker.
        val bare = """{"content":{"body":"* fixed the typo","m.relates_to":{"rel_type":"m.replace","event_id":"${'$'}first"}}}"""
        assertEquals(PushPayload.Edit("\$first", "fixed the typo"), PushPayload.edit(bare))
    }

    @Test fun `a reply, a thread reply and a plain message are not edits`() {
        assertNull(PushPayload.edit("""{"content":{"body":"ok","m.relates_to":{"rel_type":"m.thread","event_id":"${'$'}root"}}}"""))
        assertNull(PushPayload.edit("""{"content":{"body":"ok","m.relates_to":{"m.in_reply_to":{"event_id":"${'$'}q"}}}}"""))
        assertNull(PushPayload.edit("""{"content":{"body":"* not an edit, just a star"}}"""))
        assertNull(PushPayload.edit("""{"content":{"body":"x","m.relates_to":{"rel_type":"m.replace"}}}"""))
        assertNull(PushPayload.edit("not json"))
    }

    @Test fun `an edit of a message that already notified is not news a second time`() {
        // An answer written an edit at a time: one notification, kept current while it's up…
        assertEquals(EditNews.Refresh, editNews(asks = false, fromAgent = true, showing = true, notified = true))
        // …and once it's been opened or swiped away, nothing more.
        assertEquals(EditNews.None, editNews(asks = false, fromAgent = true, showing = false, notified = true))
    }

    @Test fun `an edit is news when it is the first you hear of the message`() {
        // A step log (never notified) edited into the answer.
        assertEquals(EditNews.New, editNews(asks = false, fromAgent = true, showing = false, notified = false))
        // A person's edit only gets here for its mention of you.
        assertEquals(EditNews.New, editNews(asks = false, fromAgent = false, showing = false, notified = false))
        assertEquals(EditNews.New, editNews(asks = false, fromAgent = false, showing = false, notified = true))
        // A status message edited into a prompt waits on you, whatever came before.
        assertEquals(EditNews.New, editNews(asks = true, fromAgent = true, showing = true, notified = true))
        assertEquals(EditNews.New, editNews(asks = true, fromAgent = true, showing = false, notified = true))
    }

    @Test fun `a message that cannot be decrypted yet still says who and where`() {
        // It used to be dropped as "not a renderable message": no notification at all.
        val note = Notifier.undecryptedNote("!r:x", "\$e", "\$root", "@hermes:x", "Hermes", "maintdx")
        assertEquals(Note.Kind.Message, note.kind)
        assertEquals("Hermes · maintdx", note.title)
        assertEquals("New message from Hermes", note.body)
        // Tapping it opens the session the message is in.
        assertEquals("\$root", note.threadRoot)
        assertTrue(note.choices.isEmpty())
        // A DM named after the agent doesn't say the name twice.
        assertEquals("Hermes", Notifier.undecryptedNote("!r:x", "\$e", null, "@hermes:x", "Hermes", "Hermes").title)
    }

    @Test fun `a condition that holds for every event is reported once per streak`() {
        // "No foreground service" on every step of a run filled Diagnostics' 150 lines.
        val quiet = Streak()
        assertTrue(quiet.starts("refused"))
        assertFalse(quiet.starts("refused"))
        assertFalse(quiet.starts("refused"))
        // Another condition is its own streak.
        assertTrue(quiet.starts("steps:!r:x"))
        // Over, then back: worth saying again.
        assertTrue(quiet.ends("refused"))
        assertFalse(quiet.ends("refused"))
        assertTrue(quiet.starts("refused"))
    }
}
