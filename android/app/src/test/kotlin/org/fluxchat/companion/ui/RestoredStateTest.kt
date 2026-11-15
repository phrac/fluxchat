package org.fluxchat.companion.ui

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import org.fluxchat.companion.agents.Media
import org.fluxchat.companion.matrix.AgentList
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * What has to still be there after the phone is turned (the activity is
 * recreated, and only saved state comes back): the photo you turned it to
 * look at, and what you had typed into a dialog or the sign-in form.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
// No dp qualifiers: see ProjectCreateFlowTest (a dialog with a text field never goes idle with them).
@Config(sdk = [35])
class RestoredStateTest {
    @get:Rule val compose = createComposeRule()

    private fun restorable(content: @Composable () -> Unit): StateRestorationTester {
        val tester = StateRestorationTester(compose)
        tester.setContent { FluxTheme { Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) { content() } } }
        return tester
    }

    @Test
    fun aPhotoOpenFullScreenIsStillOpenAfterThePhoneIsTurned() {
        var viewer: ImageViewerState? = null
        val tester = restorable {
            val state = rememberImageViewer()
            viewer = state
            ImageViewerHost(state) {}
        }
        compose.onNodeWithContentDescription("Close").assertDoesNotExist()
        compose.runOnIdle { viewer!!.viewing = Media("{\"url\":\"mxc://example.org/photo\"}", null, 800, 600) }
        compose.onNodeWithContentDescription("Close").assertIsDisplayed()
        // As the tapped message's own state the viewer was gone here.
        tester.emulateSavedInstanceStateRestore()
        compose.onNodeWithContentDescription("Close").assertIsDisplayed()
        compose.onNodeWithContentDescription("Close").performClick()
        compose.onNodeWithContentDescription("Close").assertDoesNotExist()
    }

    @Test
    fun aNewProjectsNameIsStillThere() {
        val tester = restorable {
            CreateProjectFlow(list = AgentList(), go = {}, onCreated = {}, onDismiss = {}, create = { _, _ -> null })
        }
        compose.onNodeWithText("Project name").performTextInput("Weekend build")
        tester.emulateSavedInstanceStateRestore()
        compose.onNodeWithText("Weekend build").assertIsDisplayed()
        compose.onNodeWithText("Create").assertIsEnabled()
    }

    @Test
    fun anAgentsIdBeingTypedIsStillThere() {
        val tester = restorable { AgentsScreen(back = {}) }
        compose.onNodeWithText("Add agent", useUnmergedTree = true).performClick()
        compose.onNodeWithText("Agent ID").performTextInput("@ricky:example.org")
        // The dialog itself was not saved either: it closed, with the ID.
        tester.emulateSavedInstanceStateRestore()
        compose.onNodeWithText("@ricky:example.org").assertIsDisplayed()
        compose.onNodeWithText("Add").assertIsEnabled()
    }

    @Test
    fun theSignInFormKeepsWhatWasTypedExceptThePassword() {
        val tester = restorable { LoginScreen() }
        compose.onNodeWithText("Sign in on another server").performScrollTo().performClick()
        compose.onNodeWithText("Server address").performScrollTo().performTextInput("example.org")
        compose.onNodeWithText("Username").performScrollTo().performTextInput("derek")
        compose.onNodeWithText("Password").performScrollTo().performTextInput("hunter2")
        compose.onNodeWithText("Sign in").performScrollTo().assertIsEnabled()
        tester.emulateSavedInstanceStateRestore()
        compose.onNodeWithText("example.org").assertExists()
        compose.onNodeWithText("derek").assertExists()
        // The password isn't put into the saved state: the button waits for it again.
        compose.onNodeWithText("Sign in").assertIsNotEnabled()
    }
}
