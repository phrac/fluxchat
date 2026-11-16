package org.fluxchat.companion.push

import android.app.NotificationManager
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Notifications as they are posted: what is up for an event, and what stands in when something didn't go out. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35]) // the newer sandbox image breaks on the build host's JDK
class NotifierShownTest {
    private val context: Context by lazy { ApplicationProvider.getApplicationContext() }
    private val manager get() = context.getSystemService(NotificationManager::class.java)

    private val approval = Note(
        "!ops:x", "\$prompt", "\$request", "@hermes:x", "maintdx · Hermes needs you", "Run `rm -rf build`?", Note.Kind.Prompt,
        listOf("✅" to "Approve once", "🌀" to "For this session", "♾️" to "Always", "❌" to "Deny"),
    )

    @Before
    fun reset() = manager.cancelAll()

    private fun up(tag: String) = manager.activeNotifications.firstOrNull { it.tag == tag }?.notification

    @Test fun `an approval goes up with its first and last choice, and the rest as chips`() {
        Notifier.show(context, approval)
        val n = up("\$prompt")!!
        assertEquals(listOf("✅ Approve once", "❌ Deny", "More"), n.actions.map { it.title.toString() })
        assertEquals(
            listOf("✅ Approve once", "🌀 For this session", "♾️ Always", "❌ Deny"),
            n.actions.last().remoteInputs.single().choices.map { it.toString() },
        )
        assertTrue(Notifier.isShowing(context, "\$prompt"))
        assertTrue(Notifier.wasNotified("\$prompt"))
    }

    @Test fun `the line that follows an answer is not the message's notification`() {
        // An edit of the prompt arriving in those four seconds found "its
        // notification" up and put the whole message back over "Answered".
        Notifier.show(context, approval)
        Notifier.answered(context, "\$prompt", "Answered: ✅ Approve once")
        assertEquals("Answered: ✅ Approve once", up("\$prompt")!!.extras.getString("android.title"))
        assertFalse(Notifier.isShowing(context, "\$prompt"))
        // It did notify, all the same: its edits are not news.
        assertTrue(Notifier.wasNotified("\$prompt"))
    }

    @Test fun `an answer that did not go out puts its prompt back`() {
        Notifier.show(context, approval)
        assertTrue(Notifier.failed(context, "!ops:x", "\$request", "\$prompt"))
        val n = up("\$prompt")!!
        assertEquals("Not sent · maintdx · Hermes needs you", n.extras.getString("android.title"))
        assertEquals(3, n.actions.size)
    }

    @Test fun `a Stop that did not go out says so in its own words`() {
        // "Couldn't send your answer · Open fluxchat to answer", for a Stop
        // tapped on the live runs notification with no signal.
        assertFalse(Notifier.failed(context, "!ops:x", "\$request", "stop:\$request", stop = true))
        val n = up("stop:\$request")!!
        assertEquals("Couldn't send Stop", n.extras.getString("android.title"))
        assertEquals("The run is still going. Open fluxchat to stop it.", n.extras.getString("android.text"))
        // An answer this process no longer has the prompt for keeps its words.
        assertFalse(Notifier.failed(context, "!ops:x", "\$request", "\$gone"))
        assertEquals("Couldn't send your answer", up("\$gone")!!.extras.getString("android.title"))
    }
}
