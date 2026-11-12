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
