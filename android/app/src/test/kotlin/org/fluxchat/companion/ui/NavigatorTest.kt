package org.fluxchat.companion.ui

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTextInput
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The navigator itself, with nobody signed in (its screens come up empty,
 * which is enough): what a screen asked for from outside — a notification,
 * the quick prompt — does to the one that is already there.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35])
class NavigatorTest {
    @get:Rule val compose = createComposeRule()

    private val room = "!r:example.org"

    @Before
    fun setUp() {
        ShareInbox.pending.value = null
        ShareInbox.target.value = null
    }

    @After
    fun tearDown() {
        ShareInbox.pending.value = null
        ShareInbox.target.value = null
    }

    private fun navigator(opened: MutableStateFlow<Screen?>): StateRestorationTester {
        val tester = StateRestorationTester(compose)
        tester.setContent { FluxTheme { Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) { Navigator(opened) } } }
        return tester
    }

    @Test
    fun theScreenInFrontIsStillSavedAfterBeingAskedForFromOutside() {
        val opened = MutableStateFlow<Screen?>(null)
        val tester = navigator(opened)
        opened.value = Screen.Search
        compose.onNode(hasSetTextAction()).performTextInput("deploy")
        // A notification for the screen you are on (any screen will do here):
        // it was forgotten like the ones it replaces, while it stayed up — and
        // from then on nothing of it was saved.
        opened.value = Screen.Search
        compose.waitForIdle()
        compose.onNodeWithText("deploy").assertExists()
        tester.emulateSavedInstanceStateRestore()
        compose.onNodeWithText("deploy").assertExists()
    }

    @Test
    fun whatTheQuickPromptHandsToAProjectLandsInTheProjectsMessageBox() {
        // The app was left on one of the project's sessions.
        val opened = MutableStateFlow<Screen?>(Screen.Thread(room, "\$a"))
        navigator(opened)
        compose.waitForIdle()
        // "Open the project" from the quick prompt, with something typed.
        assertTrue(ShareInbox.handOver(room, "fix the build"))
        opened.value = Screen.Room(room)
        compose.waitForIdle()
        // The session's message box took it — and was closed the next moment.
        compose.onNodeWithText("fix the build").assertExists()
    }
}
