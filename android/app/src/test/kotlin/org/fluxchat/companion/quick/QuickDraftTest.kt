package org.fluxchat.companion.quick

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The quick prompt's unsent text, kept per project: a tap beside the sheet
 * closes it and used to throw away whatever was typed.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35]) // the newer sandbox image breaks on the build host's JDK
class QuickDraftTest {
    private val context: Context by lazy { ApplicationProvider.getApplicationContext() }

    @Before
    fun reset() {
        context.getSharedPreferences("quick", Context.MODE_PRIVATE).edit().clear().commit()
    }

    @Test fun `a draft comes back for the project it was typed for`() {
        Quick.saveDraft(context, "!a:x", "deploy the staging branch")
        assertEquals("deploy the staging branch", Quick.draft(context, "!a:x"))
        assertEquals("", Quick.draft(context, "!b:x"))
    }

    @Test fun `each project keeps its own`() {
        Quick.saveDraft(context, "!a:x", "one")
        Quick.saveDraft(context, "!b:x", "two")
        assertEquals("one", Quick.draft(context, "!a:x"))
        assertEquals("two", Quick.draft(context, "!b:x"))
    }

    @Test fun `sending it, or emptying the box, leaves no draft`() {
        Quick.saveDraft(context, "!a:x", "one")
        Quick.saveDraft(context, "!a:x", "")
        assertEquals("", Quick.draft(context, "!a:x"))
        Quick.saveDraft(context, "!a:x", "two")
        Quick.saveDraft(context, "!a:x", "   ")
        assertEquals("", Quick.draft(context, "!a:x"))
    }

    @Test fun `signing out removes the drafts and nothing else`() {
        // Drafts are the account's words; which project a widget opens is the phone's setup.
        Quick.saveDraft(context, "!a:x", "one")
        Quick.setWidgetRoom(context, 7, "!a:x")
        Quick.clearDrafts(context)
        assertEquals("", Quick.draft(context, "!a:x"))
        assertEquals("!a:x", Quick.widgetRoom(context, 7))
    }
}
