package org.fluxchat.companion.ui

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The back stack as saved text: it has to come back as it was after the phone is turned. */
class ScreenStackTest {
    private val room = "!r:x"

    @After
    fun tearDown() {
        ShareInbox.pending.value = null
        ShareInbox.target.value = null
    }

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
    fun aShareComesBackOnlyWhileWhatWasSharedIsStillThere() {
        // The phone was turned at "Send to…": the same process, the share still waiting.
        ShareInbox.pending.value = Shared("look at this", null)
        assertEquals(Screen.Share, decodeScreen(encodeScreen(Screen.Share)!!))
        // A new process: what was shared was only in memory, so there is nothing to pick a project for.
        ShareInbox.pending.value = null
        assertNull(decodeScreen(encodeScreen(Screen.Share)!!))
    }

    @Test
    fun whatIsNotAScreenIsDropped() {
        assertNull(decodeScreen("nonsense"))
        assertNull(decodeScreen("thread\n!r:x"))
    }

    @Test
    fun aNotificationForTheSessionYouAreOnLeavesItAsItIs() {
        val session = Screen.Thread(room, "\$a")
        val stack = mutableListOf(Screen.Rooms, Screen.Room(room), session)
        // Forgotten, the screen stayed up but was never saved again: the next
        // turn of the phone emptied its message box and lost its place.
        val forgotten = stack.openFromOutside(session)
        assertEquals(listOf<Screen>(Screen.Room(room)), forgotten)
        assertEquals(listOf(Screen.Rooms, Screen.Room(room), session), stack)
    }

    @Test
    fun aNotificationForAnotherSessionOpensItAfresh() {
        val was = Screen.Thread(room, "\$a")
        val asked = Screen.Thread(room, "\$b")
        val stack = mutableListOf(Screen.Rooms, Screen.Room(room), was, Screen.Gallery(room, "\$a"))
        assertEquals(listOf(Screen.Room(room), was, Screen.Gallery(room, "\$a")), stack.openFromOutside(asked))
        assertEquals(listOf(Screen.Rooms, Screen.Room(room), asked), stack)
        // One that was open underneath is a new opening too: at its newest answer, not where it was left.
        val under = mutableListOf(Screen.Rooms, Screen.Room(room), was, Screen.Gallery(room, "\$a"))
        assertEquals(listOf(Screen.Room(room), was, Screen.Gallery(room, "\$a")), under.openFromOutside(was))
        assertEquals(listOf(Screen.Rooms, Screen.Room(room), was), under)
        // A project, or the share picker, from Projects.
        val home = mutableListOf<Screen>(Screen.Rooms)
        assertEquals(emptyList<Screen>(), home.openFromOutside(Screen.Room(room)))
        assertEquals(listOf(Screen.Rooms, Screen.Room(room)), home)
        assertEquals(listOf<Screen>(Screen.Room(room)), home.openFromOutside(Screen.Share))
        assertEquals(listOf(Screen.Rooms, Screen.Share), home)
    }

    @Test
    fun aSecondTapOnARowDoesNotOpenItTwice() {
        val stack = mutableListOf<Screen>(Screen.Rooms)
        stack.push(Screen.Room(room))
        // The row is still there for a moment while the project slides in.
        stack.push(Screen.Room(room))
        assertEquals(listOf(Screen.Rooms, Screen.Room(room)), stack)
        // The same project further down is another matter (from a search result, say).
        stack.push(Screen.Search)
        stack.push(Screen.Room(room))
        assertEquals(listOf(Screen.Rooms, Screen.Room(room), Screen.Search, Screen.Room(room)), stack)
    }
}
