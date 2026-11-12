package org.fluxchat.companion.push

import org.fluxchat.companion.ui.Screen
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * When a push is not worth a notification because you're looking at the
 * thing it's about — and, by the same rule, which notifications opening a
 * screen takes down. The push path used to notify for the very session on
 * screen, and nothing ever cleared a notification but tapping it.
 */
class AttentionTest {
    private val room = "!project:x"
    private val root = "\$request"
    private val event = "\$answer"

    private fun watching(top: Screen?, visible: Boolean = true, locked: Boolean = false, thread: String? = root, asks: Boolean = false) =
        Attention.watching(top, visible, locked, room, thread, event, asks)

    @Test fun `an answer in the session on screen does not notify`() {
        assertTrue(watching(Screen.Thread(room, root)))
    }

    @Test fun `an answer in another session of the same project still notifies`() {
        assertFalse(watching(Screen.Thread(room, "\$other")))
    }

    @Test fun `the project's own screen covers its news, sessions included`() {
        // Its session cards show each session's latest answer as it arrives.
        assertTrue(watching(Screen.Room(room)))
        assertTrue(watching(Screen.Room(room), thread = null))
    }

    @Test fun `a prompt inside a session still alerts from the project's screen`() {
        // It can't be answered from there: the buttons are in the session.
        assertFalse(watching(Screen.Room(room), asks = true))
        assertTrue(watching(Screen.Thread(room, root), asks = true))
    }

    @Test fun `a prompt asked in the project itself is covered by the project's screen`() {
        assertTrue(watching(Screen.Room(room), thread = null, asks = true))
    }

    @Test fun `a session opened on the message that started it shows that message`() {
        // An agent's unthreaded message that became a session's root has no
        // thread root of its own; the session on screen is still its session.
        assertTrue(Attention.watching(Screen.Thread(room, event), true, false, room, null, event, false))
    }

    @Test fun `another project, or any other screen, covers nothing`() {
        assertFalse(watching(Screen.Room("!other:x")))
        assertFalse(watching(Screen.Thread("!other:x", root)))
        assertFalse(watching(Screen.Rooms))
        assertFalse(watching(Screen.Gallery(room, root)))
        assertFalse(watching(Screen.Settings))
        assertFalse(watching(null))
    }

    @Test fun `in the background or behind the lock nothing is being watched`() {
        // The session is still on top of the stack, but nobody is reading it:
        // the screen is off, another app is in front (the quick prompt over
        // it counts as that too), or the app lock covers it.
        assertFalse(watching(Screen.Thread(room, root), visible = false))
        assertFalse(watching(Screen.Thread(room, root), locked = true))
    }
}
