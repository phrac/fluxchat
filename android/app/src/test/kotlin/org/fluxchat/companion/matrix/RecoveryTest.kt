package org.fluxchat.companion.matrix

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RecoveryTest {
    // --- whether the key is wanted ---

    @Test fun `a verified phone has nothing to unlock`() {
        assertEquals(false, RecoveryPrompt.needs(verified = true, recoverable = true))
        assertEquals(false, RecoveryPrompt.needs(verified = true, recoverable = null))
    }

    @Test fun `an unverified phone on an account with a recovery key wants it`() {
        assertEquals(true, RecoveryPrompt.needs(verified = false, recoverable = true))
    }

    @Test fun `an account without a recovery key is never asked for one`() {
        assertEquals(false, RecoveryPrompt.needs(verified = false, recoverable = false))
    }

    @Test fun `offline at start is not an answer`() {
        // Read as "no", it dismissed a prompt that was still due and forgot it was.
        assertNull(RecoveryPrompt.needs(verified = false, recoverable = null))
    }

    // --- a saved session's first screen ---

    @Test fun `a saved session opens straight into the app`() {
        assertEquals(Auth.Ready, RecoveryPrompt.atRestore(RecoveryStatus()))
    }

    @Test fun `a prompt nobody answered is the first screen again`() {
        assertEquals(Auth.NeedsRecovery, RecoveryPrompt.atRestore(RecoveryStatus(needed = true)))
    }

    @Test fun `not now is remembered at the next start`() {
        assertEquals(Auth.Ready, RecoveryPrompt.atRestore(RecoveryStatus(needed = true, skipped = true)))
    }

    // --- the answer arriving ---

    @Test fun `a new sign-in goes to the prompt when the key is wanted`() {
        assertEquals(Auth.NeedsRecovery, RecoveryPrompt.onAnswer(Auth.SignedOut, needs = true, skipped = false))
        assertEquals(Auth.NeedsRecovery, RecoveryPrompt.onAnswer(Auth.Starting, needs = true, skipped = false))
    }

    @Test fun `a late answer never pulls you out of the app`() {
        // The prompt replaces the whole screen: the open session and a
        // half-typed request would go with it.
        assertEquals(Auth.Ready, RecoveryPrompt.onAnswer(Auth.Ready, needs = true, skipped = false))
    }

    @Test fun `not now keeps the prompt away whatever is on screen`() {
        for (shown in listOf(Auth.Starting, Auth.SignedOut, Auth.Ready)) {
            assertEquals(shown, RecoveryPrompt.onAnswer(shown, needs = true, skipped = true))
        }
    }

    @Test fun `a prompt with nothing left to unlock goes away`() {
        // Verified from another device while the prompt was up.
        assertEquals(Auth.Ready, RecoveryPrompt.onAnswer(Auth.NeedsRecovery, needs = false, skipped = false))
    }

    @Test fun `an answer never touches a failed restore or the sign-in form otherwise`() {
        assertEquals(Auth.RestoreFailed, RecoveryPrompt.onAnswer(Auth.RestoreFailed, needs = true, skipped = false))
        assertEquals(Auth.RestoreFailed, RecoveryPrompt.onAnswer(Auth.RestoreFailed, needs = false, skipped = false))
        assertEquals(Auth.SignedOut, RecoveryPrompt.onAnswer(Auth.SignedOut, needs = false, skipped = false))
        assertEquals(Auth.NeedsRecovery, RecoveryPrompt.onAnswer(Auth.NeedsRecovery, needs = true, skipped = false))
    }

    // --- what is offered ---

    @Test fun `the card is offered only while needed and not turned down`() {
        assertTrue(RecoveryStatus(needed = true).offer)
        assertFalse(RecoveryStatus(needed = true, skipped = true).offer)
        assertFalse(RecoveryStatus(needed = false).offer)
    }
}
