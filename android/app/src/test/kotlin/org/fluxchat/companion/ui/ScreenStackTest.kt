package org.fluxchat.companion.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The back stack as saved text: it has to come back as it was after the phone is turned. */
class ScreenStackTest {
    @Test
    fun everyScreenComesBackAsItWas() {
        val stack = listOf(
            Screen.Rooms,
            Screen.Room("!r:x"),
            Screen.Room("!r:x", "\$hit"),
            Screen.Thread("!r:x", "\$root"),
            Screen.Thread("!r:x", "\$root", "\$hit"),
            Screen.Gallery("!r:x", null),
            Screen.Gallery("!r:x", "\$root"),
            Screen.Agents, Screen.History, Screen.Search, Screen.Settings, Screen.Diagnostics,
        )
        assertEquals(stack, stack.map { decodeScreen(encodeScreen(it)!!) })
    }

    @Test
    fun aShareInProgressIsNotSaved() {
        // What was shared is only in memory.
        assertNull(encodeScreen(Screen.Share))
        assertNull(decodeScreen("nonsense"))
        assertNull(decodeScreen("thread\n!r:x"))
    }
}
